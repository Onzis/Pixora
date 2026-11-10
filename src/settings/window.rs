use crate::config::AppConfig;
use crate::settings::storage::{open_folder_in_explorer, set_run_at_startup};
use crate::shortcuts::hotkeys::{format_keystroke, update_global_hotkeys};
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
}

impl SettingsWindowView {
    pub fn new(config: AppConfig, cx: &mut Context<Self>) -> Self {
        Self {
            config,
            theme: PixoraTheme::default(),
            current_tab: SettingsTab::General,
            status_message: None,
            rebinding_target: None,
            scroll_handle: ScrollHandle::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn save_settings(&mut self, cx: &mut Context<Self>) {
        let _ = set_run_at_startup(self.config.run_at_startup);
        update_global_hotkeys(
            &self.config.hotkey_region,
            &self.config.hotkey_fullscreen,
            &self.config.hotkey_window,
        );
        if let Err(e) = self.config.save() {
            self.status_message = Some(format!("Error: {}", e));
        } else {
            self.status_message = Some("Settings saved successfully".to_string());
        }
        cx.notify();
    }
}

impl Render for SettingsWindowView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let tab = self.current_tab;

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
                    if ev.keystroke.key == "escape" {
                        this.rebinding_target = None;
                        this.status_message = Some("Shortcut reassignment cancelled".to_string());
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
                        this.status_message = Some(format!("Assigned: {}", hotkey_str));
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
                                    .bg(theme.accent)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0xFFFFFF))
                                    .child("P"),
                            )
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("Pixora Settings"),
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
                            .w(px(160.0))
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
                                "General",
                                tab == SettingsTab::General,
                                cx.listener(|this, _, _, cx| {
                                    this.current_tab = SettingsTab::General;
                                    cx.notify();
                                }),
                            ))
                            .child(render_tab_button(
                                "tab-capture",
                                "icons/camera.svg",
                                "Capture",
                                tab == SettingsTab::Capture,
                                cx.listener(|this, _, _, cx| {
                                    this.current_tab = SettingsTab::Capture;
                                    cx.notify();
                                }),
                            ))
                            .child(render_tab_button(
                                "tab-shortcuts",
                                "icons/keyboard.svg",
                                "Shortcuts",
                                tab == SettingsTab::Shortcuts,
                                cx.listener(|this, _, _, cx| {
                                    this.current_tab = SettingsTab::Shortcuts;
                                    cx.notify();
                                }),
                            ))
                            .child(render_tab_button(
                                "tab-about",
                                "icons/info.svg",
                                "About",
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
                                        SettingsTab::About => self.render_about_tab(),
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
                                                            .child("Save Settings"),
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
                                                        window.remove_window();
                                                    }))
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.text_secondary)
                                                            .child("Close"),
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
        let save_dir = self.config.save_directory.display().to_string();

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(render_toggle(
                "toggle-startup",
                "Run at Windows startup",
                "Launch Pixora automatically in the background when logging into Windows",
                self.config.run_at_startup,
                cx.listener(|this, _, _, cx| {
                    this.config.run_at_startup = !this.config.run_at_startup;
                    cx.notify();
                }),
            ))
            .child(render_toggle(
                "toggle-autocopy",
                "Auto-copy to clipboard",
                "Automatically copy screenshot to clipboard immediately after selection or capture",
                self.config.auto_copy_to_clipboard,
                cx.listener(|this, _, _, cx| {
                    this.config.auto_copy_to_clipboard = !this.config.auto_copy_to_clipboard;
                    cx.notify();
                }),
            ))
            .child(render_toggle(
                "toggle-sound",
                "Sound effects",
                "Play subtle camera shutter audio tone upon successful capture",
                self.config.sound_effects,
                cx.listener(|this, _, _, cx| {
                    this.config.sound_effects = !this.config.sound_effects;
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
                            .child("Screenshots Directory"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
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
                                            .child("Open Folder"),
                                    ),
                            ),
                    ),
            )
    }

    fn render_capture_tab(&self, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(render_toggle(
                "toggle-dimensions",
                "Show selection dimensions",
                "Display width and height pixel badge beside selection box (e.g. 1920 × 1080)",
                self.config.show_dimensions,
                cx.listener(|this, _, _, cx| {
                    this.config.show_dimensions = !this.config.show_dimensions;
                    cx.notify();
                }),
            ))
            .child(render_toggle(
                "toggle-magnifier",
                "Show pixel magnifier & loupe",
                "Display zoomed pixel grid, coordinates and HEX color inspector near the cursor",
                self.config.show_magnifier,
                cx.listener(|this, _, _, cx| {
                    this.config.show_magnifier = !this.config.show_magnifier;
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
                            .child("Default Stroke Width"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(render_size_chip(
                                "chip-2px",
                                "2px Thin",
                                (self.config.default_stroke_width - 2.0).abs() < 0.1,
                                cx.listener(|this, _, _, cx| {
                                    this.config.default_stroke_width = 2.0;
                                    cx.notify();
                                }),
                            ))
                            .child(render_size_chip(
                                "chip-4px",
                                "4px Medium",
                                (self.config.default_stroke_width - 4.0).abs() < 0.1,
                                cx.listener(|this, _, _, cx| {
                                    this.config.default_stroke_width = 4.0;
                                    cx.notify();
                                }),
                            ))
                            .child(render_size_chip(
                                "chip-8px",
                                "8px Bold",
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

        let global_shortcuts = [
            (
                "Region Capture",
                "Capture selected screen area with interactive overlay",
                &self.config.hotkey_region,
                ShortcutRebindTarget::Region,
            ),
            (
                "Fullscreen Capture",
                "Instantly capture entire screen without overlay",
                &self.config.hotkey_fullscreen,
                ShortcutRebindTarget::Fullscreen,
            ),
            (
                "Active Window Capture",
                "Capture the currently active foreground window directly",
                &self.config.hotkey_window,
                ShortcutRebindTarget::Window,
            ),
        ];

        let editor_shortcuts = [
            ("Copy to Clipboard", "Ctrl + C"),
            ("Save to PNG", "Ctrl + S"),
            ("Undo Action", "Ctrl + Z"),
            ("Redo Action", "Ctrl + Y"),
            ("Confirm / Finish", "Enter"),
            ("Cancel / Close", "Escape"),
            ("Brush Size Stepper", "Mouse Wheel / [ - ] [ + ]"),
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
                            .child("Global Hotkeys (Rebindable)"),
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
                                this.config.hotkey_region = "PrintScreen".to_string();
                                this.config.hotkey_fullscreen = "Shift + PrintScreen".to_string();
                                this.config.hotkey_window = "Alt + PrintScreen".to_string();
                                this.rebinding_target = None;
                                this.status_message = Some("Reset hotkeys to default".to_string());
                                this.save_settings(cx);
                            }))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_secondary)
                                    .child("Reset Defaults"),
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
                                                "Press keys... (Esc)".to_string()
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
                                                if this.rebinding_target == Some(target) {
                                                    this.rebinding_target = None;
                                                    this.status_message = None;
                                                } else {
                                                    this.rebinding_target = Some(target);
                                                    this.focus_handle.focus(window);
                                                    this.status_message = Some(
                                                        "Press any key combo... (Esc to cancel)".to_string(),
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
                                                    .child(if is_rebinding { "Listening" } else { "Change" }),
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
                            .child("In-Overlay Shortcuts"),
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

    fn render_about_tab(&self) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_base()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(0x8174FF))
                    .child(concat!("Pixora v", env!("CARGO_PKG_VERSION"), " — High-Performance Screen Capture")),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0xA2A2B0))
                    .child("Built with pure Rust and GPUI framework for Windows 10 & 11."),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x656575))
                    .child("Features: multi-monitor support, lossless PNG export, Windows OCR, smart loupe, rich annotation canvas, global hotkeys and tray integration. 100% offline and private."),
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
