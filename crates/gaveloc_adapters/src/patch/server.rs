//! Square Enix patch server client implementation
//!
//! Handles communication with the FFXIV patch servers for version checking
//! and patch list retrieval.

use std::path::Path;

use async_trait::async_trait;
use reqwest::Client;
use tracing::instrument;

use gaveloc_core::entities::{GameVersion, PatchEntry, Repository};
use gaveloc_core::error::Error;
use gaveloc_core::ports::{PatchServer, VersionRepository};

use super::version::FileVersionRepository;
use crate::network::build_patch_client;

/// Boot version check URL
/// Format: http://patch-bootver.ffxiv.com/http/win32/ffxivneo_release_boot/{version}
const BOOT_VERSION_URL: &str = "http://patch-bootver.ffxiv.com/http/win32/ffxivneo_release_boot";

/// Game version check/session registration URL
/// Format: https://patch-gamever.ffxiv.com/http/win32/ffxivneo_release_game/{version}/{session_id}
const GAME_VERSION_URL: &str = "https://patch-gamever.ffxiv.com/http/win32/ffxivneo_release_game";

/// Square Enix patch server client
pub struct SquareEnixPatchServer {
    client: Client,
    version_repo: FileVersionRepository,
}

impl SquareEnixPatchServer {
    pub fn new() -> Result<Self, Error> {
        let client = build_patch_client()?;
        Ok(Self {
            client,
            version_repo: FileVersionRepository::new(),
        })
    }

    /// Parse the patch list response from the server
    ///
    /// Response format (one patch per line):
    /// ```text
    /// Content-Length: <size>
    /// Content-Type: text/plain
    ///
    /// <version_id>\t<url>\t<size>\t<hash_type>\t<hash_block_size>\t<hashes...>
    /// ```
    ///
    /// Or for no patches needed:
    /// ```text
    /// Content-Length: 0
    /// ```
    fn parse_patch_list(body: &str, repository: Repository) -> Result<Vec<PatchEntry>, Error> {
        let mut patches = Vec::new();

        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Parse tab-separated patch entry
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 3 {
                tracing::debug!("Skipping malformed patch line: {}", line);
                continue;
            }

            let version_id = parts[0].to_string();
            let url = parts[1].to_string();
            let length = parts[2]
                .parse::<u64>()
                .map_err(|_| Error::PatchServer(format!("Invalid patch size: {}", parts[2])))?;

            // Hash info is optional (boot patches may not have it)
            let (hash_type, hash_block_size, hashes) = if parts.len() >= 5 {
                let hash_type = Some(parts[3].to_string());
                let hash_block_size = parts[4].parse::<u64>().ok();
                let hashes = if parts.len() > 5 {
                    Some(parts[5..].iter().map(|s| s.to_string()).collect())
                } else {
                    None
                };
                (hash_type, hash_block_size, hashes)
            } else {
                (None, None, None)
            };

            patches.push(PatchEntry {
                version_id,
                url,
                length,
                hash_type,
                hash_block_size,
                hashes,
                repository,
            });
        }

        Ok(patches)
    }
}


#[async_trait]
impl PatchServer for SquareEnixPatchServer {
    #[instrument(skip(self))]
    async fn check_boot_version(
        &self,
        game_path: &Path,
        boot_version: &GameVersion,
    ) -> Result<Vec<PatchEntry>, Error> {
        // Get boot hash for verification
        let boot_hash = self.version_repo.get_boot_version_hash(game_path).await?;

        // Build request URL
        let url = format!("{}/{}", BOOT_VERSION_URL, boot_version.as_str());

        tracing::debug!("Checking boot version at: {}", url);

        // Make request with boot hash in header
        let response = self
            .client
            .get(&url)
            .header("X-Hash-Check", &boot_hash)
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        // Handle response status
        let status = response.status();
        if status.as_u16() == 204 {
            // No patches needed
            return Ok(Vec::new());
        }

        if !status.is_success() {
            return Err(Error::PatchServer(format!(
                "Boot version check failed with status: {}",
                status
            )));
        }

        // Parse patch list from response body
        let body = response
            .text()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        Self::parse_patch_list(&body, Repository::Boot)
    }

    #[instrument(skip(self, session_id))]
    async fn register_session(
        &self,
        session_id: &str,
        game_path: &Path,
        max_expansion: u32,
    ) -> Result<(String, Vec<PatchEntry>), Error> {
        // Get the game version
        let game_version = self
            .version_repo
            .get_version(game_path, Repository::Ffxiv)
            .await?;

        // Get version report for all expansion versions
        let version_report = self
            .version_repo
            .get_version_report(game_path, max_expansion)
            .await?;

        // Build request URL
        // Format: /http/win32/ffxivneo_release_game/{version}/{session_id}
        let url = format!(
            "{}/{}/{}",
            GAME_VERSION_URL,
            game_version.as_str(),
            session_id
        );

        tracing::debug!("Registering session at: {}", url);
        tracing::trace!("Version report:\n{}", version_report);

        // Make POST request with version report as body
        let response = self
            .client
            .post(&url)
            .header("X-Hash-Check", "enabled")
            .body(version_report)
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        // Get the unique ID from response headers
        let unique_id = response
            .headers()
            .get("X-Patch-Unique-Id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| session_id.to_string());

        // Handle response status
        let status = response.status();
        if status.as_u16() == 204 || status.as_u16() == 200 {
            // Check content length - 0 means no patches
            if let Some(content_length) = response.content_length() {
                if content_length == 0 {
                    return Ok((unique_id, Vec::new()));
                }
            }
        }

        if status.as_u16() == 409 {
            // Conflict - usually means game is being updated
            return Err(Error::PatchServer(
                "Game version conflict - server may be under maintenance".to_string(),
            ));
        }

        if !status.is_success() {
            return Err(Error::PatchServer(format!(
                "Session registration failed with status: {}",
                status
            )));
        }

        // Parse patch list from response body
        let body = response
            .text()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        if body.trim().is_empty() {
            return Ok((unique_id, Vec::new()));
        }

        // Parse patches - they may be for different repositories
        let mut all_patches = Vec::new();

        // The response groups patches by repository
        // We'll parse them all as FFXIV repository for now
        // and let the URL determine the actual target
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Determine repository from URL
            let repo = if line.contains("/ex1/") {
                Repository::Ex1
            } else if line.contains("/ex2/") {
                Repository::Ex2
            } else if line.contains("/ex3/") {
                Repository::Ex3
            } else if line.contains("/ex4/") {
                Repository::Ex4
            } else if line.contains("/ex5/") {
                Repository::Ex5
            } else {
                Repository::Ffxiv
            };

            let patches = Self::parse_patch_list(line, repo)?;
            all_patches.extend(patches);
        }

        Ok((unique_id, all_patches))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_patch_list_empty() {
        let patches = SquareEnixPatchServer::parse_patch_list("", Repository::Boot).unwrap();
        assert!(patches.is_empty());
    }

    #[test]
    fn test_parse_patch_list_single() {
        let line = "2024.07.23.0000.0001\thttp://example.com/patch.zip\t1024";
        let patches = SquareEnixPatchServer::parse_patch_list(line, Repository::Boot).unwrap();

        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0].version_id, "2024.07.23.0000.0001");
        assert_eq!(patches[0].url, "http://example.com/patch.zip");
        assert_eq!(patches[0].length, 1024);
        assert_eq!(patches[0].repository, Repository::Boot);
    }

    #[test]
    fn test_parse_patch_list_with_hashes() {
        let line =
            "2024.07.23.0000.0001\thttp://example.com/patch.zip\t1024\tsha1\t1048576\tabc123\tdef456";
        let patches = SquareEnixPatchServer::parse_patch_list(line, Repository::Ffxiv).unwrap();

        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0].hash_type, Some("sha1".to_string()));
        assert_eq!(patches[0].hash_block_size, Some(1048576));
        assert_eq!(
            patches[0].hashes,
            Some(vec!["abc123".to_string(), "def456".to_string()])
        );
    }

    #[test]
    fn test_parse_patch_list_multiple() {
        let body = "2024.07.23.0000.0001\thttp://example.com/patch1.zip\t1024
2024.07.24.0000.0000\thttp://example.com/patch2.zip\t2048";
        let patches = SquareEnixPatchServer::parse_patch_list(body, Repository::Ffxiv).unwrap();

        assert_eq!(patches.len(), 2);
        assert_eq!(patches[0].version_id, "2024.07.23.0000.0001");
        assert_eq!(patches[1].version_id, "2024.07.24.0000.0000");
    }

    #[test]
    fn test_square_enix_patch_server_new() {
        let server = SquareEnixPatchServer::new();
        assert!(server.is_ok());
    }

    #[test]
    fn test_parse_patch_list_skips_malformed() {
        // Line with less than 3 parts should be skipped
        let body = "malformed\tsingle\n2024.07.23.0000.0001\thttp://example.com/patch.zip\t1024";
        let patches = SquareEnixPatchServer::parse_patch_list(body, Repository::Boot).unwrap();

        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0].version_id, "2024.07.23.0000.0001");
    }

    #[test]
    fn test_parse_patch_list_handles_whitespace() {
        let body = "  \n  \n2024.07.23.0000.0001\thttp://example.com/patch.zip\t1024\n  ";
        let patches = SquareEnixPatchServer::parse_patch_list(body, Repository::Boot).unwrap();

        assert_eq!(patches.len(), 1);
    }

    #[test]
    fn test_parse_patch_list_expansion_repositories() {
        let line = "2024.07.23.0000.0001\thttp://example.com/patch.zip\t1024";

        // Test Ex1
        let patches = SquareEnixPatchServer::parse_patch_list(line, Repository::Ex1).unwrap();
        assert_eq!(patches[0].repository, Repository::Ex1);

        // Test Ex2
        let patches = SquareEnixPatchServer::parse_patch_list(line, Repository::Ex2).unwrap();
        assert_eq!(patches[0].repository, Repository::Ex2);

        // Test Ex3
        let patches = SquareEnixPatchServer::parse_patch_list(line, Repository::Ex3).unwrap();
        assert_eq!(patches[0].repository, Repository::Ex3);
    }
}
