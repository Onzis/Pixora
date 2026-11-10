use crate::system::windows::PhysicalRect;

#[derive(Clone, Debug)]
pub struct MonitorInfo {
    pub rect: PhysicalRect,
    pub is_primary: bool,
}

#[cfg(windows)]
pub fn get_all_monitors() -> Vec<MonitorInfo> {
    use std::sync::Mutex;
    use windows::core::BOOL;
    use windows::Win32::Foundation::{LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
    };

    static MONITORS: Mutex<Vec<MonitorInfo>> = Mutex::new(Vec::new());

    unsafe extern "system" fn enum_proc(
        hmon: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        _lparam: LPARAM,
    ) -> BOOL {
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };

        let ok = unsafe { GetMonitorInfoW(hmon, &mut mi).as_bool() };
        if ok {
            let width = (mi.rcMonitor.right - mi.rcMonitor.left).max(0) as u32;
            let height = (mi.rcMonitor.bottom - mi.rcMonitor.top).max(0) as u32;
            let is_primary = (mi.dwFlags & 1) != 0;

            if let Ok(mut list) = MONITORS.lock() {
                list.push(MonitorInfo {
                    rect: PhysicalRect {
                        x: mi.rcMonitor.left,
                        y: mi.rcMonitor.top,
                        width,
                        height,
                    },
                    is_primary,
                });
            }
        }
        BOOL(1)
    }

    if let Ok(mut list) = MONITORS.lock() {
        list.clear();
    }

    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(enum_proc), LPARAM(0));
    }

    MONITORS.lock().unwrap().clone()
}

#[cfg(not(windows))]
pub fn get_all_monitors() -> Vec<MonitorInfo> {
    vec![]
}
