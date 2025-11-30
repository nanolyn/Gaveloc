use async_trait::async_trait;
use gaveloc_core::entities::{AccountId, CachedSession};
use gaveloc_core::ports::CredentialStore;
use gaveloc_core::Error;
use keyring::Entry;
use tracing::{debug, instrument};

const SERVICE_NAME: &str = "gaveloc";
const PASSWORD_PREFIX: &str = "password";
const SESSION_PREFIX: &str = "session";

/// Keyring-based credential store using libsecret on Linux
pub struct KeyringCredentialStore;

impl KeyringCredentialStore {
    pub fn new() -> Self {
        Self
    }

    fn password_key(account_id: &AccountId) -> String {
        format!("{}:{}", PASSWORD_PREFIX, account_id.as_str())
    }

    fn session_key(account_id: &AccountId) -> String {
        format!("{}:{}", SESSION_PREFIX, account_id.as_str())
    }

    fn get_entry(key: &str) -> Result<Entry, Error> {
        Entry::new(SERVICE_NAME, key)
            .map_err(|e| Error::CredentialStorage(format!("failed to create keyring entry: {}", e)))
    }
}

impl Default for KeyringCredentialStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CredentialStore for KeyringCredentialStore {
    #[instrument(skip(self, password))]
    async fn store_password(&self, account_id: &AccountId, password: &str) -> Result<(), Error> {
        let key = Self::password_key(account_id);
        let entry = Self::get_entry(&key)?;

        // Run blocking keyring operation in spawn_blocking
        let password = password.to_string();
        tokio::task::spawn_blocking(move || {
            entry
                .set_password(&password)
                .map_err(|e| Error::CredentialStorage(format!("failed to store password: {}", e)))
        })
        .await
        .map_err(|e| Error::CredentialStorage(format!("task join error: {}", e)))?
    }

    #[instrument(skip(self))]
    async fn get_password(&self, account_id: &AccountId) -> Result<Option<String>, Error> {
        let key = Self::password_key(account_id);
        let entry = Self::get_entry(&key)?;

        tokio::task::spawn_blocking(move || match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(Error::CredentialStorage(format!(
                "failed to get password: {}",
                e
            ))),
        })
        .await
        .map_err(|e| Error::CredentialStorage(format!("task join error: {}", e)))?
    }

    #[instrument(skip(self))]
    async fn delete_password(&self, account_id: &AccountId) -> Result<(), Error> {
        let key = Self::password_key(account_id);
        let entry = Self::get_entry(&key)?;

        tokio::task::spawn_blocking(move || {
            match entry.delete_credential() {
                Ok(()) => Ok(()),
                Err(keyring::Error::NoEntry) => Ok(()), // Already deleted
                Err(e) => Err(Error::CredentialStorage(format!(
                    "failed to delete password: {}",
                    e
                ))),
            }
        })
        .await
        .map_err(|e| Error::CredentialStorage(format!("task join error: {}", e)))?
    }

    #[instrument(skip(self, session))]
    async fn store_session(
        &self,
        account_id: &AccountId,
        session: &CachedSession,
    ) -> Result<(), Error> {
        let key = Self::session_key(account_id);
        let entry = Self::get_entry(&key)?;

        let json = serde_json::to_string(session)
            .map_err(|e| Error::CredentialStorage(format!("failed to serialize session: {}", e)))?;

        debug!(key = %key, "storing session in keyring");

        tokio::task::spawn_blocking(move || {
            entry
                .set_password(&json)
                .map_err(|e| Error::CredentialStorage(format!("failed to store session: {}", e)))
        })
        .await
        .map_err(|e| Error::CredentialStorage(format!("task join error: {}", e)))?
    }

    #[instrument(skip(self))]
    async fn get_session(&self, account_id: &AccountId) -> Result<Option<CachedSession>, Error> {
        let key = Self::session_key(account_id);
        let entry = Self::get_entry(&key)?;

        tokio::task::spawn_blocking(move || match entry.get_password() {
            Ok(json) => {
                let session: CachedSession = serde_json::from_str(&json).map_err(|e| {
                    Error::CredentialStorage(format!("failed to deserialize session: {}", e))
                })?;
                Ok(Some(session))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(Error::CredentialStorage(format!(
                "failed to get session: {}",
                e
            ))),
        })
        .await
        .map_err(|e| Error::CredentialStorage(format!("task join error: {}", e)))?
    }

    #[instrument(skip(self))]
    async fn delete_session(&self, account_id: &AccountId) -> Result<(), Error> {
        let key = Self::session_key(account_id);
        let entry = Self::get_entry(&key)?;

        tokio::task::spawn_blocking(move || {
            match entry.delete_credential() {
                Ok(()) => Ok(()),
                Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(Error::CredentialStorage(format!(
                    "failed to delete session: {}",
                    e
                ))),
            }
        })
        .await
        .map_err(|e| Error::CredentialStorage(format!("task join error: {}", e)))?
    }

    #[instrument(skip(self))]
    async fn has_credentials(&self, account_id: &AccountId) -> Result<bool, Error> {
        Ok(self.get_password(account_id).await?.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a working keyring service
    // They should be marked as integration tests or ignored in CI

    #[tokio::test]
    #[ignore = "requires keyring service"]
    async fn test_store_and_get_password() {
        let store = KeyringCredentialStore::new();
        let account_id = AccountId::new("test_user_gaveloc");

        // Clean up any existing entry
        let _ = store.delete_password(&account_id).await;

        // Store password
        store
            .store_password(&account_id, "test_password")
            .await
            .unwrap();

        // Retrieve password
        let password = store.get_password(&account_id).await.unwrap();
        assert_eq!(password, Some("test_password".to_string()));

        // Clean up
        store.delete_password(&account_id).await.unwrap();

        // Verify deleted
        let password = store.get_password(&account_id).await.unwrap();
        assert!(password.is_none());
    }

    #[tokio::test]
    #[ignore = "requires keyring service"]
    async fn test_store_and_get_session() {
        let store = KeyringCredentialStore::new();
        let account_id = AccountId::new("test_user_gaveloc_session");

        // Clean up any existing entry
        let _ = store.delete_session(&account_id).await;

        // Create session
        let session = CachedSession {
            unique_id: "test_uid".to_string(),
            region: 3,
            max_expansion: 5,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        };

        // Store session
        store.store_session(&account_id, &session).await.unwrap();

        // Retrieve session
        let retrieved = store.get_session(&account_id).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.unique_id, "test_uid");
        assert_eq!(retrieved.region, 3);
        assert!(retrieved.is_valid());

        // Clean up
        store.delete_session(&account_id).await.unwrap();

        // Verify deleted
        let session = store.get_session(&account_id).await.unwrap();
        assert!(session.is_none());
    }

    #[tokio::test]
    #[ignore = "requires keyring service"]
    async fn test_has_credentials() {
        let store = KeyringCredentialStore::new();
        let account_id = AccountId::new("test_user_gaveloc_has_creds");

        // Clean up
        let _ = store.delete_password(&account_id).await;

        // Should not have credentials
        assert!(!store.has_credentials(&account_id).await.unwrap());

        // Store password
        store
            .store_password(&account_id, "test_password")
            .await
            .unwrap();

        // Should have credentials now
        assert!(store.has_credentials(&account_id).await.unwrap());

        // Clean up
        store.delete_password(&account_id).await.unwrap();
    }
}
