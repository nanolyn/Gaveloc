//! HTTP patch downloader implementation
//!
//! Downloads patch files with progress reporting and hash verification.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use sha1::{Digest, Sha1};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::instrument;

use gaveloc_core::entities::PatchEntry;
use gaveloc_core::error::Error;
use gaveloc_core::ports::PatchDownloader;

use crate::network::build_patch_client;

/// HTTP-based patch downloader
pub struct HttpPatchDownloader {
    client: Client,
}

impl HttpPatchDownloader {
    pub fn new() -> Result<Self, Error> {
        let client = build_patch_client()?;
        Ok(Self { client })
    }

    /// Verify a downloaded file against block hashes
    async fn verify_file_blocks(
        file_path: &Path,
        hashes: &[String],
        block_size: u64,
    ) -> Result<bool, Error> {
        let file_data = tokio::fs::read(file_path).await?;
        let mut current_offset = 0;
        let mut block_index = 0;

        while current_offset < file_data.len() && block_index < hashes.len() {
            let end = (current_offset + block_size as usize).min(file_data.len());
            let block = &file_data[current_offset..end];

            let mut hasher = Sha1::new();
            hasher.update(block);
            let actual_hash = hex::encode(hasher.finalize());

            if actual_hash != hashes[block_index] {
                tracing::warn!(
                    "Block {} hash mismatch: expected {}, got {}",
                    block_index,
                    hashes[block_index],
                    actual_hash
                );
                return Ok(false);
            }

            current_offset = end;
            block_index += 1;
        }

        Ok(true)
    }
}


#[async_trait]
impl PatchDownloader for HttpPatchDownloader {
    #[instrument(skip(self, progress))]
    async fn download_patch<F>(
        &self,
        patch: &PatchEntry,
        dest_path: &Path,
        unique_id: Option<&str>,
        progress: F,
    ) -> Result<(), Error>
    where
        F: Fn(u64, u64) + Send + Sync + 'static,
    {
        let progress = Arc::new(progress);

        tracing::info!(
            "Downloading patch {} ({} bytes)",
            patch.version_id,
            patch.length
        );

        // Build request
        let mut request = self.client.get(&patch.url);

        // Add unique ID header if provided (required for some patch downloads)
        if let Some(uid) = unique_id {
            request = request.header("X-Patch-Unique-Id", uid);
        }

        let response = request
            .send()
            .await
            .map_err(|e| Error::PatchDownload(e.to_string()))?;

        if !response.status().is_success() {
            return Err(Error::PatchDownload(format!(
                "Download failed with status: {}",
                response.status()
            )));
        }

        // Get content length from response or use patch length
        let total_size = response.content_length().unwrap_or(patch.length);

        // Create destination file
        if let Some(parent) = dest_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut file = File::create(dest_path).await?;

        // Download with progress reporting
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| Error::PatchDownload(e.to_string()))?;

            file.write_all(&chunk).await?;

            downloaded += chunk.len() as u64;
            progress(downloaded, total_size);
        }

        file.flush().await?;

        tracing::info!("Download complete: {}", patch.version_id);

        Ok(())
    }

    #[instrument(skip(self))]
    async fn verify_patch(&self, patch: &PatchEntry, file_path: &Path) -> Result<bool, Error> {
        // Check file exists
        if !file_path.exists() {
            tracing::warn!("Patch file does not exist: {:?}", file_path);
            return Ok(false);
        }

        // Check file size
        let metadata = tokio::fs::metadata(file_path).await?;
        if metadata.len() != patch.length {
            tracing::warn!(
                "Patch file size mismatch: expected {}, got {}",
                patch.length,
                metadata.len()
            );
            return Ok(false);
        }

        // If we have block hashes, verify them
        if let (Some(hashes), Some(block_size)) = (&patch.hashes, patch.hash_block_size) {
            return Self::verify_file_blocks(file_path, hashes, block_size).await;
        }

        // No hashes available, assume OK if size matches
        tracing::debug!("No block hashes available, size check passed");
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::fs::write;

    #[tokio::test]
    async fn test_verify_patch_size_check() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.patch");

        // Create a file with known content
        let content = b"test patch content";
        write(&file_path, content).await.unwrap();

        let downloader = HttpPatchDownloader::new().unwrap();

        // Correct size should pass
        let patch = PatchEntry {
            version_id: "test".to_string(),
            url: "http://example.com".to_string(),
            length: content.len() as u64,
            hash_type: None,
            hash_block_size: None,
            hashes: None,
            repository: gaveloc_core::entities::Repository::Boot,
        };
        assert!(downloader.verify_patch(&patch, &file_path).await.unwrap());

        // Wrong size should fail
        let patch_wrong_size = PatchEntry {
            length: content.len() as u64 + 100,
            ..patch
        };
        assert!(!downloader
            .verify_patch(&patch_wrong_size, &file_path)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_verify_patch_with_hashes() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.patch");

        // Create a file with known content
        let content = b"test patch content";
        write(&file_path, content).await.unwrap();

        // Calculate expected hash
        let mut hasher = Sha1::new();
        hasher.update(content);
        let expected_hash = hex::encode(hasher.finalize());

        let downloader = HttpPatchDownloader::new().unwrap();

        // Correct hash should pass
        let patch = PatchEntry {
            version_id: "test".to_string(),
            url: "http://example.com".to_string(),
            length: content.len() as u64,
            hash_type: Some("sha1".to_string()),
            hash_block_size: Some(1048576), // 1MB blocks
            hashes: Some(vec![expected_hash.clone()]),
            repository: gaveloc_core::entities::Repository::Boot,
        };
        assert!(downloader.verify_patch(&patch, &file_path).await.unwrap());

        // Wrong hash should fail
        let patch_wrong_hash = PatchEntry {
            hashes: Some(vec!["wronghash".to_string()]),
            ..patch
        };
        assert!(!downloader
            .verify_patch(&patch_wrong_hash, &file_path)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_verify_patch_missing_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("nonexistent.patch");

        let downloader = HttpPatchDownloader::new().unwrap();
        let patch = PatchEntry {
            version_id: "test".to_string(),
            url: "http://example.com".to_string(),
            length: 100,
            hash_type: None,
            hash_block_size: None,
            hashes: None,
            repository: gaveloc_core::entities::Repository::Boot,
        };

        assert!(!downloader.verify_patch(&patch, &file_path).await.unwrap());
    }
}
