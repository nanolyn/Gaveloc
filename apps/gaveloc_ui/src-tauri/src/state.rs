use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use gaveloc_adapters::{
    configuration::get_configuration,
    FileAccountRepository, FileVersionRepository, GoatcorpIntegrityChecker,
    HttpOtpListener, KeyringCredentialStore, SquareEnixAuthenticator,
    patch::{HttpPatchDownloader, SquareEnixPatchServer},
    prefix::LinuxPrefixManager,
    process::LinuxProcessLauncher,
    runner::LinuxRunnerDetector,
    HttpNewsRepository,
};
use gaveloc_core::config::Settings;

use crate::commands::integrity::IntegrityState;
use crate::commands::patching::PatchingState;

/// Get the application config directory
fn get_config_dir() -> PathBuf {
    directories::ProjectDirs::from("com", "gaveloc", "gaveloc")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Application state shared across all Tauri commands
pub struct AppState {
    /// Account repository for managing saved accounts
    pub accounts: Arc<FileAccountRepository>,
    /// Credential store for passwords and sessions
    pub credentials: Arc<KeyringCredentialStore>,
    /// OAuth authenticator for Square Enix login
    pub authenticator: Arc<RwLock<Option<SquareEnixAuthenticator>>>,
    /// Patch server for checking updates
    pub patch_server: Arc<SquareEnixPatchServer>,
    /// Patch downloader
    pub patch_downloader: Arc<HttpPatchDownloader>,
    /// Version repository for reading/writing game versions
    pub version_repo: Arc<RwLock<Option<FileVersionRepository>>>,
    /// Integrity checker
    pub integrity_checker: Arc<GoatcorpIntegrityChecker>,
    /// Runner detector
    pub runner_detector: Arc<LinuxRunnerDetector>,
    /// OTP listener for receiving OTP from mobile app
    pub otp_listener: Arc<HttpOtpListener>,
    /// News repository
    pub news_repository: Arc<HttpNewsRepository>,
    /// Current settings (loaded on startup, can be modified)
    pub settings: Arc<RwLock<Settings>>,
    /// Patching state for tracking download/install progress
    pub patch_state: Arc<RwLock<PatchingState>>,
    /// Integrity checking state
    pub integrity_state: Arc<RwLock<IntegrityState>>,
    /// Process launcher for starting the game
    pub process_launcher: Arc<LinuxProcessLauncher>,
    /// Prefix manager for Wine prefix lifecycle
    pub prefix_manager: Arc<LinuxPrefixManager>,
    /// Track if game is currently running (PID)
    pub game_pid: Arc<RwLock<Option<u32>>>,
}

impl AppState {
    pub fn new() -> Self {
        // Load settings from config file or use defaults
        let settings = get_configuration().unwrap_or_default();
        let config_dir = get_config_dir();

        // Initialize adapters
        let accounts = Arc::new(FileAccountRepository::new(config_dir));
        let credentials = Arc::new(KeyringCredentialStore::new());
        let patch_server = Arc::new(
            SquareEnixPatchServer::new().expect("Failed to create patch server client"),
        );
        let patch_downloader = Arc::new(
            HttpPatchDownloader::new().expect("Failed to create patch downloader client"),
        );
        let integrity_checker = Arc::new(GoatcorpIntegrityChecker::with_default_client());
        let runner_detector = Arc::new(LinuxRunnerDetector::new());
        let otp_listener = Arc::new(HttpOtpListener::new());
        let process_launcher = Arc::new(LinuxProcessLauncher::new());
        let prefix_manager = Arc::new(LinuxPrefixManager::new());
        let news_repository = Arc::new(HttpNewsRepository::new());

        // Authenticator can fail to create, so we wrap in Option
        let authenticator = Arc::new(RwLock::new(SquareEnixAuthenticator::new().ok()));

        // Version repo depends on game path, initialized lazily
        let version_repo = Arc::new(RwLock::new(None));

        Self {
            accounts,
            credentials,
            authenticator,
            patch_server,
            patch_downloader,
            version_repo,
            integrity_checker,
            runner_detector,
            otp_listener,
            news_repository,
            settings: Arc::new(RwLock::new(settings)),
            patch_state: Arc::new(RwLock::new(PatchingState::default())),
            integrity_state: Arc::new(RwLock::new(IntegrityState::default())),
            process_launcher,
            prefix_manager,
            game_pid: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize the version repository
    pub async fn init_version_repo(&self) {
        let repo = FileVersionRepository::new();
        *self.version_repo.write().await = Some(repo);
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
