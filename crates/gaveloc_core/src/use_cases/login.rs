use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::Region;
use crate::entities::{
    Account, AccountId, CachedSession, Credentials, LoginResult, LoginState, OauthLoginResult,
};
use crate::error::Error;
use crate::ports::{
    AccountRepository, Authenticator, CredentialStore, PatchServer, VersionRepository,
};

/// Orchestrates the complete login flow including:
/// - Cached session validation
/// - OAuth authentication
/// - Session registration with patch server
/// - Credential and session persistence
pub struct LoginUseCase<A, C, Auth, P, V>
where
    A: AccountRepository,
    C: CredentialStore,
    Auth: Authenticator,
    P: PatchServer,
    V: VersionRepository,
{
    account_repo: Arc<A>,
    credential_store: Arc<C>,
    authenticator: Arc<Auth>,
    patch_server: Arc<P>,
    version_repo: Arc<V>,
}

impl<A, C, Auth, P, V> LoginUseCase<A, C, Auth, P, V>
where
    A: AccountRepository,
    C: CredentialStore,
    Auth: Authenticator,
    P: PatchServer,
    V: VersionRepository,
{
    pub fn new(
        account_repo: Arc<A>,
        credential_store: Arc<C>,
        authenticator: Arc<Auth>,
        patch_server: Arc<P>,
        version_repo: Arc<V>,
    ) -> Self {
        Self {
            account_repo,
            credential_store,
            authenticator,
            patch_server,
            version_repo,
        }
    }

    /// Attempt login with credentials, checking for cached session first.
    ///
    /// Flow:
    /// 1. Check for valid cached session
    /// 2. If no valid session, perform OAuth login
    /// 3. Register session with patch server to get unique_id and check for patches
    /// 4. Cache the session for future use
    /// 5. Update account metadata
    ///
    /// Returns LoginResult with state indicating if game needs patching.
    pub async fn execute(
        &self,
        credentials: &Credentials,
        game_path: &Path,
        is_free_trial: bool,
    ) -> Result<LoginResult, Error> {
        let account_id = AccountId::new(&credentials.username);

        // Try to use cached session first
        if let Some(cached) = self.try_cached_session(&account_id).await? {
            return self
                .complete_login_with_session(
                    &account_id,
                    &cached.unique_id,
                    cached.max_expansion,
                    game_path,
                )
                .await;
        }

        // Get or create account (ensures account exists for credential storage)
        let _account = self.get_or_create_account(&account_id, credentials).await?;

        // Perform OAuth login (always use Europe region for global accounts)
        let oauth_result = self
            .authenticator
            .login(credentials, Region::default(), is_free_trial)
            .await?;

        // Check login state
        if !oauth_result.playable {
            return Ok(LoginResult {
                state: LoginState::NoService,
                oauth: Some(oauth_result),
                unique_id: None,
            });
        }

        if !oauth_result.terms_accepted {
            return Ok(LoginResult {
                state: LoginState::NoTerms,
                oauth: Some(oauth_result),
                unique_id: None,
            });
        }

        // Check boot version first
        let boot_patches = self.check_boot_patches(game_path).await?;
        if !boot_patches.is_empty() {
            return Ok(LoginResult {
                state: LoginState::NeedsPatchBoot,
                oauth: Some(oauth_result),
                unique_id: None,
            });
        }

        // Register session with game version server
        let (unique_id, game_patches) = self
            .patch_server
            .register_session(&oauth_result.session_id, game_path, oauth_result.max_expansion)
            .await?;

        // Cache the session
        self.cache_session(&account_id, &unique_id, &oauth_result)
            .await?;

        // Update account last login
        self.update_account_login(&account_id).await?;

        // Determine final state
        let state = if game_patches.is_empty() {
            LoginState::Ok
        } else {
            LoginState::NeedsPatchGame
        };

        Ok(LoginResult {
            state,
            oauth: Some(oauth_result),
            unique_id: Some(unique_id),
        })
    }

    /// Login using stored credentials (for auto-login scenarios)
    pub async fn execute_with_stored_credentials(
        &self,
        account_id: &AccountId,
        game_path: &Path,
    ) -> Result<LoginResult, Error> {
        // Get stored password
        let password = self
            .credential_store
            .get_password(account_id)
            .await?
            .ok_or(Error::CredentialStorage(
                "no stored password found".to_string(),
            ))?;

        // Get account for region info
        let account = self
            .account_repo
            .get_account(account_id)
            .await?
            .ok_or(Error::Authentication("account not found".to_string()))?;

        let credentials = Credentials::new(account.username.clone(), password);

        self.execute(&credentials, game_path, account.is_free_trial)
            .await
    }

    /// Store credentials for future auto-login
    pub async fn store_credentials(
        &self,
        credentials: &Credentials,
        account: &Account,
    ) -> Result<(), Error> {
        // Save account metadata
        self.account_repo.save_account(account).await?;

        // Store password securely
        self.credential_store
            .store_password(&account.id, &credentials.password)
            .await?;

        Ok(())
    }

    /// Clear stored session (force re-authentication on next login)
    pub async fn clear_session(&self, account_id: &AccountId) -> Result<(), Error> {
        self.credential_store.delete_session(account_id).await
    }

    /// Clear all stored credentials for an account
    pub async fn clear_credentials(&self, account_id: &AccountId) -> Result<(), Error> {
        self.credential_store.delete_password(account_id).await?;
        self.credential_store.delete_session(account_id).await?;
        Ok(())
    }

    // =========================================================================
    // Private helpers
    // =========================================================================

    async fn try_cached_session(
        &self,
        account_id: &AccountId,
    ) -> Result<Option<CachedSession>, Error> {
        if let Some(session) = self.credential_store.get_session(account_id).await? {
            if session.is_valid() {
                return Ok(Some(session));
            }
            // Session expired, clean it up
            self.credential_store.delete_session(account_id).await?;
        }
        Ok(None)
    }

    async fn complete_login_with_session(
        &self,
        account_id: &AccountId,
        unique_id: &str,
        max_expansion: u32,
        game_path: &Path,
    ) -> Result<LoginResult, Error> {
        // Check boot patches
        let boot_patches = self.check_boot_patches(game_path).await?;
        if !boot_patches.is_empty() {
            return Ok(LoginResult {
                state: LoginState::NeedsPatchBoot,
                oauth: None,
                unique_id: Some(unique_id.to_string()),
            });
        }

        // Validate game installation by checking version files exist
        let _version_report = self
            .version_repo
            .get_version_report(game_path, max_expansion)
            .await?;

        // Update account last login
        self.update_account_login(account_id).await?;

        // For cached sessions we return Ok - actual patch check happens via UpdateGameUseCase
        Ok(LoginResult {
            state: LoginState::Ok,
            oauth: None,
            unique_id: Some(unique_id.to_string()),
        })
    }

    async fn get_or_create_account(
        &self,
        account_id: &AccountId,
        credentials: &Credentials,
    ) -> Result<Account, Error> {
        if let Some(account) = self.account_repo.get_account(account_id).await? {
            return Ok(account);
        }

        // Create new account
        let account = Account::new(credentials.username.clone());
        self.account_repo.save_account(&account).await?;
        Ok(account)
    }

    async fn check_boot_patches(
        &self,
        game_path: &Path,
    ) -> Result<Vec<crate::entities::PatchEntry>, Error> {
        let boot_version = self
            .version_repo
            .get_version(game_path, crate::entities::Repository::Boot)
            .await?;
        self.patch_server
            .check_boot_version(game_path, &boot_version)
            .await
    }

    async fn cache_session(
        &self,
        account_id: &AccountId,
        unique_id: &str,
        oauth: &OauthLoginResult,
    ) -> Result<(), Error> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let session = CachedSession {
            unique_id: unique_id.to_string(),
            region: oauth.region,
            max_expansion: oauth.max_expansion,
            created_at: now,
        };

        self.credential_store
            .store_session(account_id, &session)
            .await
    }

    async fn update_account_login(&self, account_id: &AccountId) -> Result<(), Error> {
        if let Some(mut account) = self.account_repo.get_account(account_id).await? {
            account.last_login = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0),
            );
            self.account_repo.save_account(&account).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // Tests would go here with mock implementations of the ports
}
