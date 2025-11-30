use std::path::PathBuf;

use async_trait::async_trait;
use gaveloc_core::entities::{Account, AccountId};
use gaveloc_core::ports::AccountRepository;
use gaveloc_core::Error;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::{debug, instrument};

#[derive(Debug, Serialize, Deserialize, Default)]
struct AccountStore {
    accounts: Vec<Account>,
    default_account: Option<String>,
}

/// File-based account repository
pub struct FileAccountRepository {
    store_path: PathBuf,
}

impl FileAccountRepository {
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            store_path: config_dir.join("accounts.json"),
        }
    }

    async fn load(&self) -> Result<AccountStore, Error> {
        if !fs::try_exists(&self.store_path).await.unwrap_or(false) {
            return Ok(AccountStore::default());
        }

        let content = fs::read_to_string(&self.store_path).await?;
        serde_json::from_str(&content)
            .map_err(|e| Error::Other(format!("failed to parse accounts file: {}", e)))
    }

    async fn save(&self, store: &AccountStore) -> Result<(), Error> {
        // Ensure parent directory exists
        if let Some(parent) = self.store_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let content = serde_json::to_string_pretty(store)
            .map_err(|e| Error::Other(format!("failed to serialize accounts: {}", e)))?;
        fs::write(&self.store_path, content).await?;
        Ok(())
    }
}

#[async_trait]
impl AccountRepository for FileAccountRepository {
    #[instrument(skip(self))]
    async fn list_accounts(&self) -> Result<Vec<Account>, Error> {
        let store = self.load().await?;
        Ok(store.accounts)
    }

    #[instrument(skip(self))]
    async fn get_account(&self, id: &AccountId) -> Result<Option<Account>, Error> {
        let store = self.load().await?;
        Ok(store.accounts.into_iter().find(|a| &a.id == id))
    }

    #[instrument(skip(self, account))]
    async fn save_account(&self, account: &Account) -> Result<(), Error> {
        let mut store = self.load().await?;

        debug!(username = %account.username, "saving account");

        // Update existing or add new
        if let Some(existing) = store.accounts.iter_mut().find(|a| a.id == account.id) {
            *existing = account.clone();
        } else {
            store.accounts.push(account.clone());
        }

        self.save(&store).await
    }

    #[instrument(skip(self))]
    async fn delete_account(&self, id: &AccountId) -> Result<(), Error> {
        let mut store = self.load().await?;
        store.accounts.retain(|a| &a.id != id);

        // Clear default if it was this account
        if store
            .default_account
            .as_ref()
            .map(|s| s.as_str())
            == Some(id.as_str())
        {
            store.default_account = None;
        }

        self.save(&store).await
    }

    #[instrument(skip(self))]
    async fn get_default_account(&self) -> Result<Option<Account>, Error> {
        let store = self.load().await?;

        if let Some(default_id) = &store.default_account {
            let id = AccountId::new(default_id);
            Ok(store.accounts.into_iter().find(|a| a.id == id))
        } else {
            // Return first account if no default set
            Ok(store.accounts.into_iter().next())
        }
    }

    #[instrument(skip(self))]
    async fn set_default_account(&self, id: &AccountId) -> Result<(), Error> {
        let mut store = self.load().await?;

        // Verify account exists
        if !store.accounts.iter().any(|a| &a.id == id) {
            return Err(Error::Other(format!("account '{}' not found", id)));
        }

        store.default_account = Some(id.as_str().to_string());
        self.save(&store).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gaveloc_core::config::Region;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_account_crud() {
        let dir = tempdir().unwrap();
        let repo = FileAccountRepository::new(dir.path().to_path_buf());

        // Initially empty
        let accounts = repo.list_accounts().await.unwrap();
        assert!(accounts.is_empty());

        // Create account
        let account = Account::new("TestUser".to_string(), Region::Europe);
        repo.save_account(&account).await.unwrap();

        // Retrieve
        let retrieved = repo.get_account(&account.id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().username, "TestUser");

        // List
        let accounts = repo.list_accounts().await.unwrap();
        assert_eq!(accounts.len(), 1);

        // Delete
        repo.delete_account(&account.id).await.unwrap();
        let accounts = repo.list_accounts().await.unwrap();
        assert!(accounts.is_empty());
    }

    #[tokio::test]
    async fn test_update_existing_account() {
        let dir = tempdir().unwrap();
        let repo = FileAccountRepository::new(dir.path().to_path_buf());

        // Create account
        let mut account = Account::new("TestUser".to_string(), Region::Europe);
        repo.save_account(&account).await.unwrap();

        // Update account
        account.use_otp = true;
        account.is_free_trial = true;
        repo.save_account(&account).await.unwrap();

        // Verify update
        let retrieved = repo.get_account(&account.id).await.unwrap().unwrap();
        assert!(retrieved.use_otp);
        assert!(retrieved.is_free_trial);

        // Should still only have one account
        let accounts = repo.list_accounts().await.unwrap();
        assert_eq!(accounts.len(), 1);
    }

    #[tokio::test]
    async fn test_default_account() {
        let dir = tempdir().unwrap();
        let repo = FileAccountRepository::new(dir.path().to_path_buf());

        let account1 = Account::new("User1".to_string(), Region::Europe);
        let account2 = Account::new("User2".to_string(), Region::NorthAmerica);

        repo.save_account(&account1).await.unwrap();
        repo.save_account(&account2).await.unwrap();

        // First account is default when none set
        let default = repo.get_default_account().await.unwrap().unwrap();
        assert_eq!(default.username, "User1");

        // Set explicit default
        repo.set_default_account(&account2.id).await.unwrap();
        let default = repo.get_default_account().await.unwrap().unwrap();
        assert_eq!(default.username, "User2");
    }

    #[tokio::test]
    async fn test_default_cleared_on_delete() {
        let dir = tempdir().unwrap();
        let repo = FileAccountRepository::new(dir.path().to_path_buf());

        let account1 = Account::new("User1".to_string(), Region::Europe);
        let account2 = Account::new("User2".to_string(), Region::NorthAmerica);

        repo.save_account(&account1).await.unwrap();
        repo.save_account(&account2).await.unwrap();
        repo.set_default_account(&account2.id).await.unwrap();

        // Delete the default account
        repo.delete_account(&account2.id).await.unwrap();

        // Default should now be account1 (first remaining)
        let default = repo.get_default_account().await.unwrap().unwrap();
        assert_eq!(default.username, "User1");
    }

    #[tokio::test]
    async fn test_set_nonexistent_default() {
        let dir = tempdir().unwrap();
        let repo = FileAccountRepository::new(dir.path().to_path_buf());

        let nonexistent = AccountId::new("nobody");
        let result = repo.set_default_account(&nonexistent).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_no_default_when_empty() {
        let dir = tempdir().unwrap();
        let repo = FileAccountRepository::new(dir.path().to_path_buf());

        let default = repo.get_default_account().await.unwrap();
        assert!(default.is_none());
    }
}
