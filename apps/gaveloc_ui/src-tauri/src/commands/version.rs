use std::path::PathBuf;

use gaveloc_adapters::FileVersionRepository;
use gaveloc_core::entities::{AccountId, Repository};
use gaveloc_core::ports::{CredentialStore, PatchServer, VersionRepository};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

/// DTO for game versions
#[derive(Debug, Serialize, Deserialize)]
pub struct GameVersionsDto {
    pub boot: Option<String>,
    pub game: Option<String>,
    pub expansions: Vec<ExpansionVersionDto>,
    pub game_path_valid: bool,
}

/// DTO for expansion version
#[derive(Debug, Serialize, Deserialize)]
pub struct ExpansionVersionDto {
    pub name: String,
    pub version: Option<String>,
    pub installed: bool,
}

/// DTO for patch entry
#[derive(Debug, Serialize, Deserialize)]
pub struct PatchEntryDto {
    pub version_id: String,
    pub url: String,
    pub size_bytes: u64,
    pub repository: String,
}

/// DTO for update check result
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCheckResultDto {
    pub has_updates: bool,
    pub patches: Vec<PatchEntryDto>,
    pub total_size_bytes: u64,
    pub error: Option<String>,
}

/// Repository display names
fn repository_name(repo: Repository) -> &'static str {
    match repo {
        Repository::Boot => "Boot",
        Repository::Ffxiv => "FFXIV",
        Repository::Ex1 => "Ex1",
        Repository::Ex2 => "Ex2",
        Repository::Ex3 => "Ex3",
        Repository::Ex4 => "Ex4",
        Repository::Ex5 => "Ex5",
    }
}

/// Expansion display names
fn expansion_display_name(expansion: u32) -> &'static str {
    match expansion {
        1 => "Heavensward",
        2 => "Stormblood",
        3 => "Shadowbringers",
        4 => "Endwalker",
        5 => "Dawntrail",
        _ => "Unknown",
    }
}

/// Get game path from settings
async fn get_game_path(state: &AppState) -> Option<PathBuf> {
    let settings = state.settings.read().await;
    settings.game.path.clone()
}

/// Initialize version repository when game path is set
#[tauri::command]
pub async fn init_version_repo(state: State<'_, AppState>) -> Result<(), String> {
    let game_path = get_game_path(&state).await;

    if game_path.is_none() {
        return Err("Game path not configured".to_string());
    }

    // Initialize version repository
    let repo = FileVersionRepository::new();
    *state.version_repo.write().await = Some(repo);

    Ok(())
}

/// Get all installed game versions
#[tauri::command]
pub async fn get_game_versions(state: State<'_, AppState>) -> Result<GameVersionsDto, String> {
    let game_path = match get_game_path(&state).await {
        Some(path) => path,
        None => {
            return Ok(GameVersionsDto {
                boot: None,
                game: None,
                expansions: vec![],
                game_path_valid: false,
            });
        }
    };

    // Ensure version repo is initialized
    let version_repo_guard = state.version_repo.read().await;
    let version_repo = match version_repo_guard.as_ref() {
        Some(repo) => repo,
        None => {
            // Try to initialize
            drop(version_repo_guard);
            let repo = FileVersionRepository::new();
            *state.version_repo.write().await = Some(repo);
            // Re-acquire read lock
            let guard = state.version_repo.read().await;
            if guard.is_none() {
                return Err("Failed to initialize version repository".to_string());
            }
            // Need to return here since we can't hold the guard across the rest of the function
            drop(guard);

            // Re-read with fresh lock
            let guard = state.version_repo.read().await;
            let repo = guard.as_ref().unwrap();
            return get_versions_from_repo(repo, &game_path).await;
        }
    };

    get_versions_from_repo(version_repo, &game_path).await
}

async fn get_versions_from_repo(
    version_repo: &FileVersionRepository,
    game_path: &PathBuf,
) -> Result<GameVersionsDto, String> {
    // Validate game installation
    let is_valid = version_repo
        .validate_game_installation(game_path)
        .await
        .unwrap_or(false);

    if !is_valid {
        return Ok(GameVersionsDto {
            boot: None,
            game: None,
            expansions: vec![],
            game_path_valid: false,
        });
    }

    // Get boot version
    let boot = version_repo
        .get_version(game_path, Repository::Boot)
        .await
        .ok()
        .map(|v| v.as_str().to_string());

    // Get game version
    let game = version_repo
        .get_version(game_path, Repository::Ffxiv)
        .await
        .ok()
        .map(|v| v.as_str().to_string());

    // Get expansion versions
    let mut expansions = Vec::new();
    for exp in 1..=5 {
        let repo = Repository::from_expansion(exp).unwrap();
        let version = version_repo
            .get_version(game_path, repo)
            .await
            .ok()
            .map(|v| v.as_str().to_string());

        expansions.push(ExpansionVersionDto {
            name: expansion_display_name(exp).to_string(),
            installed: version.is_some(),
            version,
        });
    }

    Ok(GameVersionsDto {
        boot,
        game,
        expansions,
        game_path_valid: true,
    })
}

/// Check for boot updates (no login required)
#[tauri::command]
pub async fn check_boot_updates(state: State<'_, AppState>) -> Result<UpdateCheckResultDto, String> {
    let game_path = match get_game_path(&state).await {
        Some(path) => path,
        None => {
            return Ok(UpdateCheckResultDto {
                has_updates: false,
                patches: vec![],
                total_size_bytes: 0,
                error: Some("Game path not configured".to_string()),
            });
        }
    };

    // Get version repo
    let version_repo_guard = state.version_repo.read().await;
    let version_repo = match version_repo_guard.as_ref() {
        Some(repo) => repo,
        None => {
            return Ok(UpdateCheckResultDto {
                has_updates: false,
                patches: vec![],
                total_size_bytes: 0,
                error: Some("Version repository not initialized".to_string()),
            });
        }
    };

    // Get boot version
    let boot_version = match version_repo.get_version(&game_path, Repository::Boot).await {
        Ok(v) => v,
        Err(e) => {
            return Ok(UpdateCheckResultDto {
                has_updates: false,
                patches: vec![],
                total_size_bytes: 0,
                error: Some(format!("Failed to read boot version: {}", e)),
            });
        }
    };

    // Check for boot updates
    match state
        .patch_server
        .check_boot_version(&game_path, &boot_version)
        .await
    {
        Ok(patches) => {
            let total_size: u64 = patches.iter().map(|p| p.length).sum();
            let patch_dtos: Vec<PatchEntryDto> = patches
                .iter()
                .map(|p| PatchEntryDto {
                    version_id: p.version_id.clone(),
                    url: p.url.clone(),
                    size_bytes: p.length,
                    repository: repository_name(p.repository).to_string(),
                })
                .collect();

            Ok(UpdateCheckResultDto {
                has_updates: !patches.is_empty(),
                patches: patch_dtos,
                total_size_bytes: total_size,
                error: None,
            })
        }
        Err(e) => Ok(UpdateCheckResultDto {
            has_updates: false,
            patches: vec![],
            total_size_bytes: 0,
            error: Some(e.to_string()),
        }),
    }
}

/// Check for game updates (requires valid session)
#[tauri::command]
pub async fn check_game_updates(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<UpdateCheckResultDto, String> {
    let game_path = match get_game_path(&state).await {
        Some(path) => path,
        None => {
            return Ok(UpdateCheckResultDto {
                has_updates: false,
                patches: vec![],
                total_size_bytes: 0,
                error: Some("Game path not configured".to_string()),
            });
        }
    };

    // Get cached session for the account
    let id = AccountId::new(&account_id);
    let session = match state.credentials.get_session(&id).await {
        Ok(Some(s)) if s.is_valid() => s,
        Ok(Some(_)) => {
            return Ok(UpdateCheckResultDto {
                has_updates: false,
                patches: vec![],
                total_size_bytes: 0,
                error: Some("Session expired. Please log in again.".to_string()),
            });
        }
        Ok(None) => {
            return Ok(UpdateCheckResultDto {
                has_updates: false,
                patches: vec![],
                total_size_bytes: 0,
                error: Some("Not logged in. Please log in first.".to_string()),
            });
        }
        Err(e) => {
            return Ok(UpdateCheckResultDto {
                has_updates: false,
                patches: vec![],
                total_size_bytes: 0,
                error: Some(format!("Failed to get session: {}", e)),
            });
        }
    };

    // Register session and check for game updates
    match state
        .patch_server
        .register_session(&session.unique_id, &game_path, session.max_expansion)
        .await
    {
        Ok((_unique_id, patches)) => {
            let total_size: u64 = patches.iter().map(|p| p.length).sum();
            let patch_dtos: Vec<PatchEntryDto> = patches
                .iter()
                .map(|p| PatchEntryDto {
                    version_id: p.version_id.clone(),
                    url: p.url.clone(),
                    size_bytes: p.length,
                    repository: repository_name(p.repository).to_string(),
                })
                .collect();

            Ok(UpdateCheckResultDto {
                has_updates: !patches.is_empty(),
                patches: patch_dtos,
                total_size_bytes: total_size,
                error: None,
            })
        }
        Err(e) => Ok(UpdateCheckResultDto {
            has_updates: false,
            patches: vec![],
            total_size_bytes: 0,
            error: Some(e.to_string()),
        }),
    }
}
