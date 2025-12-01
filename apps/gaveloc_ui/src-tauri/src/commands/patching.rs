use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use gaveloc_adapters::ipc::UnixSocketPatcherIpc;
use gaveloc_core::entities::{AccountId, PatchEntry, Repository};
use gaveloc_core::ports::{CredentialStore, PatchDownloader, PatcherIpc, PatchServer, VersionRepository};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::RwLock;

use crate::state::AppState;

/// Patch phase enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PatchPhase {
    Idle,
    Downloading,
    Verifying,
    Applying,
    Completed,
    Failed,
    Cancelled,
}

/// Patch status DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchStatusDto {
    pub is_patching: bool,
    pub phase: PatchPhase,
    pub current_patch_index: usize,
    pub total_patches: usize,
    pub current_version_id: Option<String>,
    pub current_repository: Option<String>,
    pub bytes_downloaded: u64,
    pub bytes_total: u64,
    pub speed_bytes_per_sec: f64,
}

/// Patch progress event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchProgressEvent {
    pub phase: PatchPhase,
    pub current_index: usize,
    pub total_patches: usize,
    pub version_id: String,
    pub repository: String,
    pub bytes_processed: u64,
    pub bytes_total: u64,
    pub speed_bytes_per_sec: f64,
}

/// Patch completed event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchCompletedEvent {
    pub index: usize,
    pub version_id: String,
    pub repository: String,
}

/// Patch error event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchErrorEvent {
    pub message: String,
    pub recoverable: bool,
}

/// Internal patching state
pub struct PatchingState {
    pub is_patching: bool,
    pub phase: PatchPhase,
    pub current_index: usize,
    pub total_patches: usize,
    pub current_version_id: Option<String>,
    pub current_repository: Option<String>,
    pub bytes_processed: u64,
    pub bytes_total: u64,
    pub speed: f64,
    pub cancel_requested: Arc<AtomicBool>,
}

impl Default for PatchingState {
    fn default() -> Self {
        Self {
            is_patching: false,
            phase: PatchPhase::Idle,
            current_index: 0,
            total_patches: 0,
            current_version_id: None,
            current_repository: None,
            bytes_processed: 0,
            bytes_total: 0,
            speed: 0.0,
            cancel_requested: Arc::new(AtomicBool::new(false)),
        }
    }
}

fn repository_name(repo: Repository) -> &'static str {
    match repo {
        Repository::Boot => "Boot",
        Repository::Ffxiv => "FFXIV",
        Repository::Ex1 => "Heavensward",
        Repository::Ex2 => "Stormblood",
        Repository::Ex3 => "Shadowbringers",
        Repository::Ex4 => "Endwalker",
        Repository::Ex5 => "Dawntrail",
    }
}

/// Get game path from settings
async fn get_game_path(state: &AppState) -> Option<PathBuf> {
    let settings = state.settings.read().await;
    settings.game.path.clone()
}

/// Start patching process for boot updates
#[tauri::command]
pub async fn start_boot_patch(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let game_path = match get_game_path(&state).await {
        Some(path) => path,
        None => return Err("Game path not configured".to_string()),
    };

    // Check if already patching
    {
        let patch_state = state.patch_state.read().await;
        if patch_state.is_patching {
            return Err("Patching already in progress".to_string());
        }
    }

    // Get version repo
    let version_repo_guard = state.version_repo.read().await;
    let version_repo = match version_repo_guard.as_ref() {
        Some(repo) => repo,
        None => return Err("Version repository not initialized".to_string()),
    };

    // Get boot version
    let boot_version = match version_repo.get_version(&game_path, Repository::Boot).await {
        Ok(v) => v,
        Err(e) => return Err(format!("Failed to read boot version: {}", e)),
    };

    // Check for boot updates
    let patches = match state
        .patch_server
        .check_boot_version(&game_path, &boot_version)
        .await
    {
        Ok(p) => p,
        Err(e) => return Err(format!("Failed to check boot updates: {}", e)),
    };

    if patches.is_empty() {
        return Ok(()); // No updates needed
    }

    drop(version_repo_guard);

    // Start patching in background
    let patch_downloader = state.patch_downloader.clone();
    let patch_state = state.patch_state.clone();
    let version_repo = state.version_repo.clone();

    tokio::spawn(async move {
        run_patching(
            app_handle,
            patches,
            game_path,
            None, // No unique_id for boot patches
            patch_downloader,
            patch_state,
            version_repo,
        )
        .await;
    });

    Ok(())
}

/// Start patching process for game updates (requires valid session)
#[tauri::command]
pub async fn start_game_patch(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    account_id: String,
) -> Result<(), String> {
    let game_path = match get_game_path(&state).await {
        Some(path) => path,
        None => return Err("Game path not configured".to_string()),
    };

    // Check if already patching
    {
        let patch_state = state.patch_state.read().await;
        if patch_state.is_patching {
            return Err("Patching already in progress".to_string());
        }
    }

    // Get cached session for the account
    let id = AccountId::new(&account_id);
    let session = match state.credentials.get_session(&id).await {
        Ok(Some(s)) if s.is_valid() => s,
        Ok(Some(_)) => return Err("Session expired. Please log in again.".to_string()),
        Ok(None) => return Err("Not logged in. Please log in first.".to_string()),
        Err(e) => return Err(format!("Failed to get session: {}", e)),
    };

    // Register session and get patches
    let (unique_id, patches) = match state
        .patch_server
        .register_session(&session.unique_id, &game_path, session.max_expansion)
        .await
    {
        Ok(result) => result,
        Err(e) => return Err(format!("Failed to check game updates: {}", e)),
    };

    if patches.is_empty() {
        return Ok(()); // No updates needed
    }

    // Start patching in background
    let patch_downloader = state.patch_downloader.clone();
    let patch_state = state.patch_state.clone();
    let version_repo = state.version_repo.clone();

    tokio::spawn(async move {
        run_patching(
            app_handle,
            patches,
            game_path,
            Some(unique_id),
            patch_downloader,
            patch_state,
            version_repo,
        )
        .await;
    });

    Ok(())
}

/// Cancel ongoing patch operation
#[tauri::command]
pub async fn cancel_patch(state: State<'_, AppState>) -> Result<(), String> {
    let patch_state = state.patch_state.read().await;
    if !patch_state.is_patching {
        return Ok(());
    }

    patch_state.cancel_requested.store(true, Ordering::SeqCst);
    Ok(())
}

/// Get current patch status
#[tauri::command]
pub async fn get_patch_status(state: State<'_, AppState>) -> Result<PatchStatusDto, String> {
    let patch_state = state.patch_state.read().await;

    Ok(PatchStatusDto {
        is_patching: patch_state.is_patching,
        phase: patch_state.phase.clone(),
        current_patch_index: patch_state.current_index,
        total_patches: patch_state.total_patches,
        current_version_id: patch_state.current_version_id.clone(),
        current_repository: patch_state.current_repository.clone(),
        bytes_downloaded: patch_state.bytes_processed,
        bytes_total: patch_state.bytes_total,
        speed_bytes_per_sec: patch_state.speed,
    })
}

/// Run the patching process
async fn run_patching(
    app_handle: AppHandle,
    patches: Vec<PatchEntry>,
    game_path: PathBuf,
    unique_id: Option<String>,
    patch_downloader: Arc<gaveloc_adapters::patch::HttpPatchDownloader>,
    patch_state: Arc<RwLock<PatchingState>>,
    version_repo: Arc<RwLock<Option<gaveloc_adapters::FileVersionRepository>>>,
) {
    let total_patches = patches.len();
    let temp_dir = std::env::temp_dir().join("gaveloc_patches");

    // Initialize state
    {
        let mut state = patch_state.write().await;
        state.is_patching = true;
        state.phase = PatchPhase::Downloading;
        state.current_index = 0;
        state.total_patches = total_patches;
        state.cancel_requested.store(false, Ordering::SeqCst);
    }

    // Create temp directory
    if let Err(e) = tokio::fs::create_dir_all(&temp_dir).await {
        emit_error(&app_handle, &patch_state, format!("Failed to create temp dir: {}", e), false).await;
        return;
    }

    // Download all patches first
    let mut downloaded_patches: Vec<(PatchEntry, PathBuf)> = Vec::new();

    for (index, patch) in patches.iter().enumerate() {
        // Check for cancellation
        if patch_state.read().await.cancel_requested.load(Ordering::SeqCst) {
            emit_cancelled(&app_handle, &patch_state).await;
            return;
        }

        let patch_file = temp_dir.join(format!("{}_{}.patch",
            repository_name(patch.repository),
            patch.version_id.replace('.', "_")
        ));

        // Update state
        {
            let mut state = patch_state.write().await;
            state.phase = PatchPhase::Downloading;
            state.current_index = index;
            state.current_version_id = Some(patch.version_id.clone());
            state.current_repository = Some(repository_name(patch.repository).to_string());
            state.bytes_processed = 0;
            state.bytes_total = patch.length;
        }

        // Emit progress event
        let _ = app_handle.emit("patch_progress", PatchProgressEvent {
            phase: PatchPhase::Downloading,
            current_index: index,
            total_patches,
            version_id: patch.version_id.clone(),
            repository: repository_name(patch.repository).to_string(),
            bytes_processed: 0,
            bytes_total: patch.length,
            speed_bytes_per_sec: 0.0,
        });

        // Download with progress
        let patch_state_clone = patch_state.clone();
        let app_handle_clone = app_handle.clone();
        let version_id = patch.version_id.clone();
        let repo_name = repository_name(patch.repository).to_string();
        let start_time = Instant::now();

        let progress_callback = move |downloaded: u64, total: u64| {
            let elapsed = start_time.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 { downloaded as f64 / elapsed } else { 0.0 };

            // Update state (spawn a task since we're in a sync callback)
            let patch_state = patch_state_clone.clone();
            let app_handle = app_handle_clone.clone();
            let version_id = version_id.clone();
            let repo_name = repo_name.clone();

            tokio::spawn(async move {
                {
                    let mut state = patch_state.write().await;
                    state.bytes_processed = downloaded;
                    state.speed = speed;
                }

                let _ = app_handle.emit("patch_progress", PatchProgressEvent {
                    phase: PatchPhase::Downloading,
                    current_index: index,
                    total_patches,
                    version_id,
                    repository: repo_name,
                    bytes_processed: downloaded,
                    bytes_total: total,
                    speed_bytes_per_sec: speed,
                });
            });
        };

        // Download
        if let Err(e) = patch_downloader
            .download_patch(patch, &patch_file, unique_id.as_deref(), progress_callback)
            .await
        {
            emit_error(&app_handle, &patch_state, format!("Download failed: {}", e), true).await;
            return;
        }

        // Verify
        {
            let mut state = patch_state.write().await;
            state.phase = PatchPhase::Verifying;
        }

        let _ = app_handle.emit("patch_progress", PatchProgressEvent {
            phase: PatchPhase::Verifying,
            current_index: index,
            total_patches,
            version_id: patch.version_id.clone(),
            repository: repository_name(patch.repository).to_string(),
            bytes_processed: patch.length,
            bytes_total: patch.length,
            speed_bytes_per_sec: 0.0,
        });

        match patch_downloader.verify_patch(patch, &patch_file).await {
            Ok(true) => {
                downloaded_patches.push((patch.clone(), patch_file));
            }
            Ok(false) => {
                emit_error(&app_handle, &patch_state, "Patch verification failed".to_string(), true).await;
                return;
            }
            Err(e) => {
                emit_error(&app_handle, &patch_state, format!("Verification error: {}", e), true).await;
                return;
            }
        }
    }

    // Now apply patches using the patcher process
    {
        let mut state = patch_state.write().await;
        state.phase = PatchPhase::Applying;
    }

    // Spawn patcher
    let patcher = match UnixSocketPatcherIpc::spawn().await {
        Ok(p) => p,
        Err(e) => {
            emit_error(&app_handle, &patch_state, format!("Failed to spawn patcher: {}", e), false).await;
            return;
        }
    };

    // Prepare patches with local file paths
    let patches_for_patcher: Vec<PatchEntry> = downloaded_patches
        .iter()
        .map(|(patch, path)| {
            let mut p = patch.clone();
            p.url = path.to_string_lossy().to_string();
            p
        })
        .collect();

    // Start patching
    if let Err(e) = patcher.start_patch(patches_for_patcher, &game_path).await {
        emit_error(&app_handle, &patch_state, format!("Failed to start patching: {}", e), false).await;
        let _ = patcher.shutdown().await;
        return;
    }

    // Poll for progress
    loop {
        // Check for cancellation
        if patch_state.read().await.cancel_requested.load(Ordering::SeqCst) {
            let _ = patcher.cancel().await;
            emit_cancelled(&app_handle, &patch_state).await;
            let _ = patcher.shutdown().await;
            return;
        }

        match patcher.receive_progress().await {
            Ok(Some(progress)) => {
                let phase = match progress.state {
                    gaveloc_core::entities::PatchState::Pending => PatchPhase::Applying,
                    gaveloc_core::entities::PatchState::Downloading => PatchPhase::Downloading,
                    gaveloc_core::entities::PatchState::Verifying => PatchPhase::Verifying,
                    gaveloc_core::entities::PatchState::Installing => PatchPhase::Applying,
                    gaveloc_core::entities::PatchState::Completed => PatchPhase::Completed,
                    gaveloc_core::entities::PatchState::Failed => PatchPhase::Failed,
                };

                // Find current index based on version_id
                let current_index = downloaded_patches
                    .iter()
                    .position(|(p, _)| p.version_id == progress.patch.version_id)
                    .unwrap_or(0);

                {
                    let mut state = patch_state.write().await;
                    state.phase = phase.clone();
                    state.current_index = current_index;
                    state.current_version_id = Some(progress.patch.version_id.clone());
                    state.current_repository = Some(repository_name(progress.patch.repository).to_string());
                    state.bytes_processed = progress.bytes_downloaded;
                    state.bytes_total = progress.bytes_total;
                }

                let _ = app_handle.emit("patch_progress", PatchProgressEvent {
                    phase,
                    current_index,
                    total_patches,
                    version_id: progress.patch.version_id.clone(),
                    repository: repository_name(progress.patch.repository).to_string(),
                    bytes_processed: progress.bytes_downloaded,
                    bytes_total: progress.bytes_total,
                    speed_bytes_per_sec: progress.speed_bytes_per_sec,
                });

                // If completed, emit patch_completed
                if progress.state == gaveloc_core::entities::PatchState::Completed {
                    let _ = app_handle.emit("patch_completed", PatchCompletedEvent {
                        index: current_index,
                        version_id: progress.patch.version_id,
                        repository: repository_name(progress.patch.repository).to_string(),
                    });
                }
            }
            Ok(None) => {
                // All completed
                break;
            }
            Err(gaveloc_core::error::Error::Cancelled) => {
                emit_cancelled(&app_handle, &patch_state).await;
                let _ = patcher.shutdown().await;
                return;
            }
            Err(e) => {
                emit_error(&app_handle, &patch_state, format!("Patch application error: {}", e), false).await;
                let _ = patcher.shutdown().await;
                return;
            }
        }

        // Small delay to avoid busy polling
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // Shutdown patcher
    let _ = patcher.shutdown().await;

    // Update version files
    let version_repo_guard = version_repo.read().await;
    if let Some(repo) = version_repo_guard.as_ref() {
        for (patch, _) in &downloaded_patches {
            if let Err(e) = repo.set_version(
                &game_path,
                patch.repository,
                &patch.version_id,
            ).await {
                eprintln!("Failed to update version for {:?}: {}", patch.repository, e);
            }
        }
    }

    // Clean up temp files
    for (_, path) in &downloaded_patches {
        let _ = tokio::fs::remove_file(path).await;
    }
    let _ = tokio::fs::remove_dir(&temp_dir).await;

    // Mark as completed
    {
        let mut state = patch_state.write().await;
        state.is_patching = false;
        state.phase = PatchPhase::Completed;
    }

    let _ = app_handle.emit("patch_all_completed", ());
}

async fn emit_error(
    app_handle: &AppHandle,
    patch_state: &Arc<RwLock<PatchingState>>,
    message: String,
    recoverable: bool,
) {
    {
        let mut state = patch_state.write().await;
        state.is_patching = false;
        state.phase = PatchPhase::Failed;
    }

    let _ = app_handle.emit("patch_error", PatchErrorEvent {
        message,
        recoverable,
    });
}

async fn emit_cancelled(app_handle: &AppHandle, patch_state: &Arc<RwLock<PatchingState>>) {
    {
        let mut state = patch_state.write().await;
        state.is_patching = false;
        state.phase = PatchPhase::Cancelled;
    }

    let _ = app_handle.emit("patch_cancelled", ());
}
