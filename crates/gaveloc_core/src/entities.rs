use std::fmt;
use std::hash::Hash;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::Region;

#[derive(Debug, Clone)]
pub struct GameVersion {
    pub version_string: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RunnerType {
    System,
    Proton,
    Lutris,
    GavelocManaged,
    #[default]
    Custom,
}

impl fmt::Display for RunnerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunnerType::System => write!(f, "System"),
            RunnerType::Proton => write!(f, "Proton"),
            RunnerType::Lutris => write!(f, "Lutris"),
            RunnerType::GavelocManaged => write!(f, "Gaveloc"),
            RunnerType::Custom => write!(f, "Custom"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WineRunner {
    pub path: PathBuf,
    pub name: String,
    pub runner_type: RunnerType,
    pub is_valid: bool,
}

/// Unique identifier for an account (derived from username, lowercase)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountId(String);

impl AccountId {
    pub fn new(username: &str) -> Self {
        Self(username.to_lowercase())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Account entity with authentication metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub username: String,
    pub region: Region,
    pub is_steam: bool,
    pub is_free_trial: bool,
    pub use_otp: bool,
    /// Last successful login timestamp (Unix epoch seconds)
    pub last_login: Option<i64>,
}

impl Account {
    pub fn new(username: String, region: Region) -> Self {
        Self {
            id: AccountId::new(&username),
            username,
            region,
            is_steam: false,
            is_free_trial: false,
            use_otp: false,
            last_login: None,
        }
    }
}

/// Credentials for authentication (never persisted to disk)
#[derive(Debug, Clone)]
pub struct Credentials {
    pub username: String,
    pub password: String,
    pub otp: Option<String>,
}

impl Credentials {
    pub fn new(username: String, password: String) -> Self {
        Self {
            username,
            password,
            otp: None,
        }
    }

    pub fn with_otp(mut self, otp: String) -> Self {
        self.otp = Some(otp);
        self
    }
}

/// Result of successful OAuth login
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OauthLoginResult {
    pub session_id: String,
    pub region: i32,
    pub terms_accepted: bool,
    pub playable: bool,
    pub max_expansion: u32,
}

/// Cached session data stored in keyring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedSession {
    pub unique_id: String,
    pub region: i32,
    pub max_expansion: u32,
    pub created_at: i64,
}

impl CachedSession {
    /// Session cache validity duration (1 day)
    const CACHE_DURATION_SECS: i64 = 24 * 60 * 60;

    pub fn is_valid(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        (now - self.created_at) < Self::CACHE_DURATION_SECS
    }

    /// Get remaining validity time in seconds
    pub fn remaining_secs(&self) -> i64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        (Self::CACHE_DURATION_SECS - (now - self.created_at)).max(0)
    }
}

/// Login state machine
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginState {
    /// Login successful
    Ok,
    /// Game needs patching
    NeedsPatchGame,
    /// Boot files need patching
    NeedsPatchBoot,
    /// No active subscription
    NoService,
    /// Terms not accepted
    NoTerms,
}

/// Complete login result
#[derive(Debug, Clone)]
pub struct LoginResult {
    pub state: LoginState,
    pub oauth: Option<OauthLoginResult>,
    pub unique_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_type_display() {
        assert_eq!(RunnerType::System.to_string(), "System");
        assert_eq!(RunnerType::Proton.to_string(), "Proton");
        assert_eq!(RunnerType::Lutris.to_string(), "Lutris");
        assert_eq!(RunnerType::GavelocManaged.to_string(), "Gaveloc");
        assert_eq!(RunnerType::Custom.to_string(), "Custom");
    }

    #[test]
    fn test_account_id_lowercase() {
        let id1 = AccountId::new("TestUser");
        let id2 = AccountId::new("testuser");
        let id3 = AccountId::new("TESTUSER");

        assert_eq!(id1, id2);
        assert_eq!(id2, id3);
        assert_eq!(id1.as_str(), "testuser");
    }

    #[test]
    fn test_account_new() {
        let account = Account::new("TestUser".to_string(), Region::Europe);

        assert_eq!(account.username, "TestUser");
        assert_eq!(account.id.as_str(), "testuser");
        assert_eq!(account.region, Region::Europe);
        assert!(!account.is_steam);
        assert!(!account.is_free_trial);
        assert!(!account.use_otp);
        assert!(account.last_login.is_none());
    }

    #[test]
    fn test_credentials_with_otp() {
        let creds = Credentials::new("user".to_string(), "pass".to_string());
        assert!(creds.otp.is_none());

        let creds_with_otp = creds.with_otp("123456".to_string());
        assert_eq!(creds_with_otp.otp, Some("123456".to_string()));
    }

    #[test]
    fn test_cached_session_validity() {
        let valid_session = CachedSession {
            unique_id: "test".to_string(),
            region: 3,
            max_expansion: 5,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        };
        assert!(valid_session.is_valid());
        assert!(valid_session.remaining_secs() > 0);

        let expired_session = CachedSession {
            unique_id: "test".to_string(),
            region: 3,
            max_expansion: 5,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
                - (25 * 60 * 60), // 25 hours ago
        };
        assert!(!expired_session.is_valid());
        assert_eq!(expired_session.remaining_secs(), 0);
    }
}
