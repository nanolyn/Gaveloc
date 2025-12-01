//! Integrity checking commands for verifying and repairing game files

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use gaveloc_core::entities::{FileIntegrityResult, IntegrityProgress, IntegrityStatus, Repository};
use gaveloc_core::ports::{IntegrityChecker, VersionRepository};

use crate::state::AppState;

// ============================================================================
// DTOs
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityResultDto {
    pub total_files: u32,
    pub valid_count: u32,
    pub mismatch_count: u32,
    pub missing_count: u32,
    pub unreadable_count: u32,
    pub problems: Vec<FileIntegrityResultDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileIntegrityResultDto {
    pub relative_path: String,
    pub status: String,
    pub expected_hash: String,
    pub actual_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileToRepairDto {
    pub relative_path: String,
    pub expected_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairResultDto {
    pub success_count: u32,
    pub failure_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityStatusDto {
    pub is_checking: bool,
    pub current_file: Option<String>,
    pub files_checked: u32,
    pub total_files: u32,
    pub bytes_processed: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityProgressEvent {
    pub current_file: String,
    pub files_checked: u32,
    pub total_files: u32,
    pub bytes_processed: u64,
    pub total_bytes: u64,
    pub percent: f64,
}

// ============================================================================
// State
// ============================================================================

/// State for tracking integrity check progress
pub struct IntegrityState {
    pub is_checking: bool,
    pub cancel_requested: Arc<AtomicBool>,
    pub current_file: Option<String>,
    pub files_checked: u32,
    pub total_files: u32,
    pub bytes_processed: u64,
    pub total_bytes: u64,
}

impl Default for IntegrityState {
    fn default() -> Self {
        Self {
            is_checking: false,
            cancel_requested: Arc::new(AtomicBool::new(false)),
            current_file: None,
            files_checked: 0,
            total_files: 0,
            bytes_processed: 0,
            total_bytes: 0,
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn status_to_string(status: IntegrityStatus) -> String {
    match status {
        IntegrityStatus::Valid => "Valid".to_string(),
        IntegrityStatus::Mismatch => "Mismatch".to_string(),
        IntegrityStatus::Missing => "Missing".to_string(),
        IntegrityStatus::Unreadable => "Unreadable".to_string(),
    }
}

async fn get_game_path(state: &AppState) -> Result<PathBuf, String> {
    let settings = state.settings.read().await;
    settings
        .game
        .path
        .clone()
        .ok_or_else(|| "Game path not configured".to_string())
}

async fn get_game_version(state: &AppState) -> Result<String, String> {
    let game_path = get_game_path(state).await?;
    let version_repo = state.version_repo.read().await;
    let repo = version_repo
        .as_ref()
        .ok_or("Version repository not initialized")?;

    repo.get_version(&game_path, Repository::Ffxiv)
        .await
        .map(|v| v.to_string())
        .map_err(|e| e.to_string())
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Start integrity verification
#[tauri::command]
pub async fn verify_integrity(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<IntegrityResultDto, String> {
    // Check if already checking
    {
        let integrity_state = state.integrity_state.read().await;
        if integrity_state.is_checking {
            return Err("Integrity check already in progress".to_string());
        }
    }

    // Get game path and version
    let game_path = get_game_path(&state).await?;
    let game_version = get_game_version(&state).await?;

    // Fetch manifest (uses 24h cache)
    let manifest = state
        .integrity_checker
        .fetch_manifest(&game_version)
        .await
        .map_err(|e| format!("Failed to fetch integrity manifest: {}", e))?;

    let total_files = manifest.hashes.len() as u32;

    // Set up state
    {
        let mut integrity_state = state.integrity_state.write().await;
        integrity_state.is_checking = true;
        integrity_state.total_files = total_files;
        integrity_state.files_checked = 0;
        integrity_state.bytes_processed = 0;
        integrity_state.total_bytes = 0;
        integrity_state.current_file = None;
        integrity_state.cancel_requested.store(false, Ordering::SeqCst);
    }

    // Clone state references for the progress callback
    let integrity_state_clone = state.integrity_state.clone();
    let app_handle_clone = app_handle.clone();

    // Progress callback
    let progress_callback = move |progress: IntegrityProgress| {
        // Update state (in blocking manner since we're in a sync callback)
        let integrity_state = integrity_state_clone.clone();
        let app_handle = app_handle_clone.clone();

        // Spawn a task to update state and emit event
        tokio::spawn(async move {
            {
                let mut state = integrity_state.write().await;
                state.current_file = Some(progress.current_file.clone());
                state.files_checked = progress.files_checked;
                state.total_files = progress.total_files;
                state.bytes_processed = progress.bytes_processed;
                state.total_bytes = progress.total_bytes;
            }

            let percent = progress.progress_percent();
            let _ = app_handle.emit(
                "integrity_progress",
                IntegrityProgressEvent {
                    current_file: progress.current_file,
                    files_checked: progress.files_checked,
                    total_files: progress.total_files,
                    bytes_processed: progress.bytes_processed,
                    total_bytes: progress.total_bytes,
                    percent,
                },
            );
        });
    };

    // Run integrity check
    let results = state
        .integrity_checker
        .check_integrity(&game_path, &manifest, progress_callback)
        .await
        .map_err(|e| {
            // Reset state on error
            let integrity_state = state.integrity_state.clone();
            tokio::spawn(async move {
                let mut state = integrity_state.write().await;
                state.is_checking = false;
            });
            format!("Integrity check failed: {}", e)
        })?;

    // Build result
    let valid_count = results
        .iter()
        .filter(|r| r.status == IntegrityStatus::Valid)
        .count() as u32;
    let mismatch_count = results
        .iter()
        .filter(|r| r.status == IntegrityStatus::Mismatch)
        .count() as u32;
    let missing_count = results
        .iter()
        .filter(|r| r.status == IntegrityStatus::Missing)
        .count() as u32;
    let unreadable_count = results
        .iter()
        .filter(|r| r.status == IntegrityStatus::Unreadable)
        .count() as u32;

    let problems: Vec<FileIntegrityResultDto> = results
        .iter()
        .filter(|r| r.status != IntegrityStatus::Valid)
        .map(|r| FileIntegrityResultDto {
            relative_path: r.relative_path.clone(),
            status: status_to_string(r.status),
            expected_hash: r.expected_hash.clone(),
            actual_hash: r.actual_hash.clone(),
        })
        .collect();

    let result = IntegrityResultDto {
        total_files: results.len() as u32,
        valid_count,
        mismatch_count,
        missing_count,
        unreadable_count,
        problems,
    };

    // Emit complete event
    let _ = app_handle.emit("integrity_complete", &result);

    // Reset state
    {
        let mut integrity_state = state.integrity_state.write().await;
        integrity_state.is_checking = false;
    }

    Ok(result)
}

/// Repair corrupted/missing files (deletes them so patching system can restore)
#[tauri::command]
pub async fn repair_files(
    state: State<'_, AppState>,
    files: Vec<FileToRepairDto>,
) -> Result<RepairResultDto, String> {
    let game_path = get_game_path(&state).await?;

    if files.is_empty() {
        return Ok(RepairResultDto {
            success_count: 0,
            failure_count: 0,
        });
    }

    // Convert DTOs to FileIntegrityResult for the repair method
    let file_results: Vec<FileIntegrityResult> = files
        .into_iter()
        .map(|f| FileIntegrityResult {
            relative_path: f.relative_path,
            expected_hash: f.expected_hash,
            actual_hash: None,
            status: IntegrityStatus::Mismatch, // Status doesn't matter for repair
        })
        .collect();

    let (success, failure) = state
        .integrity_checker
        .repair_files(&game_path, &file_results)
        .await
        .map_err(|e| format!("Failed to repair files: {}", e))?;

    Ok(RepairResultDto {
        success_count: success,
        failure_count: failure,
    })
}

/// Cancel ongoing integrity check
#[tauri::command]
pub async fn cancel_integrity_check(state: State<'_, AppState>) -> Result<(), String> {
    let integrity_state = state.integrity_state.read().await;
    if !integrity_state.is_checking {
        return Err("No integrity check in progress".to_string());
    }

    integrity_state
        .cancel_requested
        .store(true, Ordering::SeqCst);
    Ok(())
}

/// Get current integrity check status
#[tauri::command]
pub async fn get_integrity_status(state: State<'_, AppState>) -> Result<IntegrityStatusDto, String> {
    let integrity_state = state.integrity_state.read().await;

    Ok(IntegrityStatusDto {
        is_checking: integrity_state.is_checking,
        current_file: integrity_state.current_file.clone(),
        files_checked: integrity_state.files_checked,
        total_files: integrity_state.total_files,
        bytes_processed: integrity_state.bytes_processed,
        total_bytes: integrity_state.total_bytes,
    })
}
