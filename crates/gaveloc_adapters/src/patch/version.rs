//! Version file repository implementation
//!
//! Handles reading and writing version files from the game installation,
//! as well as generating version reports for session registration.

use std::path::Path;

use async_trait::async_trait;
use sha1::{Digest, Sha1};
use tokio::fs;
use tracing::instrument;

use gaveloc_core::entities::{GameVersion, Repository};
use gaveloc_core::error::Error;
use gaveloc_core::ports::VersionRepository;

/// Boot files that are hashed for version verification
const BOOT_FILES: &[&str] = &[
    "ffxivboot.exe",
    "ffxivboot64.exe",
    "ffxivlauncher.exe",
    "ffxivlauncher64.exe",
    "ffxivupdater.exe",
    "ffxivupdater64.exe",
];

/// File-based version repository implementation
pub struct FileVersionRepository;

impl FileVersionRepository {
    pub fn new() -> Self {
        Self
    }

    /// Get the full path to a version file
    fn version_file_path(game_path: &Path, repo: Repository) -> std::path::PathBuf {
        game_path.join(repo.version_file_path())
    }

    /// Hash a file using SHA1 and return hex-encoded result
    async fn hash_file(path: &Path) -> Result<String, Error> {
        let data = fs::read(path).await?;
        let mut hasher = Sha1::new();
        hasher.update(&data);
        let result = hasher.finalize();
        Ok(hex::encode(result))
    }
}

impl Default for FileVersionRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VersionRepository for FileVersionRepository {
    #[instrument(skip(self))]
    async fn get_version(&self, game_path: &Path, repo: Repository) -> Result<GameVersion, Error> {
        let ver_path = Self::version_file_path(game_path, repo);

        if !ver_path.exists() {
            return Err(Error::VersionFileNotFound(ver_path));
        }

        let content = fs::read_to_string(&ver_path).await?;
        GameVersion::parse(&content)
    }

    #[instrument(skip(self))]
    async fn set_version(
        &self,
        game_path: &Path,
        repo: Repository,
        version: &str,
    ) -> Result<(), Error> {
        let ver_path = Self::version_file_path(game_path, repo);

        // Backup the existing version file
        if ver_path.exists() {
            let backup_path = ver_path.with_extension("bck");
            fs::copy(&ver_path, &backup_path).await?;
        }

        // Write the new version
        fs::write(&ver_path, version).await?;

        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_boot_version_hash(&self, game_path: &Path) -> Result<String, Error> {
        let boot_path = game_path.join("boot");
        let mut hash_parts = Vec::new();

        for file_name in BOOT_FILES {
            let file_path = boot_path.join(file_name);
            if file_path.exists() {
                let hash = Self::hash_file(&file_path).await?;
                // Format: filename/length/hash
                let metadata = fs::metadata(&file_path).await?;
                hash_parts.push(format!("{}/{}/{}", file_name, metadata.len(), hash));
            }
        }

        // Join with commas for the version report
        Ok(hash_parts.join(","))
    }

    #[instrument(skip(self))]
    async fn get_version_report(
        &self,
        game_path: &Path,
        max_expansion: u32,
    ) -> Result<String, Error> {
        let mut report_lines = Vec::new();

        // Get all game versions up to max_expansion
        let repos = Repository::game_repos_up_to(max_expansion);

        for repo in repos {
            let version = self.get_version(game_path, repo).await?;
            // Format: "ex1/version" or "ffxiv/version"
            let repo_name = match repo {
                Repository::Ffxiv => "ffxiv",
                Repository::Ex1 => "ex1",
                Repository::Ex2 => "ex2",
                Repository::Ex3 => "ex3",
                Repository::Ex4 => "ex4",
                Repository::Ex5 => "ex5",
                Repository::Boot => continue, // Skip boot in game version report
            };
            report_lines.push(format!("{}/{}", repo_name, version.as_str()));
        }

        Ok(report_lines.join("\n"))
    }

    #[instrument(skip(self))]
    async fn validate_game_installation(&self, game_path: &Path) -> Result<bool, Error> {
        // Check for required directories
        let required_dirs = ["boot", "game", "game/sqpack"];
        for dir in required_dirs {
            let dir_path = game_path.join(dir);
            if !dir_path.exists() || !dir_path.is_dir() {
                tracing::debug!("Missing required directory: {}", dir);
                return Ok(false);
            }
        }

        // Check for boot version file
        let boot_ver = Self::version_file_path(game_path, Repository::Boot);
        if !boot_ver.exists() {
            tracing::debug!("Missing boot version file");
            return Ok(false);
        }

        // Check for game version file
        let game_ver = Self::version_file_path(game_path, Repository::Ffxiv);
        if !game_ver.exists() {
            tracing::debug!("Missing game version file");
            return Ok(false);
        }

        // Verify version files are parseable
        if let Err(e) = self.get_version(game_path, Repository::Boot).await {
            tracing::debug!("Invalid boot version file: {}", e);
            return Ok(false);
        }

        if let Err(e) = self.get_version(game_path, Repository::Ffxiv).await {
            tracing::debug!("Invalid game version file: {}", e);
            return Ok(false);
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::fs::create_dir_all;

    async fn setup_mock_game(dir: &Path) -> Result<(), Error> {
        // Create directory structure
        create_dir_all(dir.join("boot")).await?;
        create_dir_all(dir.join("game/sqpack/ffxiv")).await?;

        // Create version files
        fs::write(
            dir.join("boot/ffxivboot.ver"),
            "2024.07.23.0000.0001",
        )
        .await?;
        fs::write(
            dir.join("game/ffxivgame.ver"),
            "2024.07.23.0000.0001",
        )
        .await?;

        // Create a mock boot executable
        fs::write(dir.join("boot/ffxivboot.exe"), b"mock boot executable").await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_get_version() {
        let dir = tempdir().unwrap();
        setup_mock_game(dir.path()).await.unwrap();

        let repo = FileVersionRepository::new();
        let version = repo
            .get_version(dir.path(), Repository::Boot)
            .await
            .unwrap();

        assert_eq!(version.year, 2024);
        assert_eq!(version.month, 7);
        assert_eq!(version.day, 23);
    }

    #[tokio::test]
    async fn test_get_version_not_found() {
        let dir = tempdir().unwrap();
        let repo = FileVersionRepository::new();

        let result = repo.get_version(dir.path(), Repository::Boot).await;
        assert!(matches!(result, Err(Error::VersionFileNotFound(_))));
    }

    #[tokio::test]
    async fn test_set_version() {
        let dir = tempdir().unwrap();
        setup_mock_game(dir.path()).await.unwrap();

        let repo = FileVersionRepository::new();

        // Set new version
        repo.set_version(dir.path(), Repository::Ffxiv, "2024.08.01.0000.0000")
            .await
            .unwrap();

        // Verify the new version
        let version = repo
            .get_version(dir.path(), Repository::Ffxiv)
            .await
            .unwrap();
        assert_eq!(version.month, 8);
        assert_eq!(version.day, 1);

        // Verify backup was created
        assert!(dir.path().join("game/ffxivgame.bck").exists());
    }

    #[tokio::test]
    async fn test_validate_game_installation() {
        let dir = tempdir().unwrap();

        let repo = FileVersionRepository::new();

        // Empty directory should fail
        assert!(!repo.validate_game_installation(dir.path()).await.unwrap());

        // Setup mock game
        setup_mock_game(dir.path()).await.unwrap();

        // Should now pass
        assert!(repo.validate_game_installation(dir.path()).await.unwrap());
    }

    #[tokio::test]
    async fn test_get_boot_version_hash() {
        let dir = tempdir().unwrap();
        setup_mock_game(dir.path()).await.unwrap();

        let repo = FileVersionRepository::new();
        let hash = repo.get_boot_version_hash(dir.path()).await.unwrap();

        // Should contain the boot file we created
        assert!(hash.contains("ffxivboot.exe"));
        assert!(hash.contains("/20/")); // Length of "mock boot executable"
    }

    #[tokio::test]
    async fn test_get_version_report() {
        let dir = tempdir().unwrap();
        setup_mock_game(dir.path()).await.unwrap();

        let repo = FileVersionRepository::new();
        let report = repo.get_version_report(dir.path(), 0).await.unwrap();

        // Should contain ffxiv version
        assert!(report.contains("ffxiv/2024.07.23.0000.0001"));
    }
}
