use crate::capture::screen::ScreenCapture;
#[cfg(windows)]
use crate::capture::monitor::get_all_monitors;
#[cfg(windows)]
use crate::system::windows::PhysicalRect;
#[cfg(windows)]
use image::RgbaImage;

#[cfg(windows)]
pub fn is_wgc_supported() -> bool {
    use windows::Graphics::Capture::GraphicsCaptureSession;
    GraphicsCaptureSession::IsSupported().unwrap_or(false)
}

#[cfg(not(windows))]
pub fn is_wgc_supported() -> bool {
    false
}

#[cfg(windows)]
pub fn capture_virtual_screen_wgc() -> Result<ScreenCapture, String> {
    use std::sync::mpsc::channel;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use windows::core::{Interface, factory};
    use windows::Foundation::TypedEventHandler;
    use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem};
    use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
    use windows::Graphics::DirectX::DirectXPixelFormat;
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Gdi::HMONITOR;
    use windows::Win32::Graphics::Direct3D::{
        D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL,
        D3D_FEATURE_LEVEL_10_0, D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_11_0,
        D3D_FEATURE_LEVEL_11_1,
    };
    use windows::Win32::Graphics::Direct3D11::{
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
        D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
        D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
        D3D11_USAGE_STAGING,
    };
    use windows::Win32::Graphics::Dxgi::IDXGIDevice;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
    use windows::Win32::System::WinRT::Direct3D11::{
        CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
    };
    use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXSCREEN, SM_CXVIRTUALSCREEN, SM_CYSCREEN, SM_CYVIRTUALSCREEN,
        SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
    };

    // Ensure COM is initialized
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    if !is_wgc_supported() {
        return Err("Windows.Graphics.Capture is not supported on this system".to_string());
    }

    // Determine virtual screen metrics
    let mut virt_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let mut virt_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let mut virt_w = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let mut virt_h = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };

    if virt_w <= 0 || virt_h <= 0 {
        virt_x = 0;
        virt_y = 0;
        virt_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        virt_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    }

    if virt_w <= 0 || virt_h <= 0 {
        return Err("Failed to retrieve display metrics for WGC capture".to_string());
    }

    // 1. Create Direct3D11 Device
    let mut d3d_device: Option<ID3D11Device> = None;
    let mut d3d_context: Option<ID3D11DeviceContext> = None;
    let mut feature_level = D3D_FEATURE_LEVEL(0);
    let feature_levels = [
        D3D_FEATURE_LEVEL_11_1,
        D3D_FEATURE_LEVEL_11_0,
        D3D_FEATURE_LEVEL_10_1,
        D3D_FEATURE_LEVEL_10_0,
    ];

    let hr = unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE(std::ptr::null_mut()),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&feature_levels),
            D3D11_SDK_VERSION,
            Some(&mut d3d_device),
            Some(&mut feature_level),
            Some(&mut d3d_context),
        )
    };

    if hr.is_err() || d3d_device.is_none() {
        let hr_warp = unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_WARP,
                HMODULE(std::ptr::null_mut()),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&feature_levels),
                D3D11_SDK_VERSION,
                Some(&mut d3d_device),
                Some(&mut feature_level),
                Some(&mut d3d_context),
            )
        };
        hr_warp.map_err(|e| format!("Failed to create D3D11 device (WARP): {:?}", e))?;
    }

    let d3d_device = d3d_device.ok_or_else(|| "D3D11 device is None".to_string())?;
    let d3d_context = d3d_context.ok_or_else(|| "D3D11 context is None".to_string())?;

    // 2. Wrap DXGI Device in WinRT IDirect3DDevice
    let dxgi_device: IDXGIDevice = d3d_device
        .cast()
        .map_err(|e| format!("Failed to cast D3D11 device to IDXGIDevice: {:?}", e))?;
    let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device) }
        .map_err(|e| format!("CreateDirect3D11DeviceFromDXGIDevice failed: {:?}", e))?;
    let winrt_device: IDirect3DDevice = inspectable
        .cast()
        .map_err(|e| format!("Failed to cast inspectable to IDirect3DDevice: {:?}", e))?;

    // 3. Obtain IGraphicsCaptureItemInterop COM factory
    let interop: IGraphicsCaptureItemInterop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
        .map_err(|e| format!("Failed to get IGraphicsCaptureItemInterop factory: {:?}", e))?;

    // 4. Enumerate monitors and capture each
    let monitors = get_all_monitors();
    if monitors.is_empty() {
        return Err("No monitors found for WGC capture".to_string());
    }

    // Helper closure to capture one monitor by HMONITOR
    let capture_one_monitor = |hmon: HMONITOR, expected_w: u32, expected_h: u32| -> Result<RgbaImage, String> {
        let item: GraphicsCaptureItem = unsafe { interop.CreateForMonitor(hmon) }
            .map_err(|e| format!("CreateForMonitor failed: {:?}", e))?;

        let size = item.Size().map_err(|e| format!("item.Size failed: {:?}", e))?;
        if size.Width <= 0 || size.Height <= 0 {
            return Err("GraphicsCaptureItem size is non-positive".to_string());
        }

        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &winrt_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            1,
            size,
        )
        .map_err(|e| format!("CreateFreeThreaded frame pool failed: {:?}", e))?;

        let session = frame_pool
            .CreateCaptureSession(&item)
            .map_err(|e| format!("CreateCaptureSession failed: {:?}", e))?;

        // Disable hardware cursor drawing into the frame
        let _ = session.SetIsCursorCaptureEnabled(false);

        let (tx, rx) = channel::<()>();
        let tx_arc = Arc::new(Mutex::new(Some(tx)));
        let tx_clone = tx_arc.clone();

        let token = frame_pool
            .FrameArrived(&TypedEventHandler::new(move |_pool, _args| {
                if let Ok(mut lock) = tx_clone.lock() {
                    if let Some(sender) = lock.take() {
                        let _ = sender.send(());
                    }
                }
                Ok(())
            }))
            .map_err(|e| format!("FrameArrived registration failed: {:?}", e))?;

        session
            .StartCapture()
            .map_err(|e| format!("StartCapture failed: {:?}", e))?;

        // Wait up to 600ms for a frame
        let wait_res = rx.recv_timeout(Duration::from_millis(600));
        let _ = frame_pool.RemoveFrameArrived(token);

        if wait_res.is_err() {
            let _ = session.Close();
            let _ = frame_pool.Close();
            return Err("Timed out waiting for WGC frame".to_string());
        }

        let frame = frame_pool
            .TryGetNextFrame()
            .map_err(|e| format!("TryGetNextFrame failed: {:?}", e))?;
        let surface = frame
            .Surface()
            .map_err(|e| format!("frame.Surface failed: {:?}", e))?;

        let dxgi_access: IDirect3DDxgiInterfaceAccess = surface
            .cast()
            .map_err(|e| format!("Cast to IDirect3DDxgiInterfaceAccess failed: {:?}", e))?;
        let source_texture: ID3D11Texture2D = unsafe { dxgi_access.GetInterface() }
            .map_err(|e| format!("GetInterface ID3D11Texture2D failed: {:?}", e))?;

        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe {
            source_texture.GetDesc(&mut desc);
        }

        desc.BindFlags = 0;
        desc.MiscFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        desc.Usage = D3D11_USAGE_STAGING;

        let mut staging_texture: Option<ID3D11Texture2D> = None;
        unsafe {
            d3d_device
                .CreateTexture2D(&desc, None, Some(&mut staging_texture))
                .map_err(|e| format!("CreateTexture2D staging failed: {:?}", e))?;
        }
        let staging = staging_texture.ok_or_else(|| "Staging texture is None".to_string())?;

        unsafe {
            d3d_context.CopyResource(&staging, &source_texture);
        }

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            d3d_context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(|e| format!("Map staging texture failed: {:?}", e))?;
        }

        let tex_w = desc.Width;
        let tex_h = desc.Height;
        let row_pitch = mapped.RowPitch as usize;

        let mut raw_rgba = vec![0u8; (tex_w * tex_h * 4) as usize];
        unsafe {
            let src_ptr = mapped.pData as *const u8;
            for row in 0..tex_h as usize {
                let src_row = std::slice::from_raw_parts(src_ptr.add(row * row_pitch), tex_w as usize * 4);
                let dst_row = &mut raw_rgba[row * (tex_w as usize * 4)..(row + 1) * (tex_w as usize * 4)];
                for (chunk_in, chunk_out) in src_row.chunks_exact(4).zip(dst_row.chunks_exact_mut(4)) {
                    chunk_out[0] = chunk_in[2]; // R
                    chunk_out[1] = chunk_in[1]; // G
                    chunk_out[2] = chunk_in[0]; // B
                    chunk_out[3] = 255;          // A
                }
            }
            d3d_context.Unmap(&staging, 0);
        }

        let _ = session.Close();
        let _ = frame_pool.Close();

        let mut img = RgbaImage::from_raw(tex_w, tex_h, raw_rgba)
            .ok_or_else(|| "Failed to construct RgbaImage from WGC buffer".to_string())?;

        // If dimensions differ due to DPI scale, resize to expected monitor dimensions
        if (tex_w != expected_w || tex_h != expected_h) && expected_w > 0 && expected_h > 0 {
            img = image::imageops::resize(&img, expected_w, expected_h, image::imageops::FilterType::Nearest);
        }

        Ok(img)
    };

    // 5. Enumerate monitor handles via EnumDisplayMonitors
    use windows::core::BOOL;
    use windows::Win32::Foundation::{LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, MONITORINFO};

    struct MonItem {
        hmon: HMONITOR,
        rect: RECT,
    }

    unsafe extern "system" fn enum_hmon_proc(
        hmon: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let list_ptr = lparam.0 as *mut Vec<MonItem>;
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetMonitorInfoW(hmon, &mut mi).as_bool() } {
            unsafe {
                (*list_ptr).push(MonItem {
                    hmon,
                    rect: mi.rcMonitor,
                });
            }
        }
        BOOL(1)
    }

    let mut hmon_list: Vec<MonItem> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(enum_hmon_proc),
            LPARAM(&mut hmon_list as *mut _ as isize),
        );
    }
    if hmon_list.is_empty() {
        return Err("EnumDisplayMonitors returned no monitors for WGC".to_string());
    }

    // 6. If single monitor matching virtual screen, capture directly
    if hmon_list.len() == 1 {
        let mon = &hmon_list[0];
        let w = (mon.rect.right - mon.rect.left).abs() as u32;
        let h = (mon.rect.bottom - mon.rect.top).abs() as u32;
        let img = capture_one_monitor(mon.hmon, w, h)?;

        return Ok(ScreenCapture {
            image: img,
            bounds: PhysicalRect {
                x: mon.rect.left,
                y: mon.rect.top,
                width: w,
                height: h,
            },
        });
    }

    // 7. Multi-monitor: compose all captured monitors onto virtual screen canvas
    let mut combined = RgbaImage::new(virt_w as u32, virt_h as u32);
    for mon in &hmon_list {
        let mon_w = (mon.rect.right - mon.rect.left).abs() as u32;
        let mon_h = (mon.rect.bottom - mon.rect.top).abs() as u32;
        if mon_w == 0 || mon_h == 0 {
            continue;
        }

        let mon_img = capture_one_monitor(mon.hmon, mon_w, mon_h)?;
        let local_x = (mon.rect.left - virt_x).max(0) as i64;
        let local_y = (mon.rect.top - virt_y).max(0) as i64;
        image::imageops::overlay(&mut combined, &mon_img, local_x, local_y);
    }

    Ok(ScreenCapture {
        image: combined,
        bounds: PhysicalRect {
            x: virt_x,
            y: virt_y,
            width: virt_w as u32,
            height: virt_h as u32,
        },
    })
}

#[cfg(not(windows))]
pub fn capture_virtual_screen_wgc() -> Result<ScreenCapture, String> {
    Err("WGC capture only supported on Windows".to_string())
}
