// Command modules - will be implemented in subsequent patches
pub mod accounts;
pub mod auth;
pub mod integrity;
pub mod launcher;
pub mod news;
pub mod patching;
pub mod runners;
pub mod settings;
pub mod version;

#[tauri::command]
pub fn health_check() -> String {
    "OK".to_string()
}

