use std::path::PathBuf;
use std::process::Command as StdCommand;

use gaveloc_core::config::Region;
use gaveloc_core::entities::{AccountId, Repository};
use gaveloc_core::launch_args::{build_launch_args, EncryptedSessionId, LaunchParams};
use gaveloc_core::ports::{
    AccountRepository, CredentialStore, LaunchConfig, PrefixManager, ProcessLauncher,
    RunnerDetector, VersionRepository,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchStatusDto {
    pub is_running: bool,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightResultDto {
    pub can_launch: bool,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
}

/// Get the default Wine prefix path
fn get_default_prefix_path() -> PathBuf {
    directories::ProjectDirs::from("com", "gaveloc", "gaveloc")
        .map(|d| d.data_dir().join("prefix"))
        .unwrap_or_else(|| {
            directories::BaseDirs::new()
                .map(|b| b.data_local_dir().join("gaveloc/prefix"))
                .unwrap_or_else(|| PathBuf::from("/tmp/gaveloc/prefix"))
        })
}

/// Preflight check - validates all prerequisites before launch
#[tauri::command]
pub async fn preflight_check(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<PreflightResultDto, String> {
    let mut issues = Vec::new();
    let mut warnings = Vec::new();

    let settings = state.settings.read().await;

    // 1. Check game path
    let game_path = match settings.game.path.as_ref() {
        Some(path) => path.clone(),
        None => {
            issues.push("Game path not configured".to_string());
            return Ok(PreflightResultDto {
                can_launch: false,
                issues,
                warnings,
            });
        }
    };

    let game_exe = game_path.join("game/ffxiv_dx11.exe");
    if !game_exe.exists() {
        issues.push("Game executable not found (ffxiv_dx11.exe)".to_string());
    }

    // 2. Check runner
    if settings.wine.runner_path.is_none() {
        // Try to auto-detect
        match state.runner_detector.detect_runners().await {
            Ok(runners) if runners.is_empty() => {
                issues.push("No Wine/Proton runner detected".to_string());
            }
            Err(_) => {
                issues.push("Failed to detect Wine/Proton runners".to_string());
            }
            _ => {}
        }
    }

    // 3. Check session (must be logged in)
    let id = AccountId::new(&account_id);
    match state.credentials.get_session(&id).await {
        Ok(Some(session)) if session.is_valid() => {}
        Ok(Some(_)) => {
            issues.push("Session expired - please login again".to_string());
        }
        Ok(None) => {
            issues.push("Not logged in - valid session required".to_string());
        }
        Err(_) => {
            issues.push("Failed to check session status".to_string());
        }
    }

    // 4. Check prefix (warning if not exists - will be created)
    let prefix_path = settings
        .wine
        .prefix_path
        .clone()
        .unwrap_or_else(get_default_prefix_path);
    if !state.prefix_manager.exists(&prefix_path).await {
        warnings.push("Wine prefix does not exist - will be created on first launch".to_string());
    }

    Ok(PreflightResultDto {
        can_launch: issues.is_empty(),
        issues,
        warnings,
    })
}

/// Launch the game
#[tauri::command]
pub async fn launch_game(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<(), String> {
    let settings = state.settings.read().await;

    // 1. Get game path
    let game_path = settings
        .game
        .path
        .as_ref()
        .ok_or_else(|| "Game path not configured".to_string())?
        .clone();
    let game_exe = game_path.join("game/ffxiv_dx11.exe");

    // 2. Get runner
    let runner = if let Some(ref runner_path) = settings.wine.runner_path {
        state
            .runner_detector
            .validate_runner(runner_path.clone())
            .await
            .map_err(|e| e.to_string())?
    } else {
        // Auto-detect first available
        let runners = state
            .runner_detector
            .detect_runners()
            .await
            .map_err(|e| e.to_string())?;
        runners
            .into_iter()
            .next()
            .ok_or_else(|| "No Wine/Proton runner available".to_string())?
    };

    // 3. Get/create prefix
    let prefix_path = settings
        .wine
        .prefix_path
        .clone()
        .unwrap_or_else(get_default_prefix_path);

    if !state.prefix_manager.exists(&prefix_path).await {
        // Drop settings lock before prefix initialization (it can take a while)
        drop(settings);
        state
            .prefix_manager
            .initialize(&prefix_path, &runner)
            .await
            .map_err(|e| format!("Failed to initialize Wine prefix: {}", e))?;
        // Re-acquire settings lock
        let settings = state.settings.read().await;
        // Use settings for the rest of the function
        return launch_with_settings(&state, &settings, &game_path, &game_exe, &runner, &prefix_path, &account_id).await;
    }

    launch_with_settings(&state, &settings, &game_path, &game_exe, &runner, &prefix_path, &account_id).await
}

/// Helper to launch the game with settings
async fn launch_with_settings(
    state: &AppState,
    settings: &gaveloc_core::config::Settings,
    game_path: &PathBuf,
    game_exe: &PathBuf,
    runner: &gaveloc_core::entities::WineRunner,
    prefix_path: &PathBuf,
    account_id: &str,
) -> Result<(), String> {
    // 4. Get session info
    let id = AccountId::new(account_id);
    let session = state
        .credentials
        .get_session(&id)
        .await
        .map_err(|e| format!("Failed to get session: {}", e))?
        .ok_or_else(|| "No valid session - please login first".to_string())?;

    // Validate session is not expired
    if !session.is_valid() {
        return Err("Session expired - please login again".to_string());
    }

    // 5. Get game version
    let version_repo_guard = state.version_repo.read().await;
    let version_repo = version_repo_guard
        .as_ref()
        .ok_or_else(|| "Version repository not initialized".to_string())?;
    let game_version = version_repo
        .get_version(game_path, Repository::Ffxiv)
        .await
        .map_err(|e| format!("Failed to read game version: {}", e))?;

    // 6. Build launch arguments
    let encrypted_sid = EncryptedSessionId::new(&session.unique_id)
        .map_err(|e| format!("Failed to encrypt session: {}", e))?;

    // Get account for is_steam flag
    let account = state
        .accounts
        .get_account(&id)
        .await
        .map_err(|e| format!("Failed to get account: {}", e))?
        .ok_or_else(|| "Account not found".to_string())?;

    let launch_params = LaunchParams {
        session_id: &encrypted_sid,
        max_expansion: session.max_expansion,
        game_version: game_version.as_str(),
        is_steam: account.is_steam,
        region: Region::default(),
        language: settings.game.language,
    };

    let args = build_launch_args(&launch_params);

    // 7. Create launch config
    let launch_config = LaunchConfig {
        runner,
        prefix_path,
        game_path: game_exe,
        args: &args,
        wine_settings: &settings.wine,
        game_settings: &settings.game,
    };

    // Drop version repo lock before launching
    drop(version_repo_guard);

    // 8. Launch!
    state
        .process_launcher
        .launch(launch_config)
        .await
        .map_err(|e| format!("Failed to launch game: {}", e))?;

    // 9. Try to find the game PID after a short delay
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    if let Some(pid) = find_game_pid() {
        *state.game_pid.write().await = Some(pid);
    }

    Ok(())
}

/// Check if game is currently running
#[tauri::command]
pub async fn get_launch_status(state: State<'_, AppState>) -> Result<LaunchStatusDto, String> {
    let mut game_pid = state.game_pid.write().await;

    // Check if we have a stored PID
    if let Some(pid) = *game_pid {
        // Verify it's still running
        if is_process_running(pid) {
            return Ok(LaunchStatusDto {
                is_running: true,
                pid: Some(pid),
            });
        } else {
            // Process ended, clear PID
            *game_pid = None;
        }
    }

    // Try to find game process by name
    if let Some(pid) = find_game_pid() {
        *game_pid = Some(pid);
        return Ok(LaunchStatusDto {
            is_running: true,
            pid: Some(pid),
        });
    }

    Ok(LaunchStatusDto {
        is_running: false,
        pid: None,
    })
}

/// Helper: Find the game PID by process name
fn find_game_pid() -> Option<u32> {
    // Use pgrep to find ffxiv_dx11.exe
    let output = StdCommand::new("pgrep")
        .arg("-f")
        .arg("ffxiv_dx11.exe")
        .output()
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.lines().next().and_then(|line| line.trim().parse().ok())
    } else {
        None
    }
}

/// Helper: Check if a process is running by PID
fn is_process_running(pid: u32) -> bool {
    PathBuf::from(format!("/proc/{}", pid)).exists()
}
