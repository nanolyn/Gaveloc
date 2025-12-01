use std::time::SystemTime;

use gaveloc_core::config::Region;
use gaveloc_core::entities::{AccountId, CachedSession, Credentials};
use gaveloc_core::ports::{AccountRepository, Authenticator, CredentialStore, OtpListener};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;

/// DTO for login result
#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResultDto {
    pub success: bool,
    pub session_id: Option<String>,
    pub region: Option<i32>,
    pub max_expansion: Option<u32>,
    pub playable: Option<bool>,
    pub error: Option<String>,
    pub error_type: Option<String>,
}

/// DTO for cached session status
#[derive(Debug, Serialize, Deserialize)]
pub struct CachedSessionDto {
    pub valid: bool,
    pub unique_id: Option<String>,
    pub region: Option<i32>,
    pub max_expansion: Option<u32>,
    pub remaining_secs: Option<i64>,
}

/// DTO for session status check
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionStatusDto {
    pub has_session: bool,
    pub is_valid: bool,
    pub remaining_secs: Option<i64>,
}

/// Classify error type for frontend handling
fn classify_error(error: &str) -> &'static str {
    let lower = error.to_lowercase();
    if lower.contains("credential") || lower.contains("password") || lower.contains("id or password") {
        "invalid_credentials"
    } else if lower.contains("otp") || lower.contains("one-time") {
        "invalid_otp"
    } else if lower.contains("locked") || lower.contains("suspended") {
        "account_locked"
    } else if lower.contains("maintenance") {
        "maintenance"
    } else if lower.contains("rate") || lower.contains("too many") {
        "rate_limited"
    } else if lower.contains("playable") || lower.contains("subscription") || lower.contains("service") {
        "no_subscription"
    } else if lower.contains("terms") {
        "terms_not_accepted"
    } else {
        "unknown"
    }
}

/// Login with username/password and optional OTP
#[tauri::command]
pub async fn login(
    state: State<'_, AppState>,
    account_id: String,
    password: String,
    otp: Option<String>,
    save_password: bool,
) -> Result<LoginResultDto, String> {
    let id = AccountId::new(&account_id);

    // Get account for account flags
    let account = state
        .accounts
        .get_account(&id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Account not found: {}", account_id))?;

    let is_free_trial = account.is_free_trial;

    // Build credentials
    let mut credentials = Credentials::new(account.username.clone(), password.clone());
    if let Some(otp_value) = otp {
        if !otp_value.is_empty() {
            credentials = credentials.with_otp(otp_value);
        }
    }

    // Get authenticator
    let authenticator_guard = state.authenticator.read().await;
    let authenticator = authenticator_guard
        .as_ref()
        .ok_or_else(|| "Authenticator not initialized".to_string())?;

    // Perform login (always use Europe region for global accounts)
    match authenticator.login(&credentials, Region::default(), is_free_trial).await {
        Ok(oauth_result) => {
            // Save password if requested
            if save_password {
                let _ = state.credentials.store_password(&id, &password).await;
            }

            // Cache session
            let session = CachedSession {
                unique_id: oauth_result.session_id.clone(),
                region: oauth_result.region,
                max_expansion: oauth_result.max_expansion,
                created_at: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0),
            };
            let _ = state.credentials.store_session(&id, &session).await;

            // Update account last_login
            let mut updated_account = account.clone();
            updated_account.last_login = Some(session.created_at);
            let _ = state.accounts.save_account(&updated_account).await;

            Ok(LoginResultDto {
                success: true,
                session_id: Some(oauth_result.session_id),
                region: Some(oauth_result.region),
                max_expansion: Some(oauth_result.max_expansion),
                playable: Some(oauth_result.playable),
                error: None,
                error_type: None,
            })
        }
        Err(e) => {
            let error_str = e.to_string();
            Ok(LoginResultDto {
                success: false,
                session_id: None,
                region: None,
                max_expansion: None,
                playable: None,
                error: Some(error_str.clone()),
                error_type: Some(classify_error(&error_str).to_string()),
            })
        }
    }
}

/// Try to login using cached session
#[tauri::command]
pub async fn login_with_cached_session(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<CachedSessionDto, String> {
    let id = AccountId::new(&account_id);

    match state.credentials.get_session(&id).await {
        Ok(Some(session)) => {
            let valid = session.is_valid();
            let remaining = if valid {
                Some(session.remaining_secs())
            } else {
                None
            };

            Ok(CachedSessionDto {
                valid,
                unique_id: if valid { Some(session.unique_id) } else { None },
                region: if valid { Some(session.region) } else { None },
                max_expansion: if valid { Some(session.max_expansion) } else { None },
                remaining_secs: remaining,
            })
        }
        Ok(None) => Ok(CachedSessionDto {
            valid: false,
            unique_id: None,
            region: None,
            max_expansion: None,
            remaining_secs: None,
        }),
        Err(e) => Err(e.to_string()),
    }
}

/// Logout - clear session and optionally password
#[tauri::command]
pub async fn logout(
    state: State<'_, AppState>,
    account_id: String,
    clear_password: bool,
) -> Result<(), String> {
    let id = AccountId::new(&account_id);

    // Always clear session
    let _ = state.credentials.delete_session(&id).await;

    // Optionally clear password
    if clear_password {
        let _ = state.credentials.delete_password(&id).await;
    }

    Ok(())
}

/// Get stored password for auto-fill
#[tauri::command]
pub async fn get_stored_password(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<Option<String>, String> {
    let id = AccountId::new(&account_id);
    state
        .credentials
        .get_password(&id)
        .await
        .map_err(|e| e.to_string())
}

/// Check session status
#[tauri::command]
pub async fn get_session_status(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<SessionStatusDto, String> {
    let id = AccountId::new(&account_id);

    match state.credentials.get_session(&id).await {
        Ok(Some(session)) => {
            let valid = session.is_valid();
            Ok(SessionStatusDto {
                has_session: true,
                is_valid: valid,
                remaining_secs: if valid {
                    Some(session.remaining_secs())
                } else {
                    None
                },
            })
        }
        Ok(None) => Ok(SessionStatusDto {
            has_session: false,
            is_valid: false,
            remaining_secs: None,
        }),
        Err(e) => Err(e.to_string()),
    }
}

/// Start OTP HTTP listener and emit event when OTP is received
#[tauri::command]
pub async fn start_otp_listener(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    // Check if already running
    if state.otp_listener.is_running() {
        return Err("OTP listener is already running".to_string());
    }

    // Start the listener
    let otp_rx = state
        .otp_listener
        .start()
        .await
        .map_err(|e| e.to_string())?;

    // Spawn task to wait for OTP and emit event
    tokio::spawn(async move {
        if let Ok(otp) = otp_rx.await {
            let _ = app_handle.emit("otp_received", otp);
        }
    });

    Ok(())
}

/// Stop OTP listener
#[tauri::command]
pub async fn stop_otp_listener(state: State<'_, AppState>) -> Result<(), String> {
    state
        .otp_listener
        .stop()
        .await
        .map_err(|e| e.to_string())
}

/// Check if OTP listener is running
#[tauri::command]
pub async fn is_otp_listener_running(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.otp_listener.is_running())
}
