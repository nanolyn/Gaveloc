use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Region {
    Japan,
    #[serde(rename = "northamerica")]
    NorthAmerica,
    #[default]
    Europe,
}

impl Region {
    pub fn as_id(self) -> u32 {
        match self {
            Region::Japan => 1,
            Region::NorthAmerica => 2,
            Region::Europe => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Japanese,
    #[default]
    English,
    German,
    French,
}

impl Language {
    pub fn as_id(self) -> u32 {
        match self {
            Language::Japanese => 0,
            Language::English => 1,
            Language::German => 2,
            Language::French => 3,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Settings {
    pub game: GameSettings,
    pub wine: WineSettings,
    pub log_level: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct GamescopeSettings {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub refresh_rate: Option<u32>,
    pub fullscreen: bool,
    pub borderless: bool,
    pub extra_args: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GameSettings {
    pub path: Option<PathBuf>,
    pub region: Region,
    pub language: Language,
    pub gamemode: bool,
    pub mangohud: bool,
    pub gamescope: bool,
    #[serde(default)]
    pub gamescope_settings: GamescopeSettings,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WineSettings {
    pub runner_path: Option<PathBuf>,
    pub prefix_path: Option<PathBuf>,
    pub esync: bool,
    pub fsync: bool,
    pub winesync: bool,
    pub dxvk_hud: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            game: GameSettings::default(),
            wine: WineSettings::default(),
            log_level: "info".to_string(),
        }
    }
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            path: None,
            region: Region::default(),
            language: Language::default(),
            gamemode: true,
            mangohud: false,
            gamescope: false,
            gamescope_settings: GamescopeSettings::default(),
        }
    }
}

impl Default for WineSettings {
    fn default() -> Self {
        Self {
            runner_path: None,
            prefix_path: None,
            esync: true,
            fsync: true,
            winesync: false,
            dxvk_hud: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(Region::Japan, 1)]
    #[case(Region::NorthAmerica, 2)]
    #[case(Region::Europe, 3)]
    fn test_region_ids(#[case] region: Region, #[case] expected: u32) {
        assert_eq!(region.as_id(), expected);
    }

    #[rstest]
    #[case(Language::Japanese, 0)]
    #[case(Language::English, 1)]
    #[case(Language::German, 2)]
    #[case(Language::French, 3)]
    fn test_language_ids(#[case] language: Language, #[case] expected: u32) {
        assert_eq!(language.as_id(), expected);
    }

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();

        assert_eq!(settings.log_level, "info");

        // Game defaults
        assert_eq!(settings.game.region, Region::Europe);
        assert_eq!(settings.game.language, Language::English);
        assert!(settings.game.gamemode);
        assert!(!settings.game.mangohud);
        assert!(!settings.game.gamescope);

        // Wine defaults
        assert!(settings.wine.esync);
        assert!(settings.wine.fsync);
        assert!(!settings.wine.winesync);
        assert_eq!(settings.wine.dxvk_hud, None);
    }

    #[test]
    fn test_default_settings_snapshot() {
        let settings = Settings::default();
        insta::assert_yaml_snapshot!(settings);
    }
}
