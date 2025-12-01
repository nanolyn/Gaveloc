use std::path::{Path, PathBuf};
use directories::UserDirs;
use serde::{Deserialize, Serialize};

/// Result of validating a game path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub message: String,
}

/// Detects potential FFXIV installations on the system
pub fn detect_game_installations() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let user_dirs = UserDirs::new();

    if let Some(user_dirs) = user_dirs {
        let home = user_dirs.home_dir();

        // Common Linux paths
        let candidates = vec![
            // Steam default
            home.join(".local/share/Steam/steamapps/common/FINAL FANTASY XIV Online"),
            // Steam (ARR specific naming)
            home.join(".local/share/Steam/steamapps/common/FINAL FANTASY XIV - A Realm Reborn"),
            // Lutris default
            home.join("Games/final-fantasy-xiv-online"),
            // Common manual install
            home.join("Games/ffxiv"),
            // XIVLauncher (Wine) default location inside prefix
            home.join(".xlcore/ffxiv"),
        ];

        for path in candidates {
            if is_valid_game_path(&path) {
                paths.push(path);
            }
        }
    }

    paths
}

/// Checks if a directory looks like a valid FFXIV installation
pub fn is_valid_game_path(path: &Path) -> bool {
    if !path.exists() || !path.is_dir() {
        return false;
    }

    let boot_dir = path.join("boot");
    let game_dir = path.join("game");
    let boot_ver = boot_dir.join("ffxivboot.ver");

    boot_dir.exists() && game_dir.exists() && boot_ver.exists()
}

/// Validates a game path and returns detailed information about any issues
pub fn validate_game_path(path: &Path) -> ValidationResult {
    if !path.exists() {
        return ValidationResult {
            valid: false,
            message: "Path does not exist".to_string(),
        };
    }

    if !path.is_dir() {
        return ValidationResult {
            valid: false,
            message: "Path is not a directory".to_string(),
        };
    }

    let boot_dir = path.join("boot");
    let game_dir = path.join("game");
    let boot_ver = boot_dir.join("ffxivboot.ver");

    if !boot_dir.exists() {
        return ValidationResult {
            valid: false,
            message: "Missing 'boot' directory - not a valid FFXIV installation".to_string(),
        };
    }

    if !game_dir.exists() {
        return ValidationResult {
            valid: false,
            message: "Missing 'game' directory - not a valid FFXIV installation".to_string(),
        };
    }

    if !boot_ver.exists() {
        return ValidationResult {
            valid: false,
            message: "Missing boot version file - game may need to be installed first".to_string(),
        };
    }

    ValidationResult {
        valid: true,
        message: "Valid FFXIV installation found".to_string(),
    }
}

/// Returns a default installation path for new installs
pub fn get_default_install_path() -> PathBuf {
    let user_dirs = UserDirs::new();
    match user_dirs {
        Some(dirs) => dirs.home_dir().join("Games/ffxiv"),
        None => PathBuf::from("Games/ffxiv"),
    }
}
