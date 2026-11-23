#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectedWindow {
    pub hwnd: isize,
    pub bounds: PhysicalRect,
    pub title: String,
    pub class_name: String,
    pub is_child_element: bool,
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
                hwnd: hwnd.0 as isize,
                bounds: PhysicalRect {
                    x: rect.left,
                    y: rect.top,
                    width,
                    height,
                },
                title,
                class_name,
                is_child_element: false,
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

#[cfg(windows)]
pub fn find_element_at_point(
    screen_x: i32,
    screen_y: i32,
    overlay_hwnd: Option<isize>,
) -> Option<DetectedWindow> {
    use std::cell::RefCell;
    use windows::core::Interface;
    use windows::Win32::Foundation::{HWND, POINT};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
    };
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomation2};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, SetWindowLongW, GWL_EXSTYLE, WS_EX_TRANSPARENT,
    };

    thread_local! {
        static UIA: RefCell<Option<IUIAutomation>> = const { RefCell::new(None) };
    }

    UIA.with(|cell| {
        let mut opt = cell.borrow_mut();
        if opt.is_none() {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                if let Ok(automation) =
                    CoCreateInstance::<_, IUIAutomation>(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
                {
                    if let Ok(uia2) = automation.cast::<IUIAutomation2>() {
                        let _ = uia2.SetConnectionTimeout(100);
                        let _ = uia2.SetTransactionTimeout(100);
                    }
                    *opt = Some(automation);
                }
            }
        }

        let automation = opt.as_ref()?;

        unsafe {
            let mut old_ex = 0;
            let overlay_target =
                overlay_hwnd.and_then(|h| if h != 0 { Some(HWND(h as *mut _)) } else { None });
            if let Some(hwnd) = overlay_target {
                old_ex = GetWindowLongW(hwnd, GWL_EXSTYLE);
                let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, old_ex | (WS_EX_TRANSPARENT.0 as i32));
            }

            let pt = POINT {
                x: screen_x,
                y: screen_y,
            };
            let elem_res = automation.ElementFromPoint(pt);

            if let Some(hwnd) = overlay_target {
                let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, old_ex);
            }

            let elem = elem_res.ok()?;

            // Reject elements belonging to Pixora itself
            if let Ok(pid) = elem.CurrentProcessId() {
                if pid as u32 == std::process::id() {
                    return None;
                }
            }

            let rect = elem.CurrentBoundingRectangle().ok()?;
            let width = (rect.right - rect.left).max(0) as u32;
            let height = (rect.bottom - rect.top).max(0) as u32;

            if width < 16 || height < 12 {
                return None;
            }

            let class_name = elem.CurrentClassName().unwrap_or_default().to_string();
            if class_name == "#32769" || class_name == "Progman" || class_name == "WorkerW" {
                return None;
            }

            let name = elem.CurrentName().unwrap_or_default().to_string();
            let type_name = elem.CurrentLocalizedControlType().unwrap_or_default().to_string();

            let title = if !name.trim().is_empty() {
                name.trim().to_string()
            } else if !type_name.trim().is_empty() {
                let mut c = type_name.trim().chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            } else {
                String::new()
            };

            let native_hwnd = elem
                .CurrentNativeWindowHandle()
                .map(|h| h.0 as isize)
                .unwrap_or(0);

            Some(DetectedWindow {
                hwnd: native_hwnd,
                bounds: PhysicalRect {
                    x: rect.left,
                    y: rect.top,
                    width,
                    height,
                },
                title,
                class_name,
                is_child_element: true,
            })
        }
    })
}

#[cfg(not(windows))]
pub fn find_element_at_point(
    _screen_x: i32,
    _screen_y: i32,
    _overlay_hwnd: Option<isize>,
) -> Option<DetectedWindow> {
    None
}

#[cfg(not(windows))]
pub fn enumerate_visible_windows() -> Vec<DetectedWindow> {
    Vec::new()
}

#[cfg(windows)]
pub fn get_active_window_rect() -> Option<PhysicalRect> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowRect};

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }

        let mut rect = RECT::default();
        let hr = DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut _ as *mut _,
            std::mem::size_of::<RECT>() as u32,
        );
        if hr.is_err() && GetWindowRect(hwnd, &mut rect).is_err() {
            return None;
        }

        let width = (rect.right - rect.left).max(0) as u32;
        let height = (rect.bottom - rect.top).max(0) as u32;
        Some(PhysicalRect {
            x: rect.left,
            y: rect.top,
            width,
            height,
        })
    }
}

#[cfg(windows)]
pub fn get_window_under_cursor_rect() -> Option<PhysicalRect> {
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
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
        let hr = DwmGetWindowAttribute(
            target,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut _ as *mut _,
            std::mem::size_of::<RECT>() as u32,
        );
        if hr.is_err() && GetWindowRect(target, &mut rect).is_err() {
            return None;
        }

        let width = (rect.right - rect.left).max(0) as u32;
        let height = (rect.bottom - rect.top).max(0) as u32;
        Some(PhysicalRect {
            x: rect.left,
            y: rect.top,
            width,
            height,
        })
    }
}

pub use crate::system::sound::play_capture_sound;
