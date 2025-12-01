use async_trait::async_trait;
use gaveloc_core::config::Region;
use gaveloc_core::entities::{Credentials, OauthLoginResult};
use gaveloc_core::error::OauthError;
use gaveloc_core::ports::Authenticator;
use gaveloc_core::Error;
use regex::Regex;
use reqwest::{header, Client};
use sha1::{Digest, Sha1};
use tracing::{debug, info, instrument, warn};

use crate::network::build_oauth_client;

const OAUTH_LOGIN_URL: &str = "https://ffxiv-login.square-enix.com/oauth/ffxivarr/login/top";
const OAUTH_SEND_URL: &str = "https://ffxiv-login.square-enix.com/oauth/ffxivarr/login/login.send";

/// OAuth authenticator for Square Enix login
pub struct SquareEnixAuthenticator {
    client: Client,
    user_agent: String,
}

impl SquareEnixAuthenticator {
    pub fn new() -> Result<Self, Error> {
        let user_agent = generate_user_agent();
        let client = build_oauth_client(&user_agent)?;
        Ok(Self { client, user_agent })
    }

    /// Fetch the OAuth top page and extract the _STORED_ token
    #[instrument(skip(self))]
    async fn get_oauth_top(&self, region: Region, is_free_trial: bool) -> Result<String, Error> {
        let url = format!(
            "{}?lng=en&rgn={}&isft={}&cssmode=1&isnew=1&launchver=3",
            OAUTH_LOGIN_URL,
            region.as_id(),
            if is_free_trial { "1" } else { "0" }
        );

        debug!(url = %url, "fetching OAuth top page");

        let response = self
            .client
            .get(&url)
            .header(header::USER_AGENT, &self.user_agent)
            .header(
                header::ACCEPT,
                "image/gif, image/jpeg, image/pjpeg, application/x-ms-application, application/xaml+xml, application/x-ms-xbap, */*",
            )
            .header(header::ACCEPT_ENCODING, "gzip, deflate")
            .header(header::ACCEPT_LANGUAGE, "en-us")
            .header(header::CONNECTION, "Keep-Alive")
            .header(header::COOKIE, "_rsid=\"\"")
            .send()
            .await
            .map_err(|e| Error::Network(format!("OAuth top request failed: {}", e)))?;

        let text = response
            .text()
            .await
            .map_err(|e| Error::Network(format!("failed to read OAuth response: {}", e)))?;

        // Check for maintenance or restart messages
        if text.contains("window.external.user(\"restartup\");") {
            return Err(Error::InvalidServerResponse(
                "server requested restart".to_string(),
            ));
        }

        // Extract _STORED_ token
        let stored_regex =
            Regex::new(r#"<\s*input .* name="_STORED_" value="(?<stored>[^"]*)""#).expect("invalid regex");

        let captures = stored_regex.captures(&text).ok_or_else(|| {
            Error::InvalidServerResponse("could not find _STORED_ token".to_string())
        })?;

        let stored = captures
            .name("stored")
            .ok_or_else(|| Error::InvalidServerResponse("_STORED_ token empty".to_string()))?
            .as_str()
            .to_string();

        debug!("extracted _STORED_ token");
        Ok(stored)
    }

    /// Send login credentials and parse response
    #[instrument(skip(self, credentials, stored_token))]
    async fn send_login(
        &self,
        credentials: &Credentials,
        stored_token: &str,
        region: Region,
        is_free_trial: bool,
    ) -> Result<OauthLoginResult, Error> {
        let referer_url = format!(
            "{}?lng=en&rgn={}&isft={}&cssmode=1&isnew=1&launchver=3",
            OAUTH_LOGIN_URL,
            region.as_id(),
            if is_free_trial { "1" } else { "0" }
        );

        let form_data = [
            ("_STORED_", stored_token),
            ("sqexid", &credentials.username),
            ("password", &credentials.password),
            ("otppw", credentials.otp.as_deref().unwrap_or("")),
        ];

        debug!("sending login request");

        let response = self
            .client
            .post(OAUTH_SEND_URL)
            .header(header::USER_AGENT, &self.user_agent)
            .header(
                header::ACCEPT,
                "image/gif, image/jpeg, image/pjpeg, application/x-ms-application, application/xaml+xml, application/x-ms-xbap, */*",
            )
            .header(header::REFERER, &referer_url)
            .header(header::ACCEPT_LANGUAGE, "en-us")
            .header(header::ACCEPT_ENCODING, "gzip, deflate")
            .header(header::HOST, "ffxiv-login.square-enix.com")
            .header(header::CONNECTION, "Keep-Alive")
            .header(header::CACHE_CONTROL, "no-cache")
            .header(header::COOKIE, "_rsid=\"\"")
            .form(&form_data)
            .send()
            .await
            .map_err(|e| Error::Network(format!("login request failed: {}", e)))?;

        let text = response
            .text()
            .await
            .map_err(|e| Error::Network(format!("failed to read login response: {}", e)))?;

        // Check for success response
        // Format: window.external.user("login=auth,ok,<params>");
        let success_regex =
            Regex::new(r#"window\.external\.user\("login=auth,ok,(?<launchParams>[^"]*)"\);"#)
                .expect("invalid regex");

        if let Some(captures) = success_regex.captures(&text) {
            let params_str = captures
                .name("launchParams")
                .ok_or_else(|| Error::InvalidServerResponse("launch params empty".to_string()))?
                .as_str();

            let params: Vec<&str> = params_str.split(',').collect();

            // Parse launch parameters
            // Index mapping from reference: SessionId=1, Region=5, TermsAccepted=3, Playable=9, MaxExpansion=13
            if params.len() < 14 {
                return Err(Error::InvalidServerResponse(format!(
                    "unexpected launch params count: {}",
                    params.len()
                )));
            }

            let session_id = params[1].to_string();
            let region = params[5]
                .parse::<i32>()
                .map_err(|_| Error::InvalidServerResponse("invalid region".to_string()))?;
            let terms_accepted = params[3] != "0";
            let playable = params[9] != "0";
            let max_expansion = params[13]
                .parse::<u32>()
                .map_err(|_| Error::InvalidServerResponse("invalid max expansion".to_string()))?;

            info!(
                playable = playable,
                terms_accepted = terms_accepted,
                region = region,
                max_expansion = max_expansion,
                "OAuth login successful"
            );

            return Ok(OauthLoginResult {
                session_id,
                region,
                terms_accepted,
                playable,
                max_expansion,
            });
        }

        // Check for error response
        // Format: window.external.user("login=auth,ng,err,<error_message>");
        let error_regex =
            Regex::new(r#"window\.external\.user\("login=auth,ng,err,(?<errorMessage>[^"]*)"\);"#)
                .expect("invalid regex");

        if let Some(captures) = error_regex.captures(&text) {
            let error_msg = captures
                .name("errorMessage")
                .map(|m| m.as_str())
                .unwrap_or("unknown");

            warn!(error = error_msg, "OAuth login failed");

            let oauth_error = parse_oauth_error(error_msg);
            return Err(Error::OauthLogin(oauth_error));
        }

        // Unknown response
        Err(Error::InvalidServerResponse(
            "unexpected login response format".to_string(),
        ))
    }
}

#[async_trait]
impl Authenticator for SquareEnixAuthenticator {
    #[instrument(skip(self, credentials))]
    async fn login(
        &self,
        credentials: &Credentials,
        region: Region,
        is_free_trial: bool,
    ) -> Result<OauthLoginResult, Error> {
        // Step 1: Get OAuth top page and extract _STORED_ token
        let stored_token = self.get_oauth_top(region, is_free_trial).await?;

        // Step 2: Send login credentials
        let result = self
            .send_login(credentials, &stored_token, region, is_free_trial)
            .await?;

        // Validate result
        if !result.playable {
            return Err(Error::AccountNotPlayable);
        }

        if !result.terms_accepted {
            return Err(Error::TermsNotAccepted);
        }

        Ok(result)
    }
}

/// Generate user agent string matching SE launcher format
fn generate_user_agent() -> String {
    let machine_id = make_computer_id();
    format!("SQEXAuthor/2.0.0(Windows 6.2; ja-jp; {})", machine_id)
}

/// Generate a machine-specific identifier
/// Uses SHA1 of hostname + username + OS info
fn make_computer_id() -> String {
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let username = whoami::username();
    let os_info = std::env::consts::OS;

    let hash_input = format!("{}{}{}", hostname, username, os_info);

    let mut hasher = Sha1::new();
    hasher.update(hash_input.as_bytes());
    let result = hasher.finalize();

    // Take first 5 bytes and format as checksum + hash
    let mut bytes = [0u8; 5];
    bytes[1..5].copy_from_slice(&result[0..4]);

    // Calculate checksum (negative sum of other bytes, wrapping)
    let checksum = bytes[1..5]
        .iter()
        .fold(0u8, |acc, &b| acc.wrapping_add(b))
        .wrapping_neg();
    bytes[0] = checksum;

    hex::encode(bytes)
}

/// Parse OAuth error message into typed error
fn parse_oauth_error(message: &str) -> OauthError {
    let lower = message.to_lowercase();

    if lower.contains("id or password") || lower.contains("incorrect") {
        OauthError::InvalidCredentials
    } else if lower.contains("one-time") || lower.contains("otp") {
        OauthError::InvalidOtp
    } else if lower.contains("locked") || lower.contains("suspended") {
        OauthError::AccountLocked
    } else if lower.contains("maintenance") {
        OauthError::MaintenanceMode
    } else if lower.contains("rate") || lower.contains("too many") {
        OauthError::RateLimited
    } else {
        OauthError::Unknown(message.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_user_agent() {
        let ua = generate_user_agent();
        assert!(ua.starts_with("SQEXAuthor/2.0.0"));
        assert!(ua.contains("Windows 6.2"));
    }

    #[test]
    fn test_make_computer_id() {
        let id1 = make_computer_id();
        let id2 = make_computer_id();

        // Should be deterministic
        assert_eq!(id1, id2);

        // Should be 10 hex characters (5 bytes)
        assert_eq!(id1.len(), 10);
        assert!(id1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_parse_oauth_error() {
        assert!(matches!(
            parse_oauth_error("ID or password is incorrect"),
            OauthError::InvalidCredentials
        ));

        assert!(matches!(
            parse_oauth_error("One-time password is invalid"),
            OauthError::InvalidOtp
        ));

        assert!(matches!(
            parse_oauth_error("Account has been locked"),
            OauthError::AccountLocked
        ));

        assert!(matches!(
            parse_oauth_error("Server is under maintenance"),
            OauthError::MaintenanceMode
        ));

        assert!(matches!(
            parse_oauth_error("Too many login attempts"),
            OauthError::RateLimited
        ));

        assert!(matches!(
            parse_oauth_error("Some unknown error"),
            OauthError::Unknown(_)
        ));
    }

    #[test]
    fn test_success_response_regex() {
        let response = r#"window.external.user("login=auth,ok,sid,SESSION123,0,1,0,3,0,0,0,1,0,0,5,0");"#;
        let regex =
            Regex::new(r#"window\.external\.user\("login=auth,ok,(?<launchParams>[^"]*)"\);"#)
                .unwrap();

        let captures = regex.captures(response).unwrap();
        let params_str = captures.name("launchParams").unwrap().as_str();
        let params: Vec<&str> = params_str.split(',').collect();

        assert!(params.len() >= 14);
        assert_eq!(params[1], "SESSION123");
    }

    #[test]
    fn test_error_response_regex() {
        let response = r#"window.external.user("login=auth,ng,err,ID or password is incorrect");"#;
        let regex =
            Regex::new(r#"window\.external\.user\("login=auth,ng,err,(?<errorMessage>[^"]*)"\);"#)
                .unwrap();

        let captures = regex.captures(response).unwrap();
        let error_msg = captures.name("errorMessage").unwrap().as_str();

        assert_eq!(error_msg, "ID or password is incorrect");
    }

    #[test]
    fn test_square_enix_authenticator_new() {
        let result = SquareEnixAuthenticator::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_oauth_error_additional_cases() {
        // Test suspended account
        assert!(matches!(
            parse_oauth_error("Your account has been suspended"),
            OauthError::AccountLocked
        ));

        // Test rate limiting
        assert!(matches!(
            parse_oauth_error("Rate limited due to suspicious activity"),
            OauthError::RateLimited
        ));

        // Test OTP variant
        assert!(matches!(
            parse_oauth_error("OTP verification failed"),
            OauthError::InvalidOtp
        ));
    }
}
