//! GoatcorpIntegrityChecker implementation
//!
//! Verifies game file integrity against the goatcorp community manifest.
//! Uses SHA1 hashes with parallel file checking via rayon.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rayon::prelude::*;
use sha1::{Digest, Sha1};

use gaveloc_core::entities::{
    FileIntegrityResult, IntegrityManifest, IntegrityProgress, IntegrityStatus,
};
use gaveloc_core::error::Error;
use gaveloc_core::ports::IntegrityChecker;

const MANIFEST_BASE_URL: &str = "https://goatcorp.github.io/integrity";
const MANIFEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RETRIES: u32 = 3;
const CACHE_TTL_SECS: u64 = 86400; // 24 hours

/// Integrity checker using goatcorp community manifest
pub struct GoatcorpIntegrityChecker {
    client: reqwest::Client,
}

impl GoatcorpIntegrityChecker {
    /// Create a new integrity checker with the given HTTP client
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Create a new integrity checker with default HTTP client
    pub fn with_default_client() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Get the cache file path for a manifest version
    fn cache_path(game_version: &str) -> PathBuf {
        directories::ProjectDirs::from("com", "gaveloc", "gaveloc")
            .map(|d| {
                d.cache_dir()
                    .join("manifests")
                    .join(format!("{}.json", game_version))
            })
            .unwrap_or_else(|| PathBuf::from(format!("/tmp/gaveloc_manifest_{}.json", game_version)))
    }

    /// Fetch manifest from network with retry logic
    async fn fetch_manifest_from_network(
        &self,
        game_version: &str,
    ) -> Result<IntegrityManifest, Error> {
        let url = format!("{}/{}.json", MANIFEST_BASE_URL, game_version);

        for attempt in 0..MAX_RETRIES {
            let result = tokio::time::timeout(MANIFEST_TIMEOUT, self.client.get(&url).send()).await;

            match result {
                Ok(Ok(response)) => {
                    if response.status() == reqwest::StatusCode::NOT_FOUND {
                        return Err(Error::IntegrityManifestNotFound(game_version.to_string()));
                    }
                    if response.status().is_server_error() && attempt < MAX_RETRIES - 1 {
                        tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
                        continue;
                    }
                    if !response.status().is_success() {
                        return Err(Error::Network(format!(
                            "manifest request failed: {}",
                            response.status()
                        )));
                    }
                    return response.json().await.map_err(|e| {
                        Error::Network(format!("failed to parse integrity manifest: {}", e))
                    });
                }
                Ok(Err(e)) => {
                    if attempt < MAX_RETRIES - 1 {
                        tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
                        continue;
                    }
                    return Err(Error::Network(format!(
                        "failed to fetch integrity manifest: {}",
                        e
                    )));
                }
                Err(_) => {
                    if attempt < MAX_RETRIES - 1 {
                        continue;
                    }
                    return Err(Error::Network("manifest fetch timeout".into()));
                }
            }
        }
        unreachable!()
    }
}

#[async_trait]
impl IntegrityChecker for GoatcorpIntegrityChecker {
    async fn fetch_manifest(&self, game_version: &str) -> Result<IntegrityManifest, Error> {
        let cache_path = Self::cache_path(game_version);

        // Check cache (24-hour TTL)
        if let Ok(metadata) = tokio::fs::metadata(&cache_path).await {
            if let Ok(modified) = metadata.modified() {
                if modified.elapsed().unwrap_or(Duration::MAX)
                    < Duration::from_secs(CACHE_TTL_SECS)
                {
                    if let Ok(data) = tokio::fs::read_to_string(&cache_path).await {
                        if let Ok(manifest) = serde_json::from_str(&data) {
                            return Ok(manifest);
                        }
                    }
                }
            }
        }

        // Fetch from network
        let manifest = self.fetch_manifest_from_network(game_version).await?;

        // Save to cache
        if let Some(parent) = cache_path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        if let Ok(json) = serde_json::to_string(&manifest) {
            tokio::fs::write(&cache_path, json).await.ok();
        }

        Ok(manifest)
    }

    async fn check_integrity<F>(
        &self,
        game_path: &Path,
        manifest: &IntegrityManifest,
        progress: F,
    ) -> Result<Vec<FileIntegrityResult>, Error>
    where
        F: Fn(IntegrityProgress) + Send + Sync + 'static,
    {
        let game_path = game_path.to_path_buf();
        let hashes = manifest.hashes.clone();
        let progress = Arc::new(progress);
        let cancelled = Arc::new(AtomicBool::new(false));

        // Run CPU-bound hashing in blocking task with rayon
        tokio::task::spawn_blocking(move || {
            check_files_parallel(&game_path, &hashes, progress, &cancelled)
        })
        .await
        .map_err(|e| Error::Other(format!("integrity check task panicked: {}", e)))?
    }

    async fn repair_file(
        &self,
        game_path: &Path,
        relative_path: &str,
        _expected_hash: &str,
    ) -> Result<(), Error> {
        // Simple strategy: delete the corrupted file
        // The patching system will restore it on next update check
        let file_path = normalize_path(game_path, relative_path)?;

        if file_path.exists() {
            tokio::fs::remove_file(&file_path)
                .await
                .map_err(Error::Io)?;
        }

        Ok(())
    }

    async fn repair_files(
        &self,
        game_path: &Path,
        files: &[FileIntegrityResult],
    ) -> Result<(u32, u32), Error> {
        let game_path = game_path.to_path_buf();
        let paths: Vec<PathBuf> = files
            .iter()
            .filter_map(|f| normalize_path(&game_path, &f.relative_path).ok())
            .collect();

        // Run deletions in parallel using rayon
        let results: Vec<bool> = tokio::task::spawn_blocking(move || {
            paths
                .par_iter()
                .map(|path| {
                    if path.exists() {
                        std::fs::remove_file(path).is_ok()
                    } else {
                        true // Already deleted counts as success
                    }
                })
                .collect()
        })
        .await
        .map_err(|e| Error::Other(format!("repair task panicked: {}", e)))?;

        let success = results.iter().filter(|&&r| r).count() as u32;
        let failed = results.len() as u32 - success;
        Ok((success, failed))
    }
}

/// Convert manifest hash format to comparable format
/// Input: "A0 A1 A2 A3..." (space-separated uppercase)
/// Output: "a0a1a2a3..." (lowercase hex, no spaces)
fn normalize_manifest_hash(hash: &str) -> String {
    hash.split_whitespace().collect::<String>().to_lowercase()
}

/// Compute SHA1 hash of a file using streaming to avoid OOM on large files
fn compute_file_hash(path: &Path) -> Result<String, std::io::Error> {
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::with_capacity(65536, file);
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; 65536];

    loop {
        match reader.read(&mut buffer)? {
            0 => break,
            n => hasher.update(&buffer[..n]),
        }
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Convert manifest path to local filesystem path with path traversal protection
/// Input: "\game\sqpack\ffxiv\000000.win32.dat"
/// Output: Ok("{game_path}/game/sqpack/ffxiv/000000.win32.dat")
/// Returns error if path contains traversal attempts (..)
fn normalize_path(game_path: &Path, manifest_path: &str) -> Result<PathBuf, Error> {
    // Remove leading backslash, convert \ to /
    let relative = manifest_path
        .trim_start_matches('\\')
        .replace('\\', "/");

    // Reject paths with parent traversal
    if relative.contains("..") {
        return Err(Error::Other(format!(
            "invalid manifest path (traversal attempt): {}",
            manifest_path
        )));
    }

    Ok(game_path.join(relative))
}

/// Check files in parallel using rayon
fn check_files_parallel(
    game_path: &Path,
    hashes: &HashMap<String, String>,
    progress: Arc<dyn Fn(IntegrityProgress) + Send + Sync>,
    cancelled: &AtomicBool,
) -> Result<Vec<FileIntegrityResult>, Error> {
    let total_files = hashes.len() as u32;
    let files_checked = AtomicU32::new(0);
    let bytes_processed = AtomicU64::new(0);

    // Pre-calculate total bytes for progress reporting
    let total_bytes: u64 = hashes
        .keys()
        .filter_map(|p| normalize_path(game_path, p).ok())
        .filter_map(|p| std::fs::metadata(&p).ok())
        .map(|m| m.len())
        .sum();

    let results: Vec<FileIntegrityResult> = hashes
        .par_iter()
        .map(|(manifest_path, expected_hash)| {
            // Check cancellation at start of each file
            if cancelled.load(Ordering::Relaxed) {
                return None;
            }

            let file_path = match normalize_path(game_path, manifest_path) {
                Ok(p) => p,
                Err(_) => {
                    // Invalid path (traversal attempt) - skip silently
                    return None;
                }
            };
            let expected_normalized = normalize_manifest_hash(expected_hash);
            let file_size = std::fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);

            let result = if !file_path.exists() {
                FileIntegrityResult {
                    relative_path: manifest_path.clone(),
                    expected_hash: expected_normalized,
                    actual_hash: None,
                    status: IntegrityStatus::Missing,
                }
            } else {
                match compute_file_hash(&file_path) {
                    Ok(actual) => {
                        let status = if actual == expected_normalized {
                            IntegrityStatus::Valid
                        } else {
                            IntegrityStatus::Mismatch
                        };
                        FileIntegrityResult {
                            relative_path: manifest_path.clone(),
                            expected_hash: expected_normalized,
                            actual_hash: Some(actual),
                            status,
                        }
                    }
                    Err(e) => {
                        // Differentiate between missing and unreadable
                        let status = if e.kind() == std::io::ErrorKind::NotFound {
                            IntegrityStatus::Missing
                        } else {
                            IntegrityStatus::Unreadable
                        };
                        FileIntegrityResult {
                            relative_path: manifest_path.clone(),
                            expected_hash: expected_normalized,
                            actual_hash: None,
                            status,
                        }
                    }
                }
            };

            // Report progress
            let checked = files_checked.fetch_add(1, Ordering::SeqCst) + 1;
            let processed = bytes_processed.fetch_add(file_size, Ordering::SeqCst) + file_size;
            progress(IntegrityProgress {
                current_file: manifest_path.clone(),
                files_checked: checked,
                total_files,
                bytes_processed: processed,
                total_bytes,
            });

            Some(result)
        })
        .flatten()
        .collect();

    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_normalize_manifest_hash() {
        // Typical goatcorp hash format
        let hash = "A0 A1 A2 A3 B4 B5 C6 C7 D8 D9 EA EB FC FD 0E 0F 10 11 12 13";
        let normalized = normalize_manifest_hash(hash);
        assert_eq!(normalized, "a0a1a2a3b4b5c6c7d8d9eaebfcfd0e0f10111213");

        // Already normalized
        let hash2 = "a0a1a2a3";
        assert_eq!(normalize_manifest_hash(hash2), "a0a1a2a3");

        // Mixed case with irregular spacing
        let hash3 = "aB  Cd   eF";
        assert_eq!(normalize_manifest_hash(hash3), "abcdef");
    }

    #[test]
    fn test_normalize_path() {
        let game_path = Path::new("/home/user/ffxiv");

        // Windows-style path from manifest
        let manifest_path = r"\game\sqpack\ffxiv\000000.win32.dat";
        let result = normalize_path(game_path, manifest_path).unwrap();
        assert_eq!(
            result,
            PathBuf::from("/home/user/ffxiv/game/sqpack/ffxiv/000000.win32.dat")
        );

        // Already unix-style (shouldn't happen but handle gracefully)
        let unix_path = "game/sqpack/ffxiv/test.dat";
        let result2 = normalize_path(game_path, unix_path).unwrap();
        assert_eq!(
            result2,
            PathBuf::from("/home/user/ffxiv/game/sqpack/ffxiv/test.dat")
        );

        // Double backslash
        let double_path = r"\\boot\\ffxivboot.exe";
        let result3 = normalize_path(game_path, double_path).unwrap();
        assert_eq!(
            result3,
            PathBuf::from("/home/user/ffxiv/boot/ffxivboot.exe")
        );
    }

    #[test]
    fn test_normalize_path_traversal_rejected() {
        let game_path = Path::new("/home/user/ffxiv");

        // Path traversal attempt should be rejected
        let traversal_path = r"\game\..\..\..\etc\passwd";
        let result = normalize_path(game_path, traversal_path);
        assert!(result.is_err());

        // Another traversal attempt
        let traversal_path2 = r"\game\sqpack\..\..\boot";
        let result2 = normalize_path(game_path, traversal_path2);
        assert!(result2.is_err());
    }

    #[test]
    fn test_compute_file_hash() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        // Write known content
        std::fs::write(&file_path, "Hello, World!").unwrap();

        let hash = compute_file_hash(&file_path).unwrap();

        // SHA1 of "Hello, World!" is known
        assert_eq!(hash, "0a0a9f2a6772942557ab5355d76af442f8f65e01");
    }

    #[test]
    fn test_compute_file_hash_missing_file() {
        let result = compute_file_hash(Path::new("/nonexistent/file"));
        assert!(result.is_err());
    }

    #[test]
    fn test_check_files_parallel_all_valid() {
        let temp_dir = TempDir::new().unwrap();
        let game_path = temp_dir.path();

        // Create test directory structure
        std::fs::create_dir_all(game_path.join("game/sqpack")).unwrap();

        // Create test files with known content
        let file1_path = game_path.join("game/sqpack/test1.dat");
        let file2_path = game_path.join("game/sqpack/test2.dat");

        std::fs::write(&file1_path, "content1").unwrap();
        std::fs::write(&file2_path, "content2").unwrap();

        // Compute actual hashes
        let hash1 = compute_file_hash(&file1_path).unwrap();
        let hash2 = compute_file_hash(&file2_path).unwrap();

        // Build manifest with correct hashes (space-separated uppercase format)
        let mut hashes = HashMap::new();
        hashes.insert(
            r"\game\sqpack\test1.dat".to_string(),
            format_as_manifest_hash(&hash1),
        );
        hashes.insert(
            r"\game\sqpack\test2.dat".to_string(),
            format_as_manifest_hash(&hash2),
        );

        let progress = Arc::new(|_: IntegrityProgress| {});
        let cancelled = AtomicBool::new(false);

        let results = check_files_parallel(game_path, &hashes, progress, &cancelled).unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.status == IntegrityStatus::Valid));
    }

    #[test]
    fn test_check_files_parallel_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let game_path = temp_dir.path();

        let mut hashes = HashMap::new();
        hashes.insert(
            r"\game\missing.dat".to_string(),
            "AA BB CC DD".to_string(),
        );

        let progress = Arc::new(|_: IntegrityProgress| {});
        let cancelled = AtomicBool::new(false);

        let results = check_files_parallel(game_path, &hashes, progress, &cancelled).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, IntegrityStatus::Missing);
        assert!(results[0].actual_hash.is_none());
    }

    #[test]
    fn test_check_files_parallel_mismatch() {
        let temp_dir = TempDir::new().unwrap();
        let game_path = temp_dir.path();

        // Create test file
        std::fs::create_dir_all(game_path.join("game")).unwrap();
        let file_path = game_path.join("game/test.dat");
        std::fs::write(&file_path, "actual content").unwrap();

        // Build manifest with wrong hash
        let mut hashes = HashMap::new();
        hashes.insert(r"\game\test.dat".to_string(), "AA BB CC DD".to_string());

        let progress = Arc::new(|_: IntegrityProgress| {});
        let cancelled = AtomicBool::new(false);

        let results = check_files_parallel(game_path, &hashes, progress, &cancelled).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, IntegrityStatus::Mismatch);
        assert!(results[0].actual_hash.is_some());
    }

    #[test]
    fn test_check_files_parallel_cancelled() {
        let temp_dir = TempDir::new().unwrap();
        let game_path = temp_dir.path();

        let mut hashes = HashMap::new();
        hashes.insert(r"\game\test.dat".to_string(), "AA BB CC DD".to_string());

        let progress = Arc::new(|_: IntegrityProgress| {});
        let cancelled = AtomicBool::new(true); // Already cancelled

        let result = check_files_parallel(game_path, &hashes, progress, &cancelled);

        assert!(result.is_err());
        if let Err(Error::Cancelled) = result {
            // Expected
        } else {
            panic!("Expected Error::Cancelled");
        }
    }

    /// Helper to format a lowercase hex hash as manifest format (space-separated uppercase)
    fn format_as_manifest_hash(hex: &str) -> String {
        hex.as_bytes()
            .chunks(2)
            .map(|chunk| {
                let s = std::str::from_utf8(chunk).unwrap();
                s.to_uppercase()
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[tokio::test]
    async fn test_repair_file_deletes_corrupted() {
        let temp_dir = TempDir::new().unwrap();
        let game_path = temp_dir.path();

        // Create test file
        std::fs::create_dir_all(game_path.join("game")).unwrap();
        let file_path = game_path.join("game/corrupted.dat");
        std::fs::write(&file_path, "corrupted content").unwrap();

        assert!(file_path.exists());

        let checker = GoatcorpIntegrityChecker::with_default_client();
        checker
            .repair_file(game_path, r"\game\corrupted.dat", "expected_hash")
            .await
            .unwrap();

        assert!(!file_path.exists());
    }

    #[tokio::test]
    async fn test_repair_file_missing_is_ok() {
        let temp_dir = TempDir::new().unwrap();
        let game_path = temp_dir.path();

        let checker = GoatcorpIntegrityChecker::with_default_client();

        // Should not error if file doesn't exist
        let result = checker
            .repair_file(game_path, r"\game\nonexistent.dat", "expected_hash")
            .await;

        assert!(result.is_ok());
    }

    #[test]
    fn test_goatcorp_integrity_checker_new() {
        let client = reqwest::Client::new();
        let _checker = GoatcorpIntegrityChecker::new(client);
        // Just verify it can be constructed
    }

    #[test]
    fn test_cache_path() {
        let path = GoatcorpIntegrityChecker::cache_path("2024.01.01.0000.0000");
        assert!(path.to_string_lossy().contains("2024.01.01.0000.0000.json"));
    }

    #[test]
    fn test_check_files_parallel_with_progress() {
        let temp_dir = TempDir::new().unwrap();
        let game_path = temp_dir.path();

        // Create test file
        std::fs::create_dir_all(game_path.join("game")).unwrap();
        let file_path = game_path.join("game/test.dat");
        std::fs::write(&file_path, "test content").unwrap();

        let actual_hash = compute_file_hash(&file_path).unwrap();

        let mut hashes = HashMap::new();
        hashes.insert(
            r"\game\test.dat".to_string(),
            format_as_manifest_hash(&actual_hash),
        );

        let progress_called = Arc::new(AtomicBool::new(false));
        let progress_called_clone = Arc::clone(&progress_called);
        let progress = Arc::new(move |_: IntegrityProgress| {
            progress_called_clone.store(true, Ordering::SeqCst);
        });
        let cancelled = AtomicBool::new(false);

        let results = check_files_parallel(game_path, &hashes, progress, &cancelled).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, IntegrityStatus::Valid);
        assert!(progress_called.load(Ordering::SeqCst));
    }
}
