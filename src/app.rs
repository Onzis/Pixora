use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::time::Duration;

use gpui::{
    App, AppContext, AsyncApp, Bounds, Context, IntoElement, Window, WindowBackgroundAppearance,
    WindowBounds, WindowKind, WindowOptions, div, point, px, size,
};

use crate::capture::screen::capture_virtual_screen;
use crate::capture::window::capture_active_window;
use crate::config::AppConfig;
use crate::image::clipboard::copy_image_to_clipboard;
use crate::image::export::auto_save_screenshot;
use crate::overlay::overlay::ScreenOverlayView;
use crate::settings::storage::open_folder_in_explorer;
use crate::settings::window::{SettingsTab, SettingsWindowView};
use crate::shortcuts::hotkeys::{HotkeyAction, start_hotkey_listener};
use crate::system::tray::{show_tray_notification, start_system_tray, TrayEvent};
use crate::system::windows::play_capture_sound;

pub static IS_OVERLAY_OPEN: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug)]
pub enum AppEvent {
    TriggerRegionCapture,
    TriggerFullscreenCapture,
    TriggerActiveWindowCapture,
    OpenSettings,
    OpenFolder,
    CheckUpdates,
    Exit,
}

pub struct PixoraApp {
    pub config: AppConfig,
}

struct DaemonBackgroundView;

impl gpui::Render for DaemonBackgroundView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl PixoraApp {
    pub fn new() -> Self {
        Self {
            config: AppConfig::load(),
        }
    }

    pub fn start(&self, cx: &mut App) {
        // Keep a persistent hidden 1x1 window so GPUI event loop never quits
        // when capture overlay or settings window is closed.
        let hidden_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(-10000.0), px(-10000.0)),
                size: size(px(1.0), px(1.0)),
            })),
            titlebar: None,
            focus: false,
            show: false,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            ..Default::default()
        };

        let _ = cx.open_window(hidden_options, |_, cx| {
            cx.new(|_| DaemonBackgroundView)
        });

        let (tx, rx) = channel::<AppEvent>();

        // 1. Start Hotkey listener
        let hotkey_tx = tx.clone();
        let (hk_sender, hk_receiver) = channel::<HotkeyAction>();
        start_hotkey_listener(hk_sender);

        std::thread::spawn(move || {
            while let Ok(action) = hk_receiver.recv() {
                match action {
                    HotkeyAction::CaptureRegion => {
                        let _ = hotkey_tx.send(AppEvent::TriggerRegionCapture);
                    }
                    HotkeyAction::CaptureFullscreen => {
                        let _ = hotkey_tx.send(AppEvent::TriggerFullscreenCapture);
                    }
                    HotkeyAction::CaptureActiveWindow => {
                        let _ = hotkey_tx.send(AppEvent::TriggerActiveWindowCapture);
                    }
                }
            }
        });

        // 2. Start System Tray
        let tray_tx = tx.clone();
        let (t_sender, t_receiver) = channel::<TrayEvent>();
        start_system_tray(t_sender);

        std::thread::spawn(move || {
            while let Ok(event) = t_receiver.recv() {
                match event {
                    TrayEvent::CaptureRegion => {
                        let _ = tray_tx.send(AppEvent::TriggerRegionCapture);
                    }
                    TrayEvent::CaptureFullscreen => {
                        let _ = tray_tx.send(AppEvent::TriggerFullscreenCapture);
                    }
                    TrayEvent::OpenSettings => {
                        let _ = tray_tx.send(AppEvent::OpenSettings);
                    }
                    TrayEvent::OpenFolder => {
                        let _ = tray_tx.send(AppEvent::OpenFolder);
                    }
                    TrayEvent::CheckUpdates => {
                        let _ = tray_tx.send(AppEvent::CheckUpdates);
                    }
                    TrayEvent::Exit => {
                        let _ = tray_tx.send(AppEvent::Exit);
                    }
                }
            }
        });

        // 3. Spawn background event poller
        cx.spawn(|cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                loop {
                    while let Ok(event) = rx.try_recv() {
                        let _ = cx.update(|app| match event {
                            AppEvent::TriggerRegionCapture => {
                                Self::open_capture_overlay(app);
                            }
                            AppEvent::TriggerFullscreenCapture => {
                                Self::capture_fullscreen_direct(app);
                            }
                            AppEvent::TriggerActiveWindowCapture => {
                                Self::capture_active_window_direct(app);
                            }
                            AppEvent::OpenSettings => {
                                Self::open_settings_window(app);
                            }
                            AppEvent::OpenFolder => {
                                let cfg = AppConfig::load();
                                open_folder_in_explorer(&cfg.save_directory);
                            }
                            AppEvent::CheckUpdates => {
                                Self::handle_check_updates_from_tray(app);
                            }
                            AppEvent::Exit => {
                                app.quit();
                            }
                        });
                    }

                    cx.background_executor().timer(Duration::from_millis(40)).await;
                }
            }
        })
        .detach();

        // 4. Background auto-update check on startup if enabled
        let startup_cfg = AppConfig::load();
        if startup_cfg.auto_check_updates {
            let lang = startup_cfg.language.resolve();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(3));
                if let Ok(info) = crate::system::updater::check_latest_release() {
                    if info.is_update_available {
                        crate::system::updater::set_global_update_status(
                            crate::system::updater::UpdateStatus::Available {
                                current: info.current_version.clone(),
                                latest: info.latest_version.clone(),
                                release_url: info.release_url.clone(),
                            },
                        );
                        let title = "Pixora";
                        let msg = format!("{}: v{}", crate::i18n::t(lang, "New version available", "Доступно обновление"), info.latest_version);
                        show_tray_notification(title, &msg);
                    } else {
                        crate::system::updater::set_global_update_status(
                            crate::system::updater::UpdateStatus::UpToDate {
                                version: info.latest_version,
                            },
                        );
                    }
                }
            });
        }
    }

    pub fn open_capture_overlay(cx: &mut App) {
        if IS_OVERLAY_OPEN.swap(true, Ordering::SeqCst) {
            return;
        }

        let capture = match capture_virtual_screen() {
            Ok(cap) => cap,
            Err(err) => {
                IS_OVERLAY_OPEN.store(false, Ordering::SeqCst);
                eprintln!("Capture error: {}", err);
                return;
            }
        };

        let config = AppConfig::load();
        let screen_w = capture.bounds.width as f32;
        let screen_h = capture.bounds.height as f32;

        let bounds = Bounds {
            origin: point(px(capture.bounds.x as f32), px(capture.bounds.y as f32)),
            size: size(px(screen_w), px(screen_h)),
        };

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Fullscreen(bounds)),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("Pixora Overlay".into()),
                appears_transparent: false,
                traffic_light_position: None,
            }),
            focus: true,
            show: true,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            window_background: WindowBackgroundAppearance::Opaque,
            ..Default::default()
        };

        let cap_bounds = capture.bounds;
        let _ = cx.open_window(window_options, |window, cx| {
            #[cfg(windows)]
            {
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                use windows::Win32::Foundation::HWND;
                use windows::Win32::UI::WindowsAndMessaging::{
                    BringWindowToTop, GetWindowLongW, GWL_EXSTYLE, GWL_STYLE, HWND_TOPMOST,
                    SetForegroundWindow, SetWindowLongW, SetWindowPos, SWP_FRAMECHANGED,
                    SWP_SHOWWINDOW, WS_EX_TOPMOST, WS_POPUP,
                };

                if let Ok(wh) = window.window_handle() {
                    if let RawWindowHandle::Win32(handle) = wh.as_raw() {
                        let hwnd = HWND(handle.hwnd.get() as *mut _);
                        unsafe {
                            let style = GetWindowLongW(hwnd, GWL_STYLE);
                            let _ = SetWindowLongW(
                                hwnd,
                                GWL_STYLE,
                                (style | (WS_POPUP.0 as i32)) & !(0x00C00000 | 0x00040000),
                            );
                            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
                            let _ = SetWindowLongW(
                                hwnd,
                                GWL_EXSTYLE,
                                ex_style | (WS_EX_TOPMOST.0 as i32),
                            );

                            let _ = SetWindowPos(
                                hwnd,
                                Some(HWND_TOPMOST),
                                cap_bounds.x,
                                cap_bounds.y,
                                cap_bounds.width as i32,
                                cap_bounds.height as i32,
                                SWP_SHOWWINDOW | SWP_FRAMECHANGED,
                            );

                            let _ = BringWindowToTop(hwnd);
                            let _ = SetForegroundWindow(hwnd);
                        }
                    }
                }
            }

            let view = cx.new(|cx| ScreenOverlayView::new(capture, config, cx));
            window.focus(&view.read(cx).focus_handle);
            view
        });
    }

    pub fn capture_fullscreen_direct(_cx: &mut App) {
        if let Ok(cap) = capture_virtual_screen() {
            let config = AppConfig::load();
            if config.auto_copy_to_clipboard {
                let _ = copy_image_to_clipboard(&cap.image);
            }
            if config.auto_save_file {
                let _ = auto_save_screenshot(&cap.image, &config);
            }
            if config.sound_effects {
                play_capture_sound();
            }
        }
    }

    pub fn capture_active_window_direct(cx: &mut App) {
        if let Ok((img, _cap)) = capture_active_window() {
            let config = AppConfig::load();
            if config.auto_copy_to_clipboard {
                let _ = copy_image_to_clipboard(&img);
            }
            if config.sound_effects {
                play_capture_sound();
            }
        } else {
            Self::open_capture_overlay(cx);
        }
    }

    pub fn open_settings_window(cx: &mut App) {
        Self::open_settings_window_with_tab(cx, SettingsTab::General);
    }

    pub fn open_settings_window_with_tab(cx: &mut App, tab: SettingsTab) {
        let config = AppConfig::load();
        let bounds = Bounds::centered(None, size(px(800.0), px(580.0)), cx);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("Pixora Settings".into()),
                appears_transparent: false,
                traffic_light_position: None,
            }),
            focus: true,
            show: true,
            kind: WindowKind::Normal,
            is_movable: true,
            is_resizable: true,
            window_background: WindowBackgroundAppearance::Opaque,
            ..Default::default()
        };

        let _ = cx.open_window(window_options, |_, cx| {
            cx.new(|cx| SettingsWindowView::with_tab(config, tab, cx))
        });
    }

    pub fn handle_check_updates_from_tray(cx: &mut App) {
        let config = AppConfig::load();
        let lang = config.language.resolve();

        Self::open_settings_window_with_tab(cx, SettingsTab::About);

        std::thread::spawn(move || {
            match crate::system::updater::check_latest_release_status() {
                crate::system::updater::UpdateStatus::Available { latest, .. } => {
                    let title = "Pixora";
                    let msg = format!("{}: v{}", crate::i18n::t(lang, "New version available", "Доступно обновление"), latest);
                    show_tray_notification(title, &msg);
                }
                crate::system::updater::UpdateStatus::UpToDate { version } => {
                    let title = "Pixora";
                    let msg = format!("{}: v{}", crate::i18n::t(lang, "Pixora is up to date", "Установлена актуальная версия"), version);
                    show_tray_notification(title, &msg);
                }
                crate::system::updater::UpdateStatus::Error(err) => {
                    let title = "Pixora";
                    let msg = format!("{}: {}", crate::i18n::t(lang, "Update check failed", "Ошибка проверки обновлений"), err);
                    show_tray_notification(title, &msg);
                }
                _ => {}
            }
        });
    }
}
