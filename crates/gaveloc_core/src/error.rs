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

    // =========================================================================
    // Patching Errors
    // =========================================================================

    #[error("version file not found: {0}")]
    VersionFileNotFound(PathBuf),

    #[error("invalid version format: {0}")]
    InvalidVersionFormat(String),

    #[error("patch server error: {0}")]
    PatchServer(String),

    #[error("patch download failed: {0}")]
    PatchDownload(String),

    #[error("patch verification failed: hash mismatch")]
    PatchVerificationFailed,

    #[error("patch verification failed: block {block} mismatch (expected {expected}, got {actual})")]
    PatchBlockVerificationFailed {
        block: usize,
        expected: String,
        actual: String,
    },

    #[error("zipatch parse error: {0}")]
    ZiPatchParse(String),

    #[error("zipatch apply error: {0}")]
    ZiPatchApply(String),

    #[error("zipatch checksum mismatch at offset {offset}")]
    ZiPatchChecksumMismatch { offset: u64 },

    #[error("zipatch invalid magic header")]
    ZiPatchInvalidMagic,

    #[error("zipatch unknown chunk type: {0}")]
    ZiPatchUnknownChunk(String),

    #[error("integrity manifest not found for version {0}")]
    IntegrityManifestNotFound(String),

    #[error("file integrity mismatch: {0}")]
    IntegrityMismatch(String),

    #[error("operation cancelled")]
    Cancelled,

    #[error("not enough disk space: need {needed} bytes, have {available} bytes")]
    NotEnoughDiskSpace { needed: u64, available: u64 },

    #[error("game path not configured")]
    GamePathNotConfigured,

    #[error("game path does not exist: {0}")]
    GamePathNotFound(PathBuf),

    #[error("IPC communication error: {0}")]
    Ipc(String),
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

    #[test]
    fn test_oauth_error_display() {
        assert_eq!(
            OauthError::InvalidCredentials.to_string(),
            "invalid username or password"
        );
        assert_eq!(
            OauthError::InvalidOtp.to_string(),
            "invalid one-time password"
        );
        assert_eq!(OauthError::AccountLocked.to_string(), "account is locked");
        assert_eq!(
            OauthError::MaintenanceMode.to_string(),
            "servers under maintenance"
        );
        assert_eq!(
            OauthError::RateLimited.to_string(),
            "too many login attempts"
        );
        assert_eq!(
            OauthError::Unknown("custom error".to_string()).to_string(),
            "custom error"
        );
    }

    #[test]
    fn test_error_display_variants() {
        // Test a selection of error variants for Display
        assert_eq!(
            Error::RunnerNotFound(PathBuf::from("/test/runner")).to_string(),
            "runner not found at /test/runner"
        );
        assert_eq!(
            Error::WineBinaryNotFound(PathBuf::from("/test")).to_string(),
            "no wine binary found in /test"
        );
        assert_eq!(
            Error::HomeDirectoryNotFound.to_string(),
            "failed to detect home directory"
        );
        assert_eq!(Error::SessionExpired.to_string(), "session expired");
        assert_eq!(Error::OtpRequired.to_string(), "OTP required");
        assert_eq!(Error::Cancelled.to_string(), "operation cancelled");
        assert_eq!(
            Error::ZiPatchInvalidMagic.to_string(),
            "zipatch invalid magic header"
        );
        assert_eq!(
            Error::ZiPatchChecksumMismatch { offset: 100 }.to_string(),
            "zipatch checksum mismatch at offset 100"
        );
        assert_eq!(
            Error::PatchBlockVerificationFailed {
                block: 5,
                expected: "abc".to_string(),
                actual: "def".to_string()
            }
            .to_string(),
            "patch verification failed: block 5 mismatch (expected abc, got def)"
        );
        assert_eq!(
            Error::NotEnoughDiskSpace {
                needed: 1000,
                available: 500
            }
            .to_string(),
            "not enough disk space: need 1000 bytes, have 500 bytes"
        );
        assert_eq!(
            Error::OauthLogin(OauthError::InvalidCredentials).to_string(),
            "OAuth login failed: invalid username or password"
        );
    }
}
