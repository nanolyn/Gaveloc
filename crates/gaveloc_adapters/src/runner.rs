use std::path::{Path, PathBuf};

use async_trait::async_trait;
use gaveloc_core::entities::{RunnerType, WineRunner};
use gaveloc_core::ports::{RunnerDetector, RunnerManager};
use gaveloc_core::Error;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::info;

const WINE_SUBPATHS: &[&str] = &["files/bin/wine", "dist/bin/wine", "bin/wine"];

const STEAM_ROOTS: &[&str] = &[
    "~/.steam/root",
    "~/.steam/steam",
    "~/.local/share/Steam",
    "~/.var/app/com.valvesoftware.Steam/.local/share/Steam",
];

fn expand_home(path_str: &str) -> Option<PathBuf> {
    if let Some(suffix) = path_str.strip_prefix('~') {
        let suffix = suffix.strip_prefix('/').unwrap_or(suffix);

        // Try directories crate first
        if let Some(base_dirs) = directories::BaseDirs::new() {
            return Some(base_dirs.home_dir().join(suffix));
        }

        // Fallback to $HOME env var for headless environments
        if let Ok(home) = std::env::var("HOME") {
            return Some(PathBuf::from(home).join(suffix));
        }

        None
    } else {
        Some(PathBuf::from(path_str))
    }
}

async fn find_wine_binary(base_path: &Path) -> Option<PathBuf> {
    for subpath in WINE_SUBPATHS {
        let wine_path = base_path.join(subpath);
        if fs::try_exists(&wine_path).await.unwrap_or(false) {
            return Some(wine_path);
        }
    }
    None
}

/// Parses Steam's libraryfolders.vdf to extract library paths.
/// Handles quoted strings, comments, and nested structures.
fn parse_library_paths_from_vdf(content: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        // Look for "path" key-value pairs
        // Format: "path"		"/path/to/library"
        if !trimmed.starts_with("\"path\"") {
            continue;
        }

        // Parse quoted strings from the line
        let quotes: Vec<&str> = trimmed.split('"').collect();
        // Expected: ["", "path", separator, "value", ...]
        // Index 3 should be the path value
        if quotes.len() >= 4 {
            let path_value = quotes[3];
            // Handle escaped backslashes (common in Windows paths stored in VDF)
            let unescaped = path_value.replace("\\\\", "\\");
            if !unescaped.is_empty() {
                paths.push(PathBuf::from(unescaped));
            }
        }
    }

    paths
}

#[derive(Default)]
pub struct LinuxRunnerDetector;

impl LinuxRunnerDetector {
    pub fn new() -> Self {
        Self
    }

    async fn detect_system_wine_at_path(&self, wine_path: &Path) -> Option<WineRunner> {
        if fs::try_exists(wine_path).await.unwrap_or(false) {
            Some(WineRunner {
                path: wine_path.to_path_buf(),
                name: "System Wine".to_string(),
                runner_type: RunnerType::System,
                is_valid: true,
            })
        } else {
            None
        }
    }

    async fn detect_system_wine(&self) -> Option<WineRunner> {
        self.detect_system_wine_at_path(&PathBuf::from("/usr/bin/wine")).await
    }

    async fn detect_lutris_runners(&self) -> Vec<WineRunner> {
        let mut runners = Vec::new();
        let Some(path) = expand_home("~/.local/share/lutris/runners/wine") else {
            return runners;
        };

        let Ok(mut entries) = fs::read_dir(&path).await else {
            return runners;
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let wine_path = entry.path().join("bin/wine");
            if fs::try_exists(&wine_path).await.unwrap_or(false) {
                let name = entry.file_name().to_string_lossy().to_string();
                runners.push(WineRunner {
                    path: wine_path,
                    name: format!("Lutris - {}", name),
                    runner_type: RunnerType::Lutris,
                    is_valid: true,
                });
            }
        }
        runners
    }

    async fn get_steam_library_paths(&self, steam_root: &Path) -> Vec<PathBuf> {
        let mut paths = vec![steam_root.to_path_buf()];

        let vdf_path = steam_root.join("steamapps/libraryfolders.vdf");
        if let Ok(content) = fs::read_to_string(&vdf_path).await {
            paths.extend(parse_library_paths_from_vdf(&content));
        }
        paths
    }

    async fn detect_proton_runners(&self) -> Vec<WineRunner> {
        let mut runners = Vec::new();
        let mut search_paths = Vec::new();

        for root_str in STEAM_ROOTS {
            let Some(root) = expand_home(root_str) else {
                continue;
            };
            if !fs::try_exists(&root).await.unwrap_or(false) {
                continue;
            }

            for lib in self.get_steam_library_paths(&root).await {
                search_paths.push(lib.join("steamapps/common"));
                search_paths.push(lib.join("compatibilitytools.d"));
            }
        }

        for path in search_paths {
            let Ok(mut entries) = fs::read_dir(&path).await else {
                continue;
            };

            while let Ok(Some(entry)) = entries.next_entry().await {
                let proton_exec = entry.path().join("proton");
                if !fs::try_exists(&proton_exec).await.unwrap_or(false) {
                    continue;
                }

                if let Some(wine_path) = find_wine_binary(&entry.path()).await {
                    let name = entry.file_name().to_string_lossy().to_string();
                    runners.push(WineRunner {
                        path: wine_path,
                        name: format!("Proton - {}", name),
                        runner_type: RunnerType::Proton,
                        is_valid: true,
                    });
                }
            }
        }

        runners.sort_by(|a, b| a.path.cmp(&b.path));
        runners.dedup_by(|a, b| a.path == b.path);
        runners
    }

    async fn detect_gaveloc_runners(&self) -> Vec<WineRunner> {
        let mut runners = Vec::new();
        let Some(path) = expand_home("~/.local/share/gaveloc/runners") else {
            return runners;
        };

        let Ok(mut entries) = fs::read_dir(&path).await else {
            return runners;
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Some(wine_path) = find_wine_binary(&entry.path()).await {
                let name = entry.file_name().to_string_lossy().to_string();
                runners.push(WineRunner {
                    path: wine_path,
                    name: format!("Gaveloc - {}", name),
                    runner_type: RunnerType::GavelocManaged,
                    is_valid: true,
                });
            }
        }
        runners
    }

    async fn detect_heroic_runners(&self) -> Vec<WineRunner> {
        let mut runners = Vec::new();
        let Some(root) = expand_home("~/.config/heroic/tools") else {
            return runners;
        };

        let wine_root = root.join("wine");
        if let Ok(mut entries) = fs::read_dir(&wine_root).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let wine_bin = entry.path().join("bin/wine");
                if fs::try_exists(&wine_bin).await.unwrap_or(false) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    runners.push(WineRunner {
                        path: wine_bin,
                        name: format!("Heroic (Wine) - {}", name),
                        runner_type: RunnerType::Custom,
                        is_valid: true,
                    });
                }
            }
        }

        let proton_root = root.join("proton");
        if let Ok(mut entries) = fs::read_dir(&proton_root).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let proton_exists = fs::try_exists(entry.path().join("proton"))
                    .await
                    .unwrap_or(false);
                if !proton_exists {
                    continue;
                }

                if let Some(wine_path) = find_wine_binary(&entry.path()).await {
                    let name = entry.file_name().to_string_lossy().to_string();
                    runners.push(WineRunner {
                        path: wine_path,
                        name: format!("Heroic (Proton) - {}", name),
                        runner_type: RunnerType::Proton,
                        is_valid: true,
                    });
                }
            }
        }
        runners
    }

    async fn detect_bottles_runners(&self) -> Vec<WineRunner> {
        let mut runners = Vec::new();
        let Some(root) = expand_home("~/.local/share/bottles/runners") else {
            return runners;
        };

        let Ok(mut type_entries) = fs::read_dir(&root).await else {
            return runners;
        };

        while let Ok(Some(type_entry)) = type_entries.next_entry().await {
            let is_dir = type_entry
                .file_type()
                .await
                .map(|t| t.is_dir())
                .unwrap_or(false);
            if !is_dir {
                continue;
            }

            let type_name = type_entry.file_name().to_string_lossy().to_string();
            let Ok(mut runner_entries) = fs::read_dir(type_entry.path()).await else {
                continue;
            };

            while let Ok(Some(runner_entry)) = runner_entries.next_entry().await {
                let wine_bin = runner_entry.path().join("bin/wine");
                if fs::try_exists(&wine_bin).await.unwrap_or(false) {
                    let name = runner_entry.file_name().to_string_lossy().to_string();
                    runners.push(WineRunner {
                        path: wine_bin,
                        name: format!("Bottles ({}) - {}", type_name, name),
                        runner_type: RunnerType::Custom,
                        is_valid: true,
                    });
                }
            }
        }
        runners
    }
}

#[async_trait]
impl RunnerDetector for LinuxRunnerDetector {
    async fn detect_runners(&self) -> Result<Vec<WineRunner>, Error> {
        let mut runners = Vec::new();

        if let Some(system) = self.detect_system_wine().await {
            runners.push(system);
        }

        runners.extend(self.detect_gaveloc_runners().await);
        runners.extend(self.detect_lutris_runners().await);
        runners.extend(self.detect_proton_runners().await);
        runners.extend(self.detect_heroic_runners().await);
        runners.extend(self.detect_bottles_runners().await);

        Ok(runners)
    }

    async fn validate_runner(&self, path: PathBuf) -> Result<WineRunner, Error> {
        if fs::try_exists(&path).await? {
            Ok(WineRunner {
                path,
                name: "Custom Runner".to_string(),
                runner_type: RunnerType::Custom,
                is_valid: true,
            })
        } else {
            Err(Error::RunnerNotFound(path))
        }
    }
}

#[derive(Default)]
pub struct LinuxRunnerManager;

impl LinuxRunnerManager {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serial_test::serial;
    use tempfile::tempdir;

    #[test]
    fn test_runner_detector_new() {
        let _ = LinuxRunnerDetector::new();
    }

    #[test]
    fn test_runner_manager_new() {
        let _ = LinuxRunnerManager::new();
    }

    #[serial]
    #[test]
    fn test_expand_home() {
        // Test with no expansion needed
        assert_eq!(
            expand_home("/absolute/path"),
            Some(PathBuf::from("/absolute/path"))
        );

        // Test with ~ expansion, HOME env var set
        std::env::set_var("HOME", "/tmp/home_test");
        assert_eq!(
            expand_home("~/relative/path"),
            Some(PathBuf::from("/tmp/home_test/relative/path"))
        );
        std::env::remove_var("HOME");
    }

    // Parameterized VDF parsing tests
    #[rstest]
    #[case("", vec![])]
    #[case("// comment only", vec![])]
    #[case(r#""key"         "value""#, vec![])]
    #[case(r#""path"         C:\path\to\broken"#, vec![])]
    #[case(r#""path"		"/home/steam""#, vec!["/home/steam"])]
    #[case(r#""path"		"C:\\Program Files (x86)\\SteamLibrary""#, vec!["C:\\Program Files (x86)\\SteamLibrary"])]
    fn test_parse_vdf_cases(#[case] input: &str, #[case] expected: Vec<&str>) {
        let paths = parse_library_paths_from_vdf(input);
        let expected: Vec<PathBuf> = expected.into_iter().map(PathBuf::from).collect();
        assert_eq!(paths, expected);
    }

    #[test]
    fn test_parse_vdf_multiple_paths() {
        let content = r#"
        // This is a comment

        "path"		"/path/to/library1"
        // Another comment
        "path"         "/path/to/library2"
        "#;
        let paths = parse_library_paths_from_vdf(content);
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&PathBuf::from("/path/to/library1")));
        assert!(paths.contains(&PathBuf::from("/path/to/library2")));
    }

    #[tokio::test]
    #[serial]
    async fn test_detect_system_wine_found() {
        let dir = tempdir().unwrap();
        let wine_bin_path = dir.path().join("wine");
        std::fs::File::create(&wine_bin_path).unwrap();

        let detector = LinuxRunnerDetector::new();
        let result = detector.detect_system_wine_at_path(&wine_bin_path).await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "System Wine");
    }

    #[tokio::test]
    #[serial]
    async fn test_detect_system_wine_not_found() {
        let detector = LinuxRunnerDetector::new();
        let result = detector
            .detect_system_wine_at_path(&PathBuf::from("/nonexistent/wine"))
            .await;
        assert!(result.is_none());
    }

    // Helper to create fake wine binary
    fn create_wine_binary(dir: &Path) {
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::File::create(dir.join("bin/wine")).unwrap();
    }

    // Helper to create proton structure
    fn create_proton_structure(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::File::create(dir.join("proton")).unwrap();
        std::fs::create_dir_all(dir.join("files/bin")).unwrap();
        std::fs::File::create(dir.join("files/bin/wine")).unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_detect_lutris_runners() {
        let dir = tempdir().unwrap();
        let lutris_path = dir.path().join(".local/share/lutris/runners/wine");

        create_wine_binary(&lutris_path.join("wine-ge-8-26"));
        create_wine_binary(&lutris_path.join("wine-staging-9.0"));

        std::env::set_var("HOME", dir.path());
        let detector = LinuxRunnerDetector::new();
        let runners = detector.detect_lutris_runners().await;
        std::env::remove_var("HOME");

        assert_eq!(runners.len(), 2);
        assert!(runners.iter().all(|r| r.name.contains("Lutris")));
        assert!(runners.iter().any(|r| r.name.contains("wine-ge-8-26")));
    }

    #[tokio::test]
    #[serial]
    async fn test_detect_lutris_runners_empty_dir() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".local/share/lutris/runners/wine")).unwrap();

        std::env::set_var("HOME", dir.path());
        let detector = LinuxRunnerDetector::new();
        let runners = detector.detect_lutris_runners().await;
        std::env::remove_var("HOME");

        assert!(runners.is_empty());
    }

    #[tokio::test]
    #[serial]
    async fn test_detect_gaveloc_runners() {
        let dir = tempdir().unwrap();
        let gaveloc_path = dir.path().join(".local/share/gaveloc/runners");

        create_proton_structure(&gaveloc_path.join("GE-Proton9-1"));

        std::env::set_var("HOME", dir.path());
        let detector = LinuxRunnerDetector::new();
        let runners = detector.detect_gaveloc_runners().await;
        std::env::remove_var("HOME");

        assert_eq!(runners.len(), 1);
        assert!(runners[0].name.contains("Gaveloc"));
        assert_eq!(runners[0].runner_type, RunnerType::GavelocManaged);
    }

    #[tokio::test]
    #[serial]
    async fn test_detect_heroic_wine_runners() {
        let dir = tempdir().unwrap();
        let heroic_wine = dir.path().join(".config/heroic/tools/wine");

        create_wine_binary(&heroic_wine.join("Wine-GE-Proton8-25"));

        std::env::set_var("HOME", dir.path());
        let detector = LinuxRunnerDetector::new();
        let runners = detector.detect_heroic_runners().await;
        std::env::remove_var("HOME");

        assert_eq!(runners.len(), 1);
        assert!(runners[0].name.contains("Heroic (Wine)"));
    }

    #[tokio::test]
    #[serial]
    async fn test_detect_heroic_proton_runners() {
        let dir = tempdir().unwrap();
        let heroic_proton = dir.path().join(".config/heroic/tools/proton");

        create_proton_structure(&heroic_proton.join("GE-Proton9-5"));

        std::env::set_var("HOME", dir.path());
        let detector = LinuxRunnerDetector::new();
        let runners = detector.detect_heroic_runners().await;
        std::env::remove_var("HOME");

        assert_eq!(runners.len(), 1);
        assert!(runners[0].name.contains("Heroic (Proton)"));
        assert_eq!(runners[0].runner_type, RunnerType::Proton);
    }

    #[tokio::test]
    #[serial]
    async fn test_detect_bottles_runners() {
        let dir = tempdir().unwrap();
        let bottles_path = dir.path().join(".local/share/bottles/runners");

        // Bottles organizes by type (wine, proton, etc.)
        create_wine_binary(&bottles_path.join("wine/caffe-9.5"));
        create_wine_binary(&bottles_path.join("proton/ge-proton-9-7"));

        std::env::set_var("HOME", dir.path());
        let detector = LinuxRunnerDetector::new();
        let runners = detector.detect_bottles_runners().await;
        std::env::remove_var("HOME");

        assert_eq!(runners.len(), 2);
        assert!(runners.iter().any(|r| r.name.contains("Bottles (wine)")));
        assert!(runners.iter().any(|r| r.name.contains("Bottles (proton)")));
    }

    #[tokio::test]
    #[serial]
    async fn test_validate_runner_success() {
        let dir = tempdir().unwrap();
        let wine_path = dir.path().join("wine");
        std::fs::File::create(&wine_path).unwrap();

        let detector = LinuxRunnerDetector::new();
        let result = detector.validate_runner(wine_path.clone()).await;

        assert!(result.is_ok());
        let runner = result.unwrap();
        assert_eq!(runner.path, wine_path);
        assert_eq!(runner.runner_type, RunnerType::Custom);
    }

    #[tokio::test]
    async fn test_validate_runner_not_found() {
        let detector = LinuxRunnerDetector::new();
        let result = detector
            .validate_runner(PathBuf::from("/nonexistent/wine"))
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    #[serial]
    async fn test_find_wine_binary_files_bin() {
        let dir = tempdir().unwrap();

        // Test files/bin/wine path (common in Proton)
        std::fs::create_dir_all(dir.path().join("files/bin")).unwrap();
        std::fs::File::create(dir.path().join("files/bin/wine")).unwrap();

        let result = find_wine_binary(dir.path()).await;
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("files/bin/wine"));
    }

    #[tokio::test]
    #[serial]
    async fn test_find_wine_binary_dist_bin() {
        let dir = tempdir().unwrap();

        // Test dist/bin/wine path
        std::fs::create_dir_all(dir.path().join("dist/bin")).unwrap();
        std::fs::File::create(dir.path().join("dist/bin/wine")).unwrap();

        let result = find_wine_binary(dir.path()).await;
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("dist/bin/wine"));
    }

    #[tokio::test]
    #[serial]
    async fn test_find_wine_binary_bin() {
        let dir = tempdir().unwrap();

        // Test bin/wine path
        std::fs::create_dir_all(dir.path().join("bin")).unwrap();
        std::fs::File::create(dir.path().join("bin/wine")).unwrap();

        let result = find_wine_binary(dir.path()).await;
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("bin/wine"));
    }

    #[tokio::test]
    async fn test_find_wine_binary_not_found() {
        let dir = tempdir().unwrap();
        let result = find_wine_binary(dir.path()).await;
        assert!(result.is_none());
    }
}

#[derive(serde::Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(serde::Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[async_trait]
impl RunnerManager for LinuxRunnerManager {
    async fn install_latest_ge_proton(&self) -> Result<WineRunner, Error> {
        let client = reqwest::Client::new();
        let url = "https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases/latest";

        info!("fetching latest GE-Proton release info");
        let resp = client
            .get(url)
            .header("User-Agent", "Gaveloc")
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?
            .json::<GithubRelease>()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        info!(tag = %resp.tag_name, "found latest release");

        let asset = resp
            .assets
            .iter()
            .find(|a| a.name.ends_with(".tar.gz"))
            .ok_or_else(|| Error::Other("no .tar.gz asset found in the latest release".into()))?;

        info!(asset = %asset.name, "downloading");

        let mut response = client
            .get(&asset.browser_download_url)
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        let temp_dir = std::env::temp_dir();
        let tarball_path = temp_dir.join(&asset.name);

        let mut file = fs::File::create(&tarball_path).await?;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| Error::Network(e.to_string()))?
        {
            file.write_all(&chunk).await?;
        }

        info!("download complete, extracting");

        let install_dir =
            expand_home("~/.local/share/gaveloc/runners").ok_or(Error::HomeDirectoryNotFound)?;

        fs::create_dir_all(&install_dir).await?;

        let tarball_path_clone = tarball_path.clone();
        let install_dir_clone = install_dir.clone();
        tokio::task::spawn_blocking(move || {
            let tar_gz = std::fs::File::open(&tarball_path_clone)?;
            let tar = flate2::read::GzDecoder::new(tar_gz);
            let mut archive = tar::Archive::new(tar);
            archive.unpack(&install_dir_clone)?;
            Ok::<_, std::io::Error>(())
        })
        .await
        .map_err(|e| Error::Other(e.to_string()))??;

        info!("extraction complete");

        let _ = fs::remove_file(&tarball_path).await;

        let folder_name = asset.name.trim_end_matches(".tar.gz");
        let extracted_path = install_dir.join(folder_name);

        find_wine_binary(&extracted_path)
            .await
            .map(|bin| WineRunner {
                path: bin,
                name: format!("Gaveloc - {}", folder_name),
                runner_type: RunnerType::GavelocManaged,
                is_valid: true,
            })
            .ok_or(Error::WineBinaryNotFound(extracted_path))
    }
}
