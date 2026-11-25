use std::time::Duration;
use crate::config::AppConfig;
use crate::i18n::{detect_system_language, t, AppLanguage, LanguageChoice};
use crate::settings::storage::{choose_folder_dialog, open_folder_in_explorer, set_run_at_startup};
use crate::shortcuts::hotkeys::{format_keystroke, update_global_hotkeys};
use crate::system::updater::{open_url, UpdateStatus, GITHUB_REPO_URL};
use crate::theme::PixoraTheme;
use gpui::{
    App, ClickEvent, Context, FocusHandle, IntoElement, KeyDownEvent, Render, ScrollHandle,
    Window, div, img, prelude::*, px, rgb, rgba, svg,
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
            crate::logging::log_error("config", format!("Failed to save config: {}", e));
            self.status_message = Some(format!("{}: {}", t(lang, "Error", "Ошибка"), e));
        } else {
            self.status_message = Some(t(lang, "Saved", "Сохранено").to_string());
        }
        cx.notify();
    }
}

impl Render for SettingsWindowView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let tab = self.current_tab;
        let lang = self.config.language.resolve();

        let (tab_title, tab_subtitle) = match tab {
            SettingsTab::General => (
                t(lang, "General Settings", "Основные настройки"),
                t(lang, "Language, startup behavior, sounds and screenshot storage", "Язык интерфейса, автозапуск, системные звуки и путь сохранения"),
            ),
            SettingsTab::Capture => (
                t(lang, "Capture Parameters", "Параметры захвата"),
                t(lang, "Overlay tools, dimensions badge, pixel loupe and OCR text recognition", "Инструменты оверлея, размеры, пиксельная лупа и распознавание текста (OCR)"),
            ),
            SettingsTab::Shortcuts => (
                t(lang, "Keyboard Shortcuts", "Горячие клавиши"),
                t(lang, "Configurable global hotkeys and editor keybindings reference", "Глобальные сочетания клавиш и справочник горячих клавиш оверлея"),
            ),
            SettingsTab::About => (
                t(lang, "About Pixora", "О программе Pixora"),
                t(lang, "Version details, updates and open source repository", "Информация о сборке, авторские права и проверка обновлений"),
            ),
        };

        div()
            .id("settings-root")
            .size_full()
            .bg(theme.bg_dark)
            .text_color(theme.text_primary)
            .flex()
            .flex_row()
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
            // Left Navigation Sidebar (190px)
            .child(
                div()
                    .w(px(190.0))
                    .flex_shrink_0()
                    .bg(theme.bg_surface)
                    .border_r_1()
                    .border_color(theme.border_subtle)
                    .flex()
                    .flex_col()
                    .justify_start()
                    .p_3p5()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3p5()
                            // Brand header
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2p5()
                                    .px_1p5()
                                    .py_1()
                                    .child(
                                        div()
                                            .size(px(28.0))
                                            .flex_shrink_0()
                                            .rounded_lg()
                                            .overflow_hidden()
                                            .child(
                                                img("icons/app-icon.png")
                                                    .size(px(28.0)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(theme.text_primary)
                                                    .child("Pixora"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.accent)
                                                    .font_weight(gpui::FontWeight::MEDIUM)
                                                    .child(concat!("v", env!("CARGO_PKG_VERSION"))),
                                            ),
                                    ),
                            )
                            // Navigation list
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(render_sidebar_tab(
                                        "tab-general",
                                        "icons/settings.svg",
                                        t(lang, "General", "Основные"),
                                        tab == SettingsTab::General,
                                        cx.listener(|this, _, _, cx| {
                                            this.current_tab = SettingsTab::General;
                                            cx.notify();
                                        }),
                                    ))
                                    .child(render_sidebar_tab(
                                        "tab-capture",
                                        "icons/camera.svg",
                                        t(lang, "Capture", "Захват"),
                                        tab == SettingsTab::Capture,
                                        cx.listener(|this, _, _, cx| {
                                            this.current_tab = SettingsTab::Capture;
                                            cx.notify();
                                        }),
                                    ))
                                    .child(render_sidebar_tab(
                                        "tab-shortcuts",
                                        "icons/keyboard.svg",
                                        t(lang, "Shortcuts", "Горячие клавиши"),
                                        tab == SettingsTab::Shortcuts,
                                        cx.listener(|this, _, _, cx| {
                                            this.current_tab = SettingsTab::Shortcuts;
                                            cx.notify();
                                        }),
                                    ))
                                    .child(render_sidebar_tab(
                                        "tab-about",
                                        "icons/info.svg",
                                        t(lang, "About", "О программе"),
                                        tab == SettingsTab::About,
                                        cx.listener(|this, _, _, cx| {
                                            this.current_tab = SettingsTab::About;
                                            cx.notify();
                                        }),
                                    )),
                            ),
                    ),
            )
            // Right Content Area
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    // Content Header Bar
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
                            .bg(theme.bg_dark)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .flex()
                                    .flex_col()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(theme.text_primary)
                                            .child(tab_title),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.text_muted)
                                            .child(tab_subtitle),
                                    ),
                            ),
                    )
                    // Scrollable content area
                    .child(
                        div()
                            .id("settings-scroll-area")
                            .flex_1()
                            .min_h(px(0.0))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll_handle)
                            .px_6()
                            .py_5()
                            .pb_8()
                            .child(match tab {
                                SettingsTab::General => self.render_general_tab(cx),
                                SettingsTab::Capture => self.render_capture_tab(cx),
                                SettingsTab::Shortcuts => self.render_shortcuts_tab(cx),
                                SettingsTab::About => self.render_about_tab(cx),
                            }),
                    ),
            )
    }
}

impl SettingsWindowView {
    fn render_general_tab(&self, cx: &mut Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let lang = self.config.language.resolve();
        let save_dir = self.config.save_directory.to_string_lossy().to_string();

        let system_lang = detect_system_language();
        let auto_label = match system_lang {
            AppLanguage::Ru => t(lang, "Auto (RU)", "Авто (RU)"),
            AppLanguage::En => t(lang, "Auto (EN)", "Авто (EN)"),
        };

        div()
            .flex()
            .flex_col()
            .gap_4()
            // Section 1: Language
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(render_section_title(t(lang, "Interface Language", "Язык интерфейса")))
                    .child(
                        div()
                            .rounded_xl()
                            .bg(theme.bg_surface)
                            .border_1()
                            .border_color(theme.border_subtle)
                            .px_4()
                            .py_3()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .mr_3()
                                    .flex()
                                    .flex_col()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .text_color(theme.text_primary)
                                            .child(t(lang, "Display Language", "Язык отображения")),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.text_secondary)
                                            .child(t(
                                                lang,
                                                if cfg!(windows) {
                                                    "Choose program language or follow Windows locale"
                                                } else {
                                                    "Choose program language or follow system locale"
                                                },
                                                if cfg!(windows) {
                                                    "Язык программы или язык Windows автоматически"
                                                } else {
                                                    "Язык программы или язык системы автоматически"
                                                },
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .p_1()
                                    .rounded_lg()
                                    .bg(theme.bg_secondary)
                                    .border_1()
                                    .border_color(theme.border_subtle)
                                    .child(render_segmented_chip(
                                        "lang-auto",
                                        auto_label,
                                        self.config.language == LanguageChoice::Auto,
                                        cx.listener(|this, _, _, cx| {
                                            this.config.language = LanguageChoice::Auto;
                                            this.save_settings(cx);
                                        }),
                                    ))
                                    .child(render_segmented_chip(
                                        "lang-ru",
                                        "Русский",
                                        self.config.language == LanguageChoice::Ru,
                                        cx.listener(|this, _, _, cx| {
                                            this.config.language = LanguageChoice::Ru;
                                            this.save_settings(cx);
                                        }),
                                    ))
                                    .child(render_segmented_chip(
                                        "lang-en",
                                        "English",
                                        self.config.language == LanguageChoice::En,
                                        cx.listener(|this, _, _, cx| {
                                            this.config.language = LanguageChoice::En;
                                            this.save_settings(cx);
                                        }),
                                    )),
                            ),
                    ),
            )
            // Section 2: Behavior & Startup
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(render_section_title(t(lang, "System & Behavior", "Система и поведение")))
                    .child(
                        div()
                            .rounded_xl()
                            .bg(theme.bg_surface)
                            .border_1()
                            .border_color(theme.border_subtle)
                            .overflow_hidden()
                            // Row 1: Startup
                            .child(render_setting_row(
                                t(
                                    lang,
                                    if cfg!(windows) { "Run at Windows startup" } else { "Run at system startup" },
                                    if cfg!(windows) { "Автозапуск при старте Windows" } else { "Автозапуск при входе в систему" },
                                ),
                                t(
                                    lang,
                                    "Launch Pixora automatically in background when logging in",
                                    "Автоматически запускать Pixora в фоне при входе в систему",
                                ),
                                render_switch(
                                    "toggle-startup",
                                    self.config.run_at_startup,
                                    cx.listener(|this, _, _, cx| {
                                        this.config.run_at_startup = !this.config.run_at_startup;
                                        this.save_settings(cx);
                                    }),
                                ),
                            ))
                            .child(render_card_divider(theme))
                            // Row 2: Auto-copy
                            .child(render_setting_row(
                                t(lang, "Auto-copy to clipboard", "Автокопирование в буфер"),
                                t(
                                    lang,
                                    "Copy screenshot immediately after selection or capture",
                                    "Копировать снимок в буфер сразу после выделения или захвата",
                                ),
                                render_switch(
                                    "toggle-autocopy",
                                    self.config.auto_copy_to_clipboard,
                                    cx.listener(|this, _, _, cx| {
                                        this.config.auto_copy_to_clipboard = !this.config.auto_copy_to_clipboard;
                                        this.save_settings(cx);
                                    }),
                                ),
                            ))
                            .child(render_card_divider(theme))
                            // Row 3: Sound effects
                            .child(render_setting_row(
                                t(lang, "Sound effects", "Звуковые эффекты"),
                                t(
                                    lang,
                                    "Play shutter sound upon capture (ScreenFX)",
                                    "Воспроизводить щелчок затвора при создании снимка",
                                ),
                                div()
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .gap_2p5()
                                    .when(self.config.sound_effects, |p| {
                                        p.child(
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
                                                        .child(t(lang, "Test", "Прослушать")),
                                                ),
                                        )
                                    })
                                    .child(render_switch(
                                        "toggle-sound",
                                        self.config.sound_effects,
                                        cx.listener(|this, _, _, cx| {
                                            this.config.sound_effects = !this.config.sound_effects;
                                            if this.config.sound_effects {
                                                crate::system::sound::play_capture_sound();
                                            }
                                            this.save_settings(cx);
                                        }),
                                    )),
                            )),
                    ),
            )
            // Section 3: Screenshots Storage
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(render_section_title(t(lang, "Screenshots Storage", "Сохранение снимков")))
                    .child(
                        div()
                            .rounded_xl()
                            .bg(theme.bg_surface)
                            .border_1()
                            .border_color(theme.border_subtle)
                            .overflow_hidden()
                            // Folder row (compact single-row design)
                            .child(
                                div()
                                    .px_4()
                                    .py_3()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_3()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.0))
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(gpui::FontWeight::MEDIUM)
                                                    .text_color(theme.text_primary)
                                                    .child(t(lang, "Screenshots Directory", "Папка со снимками")),
                                            )
                                            .child(
                                                div()
                                                    .overflow_hidden()
                                                    .px_2p5()
                                                    .py_1()
                                                    .rounded_md()
                                                    .bg(theme.bg_secondary)
                                                    .border_1()
                                                    .border_color(theme.border_subtle)
                                                    .text_xs()
                                                    .text_color(theme.text_secondary)
                                                    .child(save_dir),
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
                                                    .id("btn-browse-folder")
                                                    .px_3()
                                                    .py_1p5()
                                                    .rounded_md()
                                                    .bg(theme.bg_secondary)
                                                    .border_1()
                                                    .border_color(theme.border_subtle)
                                                    .hover(|s| s.bg(theme.bg_hover).border_color(theme.accent))
                                                    .cursor_pointer()
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        if let Some(new_path) = choose_folder_dialog(&this.config.save_directory) {
                                                            this.config.save_directory = new_path;
                                                            this.save_settings(cx);
                                                        }
                                                    }))
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(gpui::FontWeight::MEDIUM)
                                                            .text_color(theme.text_primary)
                                                            .child(t(lang, "Browse...", "Обзор...")),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .id("btn-open-folder")
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
                                                            .size(px(13.0))
                                                            .text_color(theme.text_secondary),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.text_primary)
                                                            .child(t(lang, "Open Folder", "Открыть")),
                                                    ),
                                            ),
                                    ),
                            )
                            .child(render_card_divider(theme))
                            // Auto-save toggle
                            .child(render_setting_row(
                                t(lang, "Auto-save screenshots to file", "Автосохранение файлов"),
                                t(
                                    lang,
                                    "Save capture automatically to disk without prompting for filename",
                                    "Автоматически сохранять файл на диск без запроса имени",
                                ),
                                render_switch(
                                    "toggle-autosave",
                                    self.config.auto_save_file,
                                    cx.listener(|this, _, _, cx| {
                                        this.config.auto_save_file = !this.config.auto_save_file;
                                        this.save_settings(cx);
                                    }),
                                ),
                            )),
                    ),
            )
    }

    fn render_capture_tab(&self, cx: &mut Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let lang = self.config.language.resolve();

        div()
            .flex()
            .flex_col()
            .gap_4()
            // Section 1: Overlay Tools & Snapping
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(render_section_title(t(lang, "Interactive Overlay & Snapping", "Интерактивный оверлей и привязка")))
                    .child(
                        div()
                            .rounded_xl()
                            .bg(theme.bg_surface)
                            .border_1()
                            .border_color(theme.border_subtle)
                            .overflow_hidden()
                            // Row 1: Dimensions badge
                            .child(render_setting_row(
                                t(lang, "Show selection dimensions", "Размеры выделенной области"),
                                t(
                                    lang,
                                    "Display width and height pixel badge beside selection box (1920 × 1080)",
                                    "Отображать бейдж с шириной и высотой рядом с рамкой",
                                ),
                                render_switch(
                                    "toggle-dimensions",
                                    self.config.show_dimensions,
                                    cx.listener(|this, _, _, cx| {
                                        this.config.show_dimensions = !this.config.show_dimensions;
                                        this.save_settings(cx);
                                    }),
                                ),
                            ))
                            .child(render_card_divider(theme))
                            // Row 2: Loupe & Pixel magnifier
                            .child(render_setting_row(
                                t(lang, "Pixel magnifier & loupe", "Экранная пиксельная лупа"),
                                t(
                                    lang,
                                    "Display zoomed pixel grid, coordinates and HEX color near the cursor",
                                    "Отображать увеличенную сетку пикселей, координаты и HEX-код цвета",
                                ),
                                render_switch(
                                    "toggle-magnifier",
                                    self.config.show_magnifier,
                                    cx.listener(|this, _, _, cx| {
                                        this.config.show_magnifier = !this.config.show_magnifier;
                                        this.save_settings(cx);
                                    }),
                                ),
                            ))
                            .child(render_card_divider(theme))
                            // Row 3: Window snapping
                            .child(render_setting_row(
                                t(lang, "Window snapping & auto-detection", "Привязка к окнам (Window Snapping)"),
                                t(
                                    lang,
                                    "Automatically highlight and snap to windows on hover and click",
                                    "Автоматически подсвечивать и выбирать границы окон при наведении и клике",
                                ),
                                render_switch(
                                    "toggle-snapping",
                                    self.config.window_snapping,
                                    cx.listener(|this, _, _, cx| {
                                        this.config.window_snapping = !this.config.window_snapping;
                                        this.save_settings(cx);
                                    }),
                                ),
                            )),
                    ),
            )
            // Section 2: Text Recognition (OCR)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(render_section_title(t(lang, "Text Recognition (OCR)", "Распознавание текста (OCR)")))
                    .child(
                        div()
                            .rounded_xl()
                            .bg(theme.bg_surface)
                            .border_1()
                            .border_color(theme.border_subtle)
                            .px_4()
                            .py_3()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .mr_3()
                                    .flex()
                                    .flex_col()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .text_color(theme.text_primary)
                                            .child(t(lang, "OCR Recognition Language", "Язык распознавания текста")),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.text_secondary)
                                            .child(t(
                                                lang,
                                                if cfg!(windows) {
                                                    "Windows OCR model for text extraction (supports hybrid mode)"
                                                } else {
                                                    "Tesseract OCR model for text extraction (supports hybrid mode)"
                                                },
                                                if cfg!(windows) {
                                                    "Модель Windows OCR (поддерживает гибридный режим RU+EN)"
                                                } else {
                                                    "Модель Tesseract OCR (поддерживает гибридный режим RU+EN)"
                                                },
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .p_1()
                                    .rounded_lg()
                                    .bg(theme.bg_secondary)
                                    .border_1()
                                    .border_color(theme.border_subtle)
                                    .child(render_segmented_chip(
                                        "chip-ocr-auto",
                                        t(lang, "Auto (RU+EN)", "Авто (RU+EN)"),
                                        self.config.ocr_language == "auto",
                                        cx.listener(|this, _, _, cx| {
                                            this.config.ocr_language = "auto".to_string();
                                            this.save_settings(cx);
                                        }),
                                    ))
                                    .child(render_segmented_chip(
                                        "chip-ocr-ru",
                                        "Русский",
                                        self.config.ocr_language == "ru",
                                        cx.listener(|this, _, _, cx| {
                                            this.config.ocr_language = "ru".to_string();
                                            this.save_settings(cx);
                                        }),
                                    ))
                                    .child(render_segmented_chip(
                                        "chip-ocr-en",
                                        "English",
                                        self.config.ocr_language == "en",
                                        cx.listener(|this, _, _, cx| {
                                            this.config.ocr_language = "en".to_string();
                                            this.save_settings(cx);
                                        }),
                                    )),
                            ),
                    ),
            )
    }

    fn render_shortcuts_tab(&self, cx: &mut Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let lang = self.config.language.resolve();

        let global_shortcuts = [
            (
                t(lang, "Region Capture", "Снимок области"),
                t(lang, "Capture selected area with interactive overlay", "Захват выбранной области экрана с панелью инструментов"),
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
            // Section 1: Global Rebindable Hotkeys
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(render_section_title(t(lang, "Global Hotkeys", "Глобальные горячие клавиши")))
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
                                        this.config.hotkey_region = "PrintScreen".to_string();
                                        this.config.hotkey_fullscreen = "Shift + PrintScreen".to_string();
                                        this.config.hotkey_window = "Alt + PrintScreen".to_string();
                                        this.rebinding_target = None;
                                        this.save_settings(cx);
                                    }))
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .text_color(theme.text_secondary)
                                            .child(t(lang, "Reset Defaults", "Сбросить по умолчанию")),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .rounded_xl()
                            .bg(theme.bg_surface)
                            .border_1()
                            .border_color(theme.border_subtle)
                            .overflow_hidden()
                            .children(global_shortcuts.into_iter().enumerate().map(|(idx, (title, desc, key_str, target))| {
                                let is_rebinding = self.rebinding_target == Some(target);
                                div()
                                    .flex()
                                    .flex_col()
                                    .when(idx > 0, |p| p.child(render_card_divider(theme)))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .px_4()
                                            .py_3()
                                            .bg(if is_rebinding {
                                                rgba(0x8174FF12)
                                            } else {
                                                theme.bg_surface
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
                                                            .text_sm()
                                                            .font_weight(gpui::FontWeight::MEDIUM)
                                                            .text_color(theme.text_primary)
                                                            .child(title),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.text_secondary)
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
                                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                                            .text_color(if is_rebinding {
                                                                theme.accent
                                                            } else {
                                                                theme.text_primary
                                                            })
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
                                                            .px_2p5()
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
                                            ),
                                    )
                            })),
                    ),
            )
            // Section 2: In-Overlay Editor Shortcuts Reference
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(render_section_title(t(lang, "In-Editor Keybindings", "Горячие клавиши в оверлее")))
                    .child(
                        div()
                            .rounded_xl()
                            .bg(theme.bg_surface)
                            .border_1()
                            .border_color(theme.border_subtle)
                            .overflow_hidden()
                            .children(editor_shortcuts.into_iter().enumerate().map(|(idx, (action, key))| {
                                div()
                                    .flex()
                                    .flex_col()
                                    .when(idx > 0, |p| p.child(render_card_divider(theme)))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .px_4()
                                            .py_2()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.text_secondary)
                                                    .child(action),
                                            )
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_md()
                                                    .bg(theme.bg_secondary)
                                                    .border_1()
                                                    .border_color(theme.border_subtle)
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::MEDIUM)
                                                    .text_color(theme.accent)
                                                    .child(key),
                                            ),
                                    )
                            })),
                    ),
            )
    }

    fn render_about_tab(&self, cx: &mut Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let lang = self.config.language.resolve();

        div()
            .flex()
            .flex_col()
            .gap_4()
            // Section 1: Hero Info Card
            .child(
                div()
                    .rounded_xl()
                    .bg(theme.bg_surface)
                    .border_1()
                    .border_color(theme.border_subtle)
                    .p_4()
                    .flex()
                    .items_center()
                    .gap_3p5()
                    .child(
                        div()
                            .size(px(48.0))
                            .flex_shrink_0()
                            .rounded_xl()
                            .overflow_hidden()
                            .child(
                                img("icons/app-icon.png")
                                    .size(px(48.0)),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(theme.text_primary)
                                            .child("Pixora"),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_full()
                                            .bg(rgba(0x8174FF1A))
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(theme.accent)
                                            .child(concat!("v", env!("CARGO_PKG_VERSION"))),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_secondary)
                                    .child(t(
                                        lang,
                                        if cfg!(windows) {
                                            "GPU-Accelerated Screen Capture & Annotation Studio for Windows 10 & 11"
                                        } else {
                                            "GPU-Accelerated Screen Capture & Annotation Studio for Linux (Wayland & X11)"
                                        },
                                        if cfg!(windows) {
                                            "GPU-ускоренная студия создания скриншотов и аннотаций для Windows 10 и 11"
                                        } else {
                                            "GPU-ускоренная студия создания скриншотов и аннотаций для Linux (Wayland и X11)"
                                        },
                                    )),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child(t(
                                        lang,
                                        if cfg!(windows) {
                                            "Built with pure Rust & GPUI (DirectX) • Author: Onzi • 2026"
                                        } else {
                                            "Built with pure Rust & GPUI (Vulkan) • Author: Onzi • 2026"
                                        },
                                        if cfg!(windows) {
                                            "Разработано на чистом Rust и GPUI (DirectX) • Автор: Onzi • 2026"
                                        } else {
                                            "Разработано на чистом Rust и GPUI (Vulkan) • Автор: Onzi • 2026"
                                        },
                                    )),
                            ),
                    ),
            )
            // Section 2: Repository & Updates
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(render_section_title(t(lang, "Source & Updates", "Исходный код и обновления")))
                    .child(
                        div()
                            .rounded_xl()
                            .bg(theme.bg_surface)
                            .border_1()
                            .border_color(theme.border_subtle)
                            .overflow_hidden()
                            // Row 1: GitHub Link
                            .child(
                                div()
                                    .px_4()
                                    .py_3()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.0))
                                            .mr_3()
                                            .flex()
                                            .flex_col()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(gpui::FontWeight::MEDIUM)
                                                    .text_color(theme.text_primary)
                                                    .child(t(lang, "GitHub Repository", "Репозиторий GitHub")),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.accent)
                                                    .child(GITHUB_REPO_URL),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("btn-open-github")
                                            .flex_shrink_0()
                                            .px_3()
                                            .py_1p5()
                                            .rounded_md()
                                            .bg(theme.bg_secondary)
                                            .border_1()
                                            .border_color(theme.border_subtle)
                                            .hover(|s| s.bg(theme.bg_hover))
                                            .cursor_pointer()
                                            .on_click(cx.listener(|_, _, _, _| {
                                                open_url(GITHUB_REPO_URL);
                                            }))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::MEDIUM)
                                                    .text_color(theme.text_primary)
                                                    .child(t(lang, "Open in Browser", "Открыть в браузере")),
                                            ),
                                    ),
                            )
                            .child(render_card_divider(theme))
                            // Row 2: Check Updates
                            .child(
                                div()
                                    .px_4()
                                    .py_3()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.0))
                                            .mr_3()
                                            .flex()
                                            .flex_col()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(gpui::FontWeight::MEDIUM)
                                                    .text_color(theme.text_primary)
                                                    .child(t(lang, "Release Updates", "Проверка обновлений")),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.text_secondary)
                                                    .child(match &self.update_status {
                                                        UpdateStatus::Checking => t(lang, "Checking for latest release...", "Проверка наличия обновлений...").to_string(),
                                                        UpdateStatus::UpToDate { version } => format!("{}: v{}", t(lang, "Latest version installed", "Установлена последняя версия"), version),
                                                        UpdateStatus::Available { latest, .. } => format!("{}: v{}", t(lang, "New release available", "Доступен новый релиз"), latest),
                                                        UpdateStatus::Error(_) => t(lang, "Unable to check updates", "Не удалось проверить обновления").to_string(),
                                                        UpdateStatus::Idle => t(lang, "Check GitHub for new releases", "Проверить наличие новых версий на GitHub").to_string(),
                                                    }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("btn-check-updates")
                                            .flex_shrink_0()
                                            .px_3()
                                            .py_1p5()
                                            .rounded_md()
                                            .bg(theme.accent)
                                            .hover(|s| s.bg(theme.accent_hover))
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.check_for_updates(cx);
                                            }))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                                    .text_color(rgb(0xFFFFFF))
                                                    .child(t(lang, "Check Now", "Проверить сейчас")),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}

fn render_section_title(title: &'static str) -> impl IntoElement {
    div()
        .text_xs()
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(rgb(0x8A8A99))
        .child(title)
}

fn render_card_divider(theme: PixoraTheme) -> impl IntoElement {
    div()
        .h(px(1.0))
        .w_full()
        .bg(theme.border_subtle)
}

fn render_setting_row(
    title: &'static str,
    desc: &'static str,
    control: impl IntoElement,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .px_4()
        .py_3()
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .mr_3()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(rgb(0xF2F2F5))
                        .child(title),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x8A8A99))
                        .child(desc),
                ),
        )
        .child(control)
}

fn render_sidebar_tab(
    id: &'static str,
    icon_path: &'static str,
    label: &'static str,
    is_active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let active_bg = rgba(0x8174FF1F);
    let active_text = rgb(0x8174FF);
    let inactive_text = rgb(0xA2A2B0);

    div()
        .id(id)
        .flex()
        .items_center()
        .gap_2p5()
        .px_3()
        .py_2()
        .rounded_lg()
        .bg(if is_active { active_bg } else { rgba(0x00000000) })
        .hover(|s| s.bg(if is_active { active_bg } else { rgb(0x22222C) }))
        .cursor_pointer()
        .on_click(on_click)
        .child(
            svg()
                .path(icon_path)
                .size(px(16.0))
                .text_color(if is_active { active_text } else { inactive_text }),
        )
        .child(
            div()
                .text_xs()
                .font_weight(if is_active {
                    gpui::FontWeight::SEMIBOLD
                } else {
                    gpui::FontWeight::MEDIUM
                })
                .text_color(if is_active { active_text } else { inactive_text })
                .child(label),
        )
}

fn render_switch(
    id: &'static str,
    is_on: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
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
            rgb(0x282834)
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
        )
}

fn render_segmented_chip(
    id: &'static str,
    label: impl Into<gpui::SharedString>,
    is_selected: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex_shrink_0()
        .px_2p5()
        .py_1()
        .rounded_md()
        .bg(if is_selected {
            rgb(0x8174FF)
        } else {
            rgba(0x00000000)
        })
        .hover(|s| s.bg(if is_selected { rgb(0x8174FF) } else { rgb(0x282834) }))
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
                    rgb(0xFFFFFF)
                } else {
                    rgb(0xA2A2B0)
                })
                .child(label.into()),
        )
}