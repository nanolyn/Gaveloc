use std::path::PathBuf;

use async_trait::async_trait;
use gaveloc_core::config::Settings;
use gaveloc_core::ports::ConfigRepository;
use gaveloc_core::Error;
use tokio::fs;
use tracing::{debug, instrument};

/// File-based configuration repository using TOML format
pub struct FileConfigRepository {
    config_path: PathBuf,
}

impl FileConfigRepository {
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            config_path: config_dir.join("config.toml"),
        }
    }

    /// Get the path to the config file
    pub fn config_path(&self) -> &PathBuf {
        &self.config_path
    }
}

#[async_trait]
impl ConfigRepository for FileConfigRepository {
    #[instrument(skip(self))]
    async fn load_settings(&self) -> Result<Settings, Error> {
        if !fs::try_exists(&self.config_path).await.unwrap_or(false) {
            debug!("config file not found, returning defaults");
            return Ok(Settings::default());
        }

        let content = fs::read_to_string(&self.config_path).await?;
        toml::from_str(&content)
            .map_err(|e| Error::Other(format!("failed to parse config file: {}", e)))
    }

    #[instrument(skip(self, settings))]
    async fn save_settings(&self, settings: &Settings) -> Result<(), Error> {
        // Ensure parent directory exists
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        debug!("saving settings to {:?}", self.config_path);

        let content = toml::to_string_pretty(settings)
            .map_err(|e| Error::Other(format!("failed to serialize settings: {}", e)))?;
        fs::write(&self.config_path, content).await?;
        Ok(())
    }

    async fn exists(&self) -> bool {
        fs::try_exists(&self.config_path).await.unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gaveloc_core::config::Language;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_load_defaults_when_no_file() {
        let dir = tempdir().unwrap();
        let repo = FileConfigRepository::new(dir.path().to_path_buf());

        let settings = repo.load_settings().await.unwrap();

        // Should return defaults
        assert_eq!(settings.game.language, Language::English);
        assert!(settings.wine.esync);
    }

    #[tokio::test]
    async fn test_save_and_load_roundtrip() {
        let dir = tempdir().unwrap();
        let repo = FileConfigRepository::new(dir.path().to_path_buf());

        let mut settings = Settings::default();
        settings.game.language = Language::Japanese;
        settings.game.gamemode = false;
        settings.wine.fsync = false;
        settings.log_level = "debug".to_string();

        // Save
        repo.save_settings(&settings).await.unwrap();

        // Load
        let loaded = repo.load_settings().await.unwrap();

        assert_eq!(loaded.game.language, Language::Japanese);
        assert!(!loaded.game.gamemode);
        assert!(!loaded.wine.fsync);
        assert_eq!(loaded.log_level, "debug");
    }

    #[tokio::test]
    async fn test_exists() {
        let dir = tempdir().unwrap();
        let repo = FileConfigRepository::new(dir.path().to_path_buf());

        // Should not exist initially
        assert!(!repo.exists().await);

        // Save settings
        repo.save_settings(&Settings::default()).await.unwrap();

        // Should exist now
        assert!(repo.exists().await);
    }

    #[tokio::test]
    async fn test_update_settings() {
        let dir = tempdir().unwrap();
        let repo = FileConfigRepository::new(dir.path().to_path_buf());

        // Save initial settings
        let mut settings = Settings::default();
        settings.game.mangohud = false;
        repo.save_settings(&settings).await.unwrap();

        // Update
        settings.game.mangohud = true;
        settings.game.gamescope = true;
        repo.save_settings(&settings).await.unwrap();

        // Verify update
        let loaded = repo.load_settings().await.unwrap();
        assert!(loaded.game.mangohud);
        assert!(loaded.game.gamescope);
    }

    #[tokio::test]
    async fn test_partial_config_uses_defaults() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");

        // Write a partial config (only game.language)
        let partial_config = r#"
[game]
language = "japanese"
"#;
        tokio::fs::write(&config_path, partial_config).await.unwrap();

        let repo = FileConfigRepository::new(dir.path().to_path_buf());
        let settings = repo.load_settings().await.unwrap();

        // Specified value
        assert_eq!(settings.game.language, Language::Japanese);
        // Default values for unspecified fields
        assert!(settings.wine.esync);
    }

    #[tokio::test]
    async fn test_creates_parent_directory() {
        let dir = tempdir().unwrap();
        let nested_dir = dir.path().join("nested").join("config");
        let repo = FileConfigRepository::new(nested_dir.clone());

        // Directory shouldn't exist yet
        assert!(!nested_dir.exists());

        // Save should create it
        repo.save_settings(&Settings::default()).await.unwrap();

        // Now it should exist
        assert!(nested_dir.exists());
        assert!(repo.exists().await);
    }

    #[tokio::test]
    async fn test_gamescope_settings_roundtrip() {
        let dir = tempdir().unwrap();
        let repo = FileConfigRepository::new(dir.path().to_path_buf());

        let mut settings = Settings::default();
        settings.game.gamescope = true;
        settings.game.gamescope_settings.width = Some(1920);
        settings.game.gamescope_settings.height = Some(1080);
        settings.game.gamescope_settings.refresh_rate = Some(144);
        settings.game.gamescope_settings.fullscreen = true;

        repo.save_settings(&settings).await.unwrap();
        let loaded = repo.load_settings().await.unwrap();

        assert!(loaded.game.gamescope);
        assert_eq!(loaded.game.gamescope_settings.width, Some(1920));
        assert_eq!(loaded.game.gamescope_settings.height, Some(1080));
        assert_eq!(loaded.game.gamescope_settings.refresh_rate, Some(144));
        assert!(loaded.game.gamescope_settings.fullscreen);
    }

    #[tokio::test]
    async fn test_wine_settings_roundtrip() {
        let dir = tempdir().unwrap();
        let repo = FileConfigRepository::new(dir.path().to_path_buf());

        let mut settings = Settings::default();
        settings.wine.runner_path = Some(PathBuf::from("/opt/wine/bin/wine"));
        settings.wine.prefix_path = Some(PathBuf::from("/home/user/.wine"));
        settings.wine.esync = false;
        settings.wine.fsync = true;
        settings.wine.winesync = true;
        settings.wine.dxvk_hud = Some("fps,frametimes".to_string());

        repo.save_settings(&settings).await.unwrap();
        let loaded = repo.load_settings().await.unwrap();

        assert_eq!(
            loaded.wine.runner_path,
            Some(PathBuf::from("/opt/wine/bin/wine"))
        );
        assert_eq!(
            loaded.wine.prefix_path,
            Some(PathBuf::from("/home/user/.wine"))
        );
        assert!(!loaded.wine.esync);
        assert!(loaded.wine.fsync);
        assert!(loaded.wine.winesync);
        assert_eq!(loaded.wine.dxvk_hud, Some("fps,frametimes".to_string()));
    }
}
