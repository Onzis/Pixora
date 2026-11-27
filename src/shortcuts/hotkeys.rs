use std::sync::Mutex;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyAction {
    CaptureRegion,
    CaptureFullscreen,
    CaptureActiveWindow,
}

#[derive(Clone, Debug)]
pub enum HotkeyCommand {
    Update {
        region: String,
        fullscreen: String,
        window: String,
    },
    Stop,
}

static HOTKEY_CMD_TX: Mutex<Option<Sender<HotkeyCommand>>> = Mutex::new(None);

pub fn update_global_hotkeys(region: &str, fullscreen: &str, window: &str) {
    if let Ok(guard) = HOTKEY_CMD_TX.lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(HotkeyCommand::Update {
                region: region.to_string(),
                fullscreen: fullscreen.to_string(),
                window: window.to_string(),
            });
        }
    }
}

pub fn parse_hotkey_string(hotkey: &str) -> Option<(u32, u32)> {
    let mut modifiers = 0u32;
    let mut vk = 0u32;

    for part in hotkey.split('+') {
        let p = part.trim();
        if p.eq_ignore_ascii_case("ctrl") || p.eq_ignore_ascii_case("control") {
            modifiers |= 0x0002; // MOD_CONTROL
        } else if p.eq_ignore_ascii_case("alt") {
            modifiers |= 0x0001; // MOD_ALT
        } else if p.eq_ignore_ascii_case("shift") {
            modifiers |= 0x0004; // MOD_SHIFT
        } else if p.eq_ignore_ascii_case("win") || p.eq_ignore_ascii_case("super") {
            modifiers |= 0x0008; // MOD_WIN
        } else if p.eq_ignore_ascii_case("printscreen")
            || p.eq_ignore_ascii_case("prtsc")
            || p.eq_ignore_ascii_case("snapshot")
        {
            vk = 0x2C; // VK_SNAPSHOT
        } else if p.eq_ignore_ascii_case("space") {
            vk = 0x20;
        } else if p.eq_ignore_ascii_case("escape") || p.eq_ignore_ascii_case("esc") {
            vk = 0x1B;
        } else if p.eq_ignore_ascii_case("enter") || p.eq_ignore_ascii_case("return") {
            vk = 0x0D;
        } else if p.eq_ignore_ascii_case("tab") {
            vk = 0x09;
        } else if p.eq_ignore_ascii_case("f1") {
            vk = 0x70;
        } else if p.eq_ignore_ascii_case("f2") {
            vk = 0x71;
        } else if p.eq_ignore_ascii_case("f3") {
            vk = 0x72;
        } else if p.eq_ignore_ascii_case("f4") {
            vk = 0x73;
        } else if p.eq_ignore_ascii_case("f5") {
            vk = 0x74;
        } else if p.eq_ignore_ascii_case("f6") {
            vk = 0x75;
        } else if p.eq_ignore_ascii_case("f7") {
            vk = 0x76;
        } else if p.eq_ignore_ascii_case("f8") {
            vk = 0x77;
        } else if p.eq_ignore_ascii_case("f9") {
            vk = 0x78;
        } else if p.eq_ignore_ascii_case("f10") {
            vk = 0x79;
        } else if p.eq_ignore_ascii_case("f11") {
            vk = 0x7A;
        } else if p.eq_ignore_ascii_case("f12") {
            vk = 0x7B;
        } else if p.len() == 1 {
            let ch = p.chars().next().unwrap().to_ascii_uppercase();
            if ch.is_ascii_alphanumeric() {
                vk = ch as u32;
            }
        }
    }

    if vk != 0 {
        // MOD_NOREPEAT = 0x4000
        Some((modifiers | 0x4000, vk))
    } else {
        None
    }
}

pub fn format_keystroke(keystroke: &gpui::Keystroke) -> Option<String> {
    let key = keystroke.key.as_str();
    if key.eq_ignore_ascii_case("control")
        || key.eq_ignore_ascii_case("alt")
        || key.eq_ignore_ascii_case("shift")
        || key.eq_ignore_ascii_case("meta")
        || key.eq_ignore_ascii_case("super")
        || key.eq_ignore_ascii_case("windows")
    {
        return None;
    }

    let mut parts = Vec::new();
    if keystroke.modifiers.control {
        parts.push("Ctrl");
    }
    if keystroke.modifiers.alt {
        parts.push("Alt");
    }
    if keystroke.modifiers.shift {
        parts.push("Shift");
    }
    if keystroke.modifiers.platform {
        parts.push("Win");
    }

    let key_name = match key {
        " " | "space" => "Space".to_string(),
        "escape" => "Escape".to_string(),
        "enter" | "return" => "Enter".to_string(),
        "tab" => "Tab".to_string(),
        "backspace" => "Backspace".to_string(),
        "delete" => "Delete".to_string(),
        "insert" => "Insert".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        "pageup" => "PageUp".to_string(),
        "pagedown" => "PageDown".to_string(),
        "arrowup" | "up" => "Up".to_string(),
        "arrowdown" | "down" => "Down".to_string(),
        "arrowleft" | "left" => "Left".to_string(),
        "arrowright" | "right" => "Right".to_string(),
        "printscreen" | "snapshot" => "PrintScreen".to_string(),
        k if k.len() == 1 => k.to_uppercase(),
        other => {
            let mut c = other.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        }
    };

    let full_key = if parts.is_empty() {
        key_name
    } else {
        format!("{} + {}", parts.join(" + "), key_name)
    };

    Some(full_key)
}

#[cfg(windows)]
pub fn start_hotkey_listener(sender: Sender<HotkeyAction>) {
    use crate::config::AppConfig;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        HOT_KEY_MODIFIERS, RegisterHotKey, UnregisterHotKey,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage, WM_HOTKEY,
    };

    let (cmd_tx, cmd_rx) = channel::<HotkeyCommand>();
    if let Ok(mut guard) = HOTKEY_CMD_TX.lock() {
        *guard = Some(cmd_tx);
    }

    std::thread::spawn(move || unsafe {
        const ID_REGION: i32 = 101;
        const ID_FULLSCREEN: i32 = 102;
        const ID_WINDOW: i32 = 103;

        // Register initial hotkeys from loaded config
        let cfg = AppConfig::load();
        if let Some((mods, vk)) = parse_hotkey_string(&cfg.hotkey_region) {
            let _ = RegisterHotKey(None, ID_REGION, HOT_KEY_MODIFIERS(mods), vk);
        }
        if let Some((mods, vk)) = parse_hotkey_string(&cfg.hotkey_fullscreen) {
            let _ = RegisterHotKey(None, ID_FULLSCREEN, HOT_KEY_MODIFIERS(mods), vk);
        }
        if let Some((mods, vk)) = parse_hotkey_string(&cfg.hotkey_window) {
            let _ = RegisterHotKey(None, ID_WINDOW, HOT_KEY_MODIFIERS(mods), vk);
        }

        let mut msg = MSG::default();
        loop {
            // Check for hotkey configuration updates
            while let Ok(cmd) = cmd_rx.try_recv() {
                match cmd {
                    HotkeyCommand::Update {
                        region,
                        fullscreen,
                        window,
                    } => {
                        let _ = UnregisterHotKey(None, ID_REGION);
                        let _ = UnregisterHotKey(None, ID_FULLSCREEN);
                        let _ = UnregisterHotKey(None, ID_WINDOW);

                        if let Some((mods, vk)) = parse_hotkey_string(&region) {
                            let _ = RegisterHotKey(None, ID_REGION, HOT_KEY_MODIFIERS(mods), vk);
                        }
                        if let Some((mods, vk)) = parse_hotkey_string(&fullscreen) {
                            let _ =
                                RegisterHotKey(None, ID_FULLSCREEN, HOT_KEY_MODIFIERS(mods), vk);
                        }
                        if let Some((mods, vk)) = parse_hotkey_string(&window) {
                            let _ = RegisterHotKey(None, ID_WINDOW, HOT_KEY_MODIFIERS(mods), vk);
                        }
                    }
                    HotkeyCommand::Stop => {
                        let _ = UnregisterHotKey(None, ID_REGION);
                        let _ = UnregisterHotKey(None, ID_FULLSCREEN);
                        let _ = UnregisterHotKey(None, ID_WINDOW);
                        return;
                    }
                }
            }

            // Process Win32 messages
            let mut processed_any = false;
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                processed_any = true;
                if msg.message == WM_HOTKEY {
                    let id = msg.wParam.0 as i32;
                    match id {
                        ID_REGION => {
                            let _ = sender.send(HotkeyAction::CaptureRegion);
                        }
                        ID_FULLSCREEN => {
                            let _ = sender.send(HotkeyAction::CaptureFullscreen);
                        }
                        ID_WINDOW => {
                            let _ = sender.send(HotkeyAction::CaptureActiveWindow);
                        }
                        _ => {}
                    }
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            if !processed_any {
                std::thread::sleep(Duration::from_millis(15));
            }
        }
    });
}

#[cfg(not(windows))]
pub fn start_hotkey_listener(_sender: Sender<HotkeyAction>) {}
