use image::RgbaImage;

#[cfg(windows)]
pub fn copy_image_to_clipboard(image: &RgbaImage) -> Result<(), String> {
    use windows::core::w;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Graphics::Gdi::{
        BI_BITFIELDS, BI_RGB, BITMAPINFOHEADER, BITMAPV5HEADER,
    };
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};

    let width = image.width();
    let height = image.height();
    if width == 0 || height == 0 {
        return Err("Cannot copy empty image".to_string());
    }

    // 1. Prepare PNG encoded buffer for modern applications (Telegram, Discord, Figma, Browsers)
    let mut png_bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png_bytes);
    if let Err(e) = image.write_to(&mut cursor, image::ImageFormat::Png) {
        eprintln!("Warning: Failed to encode PNG for clipboard: {}", e);
    }

    // 2. Prepare CF_DIBV5 buffer with full 32-bit BGRA + Alpha channel
    let v5_header_size = std::mem::size_of::<BITMAPV5HEADER>();
    let row_stride = (width * 4) as usize;
    let data_size = row_stride * (height as usize);
    let v5_total_size = v5_header_size + data_size;

    // 3. Prepare standard CF_DIB buffer for legacy GDI applications
    let v3_header_size = std::mem::size_of::<BITMAPINFOHEADER>();
    let v3_total_size = v3_header_size + data_size;

    unsafe {
        // Allocate PNG global memory
        let hmem_png = if !png_bytes.is_empty() {
            let hmem = GlobalAlloc(GMEM_MOVEABLE, png_bytes.len())
                .map_err(|e| format!("GlobalAlloc for PNG failed: {}", e))?;
            if !hmem.0.is_null() {
                let ptr = GlobalLock(hmem) as *mut u8;
                if !ptr.is_null() {
                    std::ptr::copy_nonoverlapping(png_bytes.as_ptr(), ptr, png_bytes.len());
                    let _ = GlobalUnlock(hmem);
                    Some(hmem)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Allocate CF_DIBV5 global memory
        let hmem_dibv5 = {
            let hmem = GlobalAlloc(GMEM_MOVEABLE, v5_total_size)
                .map_err(|e| format!("GlobalAlloc for CF_DIBV5 failed: {}", e))?;
            if hmem.0.is_null() {
                return Err("GlobalAlloc returned null handle for DIBV5".to_string());
            }
            let ptr = GlobalLock(hmem) as *mut u8;
            if ptr.is_null() {
                return Err("GlobalLock failed for DIBV5".to_string());
            }

            let bmiv5 = BITMAPV5HEADER {
                bV5Size: v5_header_size as u32,
                bV5Width: width as i32,
                bV5Height: height as i32, // Positive for bottom-up DIB
                bV5Planes: 1,
                bV5BitCount: 32,
                bV5Compression: BI_BITFIELDS,
                bV5SizeImage: data_size as u32,
                bV5RedMask: 0x00FF0000,
                bV5GreenMask: 0x0000FF00,
                bV5BlueMask: 0x000000FF,
                bV5AlphaMask: 0xFF000000,
                bV5CSType: 0x73524742, // 'sRGB' (LCS_sRGB)
                bV5Intent: 4,          // LCS_GM_IMAGES
                ..Default::default()
            };

            std::ptr::copy_nonoverlapping(&bmiv5 as *const _ as *const u8, ptr, v5_header_size);

            let dest_pixels = ptr.add(v5_header_size);
            for y in 0..height {
                let src_y = height - 1 - y; // Bottom-up row order
                for x in 0..width {
                    let pixel = image.get_pixel(x, src_y);
                    let dest_idx = ((y * width + x) * 4) as usize;
                    // BGRA order
                    *dest_pixels.add(dest_idx) = pixel[2];     // B
                    *dest_pixels.add(dest_idx + 1) = pixel[1]; // G
                    *dest_pixels.add(dest_idx + 2) = pixel[0]; // R
                    *dest_pixels.add(dest_idx + 3) = pixel[3]; // A
                }
            }

            let _ = GlobalUnlock(hmem);
            hmem
        };

        // Allocate standard CF_DIB global memory
        let hmem_dib = {
            let hmem = GlobalAlloc(GMEM_MOVEABLE, v3_total_size)
                .map_err(|e| format!("GlobalAlloc for CF_DIB failed: {}", e))?;
            if !hmem.0.is_null() {
                let ptr = GlobalLock(hmem) as *mut u8;
                if !ptr.is_null() {
                    let bmi = BITMAPINFOHEADER {
                        biSize: v3_header_size as u32,
                        biWidth: width as i32,
                        biHeight: height as i32,
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        biSizeImage: data_size as u32,
                        ..Default::default()
                    };
                    std::ptr::copy_nonoverlapping(&bmi as *const _ as *const u8, ptr, v3_header_size);

                    let dest_pixels = ptr.add(v3_header_size);
                    for y in 0..height {
                        let src_y = height - 1 - y;
                        for x in 0..width {
                            let pixel = image.get_pixel(x, src_y);
                            let dest_idx = ((y * width + x) * 4) as usize;
                            *dest_pixels.add(dest_idx) = pixel[2];
                            *dest_pixels.add(dest_idx + 1) = pixel[1];
                            *dest_pixels.add(dest_idx + 2) = pixel[0];
                            *dest_pixels.add(dest_idx + 3) = pixel[3];
                        }
                    }
                    let _ = GlobalUnlock(hmem);
                    Some(hmem)
                } else {
                    None
                }
            } else {
                None
            }
        };

        OpenClipboard(None).map_err(|e| format!("OpenClipboard failed: {}", e))?;
        let _ = EmptyClipboard();

        // 1. Register "PNG" format and write
        let png_format_id = RegisterClipboardFormatW(w!("PNG"));
        if png_format_id != 0 {
            if let Some(hpng) = hmem_png {
                let _ = SetClipboardData(png_format_id, Some(HANDLE(hpng.0)));
            }
        }

        // 2. CF_DIBV5 is format ID 17
        const CF_DIBV5: u32 = 17;
        let _ = SetClipboardData(CF_DIBV5, Some(HANDLE(hmem_dibv5.0)));

        // 3. CF_DIB is format ID 8
        const CF_DIB: u32 = 8;
        if let Some(hdib) = hmem_dib {
            let _ = SetClipboardData(CF_DIB, Some(HANDLE(hdib.0)));
        }

        let _ = CloseClipboard();
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
