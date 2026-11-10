#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
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

#[cfg(windows)]
pub fn play_capture_sound() {
    unsafe {
        unsafe extern "system" {
            fn MessageBeep(uType: u32) -> i32;
        }
        let _ = MessageBeep(0);
    }
}

#[cfg(not(windows))]
pub fn get_active_window_rect() -> Option<PhysicalRect> {
    None
}

#[cfg(not(windows))]
pub fn get_window_under_cursor_rect() -> Option<PhysicalRect> {
    None
}

#[cfg(not(windows))]
pub fn play_capture_sound() {}
