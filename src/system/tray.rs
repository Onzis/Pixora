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

#[cfg(not(windows))]
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

#[cfg(not(windows))]
pub fn start_system_tray(_event_sender: Sender<TrayEvent>) {}
