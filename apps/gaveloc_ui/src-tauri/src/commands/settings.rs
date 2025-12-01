use std::path::PathBuf;

use gaveloc_core::config::Settings;
use gaveloc_adapters::game_detection::{self, ValidationResult};
use tauri::State;
use tokio::fs;

use crate::state::AppState;

/// Get the config file path
fn get_config_path() -> PathBuf {
    directories::ProjectDirs::from("com", "gaveloc", "gaveloc")
        .map(|d| d.config_dir().join("config.toml"))
        .unwrap_or_else(|| PathBuf::from("config.toml"))
}

/// Get current settings
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    let settings = state.settings.read().await;
    Ok(settings.clone())
}

/// Save settings to config file
#[tauri::command]
pub async fn save_settings(
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<(), String> {
    // Update in-memory settings
    {
        let mut current = state.settings.write().await;
        *current = settings.clone();
    }

    // Serialize to TOML
    let toml_str = toml::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    // Ensure config directory exists
    let config_path = get_config_path();
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    // Write to file
    fs::write(&config_path, toml_str)
        .await
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    Ok(())
}

/// Validate that a path is a valid FFXIV installation
#[tauri::command]
pub async fn validate_game_path(path: String) -> Result<ValidationResult, String> {
    let path = PathBuf::from(&path);
    Ok(game_detection::validate_game_path(&path))
}

/// Detect existing game installations
#[tauri::command]
pub async fn detect_game_install() -> Result<Option<PathBuf>, String> {
    let paths = game_detection::detect_game_installations();
    Ok(paths.into_iter().next())
}

/// Get the default path for a new installation
#[tauri::command]
pub async fn get_default_install_path() -> Result<PathBuf, String> {
    Ok(game_detection::get_default_install_path())
}