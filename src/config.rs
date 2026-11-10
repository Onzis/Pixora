use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::i18n::LanguageChoice;

fn default_hotkey_region() -> String {
    "PrintScreen".to_string()
}

fn default_hotkey_fullscreen() -> String {
    "Shift + PrintScreen".to_string()
}

fn default_hotkey_window() -> String {
    "Alt + PrintScreen".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub language: LanguageChoice,
    pub auto_copy_to_clipboard: bool,
    pub auto_save_file: bool,
    pub save_directory: PathBuf,
    pub filename_prefix: String,
    pub show_dimensions: bool,
    pub show_magnifier: bool,
    pub magnifier_zoom: u32,
    pub run_at_startup: bool,
    pub default_stroke_width: f32,
    pub default_color_hex: String,
    pub default_font_size: f32,
    pub sound_effects: bool,
    #[serde(default = "default_hotkey_region")]
    pub hotkey_region: String,
    #[serde(default = "default_hotkey_fullscreen")]
    pub hotkey_fullscreen: String,
    #[serde(default = "default_hotkey_window")]
    pub hotkey_window: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        let save_dir = dirs::picture_dir()
            .map(|p| p.join("Pixora"))
            .unwrap_or_else(|| PathBuf::from("."));

        Self {
            language: LanguageChoice::Auto,
            auto_copy_to_clipboard: true,
            auto_save_file: false,
            save_directory: save_dir,
            filename_prefix: "Pixora".to_string(),
            show_dimensions: true,
            show_magnifier: true,
            magnifier_zoom: 4,
            run_at_startup: false,
            default_stroke_width: 3.0,
            default_color_hex: "#FF453A".to_string(),
            default_font_size: 18.0,
            sound_effects: true,
            hotkey_region: default_hotkey_region(),
            hotkey_fullscreen: default_hotkey_fullscreen(),
            hotkey_window: default_hotkey_window(),
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .map(|p| p.join("Pixora").join("config.json"))
            .unwrap_or_else(|| PathBuf::from("pixora_config.json"))
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(config) = serde_json::from_str(&data) {
                return config;
            }
        }
        let config = Self::default();
        let _ = config.save();
        config
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}
