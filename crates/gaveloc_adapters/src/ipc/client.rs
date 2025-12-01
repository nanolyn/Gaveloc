//! IPC client implementation for communicating with the patcher process
//!
//! The UnixSocketPatcherIpc spawns the patcher binary and manages the socket
//! connection for bidirectional communication.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::tokio::Stream;
use interprocess::local_socket::{GenericFilePath, ListenerOptions, ToFsName};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use gaveloc_core::entities::{PatchEntry, PatchProgress, PatchState};
use gaveloc_core::error::Error;
use gaveloc_core::ports::PatcherIpc;

use super::protocol::{
    deserialize_message, serialize_message, PatcherRequest, PatcherResponse, MAX_MESSAGE_SIZE,
    MESSAGE_HEADER_SIZE,
};

/// Socket path prefix for patcher IPC
const SOCKET_PATH_PREFIX: &str = "/tmp/gaveloc_patcher_";

/// Timeout for initial connection to patcher
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for receiving a single message (5 minutes for long patch operations)
const RECV_TIMEOUT: Duration = Duration::from_secs(300);

/// Internal state that requires interior mutability
struct IpcState {
    stream: Option<Stream>,
    child_process: Option<Child>,
}

/// IPC client that communicates with the patcher process over Unix domain sockets
pub struct UnixSocketPatcherIpc {
    socket_path: PathBuf,
    state: Arc<Mutex<IpcState>>,
    is_running: Arc<AtomicBool>,
}

impl UnixSocketPatcherIpc {
    /// Find the patcher binary in the same directory as the current executable
    pub fn find_patcher_binary() -> Result<PathBuf, Error> {
        let exe_path = std::env::current_exe()
            .map_err(|e| Error::Ipc(format!("failed to get current exe path: {}", e)))?;

        let exe_dir = exe_path
            .parent()
            .ok_or_else(|| Error::Ipc("executable has no parent directory".into()))?;

        let patcher_path = exe_dir.join("gaveloc_patcher");

        if patcher_path.exists() {
            Ok(patcher_path)
        } else {
            // Also try with hyphen in case of different naming
            let alt_path = exe_dir.join("gaveloc-patcher");
            if alt_path.exists() {
                Ok(alt_path)
            } else {
                Err(Error::Ipc(format!(
                    "patcher binary not found at {:?} or {:?}",
                    patcher_path, alt_path
                )))
            }
        }
    }

    /// Generate a unique socket path for this launcher instance
    fn generate_socket_path() -> PathBuf {
        let pid = std::process::id();
        PathBuf::from(format!("{}{}.sock", SOCKET_PATH_PREFIX, pid))
    }

    /// Spawn the patcher process and establish IPC connection
    pub async fn spawn() -> Result<Self, Error> {
        let patcher_binary = Self::find_patcher_binary()?;
        Self::spawn_with_binary(&patcher_binary).await
    }

    /// Spawn the patcher process using a specific binary path
    pub async fn spawn_with_binary(patcher_binary: &Path) -> Result<Self, Error> {
        let socket_path = Self::generate_socket_path();

        // Clean up any stale socket file
        if socket_path.exists() {
            std::fs::remove_file(&socket_path).ok();
        }

        info!(
            "spawning patcher: {:?} with socket {:?}",
            patcher_binary, socket_path
        );

        // Create the socket listener before spawning the patcher
        let socket_name = socket_path
            .clone()
            .to_fs_name::<GenericFilePath>()
            .map_err(|e| Error::Ipc(format!("invalid socket path: {}", e)))?;

        let listener = ListenerOptions::new()
            .name(socket_name)
            .create_tokio()
            .map_err(|e| Error::Ipc(format!("failed to create socket listener: {}", e)))?;

        // Spawn the patcher process
        let child = Command::new(patcher_binary)
            .arg(&socket_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::Ipc(format!("failed to spawn patcher: {}", e)))?;

        let child_pid = child.id();
        debug!("patcher spawned with PID {}", child_pid);

        // Wait for patcher to connect with timeout
        let stream = tokio::time::timeout(CONNECTION_TIMEOUT, listener.accept())
            .await
            .map_err(|_| Error::Ipc("patcher connection timeout".into()))?
            .map_err(|e| Error::Ipc(format!("failed to accept patcher connection: {}", e)))?;

        debug!("patcher connected");

        let ipc = Self {
            socket_path,
            state: Arc::new(Mutex::new(IpcState {
                stream: Some(stream),
                child_process: Some(child),
            })),
            is_running: Arc::new(AtomicBool::new(true)),
        };

        // Wait for Ready message
        let response = ipc.recv_internal(RECV_TIMEOUT).await?;
        match response {
            PatcherResponse::Ready => {
                info!("patcher is ready");
            }
            other => {
                return Err(Error::Ipc(format!(
                    "expected Ready message, got {:?}",
                    other
                )));
            }
        }

        // Send Hello handshake
        let parent_pid = std::process::id();
        ipc.send_internal(&PatcherRequest::Hello { parent_pid })
            .await?;

        Ok(ipc)
    }

    /// Send a request to the patcher (internal method)
    async fn send_internal(&self, request: &PatcherRequest) -> Result<(), Error> {
        let bytes =
            serialize_message(request).map_err(|e| Error::Ipc(format!("serialize error: {}", e)))?;

        let mut state = self.state.lock().await;
        let stream = state
            .stream
            .as_mut()
            .ok_or_else(|| Error::Ipc("stream closed".into()))?;

        stream
            .write_all(&bytes)
            .await
            .map_err(|e| Error::Ipc(format!("write error: {}", e)))?;

        stream
            .flush()
            .await
            .map_err(|e| Error::Ipc(format!("flush error: {}", e)))?;

        Ok(())
    }

    /// Receive a response from the patcher (internal method)
    async fn recv_internal(&self, timeout: Duration) -> Result<PatcherResponse, Error> {
        let mut state = self.state.lock().await;
        let stream = state
            .stream
            .as_mut()
            .ok_or_else(|| Error::Ipc("stream closed".into()))?;

        // Read length header
        let mut header = [0u8; MESSAGE_HEADER_SIZE];
        tokio::time::timeout(timeout, stream.read_exact(&mut header))
            .await
            .map_err(|_| Error::Ipc("recv timeout".into()))?
            .map_err(|e| Error::Ipc(format!("read header error: {}", e)))?;

        let len = u32::from_be_bytes(header);

        if len > MAX_MESSAGE_SIZE {
            return Err(Error::Ipc(format!(
                "message too large: {} bytes (max {})",
                len, MAX_MESSAGE_SIZE
            )));
        }

        // Read payload
        let mut payload = vec![0u8; len as usize];
        tokio::time::timeout(timeout, stream.read_exact(&mut payload))
            .await
            .map_err(|_| Error::Ipc("recv payload timeout".into()))?
            .map_err(|e| Error::Ipc(format!("read payload error: {}", e)))?;

        let response: PatcherResponse = deserialize_message(&payload)
            .map_err(|e| Error::Ipc(format!("deserialize error: {}", e)))?;

        Ok(response)
    }

    /// Check if the child process is still alive
    async fn check_child_alive(&self) -> bool {
        let mut state = self.state.lock().await;
        if let Some(ref mut child) = state.child_process {
            match child.try_wait() {
                Ok(None) => true, // Still running
                Ok(Some(status)) => {
                    error!("patcher process exited unexpectedly: {:?}", status);
                    self.is_running.store(false, Ordering::SeqCst);
                    false
                }
                Err(e) => {
                    error!("error checking patcher status: {}", e);
                    false
                }
            }
        } else {
            false
        }
    }

    /// Request graceful shutdown of the patcher
    pub async fn shutdown(&self) -> Result<(), Error> {
        if !self.is_running.load(Ordering::SeqCst) {
            return Ok(());
        }

        debug!("sending shutdown to patcher");
        self.send_internal(&PatcherRequest::Shutdown).await?;
        self.is_running.store(false, Ordering::SeqCst);

        // Close the stream and wait for child
        let mut state = self.state.lock().await;
        state.stream = None;

        if let Some(mut child) = state.child_process.take() {
            drop(state); // Release lock before blocking

            match tokio::time::timeout(Duration::from_secs(5), async {
                tokio::task::spawn_blocking(move || child.wait())
                    .await
                    .ok()
            })
            .await
            {
                Ok(Some(Ok(status))) => {
                    debug!("patcher exited with status: {:?}", status);
                }
                Ok(Some(Err(e))) => {
                    warn!("error waiting for patcher: {}", e);
                }
                Ok(None) => {
                    warn!("failed to wait for patcher");
                }
                Err(_) => {
                    warn!("timeout waiting for patcher to exit");
                }
            }
        }

        // Clean up socket file
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path).ok();
        }

        Ok(())
    }
}

impl Drop for UnixSocketPatcherIpc {
    fn drop(&mut self) {
        // Clean up socket file on drop
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path).ok();
        }

        // Try to kill the child if still running
        // Note: We can't use async here, so we do a best-effort cleanup
        if let Ok(mut state) = self.state.try_lock() {
            if let Some(mut child) = state.child_process.take() {
                child.kill().ok();
            }
        }
    }
}

#[async_trait]
impl PatcherIpc for UnixSocketPatcherIpc {
    async fn start_patch(&self, patches: Vec<PatchEntry>, game_path: &Path) -> Result<(), Error> {
        let request = PatcherRequest::StartPatch {
            patches,
            game_path: game_path.to_path_buf(),
            keep_patches: false,
        };

        self.send_internal(&request).await
    }

    async fn receive_progress(&self) -> Result<Option<PatchProgress>, Error> {
        // Check if child is still alive
        if !self.check_child_alive().await {
            return Err(Error::Ipc("patcher process died".into()));
        }

        // Use a shorter timeout for progress polling
        let response = match self.recv_internal(Duration::from_millis(100)).await {
            Ok(resp) => resp,
            Err(Error::Ipc(msg)) if msg.contains("timeout") => {
                // Timeout is expected when no progress available
                return Ok(None);
            }
            Err(e) => return Err(e),
        };

        match response {
            PatcherResponse::Progress {
                version_id,
                repository,
                state,
                bytes_processed,
                bytes_total,
                ..
            } => {
                let patch = PatchEntry {
                    version_id,
                    url: String::new(),
                    length: bytes_total,
                    hash_type: None,
                    hash_block_size: None,
                    hashes: None,
                    repository,
                };

                Ok(Some(PatchProgress {
                    patch,
                    state,
                    bytes_downloaded: bytes_processed,
                    bytes_total,
                    speed_bytes_per_sec: 0.0, // Not tracked in IPC
                }))
            }
            PatcherResponse::PatchCompleted { version_id, .. } => {
                let patch = PatchEntry {
                    version_id,
                    url: String::new(),
                    length: 0,
                    hash_type: None,
                    hash_block_size: None,
                    hashes: None,
                    repository: gaveloc_core::entities::Repository::Ffxiv,
                };

                Ok(Some(PatchProgress {
                    patch,
                    state: PatchState::Completed,
                    bytes_downloaded: 0,
                    bytes_total: 0,
                    speed_bytes_per_sec: 0.0,
                }))
            }
            PatcherResponse::AllCompleted => {
                // Signal completion by returning None
                Ok(None)
            }
            PatcherResponse::Error { message } => Err(Error::ZiPatchApply(message)),
            PatcherResponse::Cancelled => Err(Error::Cancelled),
            other => Err(Error::Ipc(format!("unexpected response: {:?}", other))),
        }
    }

    async fn cancel(&self) -> Result<(), Error> {
        self.send_internal(&PatcherRequest::Cancel).await
    }

    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_path_generation() {
        let path = UnixSocketPatcherIpc::generate_socket_path();
        let path_str = path.to_string_lossy();
        assert!(path_str.starts_with(SOCKET_PATH_PREFIX));
        assert!(path_str.ends_with(".sock"));
    }

    #[test]
    fn test_find_patcher_binary_error() {
        // This will fail in test environment since binary doesn't exist
        let result = UnixSocketPatcherIpc::find_patcher_binary();
        assert!(result.is_err());
    }
}
