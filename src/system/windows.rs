#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectedWindow {
    pub bounds: PhysicalRect,
    pub title: String,
    pub class_name: String,
}

#[cfg(windows)]
pub fn enumerate_visible_windows() -> Vec<DetectedWindow> {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindowLongW, GetWindowRect, GetWindowTextW,
        GetWindowThreadProcessId, IsIconic, IsWindowVisible, GWL_EXSTYLE,
        WS_EX_TOOLWINDOW,
    };

    struct EnumContext {
        windows: Vec<DetectedWindow>,
        current_pid: u32,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        unsafe {
            let ctx = &mut *(lparam.0 as *mut EnumContext);

            if !IsWindowVisible(hwnd).as_bool() {
                return BOOL(1);
            }
            if IsIconic(hwnd).as_bool() {
                return BOOL(1);
            }

            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == ctx.current_pid {
                return BOOL(1);
            }

            let mut cloaked: u32 = 0;
            let hr = DwmGetWindowAttribute(
                hwnd,
                DWMWA_CLOAKED,
                &mut cloaked as *mut _ as *mut _,
                std::mem::size_of::<u32>() as u32,
            );
            if hr.is_ok() && cloaked != 0 {
                return BOOL(1);
            }

            let mut class_buf = [0u16; 256];
            let c_len = GetClassNameW(hwnd, &mut class_buf);
            let class_name = if c_len > 0 {
                String::from_utf16_lossy(&class_buf[..c_len as usize])
            } else {
                String::new()
            };

            if class_name == "Progman" || class_name == "WorkerW" {
                return BOOL(1);
            }

            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            let mut text_buf = [0u16; 512];
            let len = GetWindowTextW(hwnd, &mut text_buf);
            let title = if len > 0 {
                String::from_utf16_lossy(&text_buf[..len as usize])
            } else {
                String::new()
            };

            if (ex_style & WS_EX_TOOLWINDOW.0) != 0 && title.is_empty() {
                return BOOL(1);
            }

            let mut rect = RECT::default();
            let hr = DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut rect as *mut _ as *mut _,
                std::mem::size_of::<RECT>() as u32,
            );
            if hr.is_err() {
                let _ = GetWindowRect(hwnd, &mut rect);
            }

            let width = (rect.right - rect.left).max(0) as u32;
            let height = (rect.bottom - rect.top).max(0) as u32;

            if width < 30 || height < 30 {
                return BOOL(1);
            }

            ctx.windows.push(DetectedWindow {
                bounds: PhysicalRect {
                    x: rect.left,
                    y: rect.top,
                    width,
                    height,
                },
                title,
                class_name,
            });

            BOOL(1)
        }
    }

    let mut ctx = EnumContext {
        windows: Vec::new(),
        current_pid: std::process::id(),
    };

    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut ctx as *mut _ as isize));
    }

    ctx.windows
}

#[cfg(not(windows))]
pub fn enumerate_visible_windows() -> Vec<DetectedWindow> {
    Vec::new()
}

#[cfg(windows)]
pub fn get_active_window_rect() -> Option<PhysicalRect> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowRect};

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }

        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_ok() {
            let width = (rect.right - rect.left).max(0) as u32;
            let height = (rect.bottom - rect.top).max(0) as u32;
            Some(PhysicalRect {
                x: rect.left,
                y: rect.top,
                width,
                height,
            })
        } else {
            None
        }
    }
}

#[cfg(windows)]
pub fn get_window_under_cursor_rect() -> Option<PhysicalRect> {
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        GA_ROOT, GetAncestor, GetCursorPos, GetWindowRect, WindowFromPoint,
    };

    unsafe {
        let mut pt = POINT::default();
        if GetCursorPos(&mut pt).is_err() {
            return None;
        }

        let hwnd_pt = WindowFromPoint(pt);
        if hwnd_pt.0.is_null() {
            return None;
        }

        let root = GetAncestor(hwnd_pt, GA_ROOT);
        let target = if root.0.is_null() { hwnd_pt } else { root };

        let mut rect = RECT::default();
        if GetWindowRect(target, &mut rect).is_ok() {
            let width = (rect.right - rect.left).max(0) as u32;
            let height = (rect.bottom - rect.top).max(0) as u32;
            Some(PhysicalRect {
                x: rect.left,
                y: rect.top,
                width,
                height,
            })
        } else {
            None
        }
    }
}

pub use crate::system::sound::play_capture_sound;
