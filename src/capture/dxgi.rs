use crate::capture::screen::ScreenCapture;
use crate::system::windows::PhysicalRect;
use image::{Rgba, RgbaImage};

#[inline(always)]
pub fn f16_to_f32(h: u16) -> f32 {
    let s = (h >> 15) & 0x0001;
    let e = (h >> 10) & 0x001F;
    let f = h & 0x03FF;

    if e == 0 {
        if f == 0 {
            if s == 1 { -0.0 } else { 0.0 }
        } else {
            let val = (f as f32) / (1 << 24) as f32;
            if s == 1 { -val } else { val }
        }
    } else if e == 31 {
        if f == 0 {
            if s == 1 { f32::NEG_INFINITY } else { f32::INFINITY }
        } else {
            f32::NAN
        }
    } else {
        let exp = (e as i32) - 15 + 127;
        let mant = (f as u32) << 13;
        let bits = ((s as u32) << 31) | ((exp as u32) << 23) | mant;
        f32::from_bits(bits)
    }
}

#[inline(always)]
pub fn tone_map_hdr(val: f32, sdr_white_level: f32) -> f32 {
    let norm = (val.max(0.0)) / sdr_white_level.max(1.0);
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    ((norm * (a * norm + b)) / (norm * (c * norm + d) + e)).clamp(0.0, 1.0)
}

#[inline(always)]
pub fn linear_to_srgb(linear: f32) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let srgb = if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (srgb * 255.0).round().clamp(0.0, 255.0) as u8
}

#[cfg(windows)]
pub fn create_d3d11_device() -> Result<(
    windows::Win32::Graphics::Direct3D11::ID3D11Device,
    windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
), String> {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::{
        D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_11_0,
    };
    use windows::Win32::Graphics::Direct3D11::{
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    };

    unsafe {
        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;

        let hr = D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            7,
            Some(&mut device),
            None,
            Some(&mut context),
        );

        if hr.is_err() || device.is_none() || context.is_none() {
            // Software WARP fallback
            let hr2 = D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_WARP,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                7,
                Some(&mut device),
                None,
                Some(&mut context),
            );
            if hr2.is_err() || device.is_none() || context.is_none() {
                return Err("Failed to create D3D11 device".to_string());
            }
        }

        Ok((device.unwrap(), context.unwrap()))
    }
}

#[cfg(windows)]
pub fn capture_output_dxgi(
    dupl: &windows::Win32::Graphics::Dxgi::IDXGIOutputDuplication,
    device: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
    context: &windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
    bounds: PhysicalRect,
) -> Result<RgbaImage, String> {
    use windows::core::Interface;
    use windows::Win32::Graphics::Direct3D11::{
        ID3D11Texture2D, D3D11_CPU_ACCESS_READ, D3D11_MAP_READ,
        D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
    };
    use windows::Win32::Graphics::Dxgi::Common::{
        DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_FORMAT_R8G8B8A8_UNORM,
    };
    use windows::Win32::Graphics::Dxgi::{DXGI_OUTDUPL_FRAME_INFO, IDXGIResource};

    let width = bounds.width;
    let height = bounds.height;
    if width == 0 || height == 0 {
        return Err("Invalid output dimensions".to_string());
    }

    unsafe {
        let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut desktop_resource: Option<IDXGIResource> = None;

        // Try acquiring frame (up to 150ms timeout)
        let hr = dupl.AcquireNextFrame(150, &mut frame_info, &mut desktop_resource);
        if hr.is_err() {
            return Err("AcquireNextFrame timeout or failed".to_string());
        }

        let resource = match desktop_resource {
            Some(r) => r,
            None => {
                let _ = dupl.ReleaseFrame();
                return Err("Desktop resource is null".to_string());
            }
        };

        let texture: ID3D11Texture2D = match resource.cast() {
            Ok(t) => t,
            Err(e) => {
                let _ = dupl.ReleaseFrame();
                return Err(format!("Failed to cast resource to ID3D11Texture2D: {}", e));
            }
        };

        let mut desc = D3D11_TEXTURE2D_DESC::default();
        texture.GetDesc(&mut desc);

        // Create staging texture
        let staging_desc = D3D11_TEXTURE2D_DESC {
            Width: desc.Width,
            Height: desc.Height,
            MipLevels: 1,
            ArraySize: 1,
            Format: desc.Format,
            SampleDesc: desc.SampleDesc,
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };

        let mut staging_texture: Option<ID3D11Texture2D> = None;
        if device.CreateTexture2D(&staging_desc, None, Some(&mut staging_texture)).is_err() {
            let _ = dupl.ReleaseFrame();
            return Err("Failed to create staging texture".to_string());
        }

        let staging = match staging_texture {
            Some(t) => t,
            None => {
                let _ = dupl.ReleaseFrame();
                return Err("Staging texture is null".to_string());
            }
        };

        context.CopyResource(&staging, &texture);
        let _ = dupl.ReleaseFrame();

        // Map staging texture for CPU read
        let mut mapped = windows::Win32::Graphics::Direct3D11::D3D11_MAPPED_SUBRESOURCE::default();
        if context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)).is_err() {
            return Err("Failed to map staging texture".to_string());
        }

        let row_pitch = mapped.RowPitch as usize;
        let data_ptr = mapped.pData as *const u8;
        let mut img = RgbaImage::new(desc.Width, desc.Height);

        match desc.Format {
            DXGI_FORMAT_B8G8R8A8_UNORM => {
                // BGRA -> RGBA
                for y in 0..desc.Height {
                    let src_row = data_ptr.add(y as usize * row_pitch);
                    for x in 0..desc.Width {
                        let b = *src_row.add(x as usize * 4);
                        let g = *src_row.add(x as usize * 4 + 1);
                        let r = *src_row.add(x as usize * 4 + 2);
                        img.put_pixel(x, y, Rgba([r, g, b, 255]));
                    }
                }
            }
            DXGI_FORMAT_R8G8B8A8_UNORM => {
                // Direct RGBA
                for y in 0..desc.Height {
                    let src_row = data_ptr.add(y as usize * row_pitch);
                    for x in 0..desc.Width {
                        let r = *src_row.add(x as usize * 4);
                        let g = *src_row.add(x as usize * 4 + 1);
                        let b = *src_row.add(x as usize * 4 + 2);
                        img.put_pixel(x, y, Rgba([r, g, b, 255]));
                    }
                }
            }
            DXGI_FORMAT_R16G16B16A16_FLOAT => {
                // scRGB HDR -> SDR Tone Mapping
                let sdr_white_level = 2.50; // Standard Windows reference white ~200 nits
                let half_ptr = data_ptr as *const u16;
                let half_row_elements = row_pitch / 2;

                for y in 0..desc.Height {
                    let src_row = half_ptr.add(y as usize * half_row_elements);
                    for x in 0..desc.Width {
                        let h_r = *src_row.add(x as usize * 4);
                        let h_g = *src_row.add(x as usize * 4 + 1);
                        let h_b = *src_row.add(x as usize * 4 + 2);

                        let lin_r = f16_to_f32(h_r);
                        let lin_g = f16_to_f32(h_g);
                        let lin_b = f16_to_f32(h_b);

                        let tm_r = tone_map_hdr(lin_r, sdr_white_level);
                        let tm_g = tone_map_hdr(lin_g, sdr_white_level);
                        let tm_b = tone_map_hdr(lin_b, sdr_white_level);

                        let r = linear_to_srgb(tm_r);
                        let g = linear_to_srgb(tm_g);
                        let b = linear_to_srgb(tm_b);

                        img.put_pixel(x, y, Rgba([r, g, b, 255]));
                    }
                }
            }
            _ => {
                context.Unmap(&staging, 0);
                return Err(format!("Unsupported DXGI surface format: {:?}", desc.Format));
            }
        }

        context.Unmap(&staging, 0);
        Ok(img)
    }
}

#[cfg(windows)]
pub fn capture_virtual_screen_dxgi() -> Result<ScreenCapture, String> {
    use windows::core::Interface;
    use windows::Win32::Graphics::Dxgi::{IDXGIAdapter, IDXGIDevice, IDXGIOutput1};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN,
    };

    unsafe {
        let (device, context) = create_d3d11_device()?;
        let dxgi_device: IDXGIDevice = device
            .cast()
            .map_err(|e| format!("Failed to cast device to IDXGIDevice: {}", e))?;
        let adapter: IDXGIAdapter = dxgi_device
            .GetAdapter()
            .map_err(|e| format!("Failed to get DXGI adapter: {}", e))?;

        let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN);

        if vw <= 0 || vh <= 0 {
            return Err("Invalid virtual screen metrics".to_string());
        }

        let mut composite = RgbaImage::new(vw as u32, vh as u32);
        let mut captured_any = false;

        let mut output_idx = 0;
        while let Ok(output) = adapter.EnumOutputs(output_idx) {
            output_idx += 1;
            let desc = match output.GetDesc() {
                Ok(d) => d,
                Err(_) => continue,
            };

            let mon_rect = PhysicalRect {
                x: desc.DesktopCoordinates.left,
                y: desc.DesktopCoordinates.top,
                width: (desc.DesktopCoordinates.right - desc.DesktopCoordinates.left).max(0) as u32,
                height: (desc.DesktopCoordinates.bottom - desc.DesktopCoordinates.top).max(0) as u32,
            };

            let output1: Result<IDXGIOutput1, _> = output.cast();
            let output1 = match output1 {
                Ok(o) => o,
                Err(_) => continue,
            };

            let dupl = match output1.DuplicateOutput(&dxgi_device) {
                Ok(d) => d,
                Err(_) => continue,
            };

            if let Ok(mon_img) = capture_output_dxgi(&dupl, &device, &context, mon_rect) {
                // Blit monitor image into virtual screen composite
                let offset_x = (mon_rect.x - vx).max(0) as u32;
                let offset_y = (mon_rect.y - vy).max(0) as u32;

                for y in 0..mon_img.height() {
                    let cy = offset_y + y;
                    if cy >= composite.height() {
                        break;
                    }
                    for x in 0..mon_img.width() {
                        let cx = offset_x + x;
                        if cx >= composite.width() {
                            break;
                        }
                        composite.put_pixel(cx, cy, *mon_img.get_pixel(x, y));
                    }
                }
                captured_any = true;
            }
        }

        if !captured_any {
            return Err("No DXGI outputs could be duplicated".to_string());
        }

        Ok(ScreenCapture {
            image: composite,
            bounds: PhysicalRect {
                x: vx,
                y: vy,
                width: vw as u32,
                height: vh as u32,
            },
        })
    }
}
