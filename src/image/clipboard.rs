use image::RgbaImage;

#[cfg(windows)]
pub fn copy_image_to_clipboard(image: &RgbaImage) -> Result<(), String> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Graphics::Gdi::{BI_RGB, BITMAPINFOHEADER};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};

    let width = image.width();
    let height = image.height();
    if width == 0 || height == 0 {
        return Err("Cannot copy empty image".to_string());
    }

    let header_size = std::mem::size_of::<BITMAPINFOHEADER>();
    let row_stride = (width * 4) as usize;
    let data_size = row_stride * (height as usize);
    let total_size = header_size + data_size;

    unsafe {
        let hmem = GlobalAlloc(GMEM_MOVEABLE, total_size)
            .map_err(|e| format!("GlobalAlloc failed: {}", e))?;
        if hmem.0.is_null() {
            return Err("GlobalAlloc returned null handle".to_string());
        }

        let ptr = GlobalLock(hmem) as *mut u8;
        if ptr.is_null() {
            return Err("GlobalLock failed".to_string());
        }

        let bmi = BITMAPINFOHEADER {
            biSize: header_size as u32,
            biWidth: width as i32,
            biHeight: height as i32, // Positive for standard bottom-up CF_DIB
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: data_size as u32,
            ..Default::default()
        };

        std::ptr::copy_nonoverlapping(&bmi as *const _ as *const u8, ptr, header_size);

        let dest_pixels = ptr.add(header_size);
        for y in 0..height {
            let src_y = height - 1 - y; // Bottom-up row order
            for x in 0..width {
                let pixel = image.get_pixel(x, src_y);
                let dest_idx = ((y * width + x) * 4) as usize;
                // CF_DIB 32-bit expects BGRA
                *dest_pixels.add(dest_idx) = pixel[2];     // B
                *dest_pixels.add(dest_idx + 1) = pixel[1]; // G
                *dest_pixels.add(dest_idx + 2) = pixel[0]; // R
                *dest_pixels.add(dest_idx + 3) = pixel[3]; // A
            }
        }

        let _ = GlobalUnlock(hmem);

        OpenClipboard(None).map_err(|e| format!("OpenClipboard failed: {}", e))?;
        let _ = EmptyClipboard();

        // CF_DIB is 8
        const CF_DIB: u32 = 8;
        let set_res = SetClipboardData(CF_DIB, Some(HANDLE(hmem.0)));
        let _ = CloseClipboard();

        set_res.map_err(|e| format!("SetClipboardData failed: {}", e))?;
        Ok(())
    }
}

#[cfg(windows)]
pub fn copy_text_to_clipboard(text: &str) -> Result<(), String> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};

    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let total_bytes = wide.len() * 2;

    unsafe {
        let hmem = GlobalAlloc(GMEM_MOVEABLE, total_bytes)
            .map_err(|e| format!("GlobalAlloc failed: {}", e))?;
        if hmem.0.is_null() {
            return Err("GlobalAlloc returned null".to_string());
        }

        let ptr = GlobalLock(hmem) as *mut u16;
        if ptr.is_null() {
            return Err("GlobalLock returned null".to_string());
        }

        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
        let _ = GlobalUnlock(hmem);

        OpenClipboard(None).map_err(|e| format!("OpenClipboard failed: {}", e))?;
        let _ = EmptyClipboard();

        // CF_UNICODETEXT is 13
        const CF_UNICODETEXT: u32 = 13;
        let set_res = SetClipboardData(CF_UNICODETEXT, Some(HANDLE(hmem.0)));
        let _ = CloseClipboard();

        set_res.map_err(|e| format!("SetClipboardData failed: {}", e))?;
        Ok(())
    }
}

#[cfg(not(windows))]
pub fn copy_image_to_clipboard(_image: &RgbaImage) -> Result<(), String> {
    Err("Clipboard only implemented on Windows".to_string())
}

#[cfg(not(windows))]
pub fn copy_text_to_clipboard(_text: &str) -> Result<(), String> {
    Err("Clipboard only implemented on Windows".to_string())
}
