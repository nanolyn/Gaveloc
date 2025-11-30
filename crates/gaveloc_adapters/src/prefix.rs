use std::path::Path;

use async_trait::async_trait;
use gaveloc_core::entities::WineRunner;
use gaveloc_core::error::Error;
use gaveloc_core::ports::PrefixManager;
use tokio::fs;
use tokio::process::Command;
use tracing::info;

#[derive(Default)]
pub struct LinuxPrefixManager;

impl LinuxPrefixManager {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl PrefixManager for LinuxPrefixManager {
    async fn exists(&self, prefix_path: &Path) -> bool {
        // A healthy Wine prefix should have these registry files
        const MARKERS: &[&str] = &["system.reg", "user.reg", "userdef.reg"];

        for marker in MARKERS {
            if !fs::try_exists(prefix_path.join(marker))
                .await
                .unwrap_or(false)
            {
                return false;
            }
        }

        // Also verify drive_c exists
        fs::try_exists(prefix_path.join("drive_c"))
            .await
            .unwrap_or(false)
    }

    async fn initialize(&self, prefix_path: &Path, runner: &WineRunner) -> Result<(), Error> {
        info!(path = %prefix_path.display(), "initializing wine prefix");

        if !fs::try_exists(prefix_path).await.unwrap_or(false) {
            fs::create_dir_all(prefix_path).await?;
        }

        let wine_dir = runner
            .path
            .parent()
            .ok_or_else(|| Error::InvalidRunnerPath(runner.path.clone()))?;

        let wineboot_path = wine_dir.join("wineboot");

        let mut cmd = if wineboot_path.exists() {
            Command::new(&wineboot_path)
        } else {
            let mut cmd = Command::new(&runner.path);
            cmd.arg("wineboot");
            cmd
        };

        cmd.arg("-u")
            .env("WINEPREFIX", prefix_path)
            .env("WINEARCH", "win64")
            .kill_on_drop(true);

        info!("running wineboot");
        let output = cmd.output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::PrefixInitialization(stderr.into_owned()));
        }

        info!("prefix initialization complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use tempfile::tempdir;
    use std::fs::File;
    use std::path::PathBuf; // Added this import

    #[test]
    fn test_prefix_manager_new() {
        let _ = LinuxPrefixManager::new();
    }

    #[tokio::test]
    async fn test_exists_all_markers_present() {
        let manager = LinuxPrefixManager::new();
        let tmp_dir = tempdir().unwrap();
        let prefix_path = tmp_dir.path();

        // Create marker files
        File::create(prefix_path.join("system.reg")).unwrap();
        File::create(prefix_path.join("user.reg")).unwrap();
        File::create(prefix_path.join("userdef.reg")).unwrap();
        std::fs::create_dir(prefix_path.join("drive_c")).unwrap();

        assert!(manager.exists(prefix_path).await);
    }

    #[tokio::test]
    async fn test_exists_missing_marker_file() {
        let manager = LinuxPrefixManager::new();
        let tmp_dir = tempdir().unwrap();
        let prefix_path = tmp_dir.path();

        // Create some marker files, but leave one out
        File::create(prefix_path.join("system.reg")).unwrap();
        File::create(prefix_path.join("user.reg")).unwrap();
        std::fs::create_dir(prefix_path.join("drive_c")).unwrap();

        assert!(!manager.exists(prefix_path).await);
    }

    #[tokio::test]
    async fn test_exists_missing_drive_c() {
        let manager = LinuxPrefixManager::new();
        let tmp_dir = tempdir().unwrap();
        let prefix_path = tmp_dir.path();

        // Create marker files, but no drive_c
        File::create(prefix_path.join("system.reg")).unwrap();
        File::create(prefix_path.join("user.reg")).unwrap();
        File::create(prefix_path.join("userdef.reg")).unwrap();

        assert!(!manager.exists(prefix_path).await);
    }

    #[tokio::test]
    async fn test_exists_empty_prefix_path() {
        let manager = LinuxPrefixManager::new();
        let tmp_dir = tempdir().unwrap();
        let prefix_path = tmp_dir.path();

        assert!(!manager.exists(prefix_path).await);
    }

    // For `initialize`, we can test that it creates the directory if it doesn't exist
    #[tokio::test]
    async fn test_initialize_creates_directory() {
        let manager = LinuxPrefixManager::new();
        let tmp_dir = tempdir().unwrap();
        let new_prefix_path = tmp_dir.path().join("new_prefix");

        let dummy_runner = WineRunner {
            path: PathBuf::from("/nonexistent/runner"), // This runner path won't be used for command execution in this specific test
            name: "Dummy".to_string(),
            runner_type: gaveloc_core::entities::RunnerType::Custom,
            is_valid: true,
        };

        // We expect `initialize` to fail at the `Command::output().await?` step
        // because the dummy_runner.path does not exist and wineboot will not run.
        // However, we can assert that the directory is created.
        let result = manager.initialize(&new_prefix_path, &dummy_runner).await;
        
        // Assert that the directory was created
        assert!(new_prefix_path.exists());
        
        // We expect an error due to the dummy runner path, but we're primarily testing
        // directory creation here.
        assert!(result.is_err()); 
        if let Err(Error::Io(_)) = result {
            // Expected I/O error when trying to run a nonexistent command
        } else {
            panic!("Expected an Io error but got {:?}", result);
        }
    }
}
