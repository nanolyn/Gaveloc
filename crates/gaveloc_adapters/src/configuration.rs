use std::path::PathBuf;

use config::{Config, Environment, File};
use directories::ProjectDirs;
use gaveloc_core::config::Settings;

pub fn get_configuration_with_paths(
    current_dir_path: Option<PathBuf>,
    system_config_dir_path: Option<PathBuf>,
) -> Result<Settings, config::ConfigError> {
    let config_directory = current_dir_path.unwrap_or_else(|| {
        std::env::current_dir()
            .map(|p| p.join("config"))
            .unwrap_or_else(|_| PathBuf::from("config"))
    });

    // Correctly use system_config_dir_path if provided, otherwise default.
    let system_config_dir = if let Some(path) = system_config_dir_path {
        path
    } else {
        ProjectDirs::from("com", "gaveloc", "gaveloc")
            .map(|d| d.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("config"))
    };

    let settings = Config::builder()
        // Game settings (enums serialize to lowercase strings)
        .set_default("game.region", "europe")?
        .set_default("game.language", "english")?
        .set_default("game.gamemode", true)?
        .set_default("game.mangohud", false)?
        .set_default("game.gamescope", false)?
        // Gamescope settings (all optional, only used when gamescope=true)
        .set_default("game.gamescope_settings.fullscreen", false)?
        .set_default("game.gamescope_settings.borderless", false)?
        // Wine settings
        .set_default("wine.esync", true)?
        .set_default("wine.fsync", true)?
        .set_default("wine.winesync", false)?
        .set_default("log_level", "info")?
        .add_source(File::from(system_config_dir.join("config.toml")).required(false))
        .add_source(File::from(config_directory.join("config.toml")).required(false))
        .add_source(Environment::with_prefix("GAVELOC").separator("__"))
        .build()?;

    settings.try_deserialize::<Settings>()
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    get_configuration_with_paths(None, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gaveloc_core::config::Language;
    use tempfile::tempdir;
    use std::io::Write;
    use serial_test::serial; // Import serial_test

    #[serial]
    #[test]
    fn test_get_configuration_defaults() {
        // Clear all relevant environment variables to ensure isolation
        for (key, _) in std::env::vars() {
            if key.starts_with("GAVELOC__") {
                std::env::remove_var(&key);
            }
        }
        // Use dummy paths for config directories
        let settings = get_configuration_with_paths(Some(PathBuf::from("/nonexistent")), Some(PathBuf::from("/nonexistent"))).unwrap();

        assert_eq!(settings.game.language, Language::English);
        assert!(settings.game.gamemode);
        assert!(!settings.game.mangohud);
        assert!(!settings.game.gamescope);
        assert!(settings.wine.esync);
        assert!(settings.wine.fsync);
        assert!(!settings.wine.winesync);
        assert_eq!(settings.log_level, "info");
    }

    #[serial]
    #[test]
    fn test_get_configuration_file_override() {
        // Clear all relevant environment variables to ensure isolation
        for (key, _) in std::env::vars() {
            if key.starts_with("GAVELOC__") {
                std::env::remove_var(&key);
            }
        }

        let dir = tempdir().unwrap();
        let config_file_path = dir.path().join("config.toml");

        let config_content = r#"
        game.language = "japanese"
        game.gamemode = false
        log_level = "debug"
        "#;

        let mut file = std::fs::File::create(&config_file_path).unwrap();
        file.write_all(config_content.as_bytes()).unwrap();

        let settings = get_configuration_with_paths(Some(dir.path().to_path_buf()), Some(PathBuf::from("/nonexistent"))).unwrap();

        assert_eq!(settings.game.language, Language::Japanese);
        assert!(!settings.game.gamemode);
        assert_eq!(settings.log_level, "debug");
    }

    #[serial]
    #[test]
    fn test_get_configuration_env_override() {
        // Clear all relevant environment variables to ensure isolation
        for (key, _) in std::env::vars() {
            if key.starts_with("GAVELOC__") {
                std::env::remove_var(&key);
            }
        }

        std::env::set_var("GAVELOC__GAME__MANGOHUD", "true");
        std::env::set_var("GAVELOC__LOG_LEVEL", "trace");

        let settings = get_configuration_with_paths(Some(PathBuf::from("/nonexistent")), Some(PathBuf::from("/nonexistent"))).unwrap();

        assert!(settings.game.mangohud);
        assert_eq!(settings.log_level, "trace");

        std::env::remove_var("GAVELOC__GAME__MANGOHUD");
        std::env::remove_var("GAVELOC__LOG_LEVEL");
    }

    #[serial]
    #[test]
    fn test_get_configuration_precedence_env_over_file() {
        // Clear all relevant environment variables to ensure isolation
        for (key, _) in std::env::vars() {
            if key.starts_with("GAVELOC__") {
                std::env::remove_var(&key);
            }
        }

        let dir = tempdir().unwrap();
        let config_file_path = dir.path().join("config.toml");

        let config_content = r#"
        game.language = "japanese"
        log_level = "debug"
        "#;

        let mut file = std::fs::File::create(&config_file_path).unwrap();
        file.write_all(config_content.as_bytes()).unwrap();

        std::env::set_var("GAVELOC__GAME__LANGUAGE", "german");
        std::env::set_var("GAVELOC__LOG_LEVEL", "trace");

        let settings = get_configuration_with_paths(Some(dir.path().to_path_buf()), Some(PathBuf::from("/nonexistent"))).unwrap();

        // Environment variables should take precedence over file settings
        assert_eq!(settings.game.language, Language::German);
        assert_eq!(settings.log_level, "trace");

        std::env::remove_var("GAVELOC__GAME__LANGUAGE");
        std::env::remove_var("GAVELOC__LOG_LEVEL");
    }
}
