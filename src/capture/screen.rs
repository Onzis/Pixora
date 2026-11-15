use crate::system::windows::PhysicalRect;
use image::RgbaImage;

#[derive(Clone, Debug)]
pub struct ScreenCapture {
    pub image: RgbaImage,
    pub bounds: PhysicalRect,
}

#[cfg(windows)]
pub fn capture_virtual_screen() -> Result<ScreenCapture, String> {
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ROP_CODE, ReleaseDC, SRCCOPY,
        SelectObject,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXSCREEN, SM_CXVIRTUALSCREEN, SM_CYSCREEN, SM_CYVIRTUALSCREEN,
        SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
    };

    unsafe {
        let mut x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let mut y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let mut width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let mut height = GetSystemMetrics(SM_CYVIRTUALSCREEN);

        // Fallback to primary screen metrics if virtual screen returned 0
        if width <= 0 || height <= 0 {
            x = 0;
            y = 0;
            width = GetSystemMetrics(SM_CXSCREEN);
            height = GetSystemMetrics(SM_CYSCREEN);
        }

        if width <= 0 || height <= 0 {
            return Err("Failed to retrieve display dimensions".to_string());
        }

        let width_u = width as u32;
        let height_u = height as u32;

        let hdc_screen = GetDC(None);
        if hdc_screen.0.is_null() {
            return Err("Failed to get desktop device context".to_string());
        }

        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        if hdc_mem.0.is_null() {
            ReleaseDC(None, hdc_screen);
            return Err("Failed to create memory device context".to_string());
        }

        let hbitmap = CreateCompatibleBitmap(hdc_screen, width, height);
        if hbitmap.0.is_null() {
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(None, hdc_screen);
            return Err("Failed to create compatible bitmap".to_string());
        }

        let old_bitmap = SelectObject(hdc_mem, hbitmap.into());

        // SRCCOPY with CAPTUREBLT (0x40000000) captures translucent windows
        let rop = ROP_CODE(SRCCOPY.0 | 0x40000000);
        let blt_res = BitBlt(hdc_mem, 0, 0, width, height, Some(hdc_screen), x, y, rop);

        if blt_res.is_err() {
            SelectObject(hdc_mem, old_bitmap);
            let _ = DeleteObject(hbitmap.into());
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(None, hdc_screen);
            return Err("BitBlt screen capture failed".to_string());
        }

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // negative means top-down bitmap
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut raw_buffer = vec![0u8; (width_u * height_u * 4) as usize];
        let lines = GetDIBits(
            hdc_mem,
            hbitmap,
            0,
            height_u,
            Some(raw_buffer.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        SelectObject(hdc_mem, old_bitmap);
        let _ = DeleteObject(hbitmap.into());
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(None, hdc_screen);

        if lines == 0 {
            return Err("GetDIBits failed to copy bitmap bits".to_string());
        }

        // Convert BGRA to RGBA in place
        for chunk in raw_buffer.chunks_exact_mut(4) {
            let b = chunk[0];
            let r = chunk[2];
            chunk[0] = r;
            chunk[2] = b;
            chunk[3] = 255;
        }

        let image = RgbaImage::from_raw(width_u, height_u, raw_buffer)
            .ok_or_else(|| "Failed to construct RgbaImage from buffer".to_string())?;

        Ok(ScreenCapture {
            image,
            bounds: PhysicalRect {
                x,
                y,
                width: width_u,
                height: height_u,
            },
        })
    }
}

#[cfg(not(windows))]
pub fn capture_virtual_screen() -> Result<ScreenCapture, String> {
    Err("Screen capture only implemented on Windows".to_string())
}
