use async_trait::async_trait;
use gaveloc_core::config::GameSettings;
use gaveloc_core::error::Error;
use gaveloc_core::ports::{LaunchConfig, ProcessLauncher};
use tokio::process::Command;
use tracing::{info, warn};

#[derive(Default)]
pub struct LinuxProcessLauncher;

impl LinuxProcessLauncher {
    pub fn new() -> Self {
        Self
    }
}

fn build_wrapper_args(settings: &GameSettings) -> Vec<String> {
    let mut wrappers = Vec::new();

    if settings.gamemode {
        wrappers.push("gamemoderun".to_string());
    }
    if settings.mangohud {
        wrappers.push("mangohud".to_string());
    }
    if settings.gamescope {
        wrappers.push("gamescope".to_string());

        let gs = &settings.gamescope_settings;
        if let Some(w) = gs.width {
            wrappers.push("-W".to_string());
            wrappers.push(w.to_string());
        }
        if let Some(h) = gs.height {
            wrappers.push("-H".to_string());
            wrappers.push(h.to_string());
        }
        if let Some(r) = gs.refresh_rate {
            wrappers.push("-r".to_string());
            wrappers.push(r.to_string());
        }
        if gs.fullscreen {
            wrappers.push("-f".to_string());
        }
        if gs.borderless {
            wrappers.push("-b".to_string());
        }
        if let Some(extra) = &gs.extra_args {
            if let Some(args) = shlex::split(extra) {
                wrappers.extend(args);
            }
        }

        wrappers.push("--".to_string());
    }

    wrappers
}

#[async_trait]
impl ProcessLauncher for LinuxProcessLauncher {
    async fn launch(&self, config: LaunchConfig<'_>) -> Result<(), Error> {
        let wrappers = build_wrapper_args(config.game_settings);

        info!(
            runner = %config.runner.name,
            prefix = %config.prefix_path.display(),
            game = %config.game_path.display(),
            "launching game"
        );

        let mut cmd = if wrappers.is_empty() {
            Command::new(&config.runner.path)
        } else {
            let mut cmd = Command::new(&wrappers[0]);
            for wrapper in &wrappers[1..] {
                cmd.arg(wrapper);
            }
            cmd.arg(&config.runner.path);
            cmd
        };

        // Wine environment
        cmd.env("WINEPREFIX", config.prefix_path);
        cmd.env("WINEARCH", "win64");

        if config.wine_settings.esync {
            cmd.env("WINEESYNC", "1");
        }
        if config.wine_settings.fsync {
            cmd.env("WINEFSYNC", "1");
        }
        if config.wine_settings.winesync {
            cmd.env("WINEFSYNC_FUTEX2", "1");
        }
        if let Some(hud) = &config.wine_settings.dxvk_hud {
            cmd.env("DXVK_HUD", hud);
        }

        // Game executable and arguments
        cmd.arg(config.game_path);

        // Use shlex for proper shell-like argument parsing (handles quotes, spaces)
        if let Some(args) = shlex::split(config.args) {
            for arg in args {
                cmd.arg(arg);
            }
        } else {
            // Fallback if parsing fails (malformed quotes)
            warn!("failed to parse launch arguments, using raw split");
            for arg in config.args.split_whitespace() {
                cmd.arg(arg);
            }
        }

        // Set working directory to game directory
        if let Some(parent) = config.game_path.parent() {
            cmd.current_dir(parent);
        }

        info!("spawning game process");
        cmd.spawn()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gaveloc_core::config::{GamescopeSettings, GameSettings};
    use rstest::{fixture, rstest};

    #[fixture]
    fn base_settings() -> GameSettings {
        GameSettings {
            gamemode: false,
            mangohud: false,
            gamescope: false,
            gamescope_settings: GamescopeSettings::default(),
            ..Default::default()
        }
    }

    #[rstest]
    fn test_wrapper_empty(base_settings: GameSettings) {
        assert!(build_wrapper_args(&base_settings).is_empty());
    }

    #[rstest]
    fn test_wrapper_gamemode(mut base_settings: GameSettings) {
        base_settings.gamemode = true;
        assert_eq!(build_wrapper_args(&base_settings), vec!["gamemoderun"]);
    }

    #[rstest]
    fn test_wrapper_mangohud(mut base_settings: GameSettings) {
        base_settings.mangohud = true;
        assert_eq!(build_wrapper_args(&base_settings), vec!["mangohud"]);
    }

    #[rstest]
    fn test_wrapper_multiple(mut base_settings: GameSettings) {
        base_settings.gamemode = true;
        base_settings.mangohud = true;
        assert_eq!(
            build_wrapper_args(&base_settings),
            vec!["gamemoderun", "mangohud"]
        );
    }

    #[rstest]
    fn test_wrapper_gamescope_basic(mut base_settings: GameSettings) {
        base_settings.gamescope = true;
        let args = build_wrapper_args(&base_settings);
        assert_eq!(args, vec!["gamescope", "--"]);
    }

    #[rstest]
    fn test_wrapper_gamescope_with_options(mut base_settings: GameSettings) {
        base_settings.gamescope = true;
        base_settings.gamescope_settings = GamescopeSettings {
            width: Some(1920),
            height: Some(1080),
            refresh_rate: Some(144),
            fullscreen: true,
            borderless: false,
            extra_args: Some("--rt".to_string()),
        };

        let args = build_wrapper_args(&base_settings);

        assert!(args.contains(&"gamescope".to_string()));

        // Helper to check sequence
        let check_flag = |flag: &str, val: &str| {
            let pos = args.iter().position(|x| x == flag).unwrap();
            assert_eq!(args[pos + 1], val);
        };

        check_flag("-W", "1920");
        check_flag("-H", "1080");
        check_flag("-r", "144");
        assert!(args.contains(&"-f".to_string()));
        assert!(args.contains(&"--rt".to_string()));
        assert_eq!(args.last().unwrap(), "--");
    }

    #[test]
    fn test_gamescope_full_args_snapshot() {
        let settings = GameSettings {
            gamemode: false,
            mangohud: false,
            gamescope: true,
            gamescope_settings: GamescopeSettings {
                width: Some(2560),
                height: Some(1440),
                refresh_rate: Some(165),
                fullscreen: true,
                borderless: false,
                extra_args: Some("--rt --hdr-enabled".to_string()),
            },
            ..Default::default()
        };
        insta::assert_yaml_snapshot!(build_wrapper_args(&settings));
    }

    #[test]
    fn test_all_wrappers_snapshot() {
        let settings = GameSettings {
            gamemode: true,
            mangohud: true,
            gamescope: true,
            gamescope_settings: GamescopeSettings {
                width: Some(1920),
                height: Some(1080),
                refresh_rate: Some(60),
                fullscreen: false,
                borderless: true,
                extra_args: None,
            },
            ..Default::default()
        };
        insta::assert_yaml_snapshot!(build_wrapper_args(&settings));
    }
}
