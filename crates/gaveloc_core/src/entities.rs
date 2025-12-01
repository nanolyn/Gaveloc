use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::Error;

// =============================================================================
// News Types
// =============================================================================

/// A single news item from the headline endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsItem {
    pub date: String,
    pub title: String,
    #[serde(default)]
    pub url: String,
    pub id: String,
    #[serde(default)]
    pub tag: String,
}

/// Collection of news headlines
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Headlines {
    pub news: Vec<NewsItem>,
    pub topics: Vec<NewsItem>,
    pub pinned: Vec<NewsItem>,
}

/// A promotional banner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Banner {
    #[serde(rename(deserialize = "lsb_banner", serialize = "image_url"))]
    pub image_url: String,
    #[serde(rename(deserialize = "link", serialize = "link_url"))]
    pub link_url: String,
}

/// Detailed content of a news article
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsArticle {
    pub title: String,
    pub content_html: String,
    pub date: String,
    pub url: String,
}

// =============================================================================
// Patching & Version Types
// =============================================================================

/// Game repository identifiers for version tracking and patching
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Repository {
    /// Boot files (launcher executables)
    Boot,
    /// Base game (A Realm Reborn)
    Ffxiv,
    /// Expansion 1: Heavensward
    Ex1,
    /// Expansion 2: Stormblood
    Ex2,
    /// Expansion 3: Shadowbringers
    Ex3,
    /// Expansion 4: Endwalker
    Ex4,
    /// Expansion 5: Dawntrail
    Ex5,
}

impl Repository {
    /// Get the repository ID used in patch URLs
    pub fn patch_id(&self) -> &'static str {
        match self {
            Repository::Boot => "ffxivneo_release_boot",
            Repository::Ffxiv => "ffxivneo_release_game",
            Repository::Ex1 => "ex1",
            Repository::Ex2 => "ex2",
            Repository::Ex3 => "ex3",
            Repository::Ex4 => "ex4",
            Repository::Ex5 => "ex5",
        }
    }

    /// Get the version file name for this repository
    pub fn version_file_name(&self) -> &'static str {
        match self {
            Repository::Boot => "ffxivboot.ver",
            Repository::Ffxiv => "ffxivgame.ver",
            Repository::Ex1 => "ex1.ver",
            Repository::Ex2 => "ex2.ver",
            Repository::Ex3 => "ex3.ver",
            Repository::Ex4 => "ex4.ver",
            Repository::Ex5 => "ex5.ver",
        }
    }

    /// Get the relative path to the version file from game root
    pub fn version_file_path(&self) -> &'static str {
        match self {
            Repository::Boot => "boot/ffxivboot.ver",
            Repository::Ffxiv => "game/ffxivgame.ver",
            Repository::Ex1 => "game/sqpack/ex1/ex1.ver",
            Repository::Ex2 => "game/sqpack/ex2/ex2.ver",
            Repository::Ex3 => "game/sqpack/ex3/ex3.ver",
            Repository::Ex4 => "game/sqpack/ex4/ex4.ver",
            Repository::Ex5 => "game/sqpack/ex5/ex5.ver",
        }
    }

    /// Get repository from expansion number (0 = base game, 1-5 = expansions)
    pub fn from_expansion(expansion: u32) -> Option<Self> {
        match expansion {
            0 => Some(Repository::Ffxiv),
            1 => Some(Repository::Ex1),
            2 => Some(Repository::Ex2),
            3 => Some(Repository::Ex3),
            4 => Some(Repository::Ex4),
            5 => Some(Repository::Ex5),
            _ => None,
        }
    }

    /// Get all game repositories up to and including the specified expansion
    pub fn game_repos_up_to(max_expansion: u32) -> Vec<Self> {
        let mut repos = vec![Repository::Ffxiv];
        for exp in 1..=max_expansion.min(5) {
            if let Some(repo) = Self::from_expansion(exp) {
                repos.push(repo);
            }
        }
        repos
    }
}

impl fmt::Display for Repository {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Repository::Boot => write!(f, "Boot"),
            Repository::Ffxiv => write!(f, "FFXIV"),
            Repository::Ex1 => write!(f, "Heavensward"),
            Repository::Ex2 => write!(f, "Stormblood"),
            Repository::Ex3 => write!(f, "Shadowbringers"),
            Repository::Ex4 => write!(f, "Endwalker"),
            Repository::Ex5 => write!(f, "Dawntrail"),
        }
    }
}

/// Parsed game version with comparison support
/// Version format: YYYY.MM.DD.RRRR.BBBB (e.g., "2024.07.23.0000.0001")
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameVersion {
    /// Original version string
    pub version_string: String,
    /// Year component
    pub year: u16,
    /// Month component (1-12)
    pub month: u8,
    /// Day component (1-31)
    pub day: u8,
    /// Revision number
    pub revision: u16,
    /// Build number
    pub build: u16,
}

impl GameVersion {
    /// Create a new GameVersion from components
    pub fn new(year: u16, month: u8, day: u8, revision: u16, build: u16) -> Self {
        let version_string = format!(
            "{:04}.{:02}.{:02}.{:04}.{:04}",
            year, month, day, revision, build
        );
        Self {
            version_string,
            year,
            month,
            day,
            revision,
            build,
        }
    }

    /// Parse from version string
    pub fn parse(version_string: &str) -> Result<Self, Error> {
        let parts: Vec<&str> = version_string.trim().split('.').collect();
        if parts.len() != 5 {
            return Err(Error::InvalidVersionFormat(format!(
                "Expected 5 parts separated by '.', got {}: {}",
                parts.len(),
                version_string
            )));
        }

        let year = parts[0].parse::<u16>().map_err(|_| {
            Error::InvalidVersionFormat(format!("Invalid year: {}", parts[0]))
        })?;
        let month = parts[1].parse::<u8>().map_err(|_| {
            Error::InvalidVersionFormat(format!("Invalid month: {}", parts[1]))
        })?;
        let day = parts[2].parse::<u8>().map_err(|_| {
            Error::InvalidVersionFormat(format!("Invalid day: {}", parts[2]))
        })?;
        let revision = parts[3].parse::<u16>().map_err(|_| {
            Error::InvalidVersionFormat(format!("Invalid revision: {}", parts[3]))
        })?;
        let build = parts[4].parse::<u16>().map_err(|_| {
            Error::InvalidVersionFormat(format!("Invalid build: {}", parts[4]))
        })?;

        Ok(Self {
            version_string: version_string.trim().to_string(),
            year,
            month,
            day,
            revision,
            build,
        })
    }

    /// Get the version string for use in patch URLs/requests
    pub fn as_str(&self) -> &str {
        &self.version_string
    }
}

impl FromStr for GameVersion {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for GameVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.version_string)
    }
}

impl PartialOrd for GameVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for GameVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.year, self.month, self.day, self.revision, self.build)
            .cmp(&(other.year, other.month, other.day, other.revision, other.build))
    }
}

/// A single patch entry from the patch server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchEntry {
    /// Version this patch updates to
    pub version_id: String,
    /// Download URL for the patch file
    pub url: String,
    /// Size of the patch file in bytes
    pub length: u64,
    /// Hash type (usually "sha1" for game patches, None for boot)
    pub hash_type: Option<String>,
    /// Block size for hash verification
    pub hash_block_size: Option<u64>,
    /// SHA1 hashes for each block of the patch file
    pub hashes: Option<Vec<String>>,
    /// Which repository this patch belongs to
    pub repository: Repository,
}

impl PatchEntry {
    /// Get the filename from the URL
    pub fn filename(&self) -> Option<&str> {
        self.url.rsplit('/').next()
    }
}

/// Patch download/install state for progress tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchState {
    /// Patch is queued but not started
    Pending,
    /// Currently downloading the patch file
    Downloading,
    /// Download complete, verifying hash
    Verifying,
    /// Applying the patch to game files
    Installing,
    /// Patch successfully applied
    Completed,
    /// Patch failed (see error for details)
    Failed,
}

impl fmt::Display for PatchState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PatchState::Pending => write!(f, "Pending"),
            PatchState::Downloading => write!(f, "Downloading"),
            PatchState::Verifying => write!(f, "Verifying"),
            PatchState::Installing => write!(f, "Installing"),
            PatchState::Completed => write!(f, "Completed"),
            PatchState::Failed => write!(f, "Failed"),
        }
    }
}

/// Progress information for a patch operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchProgress {
    /// The patch being processed
    pub patch: PatchEntry,
    /// Current state of the patch
    pub state: PatchState,
    /// Bytes downloaded so far
    pub bytes_downloaded: u64,
    /// Total bytes to download
    pub bytes_total: u64,
    /// Current download speed in bytes per second
    pub speed_bytes_per_sec: f64,
}

impl PatchProgress {
    /// Get download progress as a percentage (0.0 - 100.0)
    pub fn progress_percent(&self) -> f64 {
        if self.bytes_total == 0 {
            0.0
        } else {
            (self.bytes_downloaded as f64 / self.bytes_total as f64) * 100.0
        }
    }
}

// =============================================================================
// Integrity Checking Types
// =============================================================================

/// Result of an integrity check for a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileIntegrityResult {
    /// Relative path from game directory
    pub relative_path: String,
    /// Expected SHA1 hash from manifest
    pub expected_hash: String,
    /// Actual SHA1 hash of local file (None if file is missing)
    pub actual_hash: Option<String>,
    /// Status of the file
    pub status: IntegrityStatus,
}

/// Status of a file's integrity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntegrityStatus {
    /// File matches the expected hash
    Valid,
    /// File exists but hash doesn't match
    Mismatch,
    /// File is missing
    Missing,
    /// File exists but cannot be read (permission denied or other IO error)
    Unreadable,
}

impl fmt::Display for IntegrityStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IntegrityStatus::Valid => write!(f, "Valid"),
            IntegrityStatus::Mismatch => write!(f, "Mismatch"),
            IntegrityStatus::Missing => write!(f, "Missing"),
            IntegrityStatus::Unreadable => write!(f, "Unreadable"),
        }
    }
}

/// Remote integrity manifest from goatcorp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityManifest {
    /// Map of relative file paths to their expected SHA1 hashes
    #[serde(rename = "Hashes")]
    pub hashes: HashMap<String, String>,
    /// Game version this manifest is for
    #[serde(rename = "GameVersion")]
    pub game_version: String,
    /// Previous game version (for delta checking)
    #[serde(rename = "LastGameVersion")]
    pub last_game_version: Option<String>,
}

/// Progress information for integrity checking
#[derive(Debug, Clone)]
pub struct IntegrityProgress {
    /// Currently checking file
    pub current_file: String,
    /// Number of files checked so far
    pub files_checked: u32,
    /// Total number of files to check
    pub total_files: u32,
    /// Bytes processed so far
    pub bytes_processed: u64,
    /// Total bytes to process
    pub total_bytes: u64,
}

impl IntegrityProgress {
    /// Get progress as a percentage (0.0 - 100.0)
    pub fn progress_percent(&self) -> f64 {
        if self.total_files == 0 {
            0.0
        } else {
            (self.files_checked as f64 / self.total_files as f64) * 100.0
        }
    }
}

// =============================================================================
// Existing Types (Runner, Account, etc.)
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RunnerType {
    System,
    Proton,
    Lutris,
    GavelocManaged,
    #[default]
    Custom,
}

impl fmt::Display for RunnerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunnerType::System => write!(f, "System"),
            RunnerType::Proton => write!(f, "Proton"),
            RunnerType::Lutris => write!(f, "Lutris"),
            RunnerType::GavelocManaged => write!(f, "Gaveloc"),
            RunnerType::Custom => write!(f, "Custom"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WineRunner {
    pub path: PathBuf,
    pub name: String,
    pub runner_type: RunnerType,
    pub is_valid: bool,
}

/// Unique identifier for an account (derived from username, lowercase)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountId(String);

impl AccountId {
    pub fn new(username: &str) -> Self {
        Self(username.to_lowercase())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Account entity with authentication metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub username: String,
    pub is_steam: bool,
    pub is_free_trial: bool,
    pub use_otp: bool,
    /// Last successful login timestamp (Unix epoch seconds)
    pub last_login: Option<i64>,
}

impl Account {
    pub fn new(username: String) -> Self {
        Self {
            id: AccountId::new(&username),
            username,
            is_steam: false,
            is_free_trial: false,
            use_otp: false,
            last_login: None,
        }
    }
}

/// Credentials for authentication (never persisted to disk)
#[derive(Debug, Clone)]
pub struct Credentials {
    pub username: String,
    pub password: String,
    pub otp: Option<String>,
}

impl Credentials {
    pub fn new(username: String, password: String) -> Self {
        Self {
            username,
            password,
            otp: None,
        }
    }

    pub fn with_otp(mut self, otp: String) -> Self {
        self.otp = Some(otp);
        self
    }
}

/// Result of successful OAuth login
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OauthLoginResult {
    pub session_id: String,
    pub region: i32,
    pub terms_accepted: bool,
    pub playable: bool,
    pub max_expansion: u32,
}

/// Cached session data stored in keyring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedSession {
    pub unique_id: String,
    pub region: i32,
    pub max_expansion: u32,
    pub created_at: i64,
}

impl CachedSession {
    /// Session cache validity duration (1 day)
    const CACHE_DURATION_SECS: i64 = 24 * 60 * 60;

    pub fn is_valid(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        (now - self.created_at) < Self::CACHE_DURATION_SECS
    }

    /// Get remaining validity time in seconds
    pub fn remaining_secs(&self) -> i64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        (Self::CACHE_DURATION_SECS - (now - self.created_at)).max(0)
    }
}

/// Login state machine
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginState {
    /// Login successful
    Ok,
    /// Game needs patching
    NeedsPatchGame,
    /// Boot files need patching
    NeedsPatchBoot,
    /// No active subscription
    NoService,
    /// Terms not accepted
    NoTerms,
}

/// Complete login result
#[derive(Debug, Clone)]
pub struct LoginResult {
    pub state: LoginState,
    pub oauth: Option<OauthLoginResult>,
    pub unique_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_type_display() {
        assert_eq!(RunnerType::System.to_string(), "System");
        assert_eq!(RunnerType::Proton.to_string(), "Proton");
        assert_eq!(RunnerType::Lutris.to_string(), "Lutris");
        assert_eq!(RunnerType::GavelocManaged.to_string(), "Gaveloc");
        assert_eq!(RunnerType::Custom.to_string(), "Custom");
    }

    #[test]
    fn test_account_id_lowercase() {
        let id1 = AccountId::new("TestUser");
        let id2 = AccountId::new("testuser");
        let id3 = AccountId::new("TESTUSER");

        assert_eq!(id1, id2);
        assert_eq!(id2, id3);
        assert_eq!(id1.as_str(), "testuser");
    }

    #[test]
    fn test_account_new() {
        let account = Account::new("TestUser".to_string());

        assert_eq!(account.username, "TestUser");
        assert_eq!(account.id.as_str(), "testuser");
        assert!(!account.is_steam);
        assert!(!account.is_free_trial);
        assert!(!account.use_otp);
        assert!(account.last_login.is_none());
    }

    #[test]
    fn test_credentials_with_otp() {
        let creds = Credentials::new("user".to_string(), "pass".to_string());
        assert!(creds.otp.is_none());

        let creds_with_otp = creds.with_otp("123456".to_string());
        assert_eq!(creds_with_otp.otp, Some("123456".to_string()));
    }

    #[test]
    fn test_cached_session_validity() {
        let valid_session = CachedSession {
            unique_id: "test".to_string(),
            region: 3,
            max_expansion: 5,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        };
        assert!(valid_session.is_valid());
        assert!(valid_session.remaining_secs() > 0);

        let expired_session = CachedSession {
            unique_id: "test".to_string(),
            region: 3,
            max_expansion: 5,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
                - (25 * 60 * 60), // 25 hours ago
        };
        assert!(!expired_session.is_valid());
        assert_eq!(expired_session.remaining_secs(), 0);
    }

    // ==========================================================================
    // Patching & Version Tests
    // ==========================================================================

    #[test]
    fn test_repository_patch_id() {
        assert_eq!(Repository::Boot.patch_id(), "ffxivneo_release_boot");
        assert_eq!(Repository::Ffxiv.patch_id(), "ffxivneo_release_game");
        assert_eq!(Repository::Ex1.patch_id(), "ex1");
        assert_eq!(Repository::Ex5.patch_id(), "ex5");
    }

    #[test]
    fn test_repository_version_file_path() {
        assert_eq!(Repository::Boot.version_file_path(), "boot/ffxivboot.ver");
        assert_eq!(Repository::Ffxiv.version_file_path(), "game/ffxivgame.ver");
        assert_eq!(Repository::Ex1.version_file_path(), "game/sqpack/ex1/ex1.ver");
    }

    #[test]
    fn test_repository_from_expansion() {
        assert_eq!(Repository::from_expansion(0), Some(Repository::Ffxiv));
        assert_eq!(Repository::from_expansion(1), Some(Repository::Ex1));
        assert_eq!(Repository::from_expansion(5), Some(Repository::Ex5));
        assert_eq!(Repository::from_expansion(6), None);
    }

    #[test]
    fn test_repository_game_repos_up_to() {
        let repos = Repository::game_repos_up_to(2);
        assert_eq!(repos, vec![Repository::Ffxiv, Repository::Ex1, Repository::Ex2]);

        let repos = Repository::game_repos_up_to(0);
        assert_eq!(repos, vec![Repository::Ffxiv]);

        // Should cap at 5
        let repos = Repository::game_repos_up_to(10);
        assert_eq!(repos.len(), 6);
    }

    #[test]
    fn test_game_version_parse() {
        let version = GameVersion::parse("2024.07.23.0000.0001").unwrap();
        assert_eq!(version.year, 2024);
        assert_eq!(version.month, 7);
        assert_eq!(version.day, 23);
        assert_eq!(version.revision, 0);
        assert_eq!(version.build, 1);
        assert_eq!(version.as_str(), "2024.07.23.0000.0001");
    }

    #[test]
    fn test_game_version_parse_invalid() {
        assert!(GameVersion::parse("invalid").is_err());
        assert!(GameVersion::parse("2024.07.23.0000").is_err()); // Missing build
        assert!(GameVersion::parse("2024.07.23.0000.0001.extra").is_err()); // Too many parts
    }

    #[test]
    fn test_game_version_new() {
        let version = GameVersion::new(2024, 7, 23, 0, 1);
        assert_eq!(version.version_string, "2024.07.23.0000.0001");
    }

    #[test]
    fn test_game_version_ordering() {
        let v1 = GameVersion::parse("2024.01.01.0000.0001").unwrap();
        let v2 = GameVersion::parse("2024.07.23.0000.0001").unwrap();
        let v3 = GameVersion::parse("2024.07.23.0001.0000").unwrap();

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v1 < v3);
    }

    #[test]
    fn test_game_version_from_str() {
        let version: GameVersion = "2024.07.23.0000.0001".parse().unwrap();
        assert_eq!(version.year, 2024);
    }

    #[test]
    fn test_patch_entry_filename() {
        let patch = PatchEntry {
            version_id: "2024.07.23.0000.0001".to_string(),
            url: "http://example.com/patches/D2024.07.23.0000.0001.patch".to_string(),
            length: 1024,
            hash_type: Some("sha1".to_string()),
            hash_block_size: Some(1048576),
            hashes: Some(vec!["abc123".to_string()]),
            repository: Repository::Ffxiv,
        };
        assert_eq!(patch.filename(), Some("D2024.07.23.0000.0001.patch"));
    }

    #[test]
    fn test_patch_progress_percent() {
        let patch = PatchEntry {
            version_id: "test".to_string(),
            url: "http://example.com/test.patch".to_string(),
            length: 1000,
            hash_type: None,
            hash_block_size: None,
            hashes: None,
            repository: Repository::Boot,
        };

        let progress = PatchProgress {
            patch,
            state: PatchState::Downloading,
            bytes_downloaded: 500,
            bytes_total: 1000,
            speed_bytes_per_sec: 100.0,
        };

        assert!((progress.progress_percent() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_integrity_progress_percent() {
        let progress = IntegrityProgress {
            current_file: "test.dat".to_string(),
            files_checked: 50,
            total_files: 100,
            bytes_processed: 0,
            total_bytes: 0,
        };

        assert!((progress.progress_percent() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_integrity_status_display() {
        assert_eq!(IntegrityStatus::Valid.to_string(), "Valid");
        assert_eq!(IntegrityStatus::Mismatch.to_string(), "Mismatch");
        assert_eq!(IntegrityStatus::Missing.to_string(), "Missing");
    }

    #[test]
    fn test_patch_state_display() {
        assert_eq!(PatchState::Pending.to_string(), "Pending");
        assert_eq!(PatchState::Downloading.to_string(), "Downloading");
        assert_eq!(PatchState::Installing.to_string(), "Installing");
    }
}