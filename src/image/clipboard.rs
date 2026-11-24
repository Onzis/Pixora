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
        crate::logging::log_warn("clipboard", format!("Failed to encode PNG for clipboard: {}", e));
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

#[cfg(windows)]
pub fn get_text_from_clipboard() -> Option<String> {
    use windows::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};

    unsafe {
        if OpenClipboard(None).is_err() {
            return None;
        }
        const CF_UNICODETEXT: u32 = 13;
        let handle = match GetClipboardData(CF_UNICODETEXT) {
            Ok(h) if !h.0.is_null() => h,
            _ => {
                let _ = CloseClipboard();
                return None;
            }
        };
        let ptr = GlobalLock(windows::Win32::Foundation::HGLOBAL(handle.0)) as *const u16;
        if ptr.is_null() {
            let _ = CloseClipboard();
            return None;
        }
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        let text = String::from_utf16_lossy(slice);
        let _ = GlobalUnlock(windows::Win32::Foundation::HGLOBAL(handle.0));
        let _ = CloseClipboard();
        Some(text)
    }
}

#[cfg(target_os = "linux")]
pub fn copy_image_to_clipboard(image: &RgbaImage) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut png_bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png_bytes);
    image
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG for clipboard: {}", e))?;

    // 1. Try Wayland wl-copy if running under Wayland
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        if let Ok(mut child) = Command::new("wl-copy")
            .args(["-t", "image/png"])
            .stdin(Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(&png_bytes);
            }
            if let Ok(status) = child.wait() {
                if status.success() {
                    return Ok(());
                }
            }
        }
    }

    // 2. Try X11 xclip
    if let Ok(mut child) = Command::new("xclip")
        .args(["-selection", "clipboard", "-t", "image/png"])
        .stdin(Stdio::piped())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(&png_bytes);
        }
        if let Ok(status) = child.wait() {
            if status.success() {
                return Ok(());
            }
        }
    }

    // 3. Fallback to wl-copy even without WAYLAND_DISPLAY variable
    if let Ok(mut child) = Command::new("wl-copy")
        .args(["-t", "image/png"])
        .stdin(Stdio::piped())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(&png_bytes);
        }
        if let Ok(status) = child.wait() {
            if status.success() {
                return Ok(());
            }
        }
    }

    Err("Failed to copy image to Linux clipboard. Please install 'wl-clipboard' (Wayland) or 'xclip' (X11).".to_string())
}

#[cfg(target_os = "linux")]
pub fn copy_text_to_clipboard(text: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    // 1. Try Wayland wl-copy
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        if let Ok(mut child) = Command::new("wl-copy").stdin(Stdio::piped()).spawn() {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if let Ok(status) = child.wait() {
                if status.success() {
                    return Ok(());
                }
            }
        }
    }

    // 2. Try native X11 clipboard via x11-clipboard crate
    if let Ok(clipboard) = x11_clipboard::Clipboard::new() {
        if clipboard
            .store(
                clipboard.getter.atoms.clipboard,
                clipboard.getter.atoms.utf8_string,
                text.as_bytes(),
            )
            .is_ok()
        {
            return Ok(());
        }
    }

    // 3. Fallback to xclip
    if let Ok(mut child) = Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(Stdio::piped())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        if let Ok(status) = child.wait() {
            if status.success() {
                return Ok(());
            }
        }
    }

    // 4. Fallback to wl-copy
    if let Ok(mut child) = Command::new("wl-copy").stdin(Stdio::piped()).spawn() {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        if let Ok(status) = child.wait() {
            if status.success() {
                return Ok(());
            }
        }
    }

    Err("Failed to copy text to Linux clipboard. Please install 'wl-clipboard' or 'xclip'.".to_string())
}

#[cfg(target_os = "linux")]
pub fn get_text_from_clipboard() -> Option<String> {
    use std::process::Command;

    // 1. Try Wayland wl-paste
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        if let Ok(output) = Command::new("wl-paste").args(["-n"]).output() {
            if output.status.success() && !output.stdout.is_empty() {
                return Some(String::from_utf8_lossy(&output.stdout).to_string());
            }
        }
    }

    // 2. Try native X11 clipboard via x11-clipboard crate
    if let Ok(clipboard) = x11_clipboard::Clipboard::new() {
        if let Ok(bytes) = clipboard.load(
            clipboard.getter.atoms.clipboard,
            clipboard.getter.atoms.utf8_string,
            clipboard.getter.atoms.property,
            std::time::Duration::from_millis(150),
        ) {
            let s = String::from_utf8_lossy(&bytes).to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
    }

    // 3. Fallback to xclip
    if let Ok(output) = Command::new("xclip")
        .args(["-selection", "clipboard", "-o"])
        .output()
    {
        if output.status.success() && !output.stdout.is_empty() {
            return Some(String::from_utf8_lossy(&output.stdout).to_string());
        }
    }

    // 4. Fallback to wl-paste
    if let Ok(output) = Command::new("wl-paste").args(["-n"]).output() {
        if output.status.success() && !output.stdout.is_empty() {
            return Some(String::from_utf8_lossy(&output.stdout).to_string());
        }
    }

    None
}

#[cfg(all(not(windows), not(target_os = "linux")))]
pub fn get_text_from_clipboard() -> Option<String> {
    None
}

#[cfg(all(not(windows), not(target_os = "linux")))]
pub fn copy_image_to_clipboard(_image: &RgbaImage) -> Result<(), String> {
    Err("Clipboard only implemented on Windows and Linux".to_string())
}

#[cfg(all(not(windows), not(target_os = "linux")))]
pub fn copy_text_to_clipboard(_text: &str) -> Result<(), String> {
    Err("Clipboard only implemented on Windows and Linux".to_string())
}
