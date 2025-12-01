use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::entities::{PatchEntry, Repository};
use crate::error::Error;
use crate::ports::{PatchDownloader, PatchServer, VersionRepository, ZiPatchApplier};

/// Stage of the update process
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStage {
    /// Checking for available patches
    CheckingPatches,
    /// Downloading a patch file
    Downloading {
        patch_index: usize,
        total_patches: usize,
        repository: Repository,
        version: String,
    },
    /// Verifying a downloaded patch
    Verifying {
        patch_index: usize,
        total_patches: usize,
    },
    /// Applying a patch to the game
    Applying {
        patch_index: usize,
        total_patches: usize,
        repository: Repository,
        version: String,
    },
    /// Update completed successfully
    Completed,
    /// Update failed
    Failed { error: String },
}

/// Progress information for the update process
#[derive(Debug, Clone)]
pub struct UpdateProgress {
    pub stage: UpdateStage,
    pub bytes_downloaded: u64,
    pub bytes_total: u64,
    pub overall_progress: f64,
}

impl UpdateProgress {
    pub fn download_percent(&self) -> f64 {
        if self.bytes_total == 0 {
            0.0
        } else {
            (self.bytes_downloaded as f64 / self.bytes_total as f64) * 100.0
        }
    }
}

/// Orchestrates the complete game update flow including:
/// - Boot patch checking and application
/// - Game patch checking via session registration
/// - Patch downloading with verification
/// - Patch application using ZiPatch
/// - Version file updates
pub struct UpdateGameUseCase<P, D, Z, V>
where
    P: PatchServer,
    D: PatchDownloader,
    Z: ZiPatchApplier + 'static,
    V: VersionRepository,
{
    patch_server: Arc<P>,
    downloader: Arc<D>,
    applier: Arc<Z>,
    version_repo: Arc<V>,
    patch_dir: PathBuf,
}

impl<P, D, Z, V> UpdateGameUseCase<P, D, Z, V>
where
    P: PatchServer,
    D: PatchDownloader,
    Z: ZiPatchApplier + 'static,
    V: VersionRepository,
{
    pub fn new(
        patch_server: Arc<P>,
        downloader: Arc<D>,
        applier: Arc<Z>,
        version_repo: Arc<V>,
        patch_dir: PathBuf,
    ) -> Self {
        Self {
            patch_server,
            downloader,
            applier,
            version_repo,
            patch_dir,
        }
    }

    /// Check for and apply boot patches.
    ///
    /// Returns the list of patches that were applied, empty if up to date.
    pub async fn update_boot<F>(&self, game_path: &Path, progress: F) -> Result<Vec<PatchEntry>, Error>
    where
        F: Fn(UpdateProgress) + Send + Sync + Clone + 'static,
    {
        progress(UpdateProgress {
            stage: UpdateStage::CheckingPatches,
            bytes_downloaded: 0,
            bytes_total: 0,
            overall_progress: 0.0,
        });

        // Get current boot version
        let boot_version = self
            .version_repo
            .get_version(game_path, Repository::Boot)
            .await?;

        // Check for boot patches
        let patches = self
            .patch_server
            .check_boot_version(game_path, &boot_version)
            .await?;

        if patches.is_empty() {
            progress(UpdateProgress {
                stage: UpdateStage::Completed,
                bytes_downloaded: 0,
                bytes_total: 0,
                overall_progress: 100.0,
            });
            return Ok(vec![]);
        }

        // Apply each patch
        self.apply_patches(&patches, game_path, None, progress)
            .await?;

        Ok(patches)
    }

    /// Check for and apply game patches after successful login.
    ///
    /// Requires a valid session_id from OAuth login.
    /// Returns the list of patches that were applied, empty if up to date.
    pub async fn update_game<F>(
        &self,
        session_id: &str,
        game_path: &Path,
        max_expansion: u32,
        progress: F,
    ) -> Result<(String, Vec<PatchEntry>), Error>
    where
        F: Fn(UpdateProgress) + Send + Sync + Clone + 'static,
    {
        progress(UpdateProgress {
            stage: UpdateStage::CheckingPatches,
            bytes_downloaded: 0,
            bytes_total: 0,
            overall_progress: 0.0,
        });

        // Register session and get patch list
        let (unique_id, patches) = self
            .patch_server
            .register_session(session_id, game_path, max_expansion)
            .await?;

        if patches.is_empty() {
            progress(UpdateProgress {
                stage: UpdateStage::Completed,
                bytes_downloaded: 0,
                bytes_total: 0,
                overall_progress: 100.0,
            });
            return Ok((unique_id.clone(), vec![]));
        }

        // Apply patches with unique_id for authentication
        self.apply_patches(&patches, game_path, Some(&unique_id), progress)
            .await?;

        Ok((unique_id, patches))
    }

    /// Apply a list of patches to the game installation.
    async fn apply_patches<F>(
        &self,
        patches: &[PatchEntry],
        game_path: &Path,
        unique_id: Option<&str>,
        progress: F,
    ) -> Result<(), Error>
    where
        F: Fn(UpdateProgress) + Send + Sync + Clone + 'static,
    {
        let total_patches = patches.len();
        let total_bytes: u64 = patches.iter().map(|p| p.length).sum();
        let mut cumulative_bytes: u64 = 0;

        for (index, patch) in patches.iter().enumerate() {
            let patch_path = self.patch_dir.join(
                patch
                    .filename()
                    .unwrap_or(&format!("patch_{}.patch", index)),
            );

            // Download phase
            let progress_clone = progress.clone();
            let patch_index = index;
            let repo = patch.repository;
            let version = patch.version_id.clone();
            let cumulative = cumulative_bytes;

            progress(UpdateProgress {
                stage: UpdateStage::Downloading {
                    patch_index: index + 1,
                    total_patches,
                    repository: patch.repository,
                    version: patch.version_id.clone(),
                },
                bytes_downloaded: 0,
                bytes_total: patch.length,
                overall_progress: (cumulative_bytes as f64 / total_bytes as f64) * 100.0,
            });

            self.downloader
                .download_patch(patch, &patch_path, unique_id, move |downloaded, total| {
                    progress_clone(UpdateProgress {
                        stage: UpdateStage::Downloading {
                            patch_index: patch_index + 1,
                            total_patches,
                            repository: repo,
                            version: version.clone(),
                        },
                        bytes_downloaded: downloaded,
                        bytes_total: total,
                        overall_progress: ((cumulative + downloaded) as f64 / total_bytes as f64)
                            * 100.0,
                    });
                })
                .await?;

            // Verify phase
            progress(UpdateProgress {
                stage: UpdateStage::Verifying {
                    patch_index: index + 1,
                    total_patches,
                },
                bytes_downloaded: patch.length,
                bytes_total: patch.length,
                overall_progress: ((cumulative_bytes + patch.length) as f64 / total_bytes as f64)
                    * 100.0,
            });

            let verified = self.downloader.verify_patch(patch, &patch_path).await?;
            if !verified {
                // Clean up failed patch file
                let _ = tokio::fs::remove_file(&patch_path).await;
                return Err(Error::PatchVerificationFailed);
            }

            // Apply phase
            progress(UpdateProgress {
                stage: UpdateStage::Applying {
                    patch_index: index + 1,
                    total_patches,
                    repository: patch.repository,
                    version: patch.version_id.clone(),
                },
                bytes_downloaded: patch.length,
                bytes_total: patch.length,
                overall_progress: ((cumulative_bytes + patch.length) as f64 / total_bytes as f64)
                    * 100.0,
            });

            // ZiPatch application is synchronous, run in blocking context
            let applier = self.applier.clone();
            let patch_path_clone = patch_path.clone();
            let game_path_clone = game_path.to_path_buf();
            tokio::task::spawn_blocking(move || {
                applier.apply_patch(&patch_path_clone, &game_path_clone)
            })
            .await
            .map_err(|e| Error::ZiPatchApply(e.to_string()))??;

            // Update version file after successful patch
            self.version_repo
                .set_version(game_path, patch.repository, &patch.version_id)
                .await?;

            // Clean up patch file after successful application
            let _ = tokio::fs::remove_file(&patch_path).await;

            cumulative_bytes += patch.length;
        }

        progress(UpdateProgress {
            stage: UpdateStage::Completed,
            bytes_downloaded: total_bytes,
            bytes_total: total_bytes,
            overall_progress: 100.0,
        });

        Ok(())
    }

    /// Check if game needs updates without applying them.
    ///
    /// Useful for UI to show update availability before starting.
    pub async fn check_updates(
        &self,
        session_id: &str,
        game_path: &Path,
        max_expansion: u32,
    ) -> Result<UpdateCheckResult, Error> {
        // Check boot
        let boot_version = self
            .version_repo
            .get_version(game_path, Repository::Boot)
            .await?;
        let boot_patches = self
            .patch_server
            .check_boot_version(game_path, &boot_version)
            .await?;

        // Check game
        let (unique_id, game_patches) = self
            .patch_server
            .register_session(session_id, game_path, max_expansion)
            .await?;

        let total_size: u64 = boot_patches
            .iter()
            .chain(game_patches.iter())
            .map(|p| p.length)
            .sum();

        Ok(UpdateCheckResult {
            boot_patches,
            game_patches,
            unique_id,
            total_download_size: total_size,
        })
    }
}

/// Result of checking for updates
#[derive(Debug)]
pub struct UpdateCheckResult {
    pub boot_patches: Vec<PatchEntry>,
    pub game_patches: Vec<PatchEntry>,
    pub unique_id: String,
    pub total_download_size: u64,
}

impl UpdateCheckResult {
    pub fn needs_update(&self) -> bool {
        !self.boot_patches.is_empty() || !self.game_patches.is_empty()
    }

    pub fn needs_boot_update(&self) -> bool {
        !self.boot_patches.is_empty()
    }

    pub fn needs_game_update(&self) -> bool {
        !self.game_patches.is_empty()
    }

    pub fn total_patches(&self) -> usize {
        self.boot_patches.len() + self.game_patches.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_progress_percent() {
        let progress = UpdateProgress {
            stage: UpdateStage::Downloading {
                patch_index: 1,
                total_patches: 3,
                repository: Repository::Ffxiv,
                version: "2024.01.01.0000.0000".to_string(),
            },
            bytes_downloaded: 500,
            bytes_total: 1000,
            overall_progress: 25.0,
        };

        assert!((progress.download_percent() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_update_check_result() {
        let result = UpdateCheckResult {
            boot_patches: vec![],
            game_patches: vec![],
            unique_id: "test".to_string(),
            total_download_size: 0,
        };

        assert!(!result.needs_update());
        assert!(!result.needs_boot_update());
        assert!(!result.needs_game_update());
        assert_eq!(result.total_patches(), 0);
    }
}
