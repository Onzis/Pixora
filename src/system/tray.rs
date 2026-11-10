use std::sync::mpsc::Sender;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayEvent {
    CaptureRegion,
    CaptureFullscreen,
    OpenSettings,
    OpenFolder,
    Exit,
}

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
        DispatchMessageW, GetCursorPos, GetMessageW, LoadIconW, MSG, MF_SEPARATOR, MF_STRING,
        PostQuitMessage, RegisterClassW, SetForegroundWindow, TPM_BOTTOMALIGN, TPM_RETURNCMD,
        TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE,
        WM_APP, WM_DESTROY, WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSW,
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

        const WM_TRAY: u32 = WM_APP + 10;
        let icon = LoadIconW(None, windows::Win32::UI::WindowsAndMessaging::IDI_APPLICATION).unwrap_or_default();

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
                        let _ = AppendMenuW(hmenu, MF_STRING, 1, w!("Capture Region (PrintScreen)"));
                        let _ = AppendMenuW(hmenu, MF_STRING, 2, w!("Capture Fullscreen (Shift+PrintScreen)"));
                        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, None);
                        let _ = AppendMenuW(hmenu, MF_STRING, 3, w!("Settings"));
                        let _ = AppendMenuW(hmenu, MF_STRING, 4, w!("Open Screenshots Folder"));
                        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, None);
                        let _ = AppendMenuW(hmenu, MF_STRING, 5, w!("Exit"));

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
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                return LRESULT(0);
            } else if msg == WM_DESTROY {
                PostQuitMessage(0);
                return LRESULT(0);
            }

            DefWindowProcW(hwnd, msg, _wparam, lparam)
        }
    }
}

#[cfg(not(windows))]
pub fn start_system_tray(_event_sender: Sender<TrayEvent>) {}
