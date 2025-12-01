use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand};
use dialoguer::{Confirm, Input};
use gaveloc_adapters::configuration;
use gaveloc_adapters::patch::{FileVersionRepository, HttpPatchDownloader, SquareEnixPatchServer};
use gaveloc_adapters::runner::{LinuxRunnerDetector, LinuxRunnerManager};
use gaveloc_adapters::telemetry;
use gaveloc_adapters::{
    FileAccountRepository, GoatcorpIntegrityChecker, HttpOtpListener, KeyringCredentialStore,
    SquareEnixAuthenticator, ZiPatchParser,
};
use gaveloc_core::config::Region;
use gaveloc_core::entities::{
    Account, AccountId, CachedSession, Credentials, IntegrityStatus, Repository,
};
use gaveloc_core::ports::{
    AccountRepository, Authenticator, CredentialStore, IntegrityChecker, OtpListener,
    PatchDownloader, PatchServer, RunnerDetector, RunnerManager, VersionRepository, ZiPatchApplier,
};
use indicatif::{ProgressBar, ProgressStyle};
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

    // --- Patching commands ---
    /// Check current game version
    Version {
        /// Path to game installation
        #[arg(short, long)]
        game_path: PathBuf,
    },

    /// Check for available updates
    CheckUpdates {
        /// Path to game installation
        #[arg(short, long)]
        game_path: PathBuf,

        /// Maximum expansion to check (0-5)
        #[arg(short, long, default_value = "5")]
        max_expansion: u32,
    },

    /// Verify game file integrity
    Verify {
        /// Path to game installation
        #[arg(short, long)]
        game_path: PathBuf,

        /// Only show files with problems
        #[arg(long, default_value = "false")]
        problems_only: bool,

        /// Export full report to JSON file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Repair corrupted game files
    Repair {
        /// Path to game installation
        #[arg(short, long)]
        game_path: PathBuf,

        /// Skip confirmation prompt
        #[arg(short, long, default_value = "false")]
        yes: bool,
    },

    /// Update boot files (no login required)
    Update {
        /// Path to game installation
        #[arg(short, long)]
        game_path: PathBuf,

        /// Keep downloaded patch files after applying
        #[arg(long, default_value = "false")]
        keep_patches: bool,

        /// Skip confirmation prompt
        #[arg(short, long, default_value = "false")]
        yes: bool,
    },

    /// Update game files (requires login)
    UpdateGame {
        /// Path to game installation
        #[arg(short, long)]
        game_path: PathBuf,

        /// Username (SE account ID)
        #[arg(short, long)]
        username: Option<String>,

        /// Maximum expansion to update (0-5)
        #[arg(short, long, default_value = "5")]
        max_expansion: u32,

        /// Keep downloaded patch files after applying
        #[arg(long, default_value = "false")]
        keep_patches: bool,

        /// Skip confirmation prompt
        #[arg(short, long, default_value = "false")]
        yes: bool,
    },
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
                            result.ok()
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
                .login(&credentials, Region::default(), account.is_free_trial)
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
                        "  {} {} {}{}{}",
                        if is_default { "*" } else { " " },
                        account.username,
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
            otp,
            free_trial,
        } => {
            let config_dir = get_config_dir();
            let account_repo = FileAccountRepository::new(config_dir);

            let mut account = Account::new(username.clone());
            account.use_otp = *otp;
            account.is_free_trial = *free_trial;

            account_repo.save_account(&account).await?;

            println!("Account '{}' added successfully.", username);
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

        // --- Patching commands ---
        Commands::Version { game_path } => {
            if !game_path.exists() {
                println!("Game path does not exist: {}", game_path.display());
                return Ok(());
            }

            let version_repo = FileVersionRepository;

            println!("Game versions at {}:", game_path.display());
            println!();

            // Check boot version
            match version_repo.get_version(game_path, Repository::Boot).await {
                Ok(v) => println!("  Boot:         {}", v),
                Err(_) => println!("  Boot:         (not found)"),
            }

            // Check base game version
            match version_repo.get_version(game_path, Repository::Ffxiv).await {
                Ok(v) => println!("  FFXIV:        {}", v),
                Err(_) => println!("  FFXIV:        (not found)"),
            }

            // Check expansions
            for exp in 1..=5 {
                if let Some(repo) = Repository::from_expansion(exp) {
                    match version_repo.get_version(game_path, repo).await {
                        Ok(v) => println!("  {}:   {}", repo, v),
                        Err(_) => {} // Silently skip missing expansions
                    }
                }
            }
        }

        Commands::CheckUpdates {
            game_path,
            max_expansion,
        } => {
            if !game_path.exists() {
                println!("Game path does not exist: {}", game_path.display());
                return Ok(());
            }

            let version_repo = FileVersionRepository;
            let patch_server = SquareEnixPatchServer::new()?;

            println!("Checking for updates...");

            // Check boot updates
            print!("  Boot: ");
            match version_repo.get_version(game_path, Repository::Boot).await {
                Ok(boot_version) => {
                    match patch_server.check_boot_version(game_path, &boot_version).await {
                        Ok(patches) => {
                            if patches.is_empty() {
                                println!("up to date ({})", boot_version);
                            } else {
                                println!(
                                    "{} update(s) available ({} -> {})",
                                    patches.len(),
                                    boot_version,
                                    patches.last().map(|p| &p.version_id).unwrap_or(&"?".to_string())
                                );
                            }
                        }
                        Err(e) => println!("error checking: {}", e),
                    }
                }
                Err(_) => println!("(not installed)"),
            }

            // Note: Game updates require authentication, so we just show version info
            print!("  Game: ");
            match version_repo.get_version(game_path, Repository::Ffxiv).await {
                Ok(version) => {
                    println!(
                        "{} (login required to check for updates)",
                        version
                    );
                }
                Err(_) => println!("(not installed)"),
            }

            for exp in 1..=(*max_expansion).min(5) {
                if let Some(repo) = Repository::from_expansion(exp) {
                    print!("  {}: ", repo);
                    match version_repo.get_version(game_path, repo).await {
                        Ok(version) => println!("{}", version),
                        Err(_) => println!("(not installed)"),
                    }
                }
            }
        }

        Commands::Verify {
            game_path,
            problems_only,
            output,
        } => {
            if !game_path.exists() {
                println!("Game path does not exist: {}", game_path.display());
                return Ok(());
            }

            let version_repo = FileVersionRepository;
            let integrity_checker = GoatcorpIntegrityChecker::with_default_client();

            // Get current game version
            let game_version = match version_repo.get_version(game_path, Repository::Ffxiv).await {
                Ok(v) => v,
                Err(e) => {
                    println!("Failed to read game version: {}", e);
                    return Ok(());
                }
            };

            println!(
                "Verifying integrity for version {}...",
                game_version.as_str()
            );

            // Fetch manifest
            let manifest = match integrity_checker.fetch_manifest(game_version.as_str()).await {
                Ok(m) => m,
                Err(e) => {
                    println!("Failed to fetch integrity manifest: {}", e);
                    println!("Note: Manifests may not be available for all game versions.");
                    return Ok(());
                }
            };

            println!("Manifest loaded: {} files to check", manifest.hashes.len());

            // Set up progress bar
            let pb = ProgressBar::new(manifest.hashes.len() as u64);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)")
                    .unwrap()
                    .progress_chars("#>-"),
            );

            let pb_clone = pb.clone();
            let progress = move |_progress: gaveloc_core::entities::IntegrityProgress| {
                pb_clone.inc(1);
            };

            // Run integrity check
            let results = match integrity_checker
                .check_integrity(game_path, &manifest, progress)
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    pb.finish_and_clear();
                    println!("Integrity check failed: {}", e);
                    return Ok(());
                }
            };

            pb.finish_and_clear();

            // Summarize results
            let valid_count = results.iter().filter(|r| r.status == IntegrityStatus::Valid).count();
            let mismatch_count = results.iter().filter(|r| r.status == IntegrityStatus::Mismatch).count();
            let missing_count = results.iter().filter(|r| r.status == IntegrityStatus::Missing).count();
            let unreadable_count = results.iter().filter(|r| r.status == IntegrityStatus::Unreadable).count();

            println!();
            println!("Integrity check complete:");
            println!("  Valid:      {} files", valid_count);
            if mismatch_count > 0 || missing_count > 0 || unreadable_count > 0 {
                println!("  Mismatch:   {} files", mismatch_count);
                println!("  Missing:    {} files", missing_count);
                if unreadable_count > 0 {
                    println!("  Unreadable: {} files (check permissions)", unreadable_count);
                }
            }

            // Show problematic files
            let problems: Vec<_> = results
                .iter()
                .filter(|r| r.status != IntegrityStatus::Valid)
                .collect();

            if *problems_only || !problems.is_empty() {
                if !problems.is_empty() {
                    println!();
                    println!("Files with problems:");
                    for result in problems.iter().take(50) {
                        println!("  [{}] {}", result.status, result.relative_path);
                    }
                    if problems.len() > 50 {
                        println!("  ... and {} more", problems.len() - 50);
                    }

                    println!();
                    println!("Run 'gaveloc_cli repair --game-path {}' to fix these files.", game_path.display());
                }
            }

            // Export report if requested
            if let Some(output_path) = output {
                let report = serde_json::json!({
                    "game_version": game_version.as_str(),
                    "total_files": results.len(),
                    "valid": valid_count,
                    "mismatch": mismatch_count,
                    "missing": missing_count,
                    "unreadable": unreadable_count,
                    "problems": problems.iter().map(|r| {
                        serde_json::json!({
                            "path": r.relative_path,
                            "status": format!("{}", r.status),
                            "expected": r.expected_hash,
                            "actual": r.actual_hash,
                        })
                    }).collect::<Vec<_>>(),
                });
                tokio::fs::write(&output_path, serde_json::to_string_pretty(&report)?).await?;
                println!();
                println!("Report saved to: {}", output_path.display());
            }
        }

        Commands::Repair { game_path, yes } => {
            if !game_path.exists() {
                println!("Game path does not exist: {}", game_path.display());
                return Ok(());
            }

            // Pre-flight check: verify game directory is writable
            let test_file = game_path.join(".gaveloc_write_test");
            if tokio::fs::write(&test_file, "test").await.is_err() {
                println!("Error: Game directory is not writable.");
                println!("Check permissions on: {}", game_path.display());
                return Ok(());
            }
            tokio::fs::remove_file(&test_file).await.ok();

            let version_repo = FileVersionRepository;
            let integrity_checker = GoatcorpIntegrityChecker::with_default_client();

            // Get current game version
            let game_version = match version_repo.get_version(game_path, Repository::Ffxiv).await {
                Ok(v) => v,
                Err(e) => {
                    println!("Failed to read game version: {}", e);
                    return Ok(());
                }
            };

            println!(
                "Checking integrity for version {}...",
                game_version.as_str()
            );

            // Fetch manifest
            let manifest = match integrity_checker.fetch_manifest(game_version.as_str()).await {
                Ok(m) => m,
                Err(e) => {
                    println!("Failed to fetch integrity manifest: {}", e);
                    return Ok(());
                }
            };

            // Run integrity check (silently)
            let results = match integrity_checker
                .check_integrity(game_path, &manifest, |_| {})
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    println!("Integrity check failed: {}", e);
                    return Ok(());
                }
            };

            // Find problematic files (exclude Unreadable - user needs to fix permissions)
            let unreadable_count = results
                .iter()
                .filter(|r| r.status == IntegrityStatus::Unreadable)
                .count();

            let unreadable_files: Vec<_> = results
                .iter()
                .filter(|r| r.status == IntegrityStatus::Unreadable)
                .take(5)
                .map(|r| r.relative_path.clone())
                .collect();

            let problems: Vec<_> = results
                .into_iter()
                .filter(|r| r.status == IntegrityStatus::Mismatch || r.status == IntegrityStatus::Missing)
                .collect();

            if unreadable_count > 0 {
                println!();
                println!("Warning: {} files could not be read (permission denied):", unreadable_count);
                for path in &unreadable_files {
                    println!("  {}", path);
                }
                if unreadable_count > 5 {
                    println!("  ... and {} more", unreadable_count - 5);
                }
                println!("These files will be skipped. Fix permissions manually if needed.");
            }

            if problems.is_empty() {
                println!("All readable files are valid. Nothing to repair.");
                return Ok(());
            }

            println!();
            println!("Found {} files to repair:", problems.len());
            for result in problems.iter().take(10) {
                println!("  [{}] {}", result.status, result.relative_path);
            }
            if problems.len() > 10 {
                println!("  ... and {} more", problems.len() - 10);
            }

            // Confirm repair
            let confirmed = if *yes {
                true
            } else {
                println!();
                println!("Warning: Ensure the game launcher is not running.");
                println!("Repair will delete corrupted files.");
                println!("You will need to run the launcher to re-download them.");
                Confirm::new()
                    .with_prompt("Proceed with repair?")
                    .default(false)
                    .interact()?
            };

            if !confirmed {
                println!("Cancelled.");
                return Ok(());
            }

            // Repair files in parallel
            println!();
            println!("Repairing files...");
            let (repaired, errors) = match integrity_checker
                .repair_files(game_path, &problems)
                .await
            {
                Ok(counts) => counts,
                Err(e) => {
                    println!("Repair failed: {}", e);
                    return Ok(());
                }
            };

            println!();
            println!("Repair complete: {} files removed, {} errors", repaired, errors);
            if repaired > 0 {
                println!("Run the launcher to re-download the removed files.");
            }
        }

        Commands::Update {
            game_path,
            keep_patches,
            yes,
        } => {
            if !game_path.exists() {
                println!("Game path does not exist: {}", game_path.display());
                return Ok(());
            }

            let version_repo = FileVersionRepository;
            let patch_server = SquareEnixPatchServer::new()?;
            let patch_downloader = HttpPatchDownloader::new()?;
            let patch_applier = ZiPatchParser::new();

            // Get current boot version
            let boot_version = match version_repo.get_version(game_path, Repository::Boot).await {
                Ok(v) => v,
                Err(e) => {
                    println!("Failed to read boot version: {}", e);
                    println!("Is this a valid FFXIV installation?");
                    return Ok(());
                }
            };

            println!("Current boot version: {}", boot_version);

            // Check for updates
            println!("Checking for boot updates...");
            let patches = match patch_server.check_boot_version(game_path, &boot_version).await {
                Ok(p) => p,
                Err(e) => {
                    println!("Failed to check for updates: {}", e);
                    return Ok(());
                }
            };

            if patches.is_empty() {
                println!("Boot files are up to date.");
                return Ok(());
            }

            // Calculate total download size
            let total_size: u64 = patches.iter().map(|p| p.length).sum();
            let total_size_mb = total_size as f64 / 1024.0 / 1024.0;

            println!();
            println!("Found {} boot update(s):", patches.len());
            for patch in &patches {
                println!(
                    "  {} ({:.2} MB)",
                    patch.version_id,
                    patch.length as f64 / 1024.0 / 1024.0
                );
            }
            println!("Total download: {:.2} MB", total_size_mb);

            // Confirm update
            let confirmed = if *yes {
                true
            } else {
                Confirm::new()
                    .with_prompt("Download and apply updates?")
                    .default(true)
                    .interact()?
            };

            if !confirmed {
                println!("Cancelled.");
                return Ok(());
            }

            // Create temp directory for patches
            let temp_dir = tempfile::tempdir()?;
            let patch_dir = temp_dir.path();

            println!();

            // Download and apply each patch
            for (idx, patch) in patches.iter().enumerate() {
                let patch_filename = patch.filename().unwrap_or(&patch.version_id);
                let patch_path = patch_dir.join(patch_filename);

                // Download
                println!(
                    "[{}/{}] Downloading {}...",
                    idx + 1,
                    patches.len(),
                    patch.version_id
                );

                let pb = ProgressBar::new(patch.length);
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")
                        .unwrap()
                        .progress_chars("#>-"),
                );

                let pb_clone = pb.clone();
                let progress = move |downloaded: u64, _total: u64| {
                    pb_clone.set_position(downloaded);
                };

                if let Err(e) = patch_downloader
                    .download_patch(patch, &patch_path, None, progress)
                    .await
                {
                    pb.finish_and_clear();
                    println!("  Download failed: {}", e);
                    return Ok(());
                }
                pb.finish_and_clear();

                // Verify
                print!("  Verifying... ");
                if !patch_downloader.verify_patch(patch, &patch_path).await? {
                    println!("FAILED");
                    println!("  Patch verification failed. Please try again.");
                    return Ok(());
                }
                println!("OK");

                // Apply
                print!("  Applying... ");
                match patch_applier.apply_patch(&patch_path, game_path) {
                    Ok(()) => println!("OK"),
                    Err(e) => {
                        println!("FAILED");
                        println!("  Failed to apply patch: {}", e);
                        return Ok(());
                    }
                }

                // Update version file
                version_repo
                    .set_version(game_path, Repository::Boot, &patch.version_id)
                    .await?;

                // Clean up patch file unless keeping
                if !keep_patches {
                    tokio::fs::remove_file(&patch_path).await.ok();
                }
            }

            println!();
            println!("Boot update complete!");

            // Show new version
            if let Ok(new_version) = version_repo.get_version(game_path, Repository::Boot).await {
                println!("New boot version: {}", new_version);
            }
        }

        Commands::UpdateGame {
            game_path,
            username,
            max_expansion,
            keep_patches,
            yes,
        } => {
            if !game_path.exists() {
                println!("Game path does not exist: {}", game_path.display());
                return Ok(());
            }

            let config_dir = get_config_dir();
            let account_repo = FileAccountRepository::new(config_dir);
            let credential_store = KeyringCredentialStore::new();
            let authenticator = SquareEnixAuthenticator::new()?;
            let version_repo = FileVersionRepository;
            let patch_server = SquareEnixPatchServer::new()?;
            let patch_downloader = HttpPatchDownloader::new()?;
            let patch_applier = ZiPatchParser::new();

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

            println!("Using account: {}", account.username);

            // Check for cached session or perform login
            let session_id = if let Ok(Some(session)) = credential_store.get_session(&account.id).await
            {
                if session.is_valid() {
                    println!("Using cached session");
                    session.unique_id
                } else {
                    println!("Cached session expired, need to login");
                    perform_login(&account, &credential_store, &authenticator).await?
                }
            } else {
                println!("No cached session, need to login");
                perform_login(&account, &credential_store, &authenticator).await?
            };

            // Show current versions
            println!();
            println!("Current game version:");
            match version_repo.get_version(game_path, Repository::Ffxiv).await {
                Ok(v) => println!("  FFXIV: {}", v),
                Err(_) => println!("  FFXIV: (not found)"),
            }

            // Register session and get patches
            println!();
            println!("Checking for game updates...");
            let (unique_id, patches) = match patch_server
                .register_session(&session_id, game_path, *max_expansion)
                .await
            {
                Ok(result) => result,
                Err(e) => {
                    println!("Failed to check for updates: {}", e);
                    return Ok(());
                }
            };

            if patches.is_empty() {
                println!("Game is up to date.");
                return Ok(());
            }

            // Group patches by repository
            let mut patches_by_repo: std::collections::HashMap<Repository, Vec<_>> =
                std::collections::HashMap::new();
            for patch in &patches {
                patches_by_repo
                    .entry(patch.repository)
                    .or_default()
                    .push(patch);
            }

            // Calculate total download size
            let total_size: u64 = patches.iter().map(|p| p.length).sum();
            let total_size_mb = total_size as f64 / 1024.0 / 1024.0;

            println!();
            println!("Found {} game update(s):", patches.len());
            for (repo, repo_patches) in &patches_by_repo {
                let repo_size: u64 = repo_patches.iter().map(|p| p.length).sum();
                println!(
                    "  {}: {} patch(es) ({:.2} MB)",
                    repo,
                    repo_patches.len(),
                    repo_size as f64 / 1024.0 / 1024.0
                );
            }
            println!("Total download: {:.2} MB", total_size_mb);

            // Confirm update
            let confirmed = if *yes {
                true
            } else {
                Confirm::new()
                    .with_prompt("Download and apply updates?")
                    .default(true)
                    .interact()?
            };

            if !confirmed {
                println!("Cancelled.");
                return Ok(());
            }

            // Create temp directory for patches
            let temp_dir = tempfile::tempdir()?;
            let patch_dir = temp_dir.path();

            println!();

            // Download and apply each patch
            for (idx, patch) in patches.iter().enumerate() {
                let patch_filename = patch.filename().unwrap_or(&patch.version_id);
                let patch_path = patch_dir.join(patch_filename);

                // Download
                println!(
                    "[{}/{}] Downloading {} ({})...",
                    idx + 1,
                    patches.len(),
                    patch.version_id,
                    patch.repository
                );

                let pb = ProgressBar::new(patch.length);
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")
                        .unwrap()
                        .progress_chars("#>-"),
                );

                let pb_clone = pb.clone();
                let progress = move |downloaded: u64, _total: u64| {
                    pb_clone.set_position(downloaded);
                };

                if let Err(e) = patch_downloader
                    .download_patch(patch, &patch_path, Some(&unique_id), progress)
                    .await
                {
                    pb.finish_and_clear();
                    println!("  Download failed: {}", e);
                    return Ok(());
                }
                pb.finish_and_clear();

                // Verify
                print!("  Verifying... ");
                if !patch_downloader.verify_patch(patch, &patch_path).await? {
                    println!("FAILED");
                    println!("  Patch verification failed. Please try again.");
                    return Ok(());
                }
                println!("OK");

                // Apply
                print!("  Applying... ");
                match patch_applier.apply_patch(&patch_path, game_path) {
                    Ok(()) => println!("OK"),
                    Err(e) => {
                        println!("FAILED");
                        println!("  Failed to apply patch: {}", e);
                        return Ok(());
                    }
                }

                // Update version file
                version_repo
                    .set_version(game_path, patch.repository, &patch.version_id)
                    .await?;

                // Clean up patch file unless keeping
                if !keep_patches {
                    tokio::fs::remove_file(&patch_path).await.ok();
                }
            }

            println!();
            println!("Game update complete!");

            // Show new versions
            println!("New versions:");
            for repo in Repository::game_repos_up_to(*max_expansion) {
                if let Ok(v) = version_repo.get_version(game_path, repo).await {
                    println!("  {}: {}", repo, v);
                }
            }
        }
    }

    Ok(())
}

/// Helper function to perform login and return session ID
async fn perform_login(
    account: &Account,
    credential_store: &KeyringCredentialStore,
    authenticator: &SquareEnixAuthenticator,
) -> anyhow::Result<String> {
    // Get password
    let password = if let Ok(Some(p)) = credential_store.get_password(&account.id).await {
        println!("Using saved password");
        p
    } else {
        rpassword::prompt_password("Password: ")?
    };

    // Handle OTP
    let otp = if account.use_otp {
        let otp: String = Input::new()
            .with_prompt("One-Time Password")
            .interact_text()?;
        Some(otp)
    } else {
        None
    };

    // Build credentials
    let mut credentials = Credentials::new(account.username.clone(), password);
    if let Some(otp) = otp {
        credentials = credentials.with_otp(otp);
    }

    // Perform login
    println!("Authenticating...");
    let result = authenticator
        .login(&credentials, Region::default(), account.is_free_trial)
        .await?;

    println!("Login successful!");

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

    Ok(result.session_id)
}
