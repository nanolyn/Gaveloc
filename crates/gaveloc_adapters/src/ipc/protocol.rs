//! IPC protocol types for communication between launcher and patcher processes
//!
//! The patcher runs as a separate process to isolate patch application from the
//! main launcher UI. Communication happens over Unix domain sockets using bincode
//! serialization with length-prefixed messages.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use gaveloc_core::entities::{PatchEntry, PatchState, Repository};

/// Messages sent from the launcher to the patcher process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatcherRequest {
    /// Initial handshake - patcher should respond with Ready
    Hello {
        /// Process ID of the parent launcher
        parent_pid: u32,
    },

    /// Start applying a batch of patches
    StartPatch {
        /// List of patches to apply in order
        patches: Vec<PatchEntry>,
        /// Path to game installation directory
        game_path: PathBuf,
        /// Keep patch files after successful application (for debugging)
        keep_patches: bool,
    },

    /// Cancel the current patch operation
    Cancel,

    /// Graceful shutdown - patcher should exit after responding
    Shutdown,
}

/// Messages sent from the patcher back to the launcher
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatcherResponse {
    /// Patcher is ready to receive commands
    Ready,

    /// Progress update for current patch operation
    Progress {
        /// Index of current patch in the batch (0-based)
        patch_index: usize,
        /// Total number of patches in the batch
        total_patches: usize,
        /// Version ID of the current patch
        version_id: String,
        /// Repository being patched
        repository: Repository,
        /// Current state of the patch operation
        state: PatchState,
        /// Bytes processed for current patch
        bytes_processed: u64,
        /// Total bytes for current patch file
        bytes_total: u64,
    },

    /// A single patch was successfully applied
    PatchCompleted {
        /// Index of the completed patch
        patch_index: usize,
        /// Version ID of the completed patch
        version_id: String,
    },

    /// All patches in the batch were successfully applied
    AllCompleted,

    /// An error occurred during patch application
    Error {
        /// Human-readable error message
        message: String,
    },

    /// Operation was cancelled in response to Cancel request
    Cancelled,
}

/// Wire format for IPC messages - length-prefixed bincode
///
/// Message format:
/// - 4 bytes: message length (u32 big-endian)
/// - N bytes: bincode-serialized message
pub const MESSAGE_HEADER_SIZE: usize = 4;

/// Maximum message size to prevent memory exhaustion (16 MB)
pub const MAX_MESSAGE_SIZE: u32 = 16 * 1024 * 1024;

/// Serialize a message to bytes with length prefix
pub fn serialize_message<T: Serialize>(msg: &T) -> Result<Vec<u8>, bincode::Error> {
    let payload = bincode::serialize(msg)?;
    let len = payload.len() as u32;

    let mut buf = Vec::with_capacity(MESSAGE_HEADER_SIZE + payload.len());
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(&payload);

    Ok(buf)
}

/// Deserialize a message from bytes (without length prefix)
pub fn deserialize_message<T: for<'de> Deserialize<'de>>(data: &[u8]) -> Result<T, bincode::Error> {
    bincode::deserialize(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize_request() {
        let request = PatcherRequest::Hello { parent_pid: 12345 };
        let bytes = serialize_message(&request).unwrap();

        // Check length prefix
        let len = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert!(len > 0);
        assert_eq!(bytes.len(), MESSAGE_HEADER_SIZE + len as usize);

        // Deserialize payload (skip length prefix)
        let decoded: PatcherRequest = deserialize_message(&bytes[MESSAGE_HEADER_SIZE..]).unwrap();
        match decoded {
            PatcherRequest::Hello { parent_pid } => assert_eq!(parent_pid, 12345),
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn test_serialize_deserialize_response() {
        let response = PatcherResponse::Progress {
            patch_index: 0,
            total_patches: 5,
            version_id: "2024.07.23.0000.0001".to_string(),
            repository: Repository::Ffxiv,
            state: PatchState::Installing,
            bytes_processed: 1024,
            bytes_total: 4096,
        };

        let bytes = serialize_message(&response).unwrap();
        let decoded: PatcherResponse = deserialize_message(&bytes[MESSAGE_HEADER_SIZE..]).unwrap();

        match decoded {
            PatcherResponse::Progress {
                patch_index,
                total_patches,
                version_id,
                repository,
                state,
                bytes_processed,
                bytes_total,
            } => {
                assert_eq!(patch_index, 0);
                assert_eq!(total_patches, 5);
                assert_eq!(version_id, "2024.07.23.0000.0001");
                assert_eq!(repository, Repository::Ffxiv);
                assert_eq!(state, PatchState::Installing);
                assert_eq!(bytes_processed, 1024);
                assert_eq!(bytes_total, 4096);
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn test_serialize_start_patch() {
        let patches = vec![PatchEntry {
            version_id: "2024.07.23.0000.0001".to_string(),
            url: "http://example.com/patch.patch".to_string(),
            length: 1024,
            hash_type: Some("sha1".to_string()),
            hash_block_size: Some(1048576),
            hashes: Some(vec!["abc123".to_string()]),
            repository: Repository::Ffxiv,
        }];

        let request = PatcherRequest::StartPatch {
            patches,
            game_path: PathBuf::from("/home/user/ffxiv"),
            keep_patches: false,
        };

        let bytes = serialize_message(&request).unwrap();
        let decoded: PatcherRequest = deserialize_message(&bytes[MESSAGE_HEADER_SIZE..]).unwrap();

        match decoded {
            PatcherRequest::StartPatch {
                patches,
                game_path,
                keep_patches,
            } => {
                assert_eq!(patches.len(), 1);
                assert_eq!(patches[0].version_id, "2024.07.23.0000.0001");
                assert_eq!(game_path, PathBuf::from("/home/user/ffxiv"));
                assert!(!keep_patches);
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn test_all_request_variants() {
        // Test all variants can be serialized/deserialized
        let variants: Vec<PatcherRequest> = vec![
            PatcherRequest::Hello { parent_pid: 1 },
            PatcherRequest::StartPatch {
                patches: vec![],
                game_path: PathBuf::from("/test"),
                keep_patches: true,
            },
            PatcherRequest::Cancel,
            PatcherRequest::Shutdown,
        ];

        for request in variants {
            let bytes = serialize_message(&request).unwrap();
            let _: PatcherRequest = deserialize_message(&bytes[MESSAGE_HEADER_SIZE..]).unwrap();
        }
    }

    #[test]
    fn test_all_response_variants() {
        // Test all variants can be serialized/deserialized
        let variants: Vec<PatcherResponse> = vec![
            PatcherResponse::Ready,
            PatcherResponse::Progress {
                patch_index: 0,
                total_patches: 1,
                version_id: "test".to_string(),
                repository: Repository::Boot,
                state: PatchState::Pending,
                bytes_processed: 0,
                bytes_total: 100,
            },
            PatcherResponse::PatchCompleted {
                patch_index: 0,
                version_id: "test".to_string(),
            },
            PatcherResponse::AllCompleted,
            PatcherResponse::Error {
                message: "test error".to_string(),
            },
            PatcherResponse::Cancelled,
        ];

        for response in variants {
            let bytes = serialize_message(&response).unwrap();
            let _: PatcherResponse = deserialize_message(&bytes[MESSAGE_HEADER_SIZE..]).unwrap();
        }
    }
}
