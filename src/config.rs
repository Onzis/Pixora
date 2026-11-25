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

fn default_spotlight_dim() -> f32 {
    0.60
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

fn default_filename_pattern() -> String {
    "Pixora_%Y-%m-%d_%H-%M-%S".to_string()
}

fn default_image_format() -> String {
    "png".to_string()
}

fn default_jpeg_quality() -> u8 {
    90
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub language: LanguageChoice,
    pub auto_copy_to_clipboard: bool,
    pub auto_save_file: bool,
    pub save_directory: PathBuf,
    pub filename_prefix: String,
    #[serde(default = "default_filename_pattern")]
    pub filename_pattern: String,
    #[serde(default = "default_image_format")]
    pub image_format: String,
    #[serde(default = "default_jpeg_quality")]
    pub jpeg_quality: u8,
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
    #[serde(default = "default_spotlight_dim")]
    pub spotlight_dim: f32,
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
            filename_pattern: default_filename_pattern(),
            image_format: default_image_format(),
            jpeg_quality: default_jpeg_quality(),
            show_dimensions: true,
            show_magnifier: false,
            magnifier_zoom: 4,
            run_at_startup: false,
            default_stroke_width: default_stroke_width(),
            default_color_hex: default_color_hex(),
            default_font_size: default_font_size(),
            blur_radius: default_blur_radius(),
            pixelate_block: default_pixelate_block(),
            spotlight_dim: default_spotlight_dim(),
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

static LAST_CONFIG_MODIFIED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn has_config_file_changed() -> bool {
    let path = AppConfig::config_path();
    if let Ok(meta) = std::fs::metadata(&path) {
        if let Ok(modified) = meta.modified() {
            if let Ok(dur) = modified.duration_since(std::time::UNIX_EPOCH) {
                let secs = dur.as_secs();
                let last = LAST_CONFIG_MODIFIED.swap(secs, std::sync::atomic::Ordering::SeqCst);
                if last != 0 && last != secs {
                    return true;
                }
            }
        }
    }
    false
}

impl AppConfig {
    pub fn adjacent_config_path() -> Option<PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let dir = exe.parent()?;
        Some(dir.join("config.toml"))
    }

    pub fn appdata_config_path() -> PathBuf {
        dirs::config_dir()
            .map(|p| p.join("Pixora").join("config.toml"))
            .unwrap_or_else(|| PathBuf::from("config.toml"))
    }

    pub fn config_path() -> PathBuf {
        #[cfg(target_os = "linux")]
        {
            if let Some(adj) = Self::adjacent_config_path() {
                if adj.exists() {
                    return adj;
                }
            }
            return Self::appdata_config_path();
        }

        #[cfg(not(target_os = "linux"))]
        {
            if let Some(adj) = Self::adjacent_config_path() {
                if adj.exists() {
                    return adj;
                }
            }
            let appdata = Self::appdata_config_path();
            if appdata.exists() {
                return appdata;
            }
            if let Some(adj) = Self::adjacent_config_path() {
                return adj;
            }
            appdata
        }
    }

    pub fn load_from_str(data: &str) -> Result<Self, String> {
        // Try TOML first
        if let Ok(cfg) = toml::from_str::<AppConfig>(data) {
            return Ok(cfg);
        }
        // Fallback to JSON for seamless migration of old configs
        if let Ok(cfg) = serde_json::from_str::<AppConfig>(data) {
            return Ok(cfg);
        }
        Err("Failed to parse config as TOML".to_string())
    }

    pub fn load() -> Self {
        // Migrate and clean up any legacy adjacent config.json
        if let Some(exe) = std::env::current_exe().ok() {
            if let Some(dir) = exe.parent() {
                let legacy_json = dir.join("config.json");
                if legacy_json.exists() {
                    if let Ok(data) = std::fs::read_to_string(&legacy_json) {
                        if let Ok(config) = Self::load_from_str(&data) {
                            let _ = config.save();
                        }
                    }
                    let _ = std::fs::remove_file(&legacy_json);
                }
            }
        }

        // Migrate and clean up any legacy AppData config.json
        let legacy_appdata_json = dirs::config_dir()
            .map(|p| p.join("Pixora").join("config.json"));
        if let Some(path) = legacy_appdata_json {
            if path.exists() {
                if let Ok(data) = std::fs::read_to_string(&path) {
                    if let Ok(config) = Self::load_from_str(&data) {
                        let _ = config.save();
                    }
                }
                let _ = std::fs::remove_file(&path);
            }
        }

        let path = Self::config_path();
        if path.exists() {
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Ok(config) = Self::load_from_str(&data) {
                    return config;
                }
            }
        }

        // New default config
        let config = Self::default();
        let _ = config.save();
        config
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let toml_path = Self::config_path();

        if let Some(parent) = toml_path.parent() {
            let _ = std::fs::create_dir_all(parent);
            // Delete old config.json if present
            let old_json = parent.join("config.json");
            if old_json.exists() {
                let _ = std::fs::remove_file(&old_json);
            }
        }
        let toml_str = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        std::fs::write(&toml_path, &toml_str)?;

        // Also keep appdata in sync
        let appdata = Self::appdata_config_path();
        if toml_path != appdata {
            if let Some(parent) = appdata.parent() {
                let _ = std::fs::create_dir_all(parent);
                let old_appdata_json = parent.join("config.json");
                if old_appdata_json.exists() {
                    let _ = std::fs::remove_file(&old_appdata_json);
                }
            }
            let _ = std::fs::write(&appdata, &toml_str);
        }

        Ok(())
    }
}
