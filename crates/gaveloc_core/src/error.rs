use std::fmt;
use std::path::PathBuf;

use thiserror::Error;

/// Detailed OAuth error information
#[derive(Debug, Clone)]
pub enum OauthError {
    InvalidCredentials,
    InvalidOtp,
    AccountLocked,
    MaintenanceMode,
    RateLimited,
    Unknown(String),
}

impl fmt::Display for OauthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCredentials => write!(f, "invalid username or password"),
            Self::InvalidOtp => write!(f, "invalid one-time password"),
            Self::AccountLocked => write!(f, "account is locked"),
            Self::MaintenanceMode => write!(f, "servers under maintenance"),
            Self::RateLimited => write!(f, "too many login attempts"),
            Self::Unknown(msg) => write!(f, "{}", msg),
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("runner not found at {0}")]
    RunnerNotFound(PathBuf),

    #[error("no wine binary found in {0}")]
    WineBinaryNotFound(PathBuf),

    #[error("invalid runner path: {0}")]
    InvalidRunnerPath(PathBuf),

    #[error("failed to detect home directory")]
    HomeDirectoryNotFound,

    #[error("encryption failed: {0}")]
    Encryption(String),

    #[error("prefix initialization failed: {0}")]
    PrefixInitialization(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("authentication failed: {0}")]
    Authentication(String),

    #[error("credential storage error: {0}")]
    CredentialStorage(String),

    #[error("OAuth login failed: {0}")]
    OauthLogin(OauthError),

    #[error("session expired")]
    SessionExpired,

    #[error("OTP required")]
    OtpRequired,

    #[error("account not playable (no active subscription)")]
    AccountNotPlayable,

    #[error("terms of service not accepted")]
    TermsNotAccepted,

    #[error("invalid response from server: {0}")]
    InvalidServerResponse(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::Other(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::Other(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_from_string() {
        let err: Error = String::from("test error").into();
        if let Error::Other(msg) = err {
            assert_eq!(msg, "test error");
        } else {
            panic!("Expected Error::Other");
        }
    }

    #[test]
    fn test_error_from_str() {
        let err: Error = "test error".into();
        if let Error::Other(msg) = err {
            assert_eq!(msg, "test error");
        } else {
            panic!("Expected Error::Other");
        }
    }
}
