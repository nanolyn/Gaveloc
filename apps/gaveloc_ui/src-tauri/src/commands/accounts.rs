use gaveloc_core::entities::{Account, AccountId};
use gaveloc_core::ports::{AccountRepository, CredentialStore};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct AccountDto {
    pub id: String,
    pub username: String,
    pub is_steam: bool,
    pub is_free_trial: bool,
    pub use_otp: bool,
    pub last_login: Option<i64>,
}

impl From<Account> for AccountDto {
    fn from(account: Account) -> Self {
        Self {
            id: account.id.as_str().to_string(),
            username: account.username,
            is_steam: account.is_steam,
            is_free_trial: account.is_free_trial,
            use_otp: account.use_otp,
            last_login: account.last_login,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateAccountRequest {
    pub username: String,
    pub is_steam: bool,
    pub is_free_trial: bool,
    pub use_otp: bool,
}

#[tauri::command]
pub async fn list_accounts(state: State<'_, AppState>) -> Result<Vec<AccountDto>, String> {
    state
        .accounts
        .list_accounts()
        .await
        .map(|accounts| accounts.into_iter().map(AccountDto::from).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_default_account(state: State<'_, AppState>) -> Result<Option<AccountDto>, String> {
    state
        .accounts
        .get_default_account()
        .await
        .map(|opt| opt.map(AccountDto::from))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_account(
    state: State<'_, AppState>,
    request: CreateAccountRequest,
) -> Result<AccountDto, String> {
    let mut account = Account::new(request.username);
    account.is_steam = request.is_steam;
    account.is_free_trial = request.is_free_trial;
    account.use_otp = request.use_otp;

    state
        .accounts
        .save_account(&account)
        .await
        .map_err(|e| e.to_string())?;

    Ok(AccountDto::from(account))
}

#[tauri::command]
pub async fn update_account(
    state: State<'_, AppState>,
    request: CreateAccountRequest,
) -> Result<AccountDto, String> {
    let account_id = AccountId::new(&request.username);

    // Get existing account to preserve last_login
    let existing = state
        .accounts
        .get_account(&account_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut account = Account::new(request.username);
    account.is_steam = request.is_steam;
    account.is_free_trial = request.is_free_trial;
    account.use_otp = request.use_otp;
    account.last_login = existing.and_then(|a| a.last_login);

    state
        .accounts
        .save_account(&account)
        .await
        .map_err(|e| e.to_string())?;

    Ok(AccountDto::from(account))
}

#[tauri::command]
pub async fn remove_account(state: State<'_, AppState>, account_id: String) -> Result<(), String> {
    let id = AccountId::new(&account_id);

    // Delete credentials first (ignore errors if not found)
    let _ = state.credentials.delete_password(&id).await;
    let _ = state.credentials.delete_session(&id).await;

    // Delete the account
    state
        .accounts
        .delete_account(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_default_account(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<(), String> {
    let id = AccountId::new(&account_id);
    state
        .accounts
        .set_default_account(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn has_stored_password(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<bool, String> {
    let id = AccountId::new(&account_id);
    state
        .credentials
        .has_credentials(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn store_password(
    state: State<'_, AppState>,
    account_id: String,
    password: String,
) -> Result<(), String> {
    let id = AccountId::new(&account_id);
    state
        .credentials
        .store_password(&id, &password)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_password(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<(), String> {
    let id = AccountId::new(&account_id);
    state
        .credentials
        .delete_password(&id)
        .await
        .map_err(|e| e.to_string())
}
