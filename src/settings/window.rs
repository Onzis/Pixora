use std::time::Duration;
use crate::config::AppConfig;
use crate::i18n::{detect_system_language, t, AppLanguage, LanguageChoice};
use crate::settings::storage::{open_folder_in_explorer, set_run_at_startup};
use crate::shortcuts::hotkeys::{format_keystroke, update_global_hotkeys};
use crate::system::updater::{open_url, UpdateStatus, GITHUB_REPO_URL};
use crate::theme::PixoraTheme;
use gpui::{
    App, ClickEvent, Context, FocusHandle, IntoElement, KeyDownEvent, Render, ScrollHandle,
    Window, div, prelude::*, px, rgb, svg,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Capture,
    Shortcuts,
    About,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutRebindTarget {
    Region,
    Fullscreen,
    Window,
}

pub struct SettingsWindowView {
    pub config: AppConfig,
    pub theme: PixoraTheme,
    pub current_tab: SettingsTab,
    pub status_message: Option<String>,
    pub rebinding_target: Option<ShortcutRebindTarget>,
    pub scroll_handle: ScrollHandle,
    pub focus_handle: FocusHandle,
    pub update_status: UpdateStatus,
    pub last_checked_time: Option<String>,
}

impl SettingsWindowView {
    pub fn new(config: AppConfig, cx: &mut Context<Self>) -> Self {
        Self::with_tab(config, SettingsTab::General, cx)
    }

    pub fn with_tab(config: AppConfig, tab: SettingsTab, cx: &mut Context<Self>) -> Self {
        let global_status = crate::system::updater::get_global_update_status();
        let mut view = Self {
            config,
            theme: PixoraTheme::default(),
            current_tab: tab,
            status_message: None,
            rebinding_target: None,
            scroll_handle: ScrollHandle::new(),
            focus_handle: cx.focus_handle(),
            update_status: global_status,
            last_checked_time: None,
        };
        if tab == SettingsTab::About && view.update_status == UpdateStatus::Idle {
            view.check_for_updates(cx);
        }
        view
    }

    pub fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        self.update_status = UpdateStatus::Checking;
        crate::system::updater::set_global_update_status(UpdateStatus::Checking);
        cx.notify();

        cx.spawn(|this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
            let cx = cx.clone();
            async move {
                let task = cx.background_executor().spawn(async move {
                    crate::system::updater::check_latest_release_status()
                });

                cx.background_executor().timer(Duration::from_millis(600)).await;
                let status = task.await;

                crate::system::updater::set_global_update_status(status.clone());
                let _ = cx.update(|app| {
                    let _ = this.update(app, |view, cx| {
                        let now_str = chrono::Local::now().format("%H:%M:%S").to_string();
                        view.last_checked_time = Some(now_str);
                        let lang = view.config.language.resolve();
                        match &status {
                            UpdateStatus::UpToDate { version } => {
                                view.status_message = Some(format!(
                                    "{}: v{}",
                                    t(lang, "Pixora is up to date", "У вас установлена последняя версия"),
                                    version
                                ));
                            }
                            UpdateStatus::Available { latest, .. } => {
                                view.status_message = Some(format!(
                                    "{}: v{}",
                                    t(lang, "New update available", "Доступно обновление"),
                                    latest
                                ));
                            }
                            UpdateStatus::Error(e) => {
                                view.status_message = Some(format!(
                                    "{}: {}",
                                    t(lang, "Update check error", "Ошибка проверки обновлений"),
                                    e
                                ));
                            }
                            _ => {}
                        }
                        view.update_status = status;
                        cx.notify();
                    });
                });
            }
        })
        .detach();
    }

    pub fn save_settings(&mut self, cx: &mut Context<Self>) {
        let lang = self.config.language.resolve();
        let _ = set_run_at_startup(self.config.run_at_startup);
        update_global_hotkeys(
            &self.config.hotkey_region,
            &self.config.hotkey_fullscreen,
            &self.config.hotkey_window,
        );
        if let Err(e) = self.config.save() {
            self.status_message = Some(format!("{}: {}", t(lang, "Error", "Ошибка"), e));
        } else {
            self.status_message = Some(t(lang, "Settings saved successfully", "Настройки успешно сохранены").to_string());
        }
        cx.notify();
    }
}

impl Render for SettingsWindowView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let tab = self.current_tab;
        let lang = self.config.language.resolve();

        div()
            .id("settings-root")
            .size_full()
            .bg(theme.bg_dark)
            .text_color(theme.text_primary)
            .flex()
            .flex_col()
            .overflow_hidden()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
                if let Some(target) = this.rebinding_target {
                    let cur_lang = this.config.language.resolve();
                    if ev.keystroke.key == "escape" {
                        this.rebinding_target = None;
                        this.status_message = Some(t(cur_lang, "Shortcut reassignment cancelled", "Переназначение клавиш отменено").to_string());
                        cx.notify();
                        return;
                    }

                    if let Some(hotkey_str) = format_keystroke(&ev.keystroke) {
                        match target {
                            ShortcutRebindTarget::Region => {
                                this.config.hotkey_region = hotkey_str.clone();
                            }
                            ShortcutRebindTarget::Fullscreen => {
                                this.config.hotkey_fullscreen = hotkey_str.clone();
                            }
                            ShortcutRebindTarget::Window => {
                                this.config.hotkey_window = hotkey_str.clone();
                            }
                        }
                        this.rebinding_target = None;
                        this.status_message = Some(format!("{}: {}", t(cur_lang, "Assigned", "Назначено"), hotkey_str));
                        this.save_settings(cx);
                        cx.notify();
                    }
                }
            }))
            // Header
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_6()
                    .py_3p5()
                    .border_b_1()
                    .border_color(theme.border_subtle)
                    .bg(theme.bg_surface)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .size_8()
                                    .rounded_lg()
                                    .overflow_hidden()
                                    .child(
                                        svg()
                                            .path("icons/app-icon.svg")
                                            .size_full(),
                                    ),
                            )
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(t(lang, "Pixora Settings", "Настройки Pixora")),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child(concat!("v", env!("CARGO_PKG_VERSION"), " (Rust & GPUI)")),
                    ),
            )
            // Body
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_hidden()
                    // Sidebar with SVG vector icons (Zero Emojis)
                    .child(
                        div()
                            .w(px(180.0))
                            .flex_shrink_0()
                            .py_4()
                            .px_3()
                            .border_r_1()
                            .border_color(theme.border_subtle)
                            .bg(theme.bg_surface)
                            .flex()
                            .flex_col()
                            .gap_1p5()
                            .child(render_tab_button(
                                "tab-general",
                                "icons/settings.svg",
                                t(lang, "General", "Основные"),
                                tab == SettingsTab::General,
                                cx.listener(|this, _, _, cx| {
                                    this.current_tab = SettingsTab::General;
                                    cx.notify();
                                }),
                            ))
                            .child(render_tab_button(
                                "tab-capture",
                                "icons/camera.svg",
                                t(lang, "Capture", "Захват"),
                                tab == SettingsTab::Capture,
                                cx.listener(|this, _, _, cx| {
                                    this.current_tab = SettingsTab::Capture;
                                    cx.notify();
                                }),
                            ))
                            .child(render_tab_button(
                                "tab-shortcuts",
                                "icons/keyboard.svg",
                                t(lang, "Shortcuts", "Горячие клавиши"),
                                tab == SettingsTab::Shortcuts,
                                cx.listener(|this, _, _, cx| {
                                    this.current_tab = SettingsTab::Shortcuts;
                                    cx.notify();
                                }),
                            ))
                            .child(render_tab_button(
                                "tab-about",
                                "icons/info.svg",
                                t(lang, "About", "О программе"),
                                tab == SettingsTab::About,
                                cx.listener(|this, _, _, cx| {
                                    this.current_tab = SettingsTab::About;
                                    cx.notify();
                                }),
                            )),
                    )
                    // Content panel with Scroll & Fixed Footer
                    .child(
                        div()
                            .flex_1()
                            .min_h(px(0.0))
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            // Scrollable content area
                            .child(
                                div()
                                    .id("settings-scroll-area")
                                    .flex_1()
                                    .min_h(px(0.0))
                                    .overflow_y_scroll()
                                    .track_scroll(&self.scroll_handle)
                                    .p_6()
                                    .child(match tab {
                                        SettingsTab::General => self.render_general_tab(cx),
                                        SettingsTab::Capture => self.render_capture_tab(cx),
                                        SettingsTab::Shortcuts => self.render_shortcuts_tab(cx),
                                        SettingsTab::About => self.render_about_tab(cx),
                                    }),
                            )
                            // Fixed Footer (Always locked at bottom, never clipped or pushed out)
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .px_6()
                                    .py_3p5()
                                    .border_t_1()
                                    .border_color(theme.border_subtle)
                                    .bg(theme.bg_surface)
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.0))
                                            .mr_4()
                                            .text_xs()
                                            .text_color(theme.accent)
                                            .child(
                                                self.status_message
                                                    .clone()
                                                    .unwrap_or_default(),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .flex()
                                            .gap_3()
                                            .child(
                                                div()
                                                    .id("btn-save-settings")
                                                    .px_4()
                                                    .py_1p5()
                                                    .rounded_md()
                                                    .bg(theme.accent)
                                                    .hover(|s| s.bg(theme.accent_hover))
                                                    .cursor_pointer()
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.save_settings(cx);
                                                    }))
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                                            .text_color(rgb(0xFFFFFF))
                                                            .child(t(lang, "Save Settings", "Сохранить настройки")),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .id("btn-close-settings")
                                                    .px_4()
                                                    .py_1p5()
                                                    .rounded_md()
                                                    .bg(theme.bg_surface)
                                                    .border_1()
                                                    .border_color(theme.border_subtle)
                                                    .hover(|s| s.bg(theme.bg_hover))
                                                    .cursor_pointer()
                                                    .on_click(cx.listener(|_, _, window, _| {
                                                        crate::app::clear_settings_window_handle();
                                                        window.remove_window();
                                                    }))
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.text_secondary)
                                                            .child(t(lang, "Close", "Закрыть")),
                                                    ),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}

impl SettingsWindowView {
    fn render_general_tab(&self, cx: &mut Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let lang = self.config.language.resolve();
        let save_dir = self.config.save_directory.display().to_string();
        let detected = detect_system_language();
        let auto_desc = match detected {
            AppLanguage::Ru => "Авто (Система: RU)",
            AppLanguage::En => "Auto (System: EN)",
        };

        div()
            .flex()
            .flex_col()
            .gap_4()
            // Language selector
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3p5()
                    .rounded_lg()
                    .bg(theme.bg_surface)
                    .border_1()
                    .border_color(theme.border_subtle)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.text_primary)
                                    .child(t(lang, "Interface Language", "Язык интерфейса")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child(t(
                                        lang,
                                        "Select application language or follow Windows system language automatically",
                                        "Выберите язык интерфейса или используйте язык системы Windows автоматически",
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_wrap()
                            .gap_2()
                            .child(render_language_chip(
                                "lang-auto",
                                auto_desc,
                                self.config.language == LanguageChoice::Auto,
                                cx.listener(|this, _, _, cx| {
                                    this.config.language = LanguageChoice::Auto;
                                    this.save_settings(cx);
                                }),
                            ))
                            .child(render_language_chip(
                                "lang-ru",
                                "Русский",
                                self.config.language == LanguageChoice::Ru,
                                cx.listener(|this, _, _, cx| {
                                    this.config.language = LanguageChoice::Ru;
                                    this.save_settings(cx);
                                }),
                            ))
                            .child(render_language_chip(
                                "lang-en",
                                "English",
                                self.config.language == LanguageChoice::En,
                                cx.listener(|this, _, _, cx| {
                                    this.config.language = LanguageChoice::En;
                                    this.save_settings(cx);
                                }),
                            )),
                    ),
            )
            .child(render_toggle(
                "toggle-startup",
                t(lang, "Run at Windows startup", "Автозапуск при старте Windows"),
                t(
                    lang,
                    "Launch Pixora automatically in the background when logging into Windows",
                    "Автоматически запускать Pixora в фоне при входе в Windows",
                ),
                self.config.run_at_startup,
                cx.listener(|this, _, _, cx| {
                    this.config.run_at_startup = !this.config.run_at_startup;
                    cx.notify();
                }),
            ))
            .child(render_toggle(
                "toggle-autocopy",
                t(lang, "Auto-copy to clipboard", "Автокопирование в буфер"),
                t(
                    lang,
                    "Automatically copy screenshot to clipboard immediately after selection or capture",
                    "Автоматически копировать снимок в буфер сразу после выделения или захвата",
                ),
                self.config.auto_copy_to_clipboard,
                cx.listener(|this, _, _, cx| {
                    this.config.auto_copy_to_clipboard = !this.config.auto_copy_to_clipboard;
                    cx.notify();
                }),
            ))
            .child(render_toggle(
                "toggle-sound",
                t(lang, "Sound effects", "Звуковые эффекты"),
                t(
                    lang,
                    "Play camera shutter sound upon capture (ScreenFX)",
                    "Воспроизводить щелчок затвора при создании снимка (ScreenFX)",
                ),
                self.config.sound_effects,
                cx.listener(|this, _, _, cx| {
                    this.config.sound_effects = !this.config.sound_effects;
                    if this.config.sound_effects {
                        crate::system::sound::play_capture_sound();
                    }
                    cx.notify();
                }),
            ))
            .when(self.config.sound_effects, |p| {
                p.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .pb_1()
                        .child(
                            div()
                                .id("btn-test-sound")
                                .px_2p5()
                                .py_1()
                                .rounded_md()
                                .bg(theme.bg_secondary)
                                .border_1()
                                .border_color(theme.border_subtle)
                                .hover(|s| s.bg(theme.bg_hover))
                                .cursor_pointer()
                                .on_click(cx.listener(|_, _, _, _| {
                                    crate::system::sound::play_capture_sound();
                                }))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(theme.text_secondary)
                                        .child(t(lang, "Play test sound", "Прослушать звук")),
                                ),
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .pt_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(t(lang, "Screenshots Directory", "Папка со снимками")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .overflow_hidden()
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(theme.bg_secondary)
                                    .border_1()
                                    .border_color(theme.border_subtle)
                                    .text_xs()
                                    .text_color(theme.text_secondary)
                                    .child(save_dir),
                            )
                            .child(
                                div()
                                    .id("btn-open-folder")
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .gap_1p5()
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(theme.bg_surface)
                                    .border_1()
                                    .border_color(theme.border_subtle)
                                    .hover(|s| s.bg(theme.bg_hover))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, _| {
                                        open_folder_in_explorer(&this.config.save_directory);
                                    }))
                                    .child(
                                        svg()
                                            .path("icons/folder.svg")
                                            .size(px(14.0))
                                            .text_color(theme.text_secondary),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.text_primary)
                                            .child(t(lang, "Open Folder", "Открыть папку")),
                                    ),
                            ),
                    ),
            )
    }

    fn render_capture_tab(&self, cx: &mut Context<Self>) -> gpui::Div {
        let lang = self.config.language.resolve();

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(render_toggle(
                "toggle-dimensions",
                t(lang, "Show selection dimensions", "Показывать размеры выделения"),
                t(
                    lang,
                    "Display width and height pixel badge beside selection box (e.g. 1920 × 1080)",
                    "Отображать бейдж с шириной и высотой рядом с рамкой (напр. 1920 × 1080)",
                ),
                self.config.show_dimensions,
                cx.listener(|this, _, _, cx| {
                    this.config.show_dimensions = !this.config.show_dimensions;
                    cx.notify();
                }),
            ))
            .child(render_toggle(
                "toggle-magnifier",
                t(lang, "Show pixel magnifier & loupe", "Экранная лупа (Pixel Loupe)"),
                t(
                    lang,
                    "Display zoomed pixel grid, coordinates and HEX color inspector near the cursor",
                    "Отображать увеличенную сетку пикселей, координаты и HEX-код цвета около курсора",
                ),
                self.config.show_magnifier,
                cx.listener(|this, _, _, cx| {
                    this.config.show_magnifier = !this.config.show_magnifier;
                    cx.notify();
                }),
            ))
            .child(render_toggle(
                "toggle-snapping",
                t(lang, "Window snapping & auto-detection", "Привязка к окнам (Window Snapping)"),
                t(
                    lang,
                    "Automatically highlight and snap to windows on hover and click",
                    "Автоматически подсвечивать и выбирать окна при наведении и клике",
                ),
                self.config.window_snapping,
                cx.listener(|this, _, _, cx| {
                    this.config.window_snapping = !this.config.window_snapping;
                    cx.notify();
                }),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .pt_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(t(lang, "OCR Recognition Language", "Язык распознавания текста (OCR)")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x8A8A99))
                            .child(t(
                                lang,
                                "Windows OCR model for text extraction (supports hybrid bilingual RU+EN refinement)",
                                "Модель Windows OCR для извлечения текста (поддерживает гибридный режим RU+EN)",
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .pt_1()
                            .child(render_size_chip(
                                "chip-ocr-auto",
                                t(lang, "Auto", "Авто (RU + EN)"),
                                self.config.ocr_language == "auto",
                                cx.listener(|this, _, _, cx| {
                                    this.config.ocr_language = "auto".to_string();
                                    cx.notify();
                                }),
                            ))
                            .child(render_size_chip(
                                "chip-ocr-ru",
                                "Русский",
                                self.config.ocr_language == "ru",
                                cx.listener(|this, _, _, cx| {
                                    this.config.ocr_language = "ru".to_string();
                                    cx.notify();
                                }),
                            ))
                            .child(render_size_chip(
                                "chip-ocr-en",
                                "English",
                                self.config.ocr_language == "en",
                                cx.listener(|this, _, _, cx| {
                                    this.config.ocr_language = "en".to_string();
                                    cx.notify();
                                }),
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .pt_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(t(lang, "Default Stroke Width", "Толщина линий по умолчанию")),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(render_size_chip(
                                "chip-2px",
                                t(lang, "2px Thin", "2px Тонкая"),
                                (self.config.default_stroke_width - 2.0).abs() < 0.1,
                                cx.listener(|this, _, _, cx| {
                                    this.config.default_stroke_width = 2.0;
                                    cx.notify();
                                }),
                            ))
                            .child(render_size_chip(
                                "chip-4px",
                                t(lang, "4px Medium", "4px Средняя"),
                                (self.config.default_stroke_width - 4.0).abs() < 0.1,
                                cx.listener(|this, _, _, cx| {
                                    this.config.default_stroke_width = 4.0;
                                    cx.notify();
                                }),
                            ))
                            .child(render_size_chip(
                                "chip-8px",
                                t(lang, "8px Bold", "8px Толстая"),
                                (self.config.default_stroke_width - 8.0).abs() < 0.1,
                                cx.listener(|this, _, _, cx| {
                                    this.config.default_stroke_width = 8.0;
                                    cx.notify();
                                }),
                            )),
                    ),
            )
    }

    fn render_shortcuts_tab(&self, cx: &mut Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let lang = self.config.language.resolve();

        let global_shortcuts = [
            (
                t(lang, "Region Capture", "Снимок области"),
                t(lang, "Capture selected screen area with interactive overlay", "Захват выбранной области экрана с интерактивной панелью"),
                &self.config.hotkey_region,
                ShortcutRebindTarget::Region,
            ),
            (
                t(lang, "Fullscreen Capture", "Снимок всего экрана"),
                t(lang, "Instantly capture entire screen without overlay", "Мгновенный захват экрана целиком без оверлея"),
                &self.config.hotkey_fullscreen,
                ShortcutRebindTarget::Fullscreen,
            ),
            (
                t(lang, "Active Window Capture", "Снимок активного окна"),
                t(lang, "Capture the currently active foreground window directly", "Прямой захват активного окна переднего плана"),
                &self.config.hotkey_window,
                ShortcutRebindTarget::Window,
            ),
        ];

        let editor_shortcuts = [
            (t(lang, "Copy to Clipboard", "Копировать в буфер"), "Ctrl + C"),
            (t(lang, "Save to PNG", "Сохранить в PNG"), "Ctrl + S"),
            (t(lang, "Undo Action", "Отменить действие"), "Ctrl + Z"),
            (t(lang, "Redo Action", "Повторить действие"), "Ctrl + Y"),
            (t(lang, "Confirm / Finish", "Подтвердить"), "Enter"),
            (t(lang, "Cancel / Close", "Закрыть"), "Escape"),
            (t(lang, "Brush Size Stepper", "Размер кисти"), "Mouse Wheel / [ - ] [ + ]"),
        ];

        div()
            .flex()
            .flex_col()
            .gap_4()
            // Global configurable shortcuts header & reset button
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pb_2()
                    .border_b_1()
                    .border_color(theme.border_subtle)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .child(t(lang, "Global Hotkeys (Rebindable)", "Глобальные горячие клавиши")),
                    )
                    .child(
                        div()
                            .id("btn-reset-hotkeys")
                            .px_2p5()
                            .py_1()
                            .rounded_md()
                            .bg(theme.bg_secondary)
                            .border_1()
                            .border_color(theme.border_subtle)
                            .hover(|s| s.bg(theme.bg_hover))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                let cur_lang = this.config.language.resolve();
                                this.config.hotkey_region = "PrintScreen".to_string();
                                this.config.hotkey_fullscreen = "Shift + PrintScreen".to_string();
                                this.config.hotkey_window = "Alt + PrintScreen".to_string();
                                this.rebinding_target = None;
                                this.status_message = Some(t(cur_lang, "Reset hotkeys to default", "Горячие клавиши сброшены по умолчанию").to_string());
                                this.save_settings(cx);
                            }))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_secondary)
                                    .child(t(lang, "Reset Defaults", "Сбросить")),
                            ),
                    ),
            )
            // Global configurable shortcut rows
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2p5()
                    .children(global_shortcuts.into_iter().map(|(title, desc, key_str, target)| {
                        let is_rebinding = self.rebinding_target == Some(target);
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .p_3()
                            .rounded_lg()
                            .bg(if is_rebinding {
                                rgb(0x28283E)
                            } else {
                                theme.bg_surface
                            })
                            .border_1()
                            .border_color(if is_rebinding {
                                theme.accent
                            } else {
                                theme.border_subtle
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .mr_4()
                                    .flex()
                                    .flex_col()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(theme.text_primary)
                                            .child(title),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.text_muted)
                                            .child(desc),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .px_2p5()
                                            .py_1()
                                            .rounded_md()
                                            .bg(theme.bg_secondary)
                                            .border_1()
                                            .border_color(if is_rebinding {
                                                theme.accent
                                            } else {
                                                theme.border_subtle
                                            })
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(theme.accent)
                                            .child(if is_rebinding {
                                                t(lang, "Press keys... (Esc)", "Нажмите клавиши... (Esc)").to_string()
                                            } else {
                                                key_str.clone()
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id(match target {
                                                ShortcutRebindTarget::Region => "btn-rebind-region",
                                                ShortcutRebindTarget::Fullscreen => "btn-rebind-fs",
                                                ShortcutRebindTarget::Window => "btn-rebind-win",
                                            })
                                            .px_3()
                                            .py_1()
                                            .rounded_md()
                                            .bg(if is_rebinding {
                                                theme.accent
                                            } else {
                                                theme.bg_surface
                                            })
                                            .border_1()
                                            .border_color(if is_rebinding {
                                                theme.accent
                                            } else {
                                                theme.border_subtle
                                            })
                                            .hover(|s| s.bg(theme.bg_hover))
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                let cur_lang = this.config.language.resolve();
                                                if this.rebinding_target == Some(target) {
                                                    this.rebinding_target = None;
                                                    this.status_message = None;
                                                } else {
                                                    this.rebinding_target = Some(target);
                                                    this.focus_handle.focus(window);
                                                    this.status_message = Some(
                                                        t(cur_lang, "Press any key combo... (Esc to cancel)", "Нажмите комбинацию клавиш... (Esc для отмены)").to_string(),
                                                    );
                                                }
                                                cx.notify();
                                            }))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::MEDIUM)
                                                    .text_color(if is_rebinding {
                                                        rgb(0xFFFFFF)
                                                    } else {
                                                        theme.text_secondary
                                                    })
                                                    .child(if is_rebinding { t(lang, "Listening", "Ожидание") } else { t(lang, "Change", "Изменить") }),
                                            ),
                                    ),
                            )
                    })),
            )
            // Overlay in-editor shortcuts reference
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .pt_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .child(t(lang, "In-Overlay Shortcuts", "Горячие клавиши в оверлее")),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .children(editor_shortcuts.into_iter().map(|(action, key)| {
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .py_1p5()
                                    .px_2()
                                    .border_b_1()
                                    .border_color(rgb(0x1F1F27))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0xD1D1D6))
                                            .child(action),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(rgb(0x1C1C22))
                                            .border_1()
                                            .border_color(rgb(0x282832))
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .text_color(rgb(0x8174FF))
                                            .child(key),
                                    )
                            })),
                    ),
            )
    }

    fn render_about_tab(&self, cx: &mut Context<Self>) -> gpui::Div {
        let lang = self.config.language.resolve();

        div()
            .flex()
            .flex_col()
            .gap_4()
            // Header card
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(
                        div()
                            .text_base()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(rgb(0x8174FF))
                            .child(format!(
                                "Pixora v{} — {}",
                                env!("CARGO_PKG_VERSION"),
                                t(lang, "High-Performance Screen Capture", "Создание и аннотирование снимков экрана")
                            )),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0xA2A2B0))
                            .child(t(
                                lang,
                                "Built with pure Rust and GPUI framework for Windows 10 & 11.",
                                "Разработано на чистом Rust и фреймворке GPUI для Windows 10 и 11.",
                            )),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(0x8174FF))
                            .child(t(
                                lang,
                                "Author: Onzi • Copyright © 2026",
                                "Автор: Onzi • Copyright © 2026",
                            )),
                    ),
            )
            // GitHub Project Card
            .child(
                div()
                    .p_4()
                    .rounded_lg()
                    .bg(rgb(0x181820))
                    .border_1()
                    .border_color(rgb(0x282832))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(t(lang, "GitHub Repository", "Репозиторий GitHub")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x8174FF))
                                    .child(GITHUB_REPO_URL),
                            ),
                    )
                    .child(
                        div()
                            .id("btn-open-github")
                            .px_3()
                            .py_1p5()
                            .rounded_md()
                            .bg(rgb(0x282838))
                            .border_1()
                            .border_color(rgb(0x8174FF))
                            .hover(|s| s.bg(rgb(0x35354A)))
                            .cursor_pointer()
                            .on_click(cx.listener(|_, _, _, _| {
                                open_url(GITHUB_REPO_URL);
                            }))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(rgb(0xFFFFFF))
                                    .child(t(lang, "Open in Browser", "Открыть в браузере")),
                            ),
                    ),
            )
            // Updates card
            .child(
                div()
                    .p_4()
                    .rounded_lg()
                    .bg(rgb(0x181820))
                    .border_1()
                    .border_color(rgb(0x282832))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        render_toggle(
                            "toggle-auto-updates",
                            t(lang, "Check for updates automatically", "Автоматически проверять обновления"),
                            t(
                                lang,
                                "Check GitHub Releases for new versions when Pixora starts",
                                "Проверять наличие новых релизов на GitHub при запуске Pixora",
                            ),
                            self.config.auto_check_updates,
                            cx.listener(|this, _, _, cx| {
                                this.config.auto_check_updates = !this.config.auto_check_updates;
                                cx.notify();
                            }),
                        ),
                    )
                    .child(div().h(px(1.0)).bg(rgb(0x282832)))
                    .child(match &self.update_status {
                        UpdateStatus::Idle => {
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .child(format!(
                                                    "{} v{}",
                                                    t(lang, "Current Version:", "Текущая версия:"),
                                                    env!("CARGO_PKG_VERSION")
                                                )),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(0x8A8A99))
                                                .child(t(
                                                    lang,
                                                    "Check GitHub Releases for updates",
                                                    "Проверить наличие обновлений на GitHub",
                                                )),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("btn-check-updates-idle")
                                        .px_4()
                                        .py_2()
                                        .rounded_md()
                                        .bg(rgb(0x8174FF))
                                        .hover(|s| s.bg(rgb(0x9488FF)))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.check_for_updates(cx);
                                        }))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                .text_color(rgb(0xFFFFFF))
                                                .child(t(lang, "Check for Updates", "Проверить обновление")),
                                        ),
                                )
                        }
                        UpdateStatus::Checking => {
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(rgb(0x8174FF))
                                                .child(t(lang, "Checking for updates...", "Проверка обновлений...")),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(0x8A8A99))
                                                .child(t(
                                                    lang,
                                                    "Connecting to GitHub Releases...",
                                                    "Подключение к релизам GitHub...",
                                                )),
                                        ),
                                )
                                .child(
                                    div()
                                        .px_4()
                                        .py_2()
                                        .rounded_md()
                                        .bg(rgb(0x282832))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(rgb(0x8A8A99))
                                                .child(t(lang, "Checking...", "Проверка...")),
                                        ),
                                )
                        }
                        UpdateStatus::UpToDate { version } => {
                            let ver_str = version.clone();
                            let desc = if let Some(time) = &self.last_checked_time {
                                format!(
                                    "{} v{} • {}: {}",
                                    t(lang, "Latest version installed:", "Установлена актуальная версия:"),
                                    ver_str,
                                    t(lang, "Checked at", "Проверено в"),
                                    time
                                )
                            } else {
                                format!(
                                    "{} v{}",
                                    t(lang, "Latest version installed:", "Установлена актуальная версия:"),
                                    ver_str
                                )
                            };
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                .text_color(rgb(0x34C759))
                                                .child(t(lang, "Pixora is up to date", "У вас установлена последняя версия")),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(0x8A8A99))
                                                .child(desc),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("btn-check-updates-again")
                                        .px_3()
                                        .py_1p5()
                                        .rounded_md()
                                        .bg(rgb(0x282838))
                                        .border_1()
                                        .border_color(rgb(0x383848))
                                        .hover(|s| s.bg(rgb(0x323242)))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.check_for_updates(cx);
                                        }))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(rgb(0xA2A2B0))
                                                .child(t(lang, "Check Again", "Проверить снова")),
                                        ),
                                )
                        }
                        UpdateStatus::Available { current, latest, release_url } => {
                            let rel_url = release_url.clone();
                            let cur_str = current.clone();
                            let lat_str = latest.clone();
                            div()
                                .p_3p5()
                                .rounded_md()
                                .bg(rgb(0x221D38))
                                .border_1()
                                .border_color(rgb(0x8174FF))
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(rgb(0x8174FF))
                                                .child(format!(
                                                    "{} v{}",
                                                    t(lang, "New Version Available:", "Доступно обновление:"),
                                                    lat_str
                                                )),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(0xD0D0E0))
                                                .child(format!(
                                                    "{} v{} • {} v{}",
                                                    t(lang, "Current:", "Текущая:"),
                                                    cur_str,
                                                    t(lang, "Latest:", "Новая:"),
                                                    lat_str
                                                )),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .id("btn-download-update")
                                                .px_4()
                                                .py_2()
                                                .rounded_md()
                                                .bg(rgb(0x8174FF))
                                                .hover(|s| s.bg(rgb(0x9488FF)))
                                                .cursor_pointer()
                                                .on_click(cx.listener(move |_, _, _, _| {
                                                    open_url(&rel_url);
                                                }))
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                                        .text_color(rgb(0xFFFFFF))
                                                        .child(t(lang, "Open Release", "Открыть релиз")),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .id("btn-check-again-after-avail")
                                                .px_3()
                                                .py_2()
                                                .rounded_md()
                                                .bg(rgb(0x282838))
                                                .hover(|s| s.bg(rgb(0x323242)))
                                                .cursor_pointer()
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.check_for_updates(cx);
                                                }))
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(gpui::FontWeight::MEDIUM)
                                                        .text_color(rgb(0xA2A2B0))
                                                        .child(t(lang, "Check Again", "Проверить снова")),
                                                ),
                                        ),
                                )
                        }
                        UpdateStatus::Error(err) => {
                            let err_msg = err.clone();
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(rgb(0xFF453A))
                                                .child(t(lang, "Update Check Failed", "Ошибка проверки обновлений")),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(0x8A8A99))
                                                .child(err_msg),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("btn-retry-update")
                                        .px_3()
                                        .py_1p5()
                                        .rounded_md()
                                        .bg(rgb(0x282838))
                                        .border_1()
                                        .border_color(rgb(0x383848))
                                        .hover(|s| s.bg(rgb(0x323242)))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.check_for_updates(cx);
                                        }))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(rgb(0xA2A2B0))
                                                .child(t(lang, "Retry", "Повторить")),
                                        ),
                                )
                        }
                    }),
            )
            // Privacy and features notice
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x656575))
                    .child(t(
                        lang,
                        "Features: multi-monitor support, lossless PNG export, Windows OCR, smart loupe, rich annotation canvas, global hotkeys and tray integration. 100% offline and private.",
                        "Возможности: поддержка нескольких мониторов, сохранение в PNG без потерь, Windows OCR, экранная лупа, аннотации, глобальные горячие клавиши и системный трей. 100% оффлайн и конфиденциально.",
                    )),
            )
    }
}

fn render_tab_button(
    id: &'static str,
    icon_path: &'static str,
    label: &'static str,
    is_active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let bg = if is_active {
        rgb(0x282838)
    } else {
        rgb(0x151519)
    };
    let text_c = if is_active {
        rgb(0x8174FF)
    } else {
        rgb(0xA2A2B0)
    };

    div()
        .id(id)
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(bg)
        .hover(|s| s.bg(rgb(0x22222C)))
        .cursor_pointer()
        .on_click(on_click)
        .child(
            svg()
                .path(icon_path)
                .size(px(16.0))
                .text_color(text_c),
        )
        .child(
            div()
                .text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(text_c)
                .child(label),
        )
}

fn render_toggle(
    id: &'static str,
    title: &'static str,
    desc: &'static str,
    is_on: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .py_1()
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .mr_4()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(title),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x8A8A99))
                        .child(desc),
                ),
        )
        .child(
            div()
                .id(id)
                .flex_shrink_0()
                .w(px(38.0))
                .h(px(22.0))
                .p_0p5()
                .rounded_full()
                .bg(if is_on {
                    rgb(0x8174FF)
                } else {
                    rgb(0x282832)
                })
                .cursor_pointer()
                .on_click(on_click)
                .flex()
                .items_center()
                .child(
                    div()
                        .size_4()
                        .rounded_full()
                        .bg(rgb(0xFFFFFF))
                        .shadow_sm()
                        .when(is_on, |s| s.ml_auto()),
                ),
        )
}

fn render_size_chip(
    id: &'static str,
    label: &'static str,
    is_selected: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .bg(if is_selected {
            rgb(0x282838)
        } else {
            rgb(0x1C1C22)
        })
        .border_1()
        .border_color(if is_selected {
            rgb(0x8174FF)
        } else {
            rgb(0x282832)
        })
        .cursor_pointer()
        .on_click(on_click)
        .child(
            div()
                .text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(if is_selected {
                    rgb(0x8174FF)
                } else {
                    rgb(0xA2A2B0)
                })
                .child(label),
        )
}

fn render_language_chip(
    id: &'static str,
    label: &'static str,
    is_selected: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px_3()
        .py_1p5()
        .rounded_md()
        .bg(if is_selected {
            rgb(0x282838)
        } else {
            rgb(0x1C1C22)
        })
        .border_1()
        .border_color(if is_selected {
            rgb(0x8174FF)
        } else {
            rgb(0x282832)
        })
        .hover(|s| s.border_color(rgb(0x8174FF)))
        .cursor_pointer()
        .on_click(on_click)
        .child(
            div()
                .text_xs()
                .font_weight(if is_selected {
                    gpui::FontWeight::SEMIBOLD
                } else {
                    gpui::FontWeight::MEDIUM
                })
                .text_color(if is_selected {
                    rgb(0x8174FF)
                } else {
                    rgb(0xA2A2B0)
                })
                .child(label),
        )
}
