//! Shared HTTP client configuration for network operations.
//!
//! Provides factory functions for creating properly configured HTTP clients
//! with appropriate timeouts, user agents, and settings for different use cases.

use std::time::Duration;

use gaveloc_core::Error;
use reqwest::Client;

/// Default timeout for HTTP requests (30 seconds)
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default connect timeout (10 seconds)
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// User agent for patch operations (mimics official launcher)
pub const PATCHER_USER_AGENT: &str = "FFXIV PATCH CLIENT";

/// Build a configured HTTP client for Square Enix OAuth requests.
///
/// This client is configured with:
/// - Cookie store disabled (manual cookie management for OAuth flow)
/// - Custom user agent for SE authentication
/// - Request and connect timeouts
pub fn build_oauth_client(user_agent: &str) -> Result<Client, Error> {
    Client::builder()
        .cookie_store(false)
        .user_agent(user_agent)
        .timeout(DEFAULT_TIMEOUT)
        .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
        .build()
        .map_err(|e| Error::Network(format!("failed to create OAuth HTTP client: {}", e)))
}

/// Build a configured HTTP client for patch operations.
///
/// This client is configured with:
/// - FFXIV PATCH CLIENT user agent
/// - Request and connect timeouts
pub fn build_patch_client() -> Result<Client, Error> {
    Client::builder()
        .user_agent(PATCHER_USER_AGENT)
        .timeout(DEFAULT_TIMEOUT)
        .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
        .build()
        .map_err(|e| Error::Network(format!("failed to create patch HTTP client: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_oauth_client() {
        let client = build_oauth_client("TestAgent/1.0");
        assert!(client.is_ok());
    }

    #[test]
    fn test_build_patch_client() {
        let client = build_patch_client();
        assert!(client.is_ok());
    }

    #[test]
    fn test_timeout_constants() {
        assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(30));
        assert_eq!(DEFAULT_CONNECT_TIMEOUT, Duration::from_secs(10));
    }
}
