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

fn default_ocr_language() -> String {
    "auto".to_string()
}

fn default_stroke_width() -> f32 {
    6.0
}

fn default_font_size() -> f32 {
    18.0
}

fn default_blur_radius() -> u32 {
    12
}

fn default_pixelate_block() -> u32 {
    10
}

fn default_color_hex() -> String {
    "#FF453A".to_string()
}

fn default_auto_check_updates() -> bool {
    true
}

fn default_window_snapping() -> bool {
    true
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
    #[serde(default = "default_stroke_width")]
    pub default_stroke_width: f32,
    #[serde(default = "default_color_hex")]
    pub default_color_hex: String,
    #[serde(default = "default_font_size")]
    pub default_font_size: f32,
    #[serde(default = "default_blur_radius")]
    pub blur_radius: u32,
    #[serde(default = "default_pixelate_block")]
    pub pixelate_block: u32,
    pub sound_effects: bool,
    #[serde(default = "default_hotkey_region")]
    pub hotkey_region: String,
    #[serde(default = "default_hotkey_fullscreen")]
    pub hotkey_fullscreen: String,
    #[serde(default = "default_hotkey_window")]
    pub hotkey_window: String,
    #[serde(default = "default_ocr_language")]
    pub ocr_language: String,
    #[serde(default = "default_auto_check_updates")]
    pub auto_check_updates: bool,
    #[serde(default = "default_window_snapping")]
    pub window_snapping: bool,
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
            show_magnifier: false,
            magnifier_zoom: 4,
            run_at_startup: false,
            default_stroke_width: default_stroke_width(),
            default_color_hex: default_color_hex(),
            default_font_size: default_font_size(),
            blur_radius: default_blur_radius(),
            pixelate_block: default_pixelate_block(),
            sound_effects: true,
            hotkey_region: default_hotkey_region(),
            hotkey_fullscreen: default_hotkey_fullscreen(),
            hotkey_window: default_hotkey_window(),
            ocr_language: default_ocr_language(),
            auto_check_updates: default_auto_check_updates(),
            window_snapping: default_window_snapping(),
        }
    }
}

impl AppConfig {
    pub fn adjacent_config_path() -> Option<PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let dir = exe.parent()?;
        Some(dir.join("config.json"))
    }

    pub fn appdata_config_path() -> PathBuf {
        dirs::config_dir()
            .map(|p| p.join("Pixora").join("config.json"))
            .unwrap_or_else(|| PathBuf::from("config.json"))
    }

    pub fn config_path() -> PathBuf {
        if let Some(adj) = Self::adjacent_config_path() {
            return adj;
        }
        Self::appdata_config_path()
    }

    pub fn load() -> Self {
        let adjacent_opt = Self::adjacent_config_path();
        let appdata_path = Self::appdata_config_path();

        // 1. Check if config.json exists next to exe
        if let Some(ref adj) = adjacent_opt {
            if adj.exists() {
                if let Ok(data) = std::fs::read_to_string(adj) {
                    if let Ok(config) = serde_json::from_str(&data) {
                        return config;
                    }
                }
            }
        }

        // 2. Migration fallback: check if config.json exists in AppData
        if appdata_path.exists() {
            if let Ok(data) = std::fs::read_to_string(&appdata_path) {
                if let Ok(config) = serde_json::from_str::<AppConfig>(&data) {
                    let _ = config.save();
                    return config;
                }
            }
        }

        // 3. New default config
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
        std::fs::write(&path, &json)?;

        // Also keep appdata in sync
        let appdata = Self::appdata_config_path();
        if path != appdata {
            if let Some(parent) = appdata.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&appdata, &json);
        }

        Ok(())
    }
}
