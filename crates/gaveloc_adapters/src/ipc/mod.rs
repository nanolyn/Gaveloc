//! IPC module for patcher process communication
//!
//! This module provides the infrastructure for the launcher to communicate
//! with the separate patcher process. The patcher runs as an isolated process
//! to ensure the UI remains responsive during patch application.

mod client;
mod protocol;

pub use client::UnixSocketPatcherIpc;
pub use protocol::{
    deserialize_message, serialize_message, PatcherRequest, PatcherResponse, MAX_MESSAGE_SIZE,
    MESSAGE_HEADER_SIZE,
};
