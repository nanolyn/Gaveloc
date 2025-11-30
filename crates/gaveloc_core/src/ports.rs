use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::config::{GameSettings, Region, WineSettings};
use crate::entities::{
    Account, AccountId, CachedSession, Credentials, GameVersion, LoginResult, OauthLoginResult,
    WineRunner,
};
use crate::error::Error;

#[async_trait]
pub trait PatchRepository {
    async fn get_latest_version(&self) -> Result<GameVersion, Error>;
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

    /// Register session and get unique ID for patching/launching
    async fn register_session(
        &self,
        oauth_result: &OauthLoginResult,
        game_path: &Path,
    ) -> Result<LoginResult, Error>;
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
