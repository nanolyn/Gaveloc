use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::config::{GameSettings, Region, Settings, WineSettings};
use crate::entities::{
    Account, AccountId, CachedSession, Credentials, FileIntegrityResult, GameVersion,
    IntegrityManifest, IntegrityProgress, OauthLoginResult, PatchEntry, PatchProgress, Repository,
    WineRunner,
};
use crate::error::Error;
use crate::zipatch::ZiPatchChunk;

// ============================================================================
// News Ports
// ============================================================================

#[async_trait]
pub trait NewsRepository: Send + Sync {
    async fn get_headlines(&self, language: &str) -> Result<crate::entities::Headlines, Error>;
    async fn get_banners(&self, language: &str) -> Result<Vec<crate::entities::Banner>, Error>;
    async fn get_article(&self, url: &str) -> Result<crate::entities::NewsArticle, Error>;
}

// ============================================================================
// Patching Ports
// ============================================================================

/// Version file operations - reads/writes version files from game installation
#[async_trait]
pub trait VersionRepository: Send + Sync {
    /// Read version from a .ver file for the specified repository
    async fn get_version(&self, game_path: &Path, repo: Repository) -> Result<GameVersion, Error>;

    /// Write version to a .ver file after successful patch
    async fn set_version(
        &self,
        game_path: &Path,
        repo: Repository,
        version: &str,
    ) -> Result<(), Error>;

    /// Generate boot version hash string for session registration
    /// This is a hash of boot executables used to verify launcher integrity
    async fn get_boot_version_hash(&self, game_path: &Path) -> Result<String, Error>;

    /// Generate version report for all repositories up to max_expansion
    /// Format: "repo/version" joined by newlines, used for session registration
    async fn get_version_report(
        &self,
        game_path: &Path,
        max_expansion: u32,
    ) -> Result<String, Error>;

    /// Check if all required version files exist
    async fn validate_game_installation(&self, game_path: &Path) -> Result<bool, Error>;
}

/// Patch server communication - queries SE servers for available patches
#[async_trait]
pub trait PatchServer: Send + Sync {
    /// Check boot version and get list of available boot patches
    async fn check_boot_version(
        &self,
        game_path: &Path,
        boot_version: &GameVersion,
    ) -> Result<Vec<PatchEntry>, Error>;

    /// Register session with game version server and get patch list
    /// Returns (unique_id, patches) - unique_id is needed for downloads
    async fn register_session(
        &self,
        session_id: &str,
        game_path: &Path,
        max_expansion: u32,
    ) -> Result<(String, Vec<PatchEntry>), Error>;
}

/// Patch downloading with progress reporting
#[async_trait]
pub trait PatchDownloader: Send + Sync {
    /// Download a patch file to the destination path
    /// Progress callback receives (bytes_downloaded, bytes_total)
    async fn download_patch<F>(
        &self,
        patch: &PatchEntry,
        dest_path: &Path,
        unique_id: Option<&str>,
        progress: F,
    ) -> Result<(), Error>
    where
        F: Fn(u64, u64) + Send + Sync + 'static;

    /// Verify patch file integrity using block hashes
    async fn verify_patch(&self, patch: &PatchEntry, file_path: &Path) -> Result<bool, Error>;
}

/// ZiPatch file parser and applier (synchronous - runs in blocking context)
pub trait ZiPatchApplier: Send + Sync {
    /// Apply a ZiPatch file to the game installation
    fn apply_patch(&self, patch_path: &Path, game_path: &Path) -> Result<(), Error>;

    /// Parse a ZiPatch file and return its chunks (for debugging/verification)
    fn parse_patch(&self, patch_path: &Path) -> Result<Vec<ZiPatchChunk>, Error>;
}

/// Integrity checking against community manifest
#[async_trait]
pub trait IntegrityChecker: Send + Sync {
    /// Fetch integrity manifest from goatcorp for the specified version
    async fn fetch_manifest(&self, game_version: &str) -> Result<IntegrityManifest, Error>;

    /// Run integrity check on game files
    /// Progress callback receives current progress info
    async fn check_integrity<F>(
        &self,
        game_path: &Path,
        manifest: &IntegrityManifest,
        progress: F,
    ) -> Result<Vec<FileIntegrityResult>, Error>
    where
        F: Fn(IntegrityProgress) + Send + Sync + 'static;

    /// Repair a corrupted or missing file by re-downloading
    async fn repair_file(
        &self,
        game_path: &Path,
        relative_path: &str,
        expected_hash: &str,
    ) -> Result<(), Error>;

    /// Repair multiple files in parallel (more efficient for batch operations)
    /// Returns (success_count, failure_count)
    async fn repair_files(
        &self,
        game_path: &Path,
        files: &[FileIntegrityResult],
    ) -> Result<(u32, u32), Error>;
}

/// IPC communication for the separate patcher process
#[async_trait]
pub trait PatcherIpc: Send + Sync {
    /// Send a patch job to the patcher process
    async fn start_patch(&self, patches: Vec<PatchEntry>, game_path: &Path)
        -> Result<(), Error>;

    /// Receive progress updates from the patcher
    async fn receive_progress(&self) -> Result<Option<PatchProgress>, Error>;

    /// Cancel the current patch operation
    async fn cancel(&self) -> Result<(), Error>;

    /// Check if the patcher process is running
    fn is_running(&self) -> bool;
}

#[async_trait]
pub trait RunnerDetector {
    async fn detect_runners(&self) -> Result<Vec<WineRunner>, Error>;
    async fn validate_runner(&self, path: PathBuf) -> Result<WineRunner, Error>;
}

#[async_trait]
pub trait RunnerManager {
    async fn install_latest_ge_proton(&self) -> Result<WineRunner, Error>;
}

#[async_trait]
pub trait PrefixManager {
    async fn exists(&self, prefix_path: &Path) -> bool;
    async fn initialize(&self, prefix_path: &Path, runner: &WineRunner) -> Result<(), Error>;
}

/// Configuration for launching the game.
pub struct LaunchConfig<'a> {
    pub runner: &'a WineRunner,
    pub prefix_path: &'a Path,
    pub game_path: &'a Path,
    pub args: &'a str,
    pub wine_settings: &'a WineSettings,
    pub game_settings: &'a GameSettings,
}

#[async_trait]
pub trait ProcessLauncher {
    async fn launch(&self, config: LaunchConfig<'_>) -> Result<(), Error>;
}

// ============================================================================
// Authentication Ports
// ============================================================================

/// Secure credential storage interface (libsecret/keyring)
#[async_trait]
pub trait CredentialStore: Send + Sync {
    /// Store password for an account
    async fn store_password(&self, account_id: &AccountId, password: &str) -> Result<(), Error>;

    /// Retrieve stored password
    async fn get_password(&self, account_id: &AccountId) -> Result<Option<String>, Error>;

    /// Delete stored password
    async fn delete_password(&self, account_id: &AccountId) -> Result<(), Error>;

    /// Store cached session data
    async fn store_session(
        &self,
        account_id: &AccountId,
        session: &CachedSession,
    ) -> Result<(), Error>;

    /// Retrieve cached session
    async fn get_session(&self, account_id: &AccountId) -> Result<Option<CachedSession>, Error>;

    /// Delete cached session
    async fn delete_session(&self, account_id: &AccountId) -> Result<(), Error>;

    /// Check if credentials exist for account
    async fn has_credentials(&self, account_id: &AccountId) -> Result<bool, Error>;
}

/// Account persistence (non-secret data)
#[async_trait]
pub trait AccountRepository: Send + Sync {
    /// List all saved accounts
    async fn list_accounts(&self) -> Result<Vec<Account>, Error>;

    /// Get account by ID
    async fn get_account(&self, id: &AccountId) -> Result<Option<Account>, Error>;

    /// Save or update account
    async fn save_account(&self, account: &Account) -> Result<(), Error>;

    /// Delete account
    async fn delete_account(&self, id: &AccountId) -> Result<(), Error>;

    /// Get default/last-used account
    async fn get_default_account(&self) -> Result<Option<Account>, Error>;

    /// Set default account
    async fn set_default_account(&self, id: &AccountId) -> Result<(), Error>;
}

/// Configuration persistence (TOML-based settings storage)
#[async_trait]
pub trait ConfigRepository: Send + Sync {
    /// Load settings from storage (returns defaults if file doesn't exist)
    async fn load_settings(&self) -> Result<Settings, Error>;

    /// Save settings to storage
    async fn save_settings(&self, settings: &Settings) -> Result<(), Error>;

    /// Check if config file exists
    async fn exists(&self) -> bool;
}

/// OAuth authentication with Square Enix servers
#[async_trait]
pub trait Authenticator: Send + Sync {
    /// Perform full OAuth login flow
    async fn login(
        &self,
        credentials: &Credentials,
        region: Region,
        is_free_trial: bool,
    ) -> Result<OauthLoginResult, Error>;
}

/// OTP listener for mobile app integration
#[async_trait]
pub trait OtpListener: Send + Sync {
    /// Start listening for OTP on localhost:4646
    /// Returns a receiver that will yield the OTP when received
    async fn start(&self) -> Result<tokio::sync::oneshot::Receiver<String>, Error>;

    /// Stop the listener
    async fn stop(&self) -> Result<(), Error>;

    /// Check if listener is running
    fn is_running(&self) -> bool;
}
