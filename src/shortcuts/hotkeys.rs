use std::sync::Mutex;
use std::sync::mpsc::Sender;
#[cfg(any(windows, target_os = "linux"))]
use std::sync::mpsc::channel;
#[cfg(any(windows, target_os = "linux"))]
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyAction {
    CaptureRegion,
    CaptureFullscreen,
    CaptureActiveWindow,
    OpenSettings,
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

#[cfg(target_os = "linux")]
pub fn get_linux_ipc_socket_path() -> std::path::PathBuf {
    let mut uid = 1000u32;
    if let Ok(uid_str) = std::env::var("UID") {
        if let Ok(u) = uid_str.parse::<u32>() {
            uid = u;
        }
    } else if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("Uid:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() > 1 {
                    if let Ok(u) = parts[1].parse::<u32>() {
                        uid = u;
                        break;
                    }
                }
            }
        }
    }
    std::env::temp_dir().join(format!("pixora-{}.sock", uid))
}

#[cfg(target_os = "linux")]
pub fn try_send_linux_ipc_command(cmd: &str) -> bool {
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    let socket_path = get_linux_ipc_socket_path();
    if !socket_path.exists() {
        return false;
    }

    if let Ok(mut stream) = UnixStream::connect(&socket_path) {
        if writeln!(stream, "{}", cmd).is_ok() {
            return true;
        }
    }
    false
}

#[cfg(target_os = "linux")]
fn start_linux_ipc_listener(sender: Sender<HotkeyAction>) {
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::UnixListener;

    let socket_path = get_linux_ipc_socket_path();
    let _ = std::fs::remove_file(&socket_path);

    std::thread::spawn(move || {
        let listener = match UnixListener::bind(&socket_path) {
            Ok(l) => l,
            Err(e) => {
                crate::logging::log_warn("hotkeys", format!("Failed to bind Linux IPC socket: {}", e));
                return;
            }
        };

        for stream in listener.incoming() {
            if let Ok(stream) = stream {
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                if reader.read_line(&mut line).is_ok() {
                    let cmd = line.trim();
                    match cmd {
                        "region" | "capture" => {
                            let _ = sender.send(HotkeyAction::CaptureRegion);
                        }
                        "fullscreen" => {
                            let _ = sender.send(HotkeyAction::CaptureFullscreen);
                        }
                        "window" => {
                            let _ = sender.send(HotkeyAction::CaptureActiveWindow);
                        }
                        "settings" => {
                            let _ = sender.send(HotkeyAction::OpenSettings);
                        }
                        _ => {}
                    }
                }
            }
        }
    });
}

#[cfg(target_os = "linux")]
fn parse_linux_hotkey_keysym(hotkey: &str) -> Option<(u16, u32)> {
    use x11rb::protocol::xproto::ModMask;

    let mut mods: u16 = 0;
    let mut keysym: u32 = 0;

    for part in hotkey.split('+') {
        let p = part.trim();
        if p.eq_ignore_ascii_case("ctrl") || p.eq_ignore_ascii_case("control") {
            mods |= u16::from(ModMask::CONTROL);
        } else if p.eq_ignore_ascii_case("alt") {
            mods |= u16::from(ModMask::M1);
        } else if p.eq_ignore_ascii_case("shift") {
            mods |= u16::from(ModMask::SHIFT);
        } else if p.eq_ignore_ascii_case("win") || p.eq_ignore_ascii_case("super") {
            mods |= u16::from(ModMask::M4);
        } else if p.eq_ignore_ascii_case("printscreen")
            || p.eq_ignore_ascii_case("prtsc")
            || p.eq_ignore_ascii_case("snapshot")
        {
            keysym = 0xFF61; // XK_Print
        } else if p.eq_ignore_ascii_case("space") {
            keysym = 0x0020; // XK_space
        } else if p.eq_ignore_ascii_case("escape") || p.eq_ignore_ascii_case("esc") {
            keysym = 0xFF1B; // XK_Escape
        } else if p.eq_ignore_ascii_case("enter") || p.eq_ignore_ascii_case("return") {
            keysym = 0xFF0D; // XK_Return
        } else if p.eq_ignore_ascii_case("tab") {
            keysym = 0xFF09; // XK_Tab
        } else if p.eq_ignore_ascii_case("backspace") {
            keysym = 0xFF08; // XK_BackSpace
        } else if p.eq_ignore_ascii_case("delete") {
            keysym = 0xFFFF; // XK_Delete
        } else if p.eq_ignore_ascii_case("insert") {
            keysym = 0xFF63; // XK_Insert
        } else if p.eq_ignore_ascii_case("home") {
            keysym = 0xFF50; // XK_Home
        } else if p.eq_ignore_ascii_case("end") {
            keysym = 0xFF57; // XK_End
        } else if p.eq_ignore_ascii_case("pageup") {
            keysym = 0xFF55; // XK_Page_Up
        } else if p.eq_ignore_ascii_case("pagedown") {
            keysym = 0xFF56; // XK_Page_Down
        } else if p.eq_ignore_ascii_case("up") {
            keysym = 0xFF52; // XK_Up
        } else if p.eq_ignore_ascii_case("down") {
            keysym = 0xFF54; // XK_Down
        } else if p.eq_ignore_ascii_case("left") {
            keysym = 0xFF51; // XK_Left
        } else if p.eq_ignore_ascii_case("right") {
            keysym = 0xFF53; // XK_Right
        } else if let Some(num_str) = p.strip_prefix('f').or_else(|| p.strip_prefix('F')) {
            if let Ok(n) = num_str.parse::<u32>() {
                if (1..=12).contains(&n) {
                    keysym = 0xFFBE + (n - 1); // XK_F1..XK_F12
                }
            }
        } else if p.len() == 1 {
            let ch = p.chars().next().unwrap().to_ascii_lowercase();
            if ch.is_ascii_alphanumeric() {
                keysym = ch as u32;
            }
        }
    }

    if keysym != 0 {
        Some((mods, keysym))
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
fn to_xdg_portal_trigger(hotkey: &str) -> String {
    let mut out = String::new();
    let mut key_part = String::new();

    for part in hotkey.split('+') {
        let p = part.trim();
        if p.eq_ignore_ascii_case("ctrl") || p.eq_ignore_ascii_case("control") {
            out.push_str("<Control>");
        } else if p.eq_ignore_ascii_case("alt") {
            out.push_str("<Alt>");
        } else if p.eq_ignore_ascii_case("shift") {
            out.push_str("<Shift>");
        } else if p.eq_ignore_ascii_case("win") || p.eq_ignore_ascii_case("super") {
            out.push_str("<Super>");
        } else if p.eq_ignore_ascii_case("printscreen")
            || p.eq_ignore_ascii_case("prtsc")
            || p.eq_ignore_ascii_case("snapshot")
        {
            key_part = "Print".to_string();
        } else if p.len() == 1 {
            key_part = p.to_lowercase();
        } else {
            key_part = p.to_string();
        }
    }

    out.push_str(&key_part);
    out
}

#[cfg(target_os = "linux")]
fn start_linux_portal_shortcuts_listener(sender: Sender<HotkeyAction>) {
    if std::env::var("WAYLAND_DISPLAY").is_err() {
        return;
    }

    std::thread::spawn(move || {
        use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
        use futures_lite::StreamExt;

        let cfg = crate::config::AppConfig::load();
        let reg_trigger = to_xdg_portal_trigger(&cfg.hotkey_region);
        let fs_trigger = to_xdg_portal_trigger(&cfg.hotkey_fullscreen);
        let win_trigger = to_xdg_portal_trigger(&cfg.hotkey_window);

        let _ = zbus::block_on(async move {
            let portal = GlobalShortcuts::new().await.ok()?;
            let session = portal.create_session().await.ok()?;
            let shortcuts = [
                NewShortcut::new("pixora_region", "Capture Screen Region")
                    .preferred_trigger(reg_trigger.as_str()),
                NewShortcut::new("pixora_fullscreen", "Capture Fullscreen")
                    .preferred_trigger(fs_trigger.as_str()),
                NewShortcut::new("pixora_window", "Capture Active Window")
                    .preferred_trigger(win_trigger.as_str()),
            ];
            let _ = portal.bind_shortcuts(&session, &shortcuts, None).await.ok()?;
            let mut stream = std::pin::pin!(portal.receive_activated().await.ok()?);
            crate::logging::log_info("hotkeys", "Registered XDG GlobalShortcuts portal session");
            while let Some(activated) = stream.next().await {
                match activated.shortcut_id() {
                    "pixora_region" => {
                        let _ = sender.send(HotkeyAction::CaptureRegion);
                    }
                    "pixora_fullscreen" => {
                        let _ = sender.send(HotkeyAction::CaptureFullscreen);
                    }
                    "pixora_window" => {
                        let _ = sender.send(HotkeyAction::CaptureActiveWindow);
                    }
                    _ => {}
                }
            }
            Some(())
        });
    });
}

#[cfg(target_os = "linux")]
fn start_linux_x11_hotkey_listener(
    sender: Sender<HotkeyAction>,
    cmd_rx: std::sync::mpsc::Receiver<HotkeyCommand>,
) {
    use crate::config::AppConfig;
    use x11rb::connection::Connection;
    use x11rb::protocol::Event;
    use x11rb::protocol::xproto::{ConnectionExt, GrabMode, ModMask};

    fn resolve_keycode<C: Connection>(conn: &C, keysym: u32) -> Option<u8> {
        let setup = conn.setup();
        let min_kc = setup.min_keycode;
        let max_kc = setup.max_keycode;
        if max_kc < min_kc {
            return None;
        }
        let count = max_kc - min_kc + 1;
        let mapping = conn.get_keyboard_mapping(min_kc, count).ok()?.reply().ok()?;
        let syms_per_code = mapping.keysyms_per_keycode as usize;
        if syms_per_code == 0 {
            return None;
        }

        let alt_keysym = if (0x61..=0x7A).contains(&keysym) {
            keysym - 0x20
        } else {
            keysym
        };

        for idx in 0..(count as usize) {
            let start = idx * syms_per_code;
            let end = (start + syms_per_code).min(mapping.keysyms.len());
            if start < end {
                let slice = &mapping.keysyms[start..end];
                if slice.contains(&keysym) || slice.contains(&alt_keysym) {
                    return Some(min_kc + idx as u8);
                }
            }
        }
        None
    }

    #[derive(Clone, Copy)]
    struct GrabbedBinding {
        keycode: u8,
        mods: u16,
        action: HotkeyAction,
    }

    fn register_bindings<C: Connection>(
        conn: &C,
        root: u32,
        region: &str,
        fullscreen: &str,
        window: &str,
    ) -> Vec<GrabbedBinding> {
        let mut bindings = Vec::new();
        let ignore_masks: [u16; 4] = [
            0,
            u16::from(ModMask::LOCK),
            u16::from(ModMask::M2),
            u16::from(ModMask::LOCK) | u16::from(ModMask::M2),
        ];

        for (spec, action) in [
            (region, HotkeyAction::CaptureRegion),
            (fullscreen, HotkeyAction::CaptureFullscreen),
            (window, HotkeyAction::CaptureActiveWindow),
        ] {
            if let Some((mods, keysym)) = parse_linux_hotkey_keysym(spec) {
                if let Some(keycode) = resolve_keycode(conn, keysym) {
                    for extra in ignore_masks {
                        let _ = conn.grab_key(
                            true,
                            root,
                            ModMask::from(mods | extra),
                            keycode,
                            GrabMode::ASYNC,
                            GrabMode::ASYNC,
                        );
                    }
                    bindings.push(GrabbedBinding {
                        keycode,
                        mods,
                        action,
                    });
                }
            }
        }
        let _ = conn.flush();
        bindings
    }

    fn clear_bindings<C: Connection>(conn: &C, root: u32, bindings: &[GrabbedBinding]) {
        let ignore_masks: [u16; 4] = [
            0,
            u16::from(ModMask::LOCK),
            u16::from(ModMask::M2),
            u16::from(ModMask::LOCK) | u16::from(ModMask::M2),
        ];
        for b in bindings {
            for extra in ignore_masks {
                let _ = conn.ungrab_key(b.keycode, root, ModMask::from(b.mods | extra));
            }
        }
        let _ = conn.flush();
    }

    std::thread::spawn(move || {
        let (conn, screen_num) = match x11rb::connect(None) {
            Ok(c) => c,
            Err(_) => return, // Not running under X11 (e.g. pure Wayland session)
        };

        let screen = match conn.setup().roots.get(screen_num) {
            Some(s) => s,
            None => return,
        };
        let root = screen.root;

        let cfg = AppConfig::load();
        let mut bindings = register_bindings(
            &conn,
            root,
            &cfg.hotkey_region,
            &cfg.hotkey_fullscreen,
            &cfg.hotkey_window,
        );

        let clean_mask = !(u16::from(ModMask::LOCK) | u16::from(ModMask::M2));

        loop {
            while let Ok(cmd) = cmd_rx.try_recv() {
                match cmd {
                    HotkeyCommand::Update {
                        region,
                        fullscreen,
                        window,
                    } => {
                        clear_bindings(&conn, root, &bindings);
                        bindings = register_bindings(&conn, root, &region, &fullscreen, &window);
                    }
                    HotkeyCommand::Stop => {
                        clear_bindings(&conn, root, &bindings);
                        return;
                    }
                }
            }

            let mut processed_any = false;
            while let Ok(Some(ev)) = conn.poll_for_event() {
                processed_any = true;
                if let Event::KeyPress(kp) = ev {
                    let pressed_mods = u16::from(kp.state) & clean_mask;
                    for b in &bindings {
                        if kp.detail == b.keycode && pressed_mods == b.mods {
                            let _ = sender.send(b.action);
                            break;
                        }
                    }
                }
            }

            if !processed_any {
                std::thread::sleep(Duration::from_millis(15));
            }
        }
    });
}

#[cfg(target_os = "linux")]
pub fn start_hotkey_listener(sender: Sender<HotkeyAction>) {
    let (cmd_tx, cmd_rx) = channel::<HotkeyCommand>();
    if let Ok(mut guard) = HOTKEY_CMD_TX.lock() {
        *guard = Some(cmd_tx);
    }

    start_linux_ipc_listener(sender.clone());
    start_linux_portal_shortcuts_listener(sender.clone());
    start_linux_x11_hotkey_listener(sender, cmd_rx);
}

#[cfg(all(not(windows), not(target_os = "linux")))]
pub fn start_hotkey_listener(_sender: Sender<HotkeyAction>) {}
