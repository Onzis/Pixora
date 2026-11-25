use std::sync::mpsc::Sender;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayEvent {
    CaptureRegion,
    CaptureFullscreen,
    OpenSettings,
    OpenFolder,
    CheckUpdates,
    Exit,
}

#[cfg(windows)]
static TRAY_HWND: std::sync::Mutex<Option<isize>> = std::sync::Mutex::new(None);

#[cfg(windows)]
pub fn show_tray_notification(title: &str, message: &str) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::{
        NOTIFYICONDATAW, NOTIFY_ICON_DATA_FLAGS, NOTIFY_ICON_INFOTIP_FLAGS, NIF_INFO,
        Shell_NotifyIconW, NIM_MODIFY,
    };
    if let Ok(lock) = TRAY_HWND.lock() {
        if let Some(h) = *lock {
            let hwnd = HWND(h as *mut core::ffi::c_void);
            let mut info = [0u16; 256];
            for (i, c) in message.encode_utf16().enumerate().take(255) {
                info[i] = c;
            }
            let mut info_title = [0u16; 64];
            for (i, c) in title.encode_utf16().enumerate().take(63) {
                info_title[i] = c;
            }
            let nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: 2001,
                uFlags: NOTIFY_ICON_DATA_FLAGS(NIF_INFO.0),
                szInfo: info,
                szInfoTitle: info_title,
                dwInfoFlags: NOTIFY_ICON_INFOTIP_FLAGS(1), // NIIF_INFO
                ..Default::default()
            };
            unsafe {
                let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
            }
        }
    }
}

#[cfg(target_os = "linux")]
pub fn show_tray_notification(title: &str, message: &str) {
    let title_owned = title.to_string();
    let msg_owned = message.to_string();
    std::thread::spawn(move || {
        use std::collections::HashMap;
        let sent_via_dbus = zbus::blocking::Connection::session()
            .and_then(|conn| {
                let hints: HashMap<&str, zbus::zvariant::Value<'_>> = HashMap::new();
                let actions: Vec<&str> = Vec::new();
                conn.call_method(
                    Some("org.freedesktop.Notifications"),
                    "/org/freedesktop/Notifications",
                    Some("org.freedesktop.Notifications"),
                    "Notify",
                    &(
                        "Pixora",
                        0u32,
                        "pixora",
                        title_owned.as_str(),
                        msg_owned.as_str(),
                        actions,
                        hints,
                        5000i32,
                    ),
                )
            })
            .is_ok();

        if !sent_via_dbus {
            let _ = std::process::Command::new("notify-send")
                .arg(&title_owned)
                .arg(&msg_owned)
                .arg("-a")
                .arg("Pixora")
                .spawn();
        }
    });
}

#[cfg(all(not(windows), not(target_os = "linux")))]
pub fn show_tray_notification(_title: &str, _message: &str) {}

#[cfg(windows)]
pub fn start_system_tray(event_sender: Sender<TrayEvent>) {
    use std::sync::Mutex;
    use windows::core::w;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows::Win32::UI::Shell::{
        NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
        NOTIFY_ICON_DATA_FLAGS, Shell_NotifyIconW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
        DispatchMessageW, GetCursorPos, GetMessageW, HICON, IDI_APPLICATION, IMAGE_ICON,
        LR_DEFAULTCOLOR, LR_LOADFROMFILE, LoadIconW, LoadImageW, MF_SEPARATOR, MF_STRING, MSG, PostQuitMessage,
        RegisterClassW, SetForegroundWindow, TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON,
        TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_DESTROY,
        WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSW,
    };

    static TRAY_SENDER: Mutex<Option<Sender<TrayEvent>>> = Mutex::new(None);
    if let Ok(mut lock) = TRAY_SENDER.lock() {
        *lock = Some(event_sender);
    }

    std::thread::spawn(move || unsafe {
        let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap_or_default();

        let class_name = w!("PIXORA_TRAY_CLASS");
        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(tray_wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };

        let _ = RegisterClassW(&wnd_class);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("PIXORA_TRAY_HWND"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            None,
            None,
            Some(hinstance.into()),
            None,
        );

        if hwnd.is_err() {
            return;
        }
        let hwnd = hwnd.unwrap();
        if let Ok(mut lock) = TRAY_HWND.lock() {
            *lock = Some(hwnd.0 as isize);
        }

        const WM_TRAY: u32 = WM_APP + 10;
        let icon = LoadImageW(
            Some(hinstance.into()),
            windows::core::PCWSTR(1 as *const u16),
            IMAGE_ICON,
            16,
            16,
            LR_DEFAULTCOLOR,
        )
        .or_else(|_| {
            let ico_path = std::path::Path::new("assets/icon.ico");
            if ico_path.exists() {
                use windows::core::HSTRING;
                let full_path = std::fs::canonicalize(ico_path).unwrap_or_else(|_| ico_path.to_path_buf());
                let hstr = HSTRING::from(full_path.as_os_str());
                LoadImageW(
                    None,
                    windows::core::PCWSTR(hstr.as_ptr()),
                    IMAGE_ICON,
                    16,
                    16,
                    LR_LOADFROMFILE | LR_DEFAULTCOLOR,
                )
            } else {
                Err(windows::core::Error::empty())
            }
        })
        .map(|h| HICON(h.0))
        .unwrap_or_else(|_| {
            LoadIconW(None, IDI_APPLICATION).unwrap_or_default()
        });

        let mut tip = [0u16; 128];
        let tip_str = "Pixora Screen Capture";
        for (i, c) in tip_str.encode_utf16().enumerate().take(127) {
            tip[i] = c;
        }

        let flags = NOTIFY_ICON_DATA_FLAGS(NIF_ICON.0 | NIF_MESSAGE.0 | NIF_TIP.0);
        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 2001,
            uFlags: flags,
            uCallbackMessage: WM_TRAY,
            hIcon: icon,
            szTip: tip,
            ..Default::default()
        };

        let _ = Shell_NotifyIconW(NIM_ADD, &nid);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    });

    unsafe extern "system" fn tray_wnd_proc(
        hwnd: HWND,
        msg: u32,
        _wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        const WM_TRAY: u32 = WM_APP + 10;

        unsafe {
            if msg == WM_TRAY {
                let event = lparam.0 as u32;
                if event == WM_LBUTTONUP {
                    if let Ok(lock) = TRAY_SENDER.lock() {
                        if let Some(sender) = lock.as_ref() {
                            let _ = sender.send(TrayEvent::CaptureRegion);
                        }
                    }
                } else if event == WM_RBUTTONUP {
                    let mut pt = POINT::default();
                    let _ = GetCursorPos(&mut pt);
                    let _ = SetForegroundWindow(hwnd);

                    let hmenu = CreatePopupMenu().unwrap_or_default();
                    if !hmenu.0.is_null() {
                        let config = crate::config::AppConfig::load();
                        let lang = config.language.resolve();

                        fn to_wide(s: &str) -> Vec<u16> {
                            s.encode_utf16().chain(std::iter::once(0)).collect()
                        }

                        let m_region = to_wide(&format!("{} ({})", crate::i18n::t(lang, "Capture Region", "Снимок области"), config.hotkey_region));
                        let m_fs = to_wide(&format!("{} ({})", crate::i18n::t(lang, "Capture Fullscreen", "Снимок всего экрана"), config.hotkey_fullscreen));
                        let m_settings = to_wide(crate::i18n::t(lang, "Settings", "Настройки"));
                        let m_folder = to_wide(crate::i18n::t(lang, "Open Screenshots Folder", "Открыть папку со снимками"));
                        let m_updates = to_wide(crate::i18n::t(lang, "Check for Updates...", "Проверить обновление..."));
                        let m_exit = to_wide(crate::i18n::t(lang, "Exit", "Выход"));

                        let _ = AppendMenuW(hmenu, MF_STRING, 1, windows::core::PCWSTR(m_region.as_ptr()));
                        let _ = AppendMenuW(hmenu, MF_STRING, 2, windows::core::PCWSTR(m_fs.as_ptr()));
                        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, None);
                        let _ = AppendMenuW(hmenu, MF_STRING, 3, windows::core::PCWSTR(m_settings.as_ptr()));
                        let _ = AppendMenuW(hmenu, MF_STRING, 4, windows::core::PCWSTR(m_folder.as_ptr()));
                        let _ = AppendMenuW(hmenu, MF_STRING, 6, windows::core::PCWSTR(m_updates.as_ptr()));
                        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, None);
                        let _ = AppendMenuW(hmenu, MF_STRING, 5, windows::core::PCWSTR(m_exit.as_ptr()));

                        let cmd = TrackPopupMenu(
                            hmenu,
                            TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_RETURNCMD,
                            pt.x,
                            pt.y,
                            Some(0),
                            hwnd,
                            None,
                        );
                        let _ = DestroyMenu(hmenu);

                        if let Ok(lock) = TRAY_SENDER.lock() {
                            if let Some(sender) = lock.as_ref() {
                                match cmd.0 {
                                    1 => { let _ = sender.send(TrayEvent::CaptureRegion); }
                                    2 => { let _ = sender.send(TrayEvent::CaptureFullscreen); }
                                    3 => { let _ = sender.send(TrayEvent::OpenSettings); }
                                    4 => { let _ = sender.send(TrayEvent::OpenFolder); }
                                    5 => { let _ = sender.send(TrayEvent::Exit); }
                                    6 => { let _ = sender.send(TrayEvent::CheckUpdates); }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                return LRESULT(0);
            } else if msg == WM_DESTROY {
                if let Ok(mut lock) = TRAY_HWND.lock() {
                    *lock = None;
                }
                PostQuitMessage(0);
                return LRESULT(0);
            }

            DefWindowProcW(hwnd, msg, _wparam, lparam)
        }
    }
}

#[cfg(target_os = "linux")]
pub fn start_system_tray(event_sender: Sender<TrayEvent>) {
    use std::collections::HashMap;
    use zbus::zvariant::{OwnedObjectPath, OwnedValue, StructureBuilder, Value};

    const EMBEDDED_TRAY_ICON_PNG: &[u8] = include_bytes!("../../assets/pixora_icon_48.png");

    fn build_argb_icon_pixmap() -> Vec<(i32, i32, Vec<u8>)> {
        if let Ok(dyn_img) = image::load_from_memory(EMBEDDED_TRAY_ICON_PNG) {
            let rgba = dyn_img.to_rgba8();
            let w = rgba.width() as i32;
            let h = rgba.height() as i32;
            let mut argb = Vec::with_capacity((w * h * 4) as usize);
            for p in rgba.pixels() {
                // StatusNotifierItem IconPixmap uses ARGB32 in network byte order (big-endian)
                argb.push(p[3]);
                argb.push(p[0]);
                argb.push(p[1]);
                argb.push(p[2]);
            }
            vec![(w, h, argb)]
        } else {
            Vec::new()
        }
    }

    fn str_ov(s: &str) -> OwnedValue {
        OwnedValue::try_from(Value::from(s.to_string())).unwrap()
    }

    fn bool_ov(b: bool) -> OwnedValue {
        OwnedValue::from(b)
    }

    fn menu_item_props(id: i32) -> HashMap<String, OwnedValue> {
        let mut props = HashMap::new();
        if id == 0 {
            props.insert("children-display".to_string(), str_ov("submenu"));
            return props;
        }
        if id == 10 || id == 11 {
            props.insert("type".to_string(), str_ov("separator"));
            return props;
        }
        let config = crate::config::AppConfig::load();
        let lang = config.language.resolve();
        let label = match id {
            1 => format!(
                "{} ({})",
                crate::i18n::t(lang, "Capture Region", "Снимок области"),
                config.hotkey_region
            ),
            2 => format!(
                "{} ({})",
                crate::i18n::t(lang, "Capture Fullscreen", "Снимок всего экрана"),
                config.hotkey_fullscreen
            ),
            3 => crate::i18n::t(lang, "Settings", "Настройки").to_string(),
            4 => crate::i18n::t(lang, "Open Screenshots Folder", "Открыть папку со снимками").to_string(),
            6 => crate::i18n::t(lang, "Check for Updates...", "Проверить обновление...").to_string(),
            5 => crate::i18n::t(lang, "Exit", "Выход").to_string(),
            _ => String::new(),
        };
        props.insert("label".to_string(), str_ov(&label));
        props.insert("enabled".to_string(), bool_ov(true));
        props.insert("visible".to_string(), bool_ov(true));
        props
    }

    fn make_child_node(id: i32) -> OwnedValue {
        let props = menu_item_props(id);
        let children: Vec<OwnedValue> = Vec::new();
        let s = StructureBuilder::new()
            .add_field(id)
            .add_field(props)
            .add_field(children)
            .build()
            .expect("DBusMenu child structure must build");
        OwnedValue::try_from(s).expect("DBusMenu child must convert to OwnedValue")
    }

    struct StatusNotifierItem {
        sender: Sender<TrayEvent>,
        icon_pixmap: Vec<(i32, i32, Vec<u8>)>,
    }

    #[zbus::interface(interface = "org.kde.StatusNotifierItem")]
    impl StatusNotifierItem {
        #[zbus(property)]
        fn category(&self) -> String {
            "ApplicationStatus".to_string()
        }

        #[zbus(property)]
        fn id(&self) -> String {
            "pixora".to_string()
        }

        #[zbus(property)]
        fn title(&self) -> String {
            "Pixora".to_string()
        }

        #[zbus(property)]
        fn status(&self) -> String {
            "Active".to_string()
        }

        #[zbus(property)]
        fn window_id(&self) -> u32 {
            0
        }

        #[zbus(property)]
        fn icon_name(&self) -> String {
            "pixora".to_string()
        }

        #[zbus(property)]
        fn icon_pixmap(&self) -> Vec<(i32, i32, Vec<u8>)> {
            self.icon_pixmap.clone()
        }

        #[zbus(property)]
        fn overlay_icon_name(&self) -> String {
            String::new()
        }

        #[zbus(property)]
        fn overlay_icon_pixmap(&self) -> Vec<(i32, i32, Vec<u8>)> {
            Vec::new()
        }

        #[zbus(property)]
        fn attention_icon_name(&self) -> String {
            String::new()
        }

        #[zbus(property)]
        fn attention_icon_pixmap(&self) -> Vec<(i32, i32, Vec<u8>)> {
            Vec::new()
        }

        #[zbus(property)]
        fn attention_movie_name(&self) -> String {
            String::new()
        }

        #[zbus(property)]
        fn tool_tip(&self) -> (String, Vec<(i32, i32, Vec<u8>)>, String, String) {
            (
                String::new(),
                Vec::new(),
                "Pixora Screen Capture".to_string(),
                String::new(),
            )
        }

        #[zbus(property)]
        fn item_is_menu(&self) -> bool {
            false
        }

        #[zbus(property)]
        fn menu(&self) -> OwnedObjectPath {
            OwnedObjectPath::try_from("/MenuBar").unwrap()
        }

        fn context_menu(&self, _x: i32, _y: i32) {}

        fn activate(&self, _x: i32, _y: i32) {
            let _ = self.sender.send(TrayEvent::CaptureRegion);
        }

        fn secondary_activate(&self, _x: i32, _y: i32) {
            let _ = self.sender.send(TrayEvent::CaptureFullscreen);
        }

        fn scroll(&self, _delta: i32, _orientation: String) {}
    }

    struct DBusMenu {
        sender: Sender<TrayEvent>,
    }

    #[zbus::interface(interface = "com.canonical.dbusmenu")]
    impl DBusMenu {
        #[zbus(property)]
        fn version(&self) -> u32 {
            3
        }

        #[zbus(property)]
        fn text_direction(&self) -> String {
            "ltr".to_string()
        }

        #[zbus(property)]
        fn status(&self) -> String {
            "normal".to_string()
        }

        #[zbus(property)]
        fn icon_theme_path(&self) -> Vec<String> {
            Vec::new()
        }

        #[zbus(out_args("revision", "layout"))]
        fn get_layout(
            &self,
            parent_id: i32,
            recursion_depth: i32,
            _property_names: Vec<String>,
        ) -> (u32, (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>)) {
            let props = menu_item_props(parent_id);
            let children = if parent_id == 0 && recursion_depth != 0 {
                vec![
                    make_child_node(1),
                    make_child_node(2),
                    make_child_node(10),
                    make_child_node(3),
                    make_child_node(4),
                    make_child_node(6),
                    make_child_node(11),
                    make_child_node(5),
                ]
            } else {
                Vec::new()
            };
            (1, (parent_id, props, children))
        }

        #[zbus(out_args("properties"))]
        fn get_group_properties(
            &self,
            ids: Vec<i32>,
            _property_names: Vec<String>,
        ) -> Vec<(i32, HashMap<String, OwnedValue>)> {
            let all_ids = if ids.is_empty() {
                vec![1, 2, 10, 3, 4, 6, 11, 5]
            } else {
                ids
            };
            all_ids
                .into_iter()
                .map(|id| (id, menu_item_props(id)))
                .collect()
        }

        #[zbus(out_args("value"))]
        fn get_property(&self, id: i32, name: String) -> OwnedValue {
            menu_item_props(id)
                .remove(&name)
                .unwrap_or_else(|| str_ov(""))
        }

        fn event(&self, id: i32, event_id: String, _data: OwnedValue, _timestamp: u32) {
            if event_id == "clicked" {
                match id {
                    1 => {
                        let _ = self.sender.send(TrayEvent::CaptureRegion);
                    }
                    2 => {
                        let _ = self.sender.send(TrayEvent::CaptureFullscreen);
                    }
                    3 => {
                        let _ = self.sender.send(TrayEvent::OpenSettings);
                    }
                    4 => {
                        let _ = self.sender.send(TrayEvent::OpenFolder);
                    }
                    6 => {
                        let _ = self.sender.send(TrayEvent::CheckUpdates);
                    }
                    5 => {
                        let _ = self.sender.send(TrayEvent::Exit);
                    }
                    _ => {}
                }
            }
        }

        #[zbus(out_args("idErrors"))]
        fn event_group(&self, events: Vec<(i32, String, OwnedValue, u32)>) -> Vec<i32> {
            for (id, event_id, data, ts) in events {
                self.event(id, event_id, data, ts);
            }
            Vec::new()
        }

        #[zbus(out_args("needUpdate"))]
        fn about_to_show(&self, _id: i32) -> bool {
            true
        }

        #[zbus(out_args("updatesNeeded", "idErrors"))]
        fn about_to_show_group(&self, _ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
            (vec![0], Vec::new())
        }
    }

    std::thread::spawn(move || {
        let service_name = format!("org.kde.StatusNotifierItem-{}-1", std::process::id());
        let sni = StatusNotifierItem {
            sender: event_sender.clone(),
            icon_pixmap: build_argb_icon_pixmap(),
        };
        let menu = DBusMenu {
            sender: event_sender,
        };

        let conn_res = zbus::block_on(async {
            zbus::connection::Builder::session()?
                .name(service_name.as_str())?
                .serve_at("/StatusNotifierItem", sni)?
                .serve_at("/MenuBar", menu)?
                .build()
                .await
        });

        let conn = match conn_res {
            Ok(c) => c,
            Err(e) => {
                crate::logging::log_warn(
                    "tray",
                    format!("Failed to initialize Linux D-Bus StatusNotifierItem tray: {}", e),
                );
                return;
            }
        };

        // Register with StatusNotifierWatcher (retry briefly in case panel is still starting at login)
        for attempt in 0..5 {
            let reg = zbus::block_on(async {
                conn.call_method(
                    Some("org.kde.StatusNotifierWatcher"),
                    "/StatusNotifierWatcher",
                    Some("org.kde.StatusNotifierWatcher"),
                    "RegisterStatusNotifierItem",
                    &(service_name.as_str(),),
                )
                .await
            });
            if reg.is_ok() {
                crate::logging::log_info("tray", "Linux StatusNotifierItem tray registered");
                break;
            }
            if attempt == 4 {
                crate::logging::log_warn(
                    "tray",
                    "StatusNotifierWatcher not found on D-Bus session (install AppIndicator extension on GNOME if needed)",
                );
            } else {
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }

        // Keep the D-Bus connection alive for the lifetime of the application
        let _keep_alive = conn;
        loop {
            std::thread::park();
        }
    });
}

#[cfg(all(not(windows), not(target_os = "linux")))]
pub fn start_system_tray(_event_sender: Sender<TrayEvent>) {}
