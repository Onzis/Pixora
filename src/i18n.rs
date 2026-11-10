use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LanguageChoice {
    #[default]
    Auto,
    En,
    Ru,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppLanguage {
    #[default]
    En,
    Ru,
}

impl AppLanguage {
    pub fn name(&self) -> &'static str {
        match self {
            AppLanguage::En => "English",
            AppLanguage::Ru => "Русский",
        }
    }
}

pub fn detect_system_language() -> AppLanguage {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetUserDefaultUILanguage() -> u16;
            fn GetUserDefaultLocaleName(lpLocaleName: *mut u16, cchLocaleName: i32) -> i32;
        }

        unsafe {
            let mut buf = [0u16; 85];
            let len = GetUserDefaultLocaleName(buf.as_mut_ptr(), 85);
            if len > 0 {
                let s = String::from_utf16_lossy(&buf[..(len as usize).saturating_sub(1)]);
                if s.to_ascii_lowercase().starts_with("ru") {
                    return AppLanguage::Ru;
                }
            }

            let lang_id = GetUserDefaultUILanguage();
            let primary = lang_id & 0x03FF;
            if primary == 0x19 {
                // 0x19 is LANG_RUSSIAN
                return AppLanguage::Ru;
            }
        }
    }

    #[cfg(not(windows))]
    {
        if let Ok(lang) = std::env::var("LANG").or_else(|_| std::env::var("LC_ALL")) {
            if lang.to_ascii_lowercase().starts_with("ru") {
                return AppLanguage::Ru;
            }
        }
    }

    AppLanguage::En
}

impl LanguageChoice {
    pub fn resolve(&self) -> AppLanguage {
        match self {
            LanguageChoice::Auto => detect_system_language(),
            LanguageChoice::En => AppLanguage::En,
            LanguageChoice::Ru => AppLanguage::Ru,
        }
    }
}

#[inline]
pub fn t(lang: AppLanguage, en: &'static str, ru: &'static str) -> &'static str {
    match lang {
        AppLanguage::Ru => ru,
        AppLanguage::En => en,
    }
}
