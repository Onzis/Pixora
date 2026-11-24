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
    pub top_level_hwnd: isize,
    pub z_order: usize,
    pub bounds: PhysicalRect,
    pub title: String,
    pub class_name: String,
    pub is_child_element: bool,
}

#[cfg(windows)]
pub fn enumerate_visible_windows() -> Vec<DetectedWindow> {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, EnumWindows, GetClassNameW, GetClientRect, GetWindowLongW,
        GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
        GWL_EXSTYLE, WS_EX_TOOLWINDOW,
    };

    struct EnumContext {
        windows: Vec<DetectedWindow>,
        current_pid: u32,
        current_z: usize,
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

            let z_order = ctx.current_z;
            ctx.current_z += 1;
            let top_level_hwnd = hwnd.0 as isize;

            // 1. Full window bounds
            ctx.windows.push(DetectedWindow {
                hwnd: hwnd.0 as isize,
                top_level_hwnd,
                z_order,
                bounds: PhysicalRect {
                    x: rect.left,
                    y: rect.top,
                    width,
                    height,
                },
                title: title.clone(),
                class_name: class_name.clone(),
                is_child_element: false,
            });

            // 2. Client area rectangle (inner area excluding title bar and borders)
            let mut client_rect = RECT::default();
            if GetClientRect(hwnd, &mut client_rect).is_ok() {
                let mut pt = POINT { x: client_rect.left, y: client_rect.top };
                let _ = ClientToScreen(hwnd, &mut pt);
                let cl_w = (client_rect.right - client_rect.left).max(0) as u32;
                let cl_h = (client_rect.bottom - client_rect.top).max(0) as u32;
                if cl_w >= 30 && cl_h >= 30 && (cl_w != width || cl_h != height) {
                    ctx.windows.push(DetectedWindow {
                        hwnd: hwnd.0 as isize,
                        top_level_hwnd,
                        z_order,
                        bounds: PhysicalRect {
                            x: pt.x,
                            y: pt.y,
                            width: cl_w,
                            height: cl_h,
                        },
                        title: title.clone(),
                        class_name: class_name.clone(),
                        is_child_element: true,
                    });
                }
            }

            // 3. Child controls and child windows (ShareX method)
            struct ChildContext {
                parent_hwnd: HWND,
                parent_rect: RECT,
                parent_title: String,
                top_level_hwnd: isize,
                z_order: usize,
                children: Vec<DetectedWindow>,
            }

            unsafe extern "system" fn enum_child(child: HWND, lparam: LPARAM) -> BOOL {
                unsafe {
                    let ctx = &mut *(lparam.0 as *mut ChildContext);
                    if !IsWindowVisible(child).as_bool() {
                        return BOOL(1);
                    }
                    let mut cr = RECT::default();
                    if GetWindowRect(child, &mut cr).is_ok() {
                        let ix0 = cr.left.max(ctx.parent_rect.left);
                        let iy0 = cr.top.max(ctx.parent_rect.top);
                        let ix1 = cr.right.min(ctx.parent_rect.right);
                        let iy1 = cr.bottom.min(ctx.parent_rect.bottom);
                        let iw = (ix1 - ix0).max(0) as u32;
                        let ih = (iy1 - iy0).max(0) as u32;
                        let pw = (ctx.parent_rect.right - ctx.parent_rect.left) as u32;
                        let ph = (ctx.parent_rect.bottom - ctx.parent_rect.top) as u32;
                        if iw >= 10 && ih >= 8 && (iw < pw || ih < ph) {
                            let mut text_buf = [0u16; 256];
                            let len = GetWindowTextW(child, &mut text_buf);
                            let title = if len > 0 {
                                String::from_utf16_lossy(&text_buf[..len as usize])
                            } else {
                                ctx.parent_title.clone()
                            };
                            let mut class_buf = [0u16; 256];
                            let c_len = GetClassNameW(child, &mut class_buf);
                            let class_name = if c_len > 0 {
                                String::from_utf16_lossy(&class_buf[..c_len as usize])
                            } else {
                                String::new()
                            };
                            ctx.children.push(DetectedWindow {
                                hwnd: child.0 as isize,
                                top_level_hwnd: ctx.top_level_hwnd,
                                z_order: ctx.z_order,
                                bounds: PhysicalRect {
                                    x: ix0,
                                    y: iy0,
                                    width: iw,
                                    height: ih,
                                },
                                title,
                                class_name,
                                is_child_element: true,
                            });
                        }
                    }
                    BOOL(1)
                }
            }

            let mut child_ctx = ChildContext {
                parent_hwnd: hwnd,
                parent_rect: rect,
                parent_title: title.clone(),
                top_level_hwnd,
                z_order,
                children: Vec::new(),
            };
            let _ = EnumChildWindows(Some(hwnd), Some(enum_child), LPARAM(&mut child_ctx as *mut _ as isize));
            ctx.windows.extend(child_ctx.children);

            BOOL(1)
        }
    }

    let mut ctx = EnumContext {
        windows: Vec::new(),
        current_pid: std::process::id(),
        current_z: 0,
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
        GetAncestor, GetWindowLongW, GetWindowTextW, SetWindowLongW, SetWindowPos, WindowFromPoint,
        GA_ROOT, GWL_EXSTYLE, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
        WS_EX_TRANSPARENT,
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
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
                );
            }

            let pt = POINT {
                x: screen_x,
                y: screen_y,
            };
            let elem_res = automation.ElementFromPoint(pt);

            if let Some(hwnd) = overlay_target {
                let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, old_ex);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
                );
            }

            let mut elem = match elem_res {
                Ok(e) => {
                    if let Ok(pid) = e.CurrentProcessId() {
                        if pid as u32 == std::process::id() {
                            None
                        } else {
                            Some(e)
                        }
                    } else {
                        Some(e)
                    }
                }
                Err(_) => None,
            };

            // Fallback: If ElementFromPoint hit Pixora or nothing, use WindowFromPoint to get the underlying HWND
            if elem.is_none() {
                let hit_hwnd = WindowFromPoint(pt);
                if !hit_hwnd.is_invalid() && hit_hwnd.0 as isize != overlay_hwnd.unwrap_or(0) {
                    if let Ok(target_elem) = automation.ElementFromHandle(hit_hwnd) {
                        elem = Some(target_elem);
                    }
                }
            }

            let elem = elem?;

            // Drill down to the innermost child element containing the cursor (buttons, tabs, list items, icons)
            let mut deepest = elem.clone();
            if let Ok(walker) = automation.ControlViewWalker() {
                let mut current = elem;
                for _ in 0..6 {
                    let mut next_current = None;
                    if let Ok(mut child) = walker.GetFirstChildElement(&current) {
                        loop {
                            if let Ok(cr) = child.CurrentBoundingRectangle() {
                                if cr.left <= screen_x
                                    && screen_x <= cr.right
                                    && cr.top <= screen_y
                                    && screen_y <= cr.bottom
                                {
                                    let cw = (cr.right - cr.left).max(0) as u32;
                                    let ch = (cr.bottom - cr.top).max(0) as u32;
                                    if cw >= 8 && ch >= 8 {
                                        next_current = Some(child.clone());
                                        break;
                                    }
                                }
                            }
                            match walker.GetNextSiblingElement(&child) {
                                Ok(next) => child = next,
                                Err(_) => break,
                            }
                        }
                    }
                    if let Some(child_elem) = next_current {
                        current = child_elem.clone();
                        deepest = child_elem;
                    } else {
                        break;
                    }
                }
            }
            let elem = deepest;

            let rect = elem.CurrentBoundingRectangle().ok()?;
            let width = (rect.right - rect.left).max(0) as u32;
            let height = (rect.bottom - rect.top).max(0) as u32;

            if width < 8 || height < 8 {
                return None;
            }

            let class_name = elem.CurrentClassName().unwrap_or_default().to_string();
            if class_name == "#32769" || class_name == "Progman" || class_name == "WorkerW" {
                return None;
            }

            let name = elem.CurrentName().unwrap_or_default().to_string();
            let type_name = elem.CurrentLocalizedControlType().unwrap_or_default().to_string();

            let native_hwnd = elem
                .CurrentNativeWindowHandle()
                .map(|h| h.0 as isize)
                .unwrap_or(0);

            let title = if !name.trim().is_empty() {
                name.trim().to_string()
            } else if !type_name.trim().is_empty() {
                let mut c = type_name.trim().chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            } else if native_hwnd != 0 {
                let root_hwnd = GetAncestor(HWND(native_hwnd as *mut _), GA_ROOT);
                let target_hwnd = if !root_hwnd.is_invalid() {
                    root_hwnd
                } else {
                    HWND(native_hwnd as *mut _)
                };
                let mut text_buf = [0u16; 256];
                let len = GetWindowTextW(target_hwnd, &mut text_buf);
                if len > 0 {
                    String::from_utf16_lossy(&text_buf[..len as usize])
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            Some(DetectedWindow {
                hwnd: native_hwnd,
                top_level_hwnd: 0,
                z_order: 0,
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

#[cfg(target_os = "linux")]
pub fn enumerate_visible_windows() -> Vec<DetectedWindow> {
    enumerate_linux_windows().unwrap_or_default()
}

#[cfg(target_os = "linux")]
fn enumerate_linux_windows() -> Option<Vec<DetectedWindow>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, Window};

    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let screen = conn.setup().roots.get(screen_num)?;
    let root = screen.root;

    let client_list_atom = conn.intern_atom(false, b"_NET_CLIENT_LIST").ok()?.reply().ok()?.atom;
    let net_wm_name_atom = conn.intern_atom(false, b"_NET_WM_NAME").ok()?.reply().ok()?.atom;
    let utf8_string_atom = conn.intern_atom(false, b"UTF8_STRING").ok()?.reply().ok()?.atom;

    let prop = conn.get_property(
        false,
        root,
        client_list_atom,
        AtomEnum::WINDOW,
        0,
        1024,
    ).ok()?.reply().ok()?;

    let mut result = Vec::new();
    let mut z_order = 0;

    for chunk in prop.value.chunks_exact(4) {
        let win_id = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as Window;
        if win_id == 0 || win_id == root {
            continue;
        }

        let geom = match conn.get_geometry(win_id).ok().and_then(|c| c.reply().ok()) {
            Some(g) => g,
            None => continue,
        };

        let coords = match conn.translate_coordinates(win_id, root, 0, 0).ok().and_then(|c| c.reply().ok()) {
            Some(tc) => tc,
            None => continue,
        };

        let w = geom.width as u32;
        let h = geom.height as u32;
        if w < 30 || h < 30 {
            continue;
        }

        let mut title = String::new();
        if let Ok(name_cookie) = conn.get_property(false, win_id, net_wm_name_atom, utf8_string_atom, 0, 128) {
            if let Ok(name_reply) = name_cookie.reply() {
                if !name_reply.value.is_empty() {
                    title = String::from_utf8_lossy(&name_reply.value).to_string();
                }
            }
        }
        if title.is_empty() {
            if let Ok(name_cookie) = conn.get_property(false, win_id, AtomEnum::WM_NAME, AtomEnum::STRING, 0, 128) {
                if let Ok(name_reply) = name_cookie.reply() {
                    if !name_reply.value.is_empty() {
                        title = String::from_utf8_lossy(&name_reply.value).to_string();
                    }
                }
            }
        }

        result.push(DetectedWindow {
            hwnd: win_id as isize,
            top_level_hwnd: win_id as isize,
            z_order,
            bounds: PhysicalRect {
                x: coords.dst_x as i32,
                y: coords.dst_y as i32,
                width: w,
                height: h,
            },
            title,
            class_name: "X11Window".to_string(),
            is_child_element: false,
        });
        z_order += 1;
    }

    Some(result)
}

#[cfg(all(not(windows), not(target_os = "linux")))]
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

#[cfg(target_os = "linux")]
pub fn get_active_window_rect() -> Option<PhysicalRect> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, Window};

    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let screen = conn.setup().roots.get(screen_num)?;
    let root = screen.root;

    let active_win_atom = conn.intern_atom(false, b"_NET_ACTIVE_WINDOW").ok()?.reply().ok()?.atom;
    let prop = conn.get_property(false, root, active_win_atom, AtomEnum::WINDOW, 0, 1).ok()?.reply().ok()?;

    if prop.value.len() >= 4 {
        let win_id = u32::from_ne_bytes([prop.value[0], prop.value[1], prop.value[2], prop.value[3]]) as Window;
        if win_id != 0 && win_id != root {
            let geom = conn.get_geometry(win_id).ok()?.reply().ok()?;
            let coords = conn.translate_coordinates(win_id, root, 0, 0).ok()?.reply().ok()?;
            return Some(PhysicalRect {
                x: coords.dst_x as i32,
                y: coords.dst_y as i32,
                width: geom.width as u32,
                height: geom.height as u32,
            });
        }
    }
    None
}

#[cfg(all(not(windows), not(target_os = "linux")))]
pub fn get_active_window_rect() -> Option<PhysicalRect> {
    None
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

#[cfg(target_os = "linux")]
pub fn get_window_under_cursor_rect() -> Option<PhysicalRect> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::ConnectionExt;

    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let screen = conn.setup().roots.get(screen_num)?;
    let root = screen.root;

    let pointer = conn.query_pointer(root).ok()?.reply().ok()?;
    let child = pointer.child;
    if child != 0 && child != root {
        let geom = conn.get_geometry(child).ok()?.reply().ok()?;
        let coords = conn.translate_coordinates(child, root, 0, 0).ok()?.reply().ok()?;
        return Some(PhysicalRect {
            x: coords.dst_x as i32,
            y: coords.dst_y as i32,
            width: geom.width as u32,
            height: geom.height as u32,
        });
    }
    None
}

#[cfg(all(not(windows), not(target_os = "linux")))]
pub fn get_window_under_cursor_rect() -> Option<PhysicalRect> {
    None
}

pub fn detect_visual_panes(
    img: &image::RgbaImage,
    win_x: u32,
    win_y: u32,
    win_w: u32,
    win_h: u32,
) -> Vec<PhysicalRect> {
    let mut panes = Vec::new();
    if win_w < 60 || win_h < 40 {
        return panes;
    }

    let max_x = (win_x + win_w).min(img.width());
    let max_y = (win_y + win_h).min(img.height());
    let h = max_y - win_y;
    let w = max_x - win_x;

    let step_y = (h / 150).max(1);
    let step_x = (w / 150).max(1);

    // 1. Detect major vertical divider columns across the window
    let mut v_cuts = Vec::new();
    let min_v_edge_count = (h as f32 * 0.35) as usize;

    for x in (win_x + 24..max_x.saturating_sub(24)).step_by(2) {
        let mut edge_count = 0;
        for y in (win_y..max_y).step_by(step_y as usize) {
            let p1 = img.get_pixel(x, y);
            let p0 = img.get_pixel(x - 1, y);
            let diff = (p1[0] as i32 - p0[0] as i32).abs()
                + (p1[1] as i32 - p0[1] as i32).abs()
                + (p1[2] as i32 - p0[2] as i32).abs();
            if diff > 20 {
                edge_count += step_y as usize;
            }
        }
        if edge_count >= min_v_edge_count {
            if v_cuts.last().map_or(true, |&last| x > last + 16) {
                v_cuts.push(x);
            }
        }
    }

    // 2. Detect major horizontal divider rows across the window
    let mut h_cuts = Vec::new();
    let min_h_edge_count = (w as f32 * 0.65) as usize;

    for y in (win_y + 20..max_y.saturating_sub(20)).step_by(2) {
        let mut edge_count = 0;
        for x in (win_x..max_x).step_by(step_x as usize) {
            let p1 = img.get_pixel(x, y);
            let p0 = img.get_pixel(x, y - 1);
            let diff = (p1[0] as i32 - p0[0] as i32).abs()
                + (p1[1] as i32 - p0[1] as i32).abs()
                + (p1[2] as i32 - p0[2] as i32).abs();
            if diff > 20 {
                edge_count += step_x as usize;
            }
        }
        if edge_count >= min_h_edge_count {
            if h_cuts.last().map_or(true, |&last| y > last + 14) {
                h_cuts.push(y);
            }
        }
    }

    // Column boundaries
    let mut xs = vec![win_x];
    xs.extend_from_slice(&v_cuts);
    xs.push(max_x);

    // Row boundaries across full window
    let mut ys = vec![win_y];
    ys.extend_from_slice(&h_cuts);
    ys.push(max_y);

    // Full vertical column panes (sidebars, major panels)
    for i in 0..xs.len() - 1 {
        let x0 = xs[i];
        let x1 = xs[i + 1];
        let pw = x1 - x0;
        if pw >= 40 && pw < win_w {
            panes.push(PhysicalRect {
                x: x0 as i32,
                y: win_y as i32,
                width: pw,
                height: h,
            });
        }
    }

    // Full horizontal row panes (toolbars, headers, status bars)
    for j in 0..ys.len() - 1 {
        let y0 = ys[j];
        let y1 = ys[j + 1];
        let ph = y1 - y0;
        if ph >= 24 && ph < win_h {
            panes.push(PhysicalRect {
                x: win_x as i32,
                y: y0 as i32,
                width: w,
                height: ph,
            });
        }
    }

    // Grid cell panes bounded by both vertical and horizontal dividers (e.g. content pane excluding sidebar, toolbar, and status bar)
    if !v_cuts.is_empty() && !h_cuts.is_empty() {
        for i in 0..xs.len() - 1 {
            let x0 = xs[i];
            let x1 = xs[i + 1];
            let pw = x1 - x0;
            if pw < 40 {
                continue;
            }
            for j in 0..ys.len() - 1 {
                let y0 = ys[j];
                let y1 = ys[j + 1];
                let ph = y1 - y0;
                if ph >= 24 && (pw < win_w || ph < win_h) {
                    panes.push(PhysicalRect {
                        x: x0 as i32,
                        y: y0 as i32,
                        width: pw,
                        height: ph,
                    });
                }
            }
        }
    }

    panes
}

#[cfg(windows)]
pub fn set_window_exclude_from_capture(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WINDOW_DISPLAY_AFFINITY};
    // WDA_EXCLUDEFROMCAPTURE = 0x00000011 (Windows 10 2004+ and Windows 11)
    unsafe {
        let _ = SetWindowDisplayAffinity(hwnd, WINDOW_DISPLAY_AFFINITY(0x00000011));
    }
}

#[cfg(not(windows))]
pub fn set_window_exclude_from_capture(_hwnd: isize) {}

pub use crate::system::sound::play_capture_sound;
