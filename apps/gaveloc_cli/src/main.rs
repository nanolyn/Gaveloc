use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand};
use dialoguer::{Confirm, Input};
use gaveloc_adapters::configuration;
use gaveloc_adapters::runner::{LinuxRunnerDetector, LinuxRunnerManager};
use gaveloc_adapters::telemetry;
use gaveloc_adapters::{
    FileAccountRepository, HttpOtpListener, KeyringCredentialStore, SquareEnixAuthenticator,
};
use gaveloc_core::config::Region;
use gaveloc_core::entities::{Account, AccountId, CachedSession, Credentials};
use gaveloc_core::ports::{
    AccountRepository, Authenticator, CredentialStore, OtpListener, RunnerDetector, RunnerManager,
};
use tracing::error;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    // --- Runner commands ---
    /// List all detected Wine/Proton runners
    Runners,
    /// Validate a custom runner path
    CheckRunner { path: String },
    /// Download and install latest GE-Proton
    InstallRunner,

    // --- Authentication commands ---
    /// Login to Square Enix account
    Login {
        /// Username (SE account ID)
        #[arg(short, long)]
        username: Option<String>,

        /// Save credentials to keyring
        #[arg(short, long, default_value = "false")]
        save: bool,

        /// Use saved credentials if available
        #[arg(short = 'c', long, default_value = "true")]
        use_cached: bool,

        /// Start OTP listener for mobile app
        #[arg(long, default_value = "false")]
        otp_listener: bool,
    },

    /// List saved accounts
    Accounts,

    /// Add a new account
    AddAccount {
        /// Username (SE account ID)
        username: String,

        /// Region (japan, northamerica, europe)
        #[arg(short, long, default_value = "europe")]
        region: String,

        /// Account uses OTP
        #[arg(long, default_value = "false")]
        otp: bool,

        /// Free trial account
        #[arg(long, default_value = "false")]
        free_trial: bool,
    },

    /// Remove a saved account
    RemoveAccount {
        /// Username to remove
        username: String,

        /// Also delete stored credentials
        #[arg(long, default_value = "true")]
        delete_credentials: bool,
    },

    /// Set default account
    SetDefault {
        /// Username to set as default
        username: String,
    },

    /// Clear cached session for an account
    ClearSession {
        /// Username (or "all" to clear all sessions)
        username: String,
    },

    /// Test credential storage (keyring)
    TestKeyring,
}

fn get_config_dir() -> PathBuf {
    directories::ProjectDirs::from("com", "gaveloc", "gaveloc")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("~/.config"))
                .join("gaveloc")
        })
}

fn parse_region(s: &str) -> Region {
    match s.to_lowercase().as_str() {
        "japan" | "jp" | "1" => Region::Japan,
        "northamerica" | "na" | "2" => Region::NorthAmerica,
        "europe" | "eu" | "3" => Region::Europe,
        _ => Region::Europe,
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let _guard = telemetry::init_subscriber("gaveloc_cli", "info");

    let _settings = match configuration::get_configuration() {
        Ok(s) => s,
        Err(e) => {
            error!(?e, "failed to load configuration");
            return Err(anyhow::anyhow!("configuration loading failed"));
        }
    };

    let cli = Cli::parse();

    match &cli.command {
        // --- Runner commands ---
        Commands::Runners => {
            println!("Detecting runners...");
            let detector = LinuxRunnerDetector;
            match detector.detect_runners().await {
                Ok(runners) => {
                    if runners.is_empty() {
                        println!("No runners detected.");
                    } else {
                        println!("Found {} runners:", runners.len());
                        for runner in runners {
                            println!(
                                "- [{}] {} ({})",
                                runner.runner_type,
                                runner.name,
                                runner.path.display()
                            );
                        }
                    }
                }
                Err(e) => {
                    error!(?e, "failed to detect runners");
                }
            }
        }
        Commands::CheckRunner { path } => {
            println!("Checking runner at: {}", path);
            let detector = LinuxRunnerDetector;
            match detector.validate_runner(PathBuf::from(path)).await {
                Ok(runner) => {
                    println!(
                        "Valid runner found: [{}] {} ({})",
                        runner.runner_type,
                        runner.name,
                        runner.path.display()
                    );
                }
                Err(e) => {
                    error!(?e, "runner validation failed");
                }
            }
        }
        Commands::InstallRunner => {
            println!("Starting installation of latest GE-Proton...");
            let manager = LinuxRunnerManager;
            match manager.install_latest_ge_proton().await {
                Ok(runner) => {
                    println!("Successfully installed runner!");
                    println!("- Name: {}", runner.name);
                    println!("- Path: {}", runner.path.display());
                }
                Err(e) => {
                    error!(?e, "failed to install runner");
                }
            }
        }

        // --- Authentication commands ---
        Commands::Login {
            username,
            save,
            use_cached,
            otp_listener,
        } => {
            let config_dir = get_config_dir();
            let account_repo = FileAccountRepository::new(config_dir);
            let credential_store = KeyringCredentialStore::new();
            let authenticator = SquareEnixAuthenticator::new()?;

            // Determine which account to use
            let account = if let Some(username) = username {
                let account_id = AccountId::new(username);
                account_repo.get_account(&account_id).await?.ok_or_else(|| {
                    anyhow::anyhow!(
                        "Account '{}' not found. Use 'add-account' first.",
                        username
                    )
                })?
            } else {
                account_repo
                    .get_default_account()
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("No accounts configured. Use 'add-account' first."))?
            };

            println!("Logging in as: {}", account.username);

            // Check for cached session
            if *use_cached {
                if let Ok(Some(session)) = credential_store.get_session(&account.id).await {
                    if session.is_valid() {
                        let hours_remaining = session.remaining_secs() / 3600;
                        println!(
                            "Using cached session (valid for {} more hours)",
                            hours_remaining
                        );
                        println!("Unique ID: {}", session.unique_id);
                        return Ok(());
                    } else {
                        println!("Cached session expired, performing fresh login...");
                    }
                }
            }

            // Get password
            let password = if *use_cached {
                credential_store.get_password(&account.id).await?
            } else {
                None
            };

            let password = match password {
                Some(p) => {
                    println!("Using saved password");
                    p
                }
                None => rpassword::prompt_password("Password: ")?,
            };

            // Handle OTP
            let otp = if account.use_otp {
                if *otp_listener {
                    println!("Starting OTP listener on port 4646...");
                    println!("Send OTP from your authenticator app or enter manually.");

                    let listener = HttpOtpListener::new();
                    let otp_rx = listener.start().await?;

                    // Race between listener and manual input with timeout
                    let otp = tokio::select! {
                        result = otp_rx => {
                            match result {
                                Ok(otp) => Some(otp),
                                Err(_) => None,
                            }
                        }
                        _ = tokio::time::sleep(Duration::from_secs(120)) => {
                            println!("OTP listener timeout, please enter manually:");
                            let input: String = Input::new()
                                .with_prompt("OTP")
                                .interact_text()?;
                            Some(input)
                        }
                    };

                    listener.stop().await?;
                    otp
                } else {
                    let otp: String = Input::new()
                        .with_prompt("One-Time Password")
                        .interact_text()?;
                    Some(otp)
                }
            } else {
                None
            };

            // Build credentials
            let mut credentials = Credentials::new(account.username.clone(), password.clone());
            if let Some(otp) = otp {
                credentials = credentials.with_otp(otp);
            }

            // Perform login
            println!("Authenticating...");
            match authenticator
                .login(&credentials, account.region, account.is_free_trial)
                .await
            {
                Ok(result) => {
                    println!("Login successful!");
                    println!("- Region: {}", result.region);
                    println!("- Max Expansion: {}", result.max_expansion);
                    println!(
                        "- Session ID: {}...",
                        &result.session_id[..8.min(result.session_id.len())]
                    );

                    // Save credentials if requested
                    if *save {
                        credential_store
                            .store_password(&account.id, &password)
                            .await?;
                        println!("Password saved to keyring");
                    }

                    // Cache session
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64;

                    let session = CachedSession {
                        unique_id: result.session_id.clone(),
                        region: result.region,
                        max_expansion: result.max_expansion,
                        created_at: now,
                    };
                    credential_store.store_session(&account.id, &session).await?;
                    println!("Session cached");
                }
                Err(e) => {
                    error!(?e, "Login failed");
                    println!("Login failed: {}", e);
                }
            }
        }

        Commands::Accounts => {
            let config_dir = get_config_dir();
            let account_repo = FileAccountRepository::new(config_dir);
            let credential_store = KeyringCredentialStore::new();

            let accounts = account_repo.list_accounts().await?;
            let default = account_repo.get_default_account().await?;

            if accounts.is_empty() {
                println!("No accounts configured.");
                println!("Use 'gaveloc_cli add-account <username>' to add one.");
            } else {
                println!("Saved accounts:");
                for account in &accounts {
                    let is_default = default.as_ref().map(|d| d.id == account.id).unwrap_or(false);
                    let has_password = credential_store
                        .has_credentials(&account.id)
                        .await
                        .unwrap_or(false);
                    let has_session = credential_store
                        .get_session(&account.id)
                        .await
                        .ok()
                        .flatten()
                        .map(|s| s.is_valid())
                        .unwrap_or(false);

                    println!(
                        "  {} {} [{:?}] {}{}{}",
                        if is_default { "*" } else { " " },
                        account.username,
                        account.region,
                        if account.use_otp { "[OTP] " } else { "" },
                        if has_password {
                            "[password saved] "
                        } else {
                            ""
                        },
                        if has_session { "[session cached]" } else { "" },
                    );
                }
            }
        }

        Commands::AddAccount {
            username,
            region,
            otp,
            free_trial,
        } => {
            let config_dir = get_config_dir();
            let account_repo = FileAccountRepository::new(config_dir);

            let region = parse_region(region);
            let mut account = Account::new(username.clone(), region);
            account.use_otp = *otp;
            account.is_free_trial = *free_trial;

            account_repo.save_account(&account).await?;

            println!("Account '{}' added successfully.", username);
            println!("  Region: {:?}", region);
            println!("  OTP: {}", if *otp { "enabled" } else { "disabled" });
            println!(
                "  Free Trial: {}",
                if *free_trial { "yes" } else { "no" }
            );
        }

        Commands::RemoveAccount {
            username,
            delete_credentials,
        } => {
            let config_dir = get_config_dir();
            let account_repo = FileAccountRepository::new(config_dir);
            let credential_store = KeyringCredentialStore::new();

            let account_id = AccountId::new(username);

            // Verify account exists
            if account_repo.get_account(&account_id).await?.is_none() {
                println!("Account '{}' not found.", username);
                return Ok(());
            }

            // Confirm deletion
            let confirmed = Confirm::new()
                .with_prompt(format!("Delete account '{}'?", username))
                .default(false)
                .interact()?;

            if !confirmed {
                println!("Cancelled.");
                return Ok(());
            }

            // Delete credentials if requested
            if *delete_credentials {
                credential_store.delete_password(&account_id).await?;
                credential_store.delete_session(&account_id).await?;
                println!("Credentials deleted from keyring.");
            }

            // Delete account
            account_repo.delete_account(&account_id).await?;
            println!("Account '{}' removed.", username);
        }

        Commands::SetDefault { username } => {
            let config_dir = get_config_dir();
            let account_repo = FileAccountRepository::new(config_dir);

            let account_id = AccountId::new(username);
            account_repo.set_default_account(&account_id).await?;

            println!("Default account set to '{}'.", username);
        }

        Commands::ClearSession { username } => {
            let config_dir = get_config_dir();
            let account_repo = FileAccountRepository::new(config_dir);
            let credential_store = KeyringCredentialStore::new();

            if username == "all" {
                let accounts = account_repo.list_accounts().await?;
                for account in accounts {
                    credential_store.delete_session(&account.id).await?;
                }
                println!("All sessions cleared.");
            } else {
                let account_id = AccountId::new(username);
                credential_store.delete_session(&account_id).await?;
                println!("Session cleared for '{}'.", username);
            }
        }

        Commands::TestKeyring => {
            println!("Testing keyring integration...");

            let store = KeyringCredentialStore::new();
            let test_id = AccountId::new("__gaveloc_test__");

            // Test store
            print!("  Storing test credential... ");
            match store
                .store_password(&test_id, "test_password_12345")
                .await
            {
                Ok(()) => println!("OK"),
                Err(e) => {
                    println!("FAILED: {}", e);
                    return Ok(());
                }
            }

            // Test retrieve
            print!("  Retrieving test credential... ");
            match store.get_password(&test_id).await {
                Ok(Some(p)) if p == "test_password_12345" => println!("OK"),
                Ok(Some(_)) => println!("FAILED: wrong value"),
                Ok(None) => println!("FAILED: not found"),
                Err(e) => println!("FAILED: {}", e),
            }

            // Test delete
            print!("  Deleting test credential... ");
            match store.delete_password(&test_id).await {
                Ok(()) => println!("OK"),
                Err(e) => println!("FAILED: {}", e),
            }

            // Verify deleted
            print!("  Verifying deletion... ");
            match store.get_password(&test_id).await {
                Ok(None) => println!("OK"),
                Ok(Some(_)) => println!("FAILED: still exists"),
                Err(e) => println!("FAILED: {}", e),
            }

            println!("Keyring test complete.");
        }
    }

    Ok(())
}
