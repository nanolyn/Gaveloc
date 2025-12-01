use std::path::PathBuf;

use gaveloc_core::entities::WineRunner;
use gaveloc_core::ports::RunnerDetector;
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::fs;

use crate::state::AppState;

/// Get the config file path
fn get_config_path() -> PathBuf {
    directories::ProjectDirs::from("com", "gaveloc", "gaveloc")
        .map(|d| d.config_dir().join("config.toml"))
        .unwrap_or_else(|| PathBuf::from("config.toml"))
}

/// DTO for WineRunner to send to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WineRunnerDto {
    pub path: String,
    pub name: String,
    pub runner_type: String,
    pub is_valid: bool,
}

impl From<WineRunner> for WineRunnerDto {
    fn from(runner: WineRunner) -> Self {
        Self {
            path: runner.path.to_string_lossy().to_string(),
            name: runner.name,
            runner_type: runner.runner_type.to_string(),
            is_valid: runner.is_valid,
        }
    }
}

/// List all detected runners on the system
#[tauri::command]
pub async fn list_runners(state: State<'_, AppState>) -> Result<Vec<WineRunnerDto>, String> {
    state
        .runner_detector
        .detect_runners()
        .await
        .map(|runners| runners.into_iter().map(WineRunnerDto::from).collect())
        .map_err(|e| e.to_string())
}

/// Validate a custom runner path
#[tauri::command]
pub async fn validate_runner(
    state: State<'_, AppState>,
    path: String,
) -> Result<WineRunnerDto, String> {
    state
        .runner_detector
        .validate_runner(PathBuf::from(path))
        .await
        .map(WineRunnerDto::from)
        .map_err(|e| e.to_string())
}

/// Get the currently selected runner from settings, or first detected if none saved
#[tauri::command]
pub async fn get_selected_runner(
    state: State<'_, AppState>,
) -> Result<Option<WineRunnerDto>, String> {
    let settings = state.settings.read().await;

    if let Some(ref path) = settings.wine.runner_path {
        // User has a saved runner path - validate it
        match state.runner_detector.validate_runner(path.clone()).await {
            Ok(runner) => return Ok(Some(WineRunnerDto::from(runner))),
            Err(_) => {
                // Saved runner is invalid, fall through to auto-detect
            }
        }
    }

    // No saved runner or invalid - return first detected
    let runners = state
        .runner_detector
        .detect_runners()
        .await
        .map_err(|e| e.to_string())?;
    Ok(runners.into_iter().next().map(WineRunnerDto::from))
}

/// Select a runner by path (validates and saves to settings)
/// If path is None, uses the first available detected runner
#[tauri::command]
pub async fn select_runner(
    state: State<'_, AppState>,
    path: Option<String>,
) -> Result<WineRunnerDto, String> {
    let runner = match path {
        Some(p) => {
            // Validate the provided path
            state
                .runner_detector
                .validate_runner(PathBuf::from(&p))
                .await
                .map_err(|e| e.to_string())?
        }
        None => {
            // Auto-detect: use first available
            let runners = state
                .runner_detector
                .detect_runners()
                .await
                .map_err(|e| e.to_string())?;
            runners
                .into_iter()
                .next()
                .ok_or_else(|| "No runners detected".to_string())?
        }
    };

    // Save to settings
    {
        let mut settings = state.settings.write().await;
        settings.wine.runner_path = Some(runner.path.clone());
    }

    // Persist settings to disk
    let settings = state.settings.read().await;
    let toml_str = toml::to_string_pretty(&*settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    let config_path = get_config_path();
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    fs::write(&config_path, toml_str)
        .await
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    Ok(WineRunnerDto::from(runner))
}
