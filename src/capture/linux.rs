use crate::capture::screen::ScreenCapture;
use crate::system::windows::PhysicalRect;
use image::RgbaImage;
use std::process::Command;

pub fn capture_virtual_screen_linux() -> Result<ScreenCapture, String> {
    // 1. If Wayland is active, try Wayland capture tools first
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        if let Ok(cap) = capture_via_grim() {
            crate::logging::log_info("capture", "Screen captured successfully via grim (Wayland)");
            return Ok(cap);
        }
        if let Ok(cap) = capture_via_gnome_screenshot() {
            crate::logging::log_info("capture", "Screen captured successfully via gnome-screenshot");
            return Ok(cap);
        }
        if let Ok(cap) = capture_via_spectacle() {
            crate::logging::log_info("capture", "Screen captured successfully via spectacle");
            return Ok(cap);
        }
    }

    // 2. Direct X11 root window capture via pure Rust x11rb (ultra fast, 0ms)
    match capture_via_x11rb() {
        Ok(cap) => {
            crate::logging::log_info("capture", "Screen captured successfully via X11 root window (x11rb)");
            return Ok(cap);
        }
        Err(err) => {
            crate::logging::log_warn("capture", format!("X11 direct capture unavailable: {}", err));
        }
    }

    // 3. Fallback to CLI screenshot utilities (scrot, maim, import, xwd)
    if let Ok(cap) = capture_via_scrot() {
        crate::logging::log_info("capture", "Screen captured successfully via scrot");
        return Ok(cap);
    }
    if let Ok(cap) = capture_via_maim() {
        crate::logging::log_info("capture", "Screen captured successfully via maim");
        return Ok(cap);
    }
    if let Ok(cap) = capture_via_import() {
        crate::logging::log_info("capture", "Screen captured successfully via ImageMagick import");
        return Ok(cap);
    }

    Err(
        "Linux screen capture failed. Ensure an X11/Wayland display server is running, or install 'grim' (Wayland) / 'scrot' (X11)."
            .to_string(),
    )
}

/// Pure Rust root window capture on X11 using x11rb
fn capture_via_x11rb() -> Result<ScreenCapture, String> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};

    let (conn, screen_num) = x11rb::connect(None).map_err(|e| format!("X11 connect failed: {}", e))?;
    let screen = conn
        .setup()
        .roots
        .get(screen_num)
        .ok_or_else(|| "Failed to get default X11 root screen".to_string())?;

    let root = screen.root;
    let width = screen.width_in_pixels as u32;
    let height = screen.height_in_pixels as u32;

    if width == 0 || height == 0 {
        return Err("X11 screen reported zero dimensions".to_string());
    }

    let img_reply = conn
        .get_image(ImageFormat::Z_PIXMAP, root, 0, 0, width as u16, height as u16, !0)
        .map_err(|e| format!("X11 get_image request failed: {}", e))?
        .reply()
        .map_err(|e| format!("X11 get_image reply failed: {}", e))?;

    let depth = img_reply.depth;
    let data = &img_reply.data;
    let mut rgba_buffer = vec![0u8; (width * height * 4) as usize];

    if depth == 24 || depth == 32 {
        let pixel_count = (width * height) as usize;
        let stride = 4;
        if data.len() < pixel_count * stride {
            return Err("X11 image data buffer too small for specified dimensions".to_string());
        }

        for i in 0..pixel_count {
            let src_idx = i * stride;
            let dst_idx = i * 4;
            // X11 ZPixmap for 24/32-bit depth is standard BGRA/BGRX
            let b = data[src_idx];
            let g = data[src_idx + 1];
            let r = data[src_idx + 2];
            rgba_buffer[dst_idx] = r;
            rgba_buffer[dst_idx + 1] = g;
            rgba_buffer[dst_idx + 2] = b;
            rgba_buffer[dst_idx + 3] = 255;
        }
    } else {
        return Err(format!("Unsupported X11 color depth: {}", depth));
    }

    let image = RgbaImage::from_raw(width, height, rgba_buffer)
        .ok_or_else(|| "Failed to construct RgbaImage from X11 pixel buffer".to_string())?;

    Ok(ScreenCapture {
        image,
        bounds: PhysicalRect {
            x: 0,
            y: 0,
            width,
            height,
        },
    })
}

/// Wayland capture via grim (standard on wlroots, Sway, Hyprland)
fn capture_via_grim() -> Result<ScreenCapture, String> {
    let output = Command::new("grim")
        .args(["-t", "png", "-"])
        .output()
        .map_err(|e| format!("Failed to spawn grim: {}", e))?;

    if !output.status.success() || output.stdout.is_empty() {
        return Err("grim exited with non-zero status or empty output".to_string());
    }

    let dyn_img = image::load_from_memory(&output.stdout)
        .map_err(|e| format!("Failed to decode grim PNG output: {}", e))?;
    let image = dyn_img.to_rgba8();
    let width = image.width();
    let height = image.height();

    Ok(ScreenCapture {
        image,
        bounds: PhysicalRect {
            x: 0,
            y: 0,
            width,
            height,
        },
    })
}

/// GNOME Wayland/X11 screenshot via gnome-screenshot CLI
fn capture_via_gnome_screenshot() -> Result<ScreenCapture, String> {
    let tmp_path = std::env::temp_dir().join(format!("pixora_cap_{}.png", std::process::id()));
    let status = Command::new("gnome-screenshot")
        .args(["-f", &tmp_path.to_string_lossy()])
        .status()
        .map_err(|e| format!("Failed to spawn gnome-screenshot: {}", e))?;

    if !status.success() || !tmp_path.exists() {
        return Err("gnome-screenshot command failed".to_string());
    }

    let bytes = std::fs::read(&tmp_path).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&tmp_path);

    let dyn_img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    let image = dyn_img.to_rgba8();
    let width = image.width();
    let height = image.height();

    Ok(ScreenCapture {
        image,
        bounds: PhysicalRect {
            x: 0,
            y: 0,
            width,
            height,
        },
    })
}

/// KDE Plasma screenshot via spectacle CLI
fn capture_via_spectacle() -> Result<ScreenCapture, String> {
    let tmp_path = std::env::temp_dir().join(format!("pixora_cap_{}.png", std::process::id()));
    let status = Command::new("spectacle")
        .args(["-b", "-n", "-o", &tmp_path.to_string_lossy()])
        .status()
        .map_err(|e| format!("Failed to spawn spectacle: {}", e))?;

    if !status.success() || !tmp_path.exists() {
        return Err("spectacle command failed".to_string());
    }

    let bytes = std::fs::read(&tmp_path).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&tmp_path);

    let dyn_img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    let image = dyn_img.to_rgba8();
    let width = image.width();
    let height = image.height();

    Ok(ScreenCapture {
        image,
        bounds: PhysicalRect {
            x: 0,
            y: 0,
            width,
            height,
        },
    })
}

/// X11 capture via scrot CLI
fn capture_via_scrot() -> Result<ScreenCapture, String> {
    let tmp_path = std::env::temp_dir().join(format!("pixora_cap_{}.png", std::process::id()));
    let status = Command::new("scrot")
        .args(["--overwrite", &tmp_path.to_string_lossy()])
        .status()
        .map_err(|e| format!("Failed to spawn scrot: {}", e))?;

    if !status.success() || !tmp_path.exists() {
        return Err("scrot command failed".to_string());
    }

    let bytes = std::fs::read(&tmp_path).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&tmp_path);

    let dyn_img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    let image = dyn_img.to_rgba8();
    let width = image.width();
    let height = image.height();

    Ok(ScreenCapture {
        image,
        bounds: PhysicalRect {
            x: 0,
            y: 0,
            width,
            height,
        },
    })
}

/// X11 capture via maim CLI
fn capture_via_maim() -> Result<ScreenCapture, String> {
    let output = Command::new("maim")
        .args(["-u"])
        .output()
        .map_err(|e| format!("Failed to spawn maim: {}", e))?;

    if !output.status.success() || output.stdout.is_empty() {
        return Err("maim command failed".to_string());
    }

    let dyn_img = image::load_from_memory(&output.stdout).map_err(|e| e.to_string())?;
    let image = dyn_img.to_rgba8();
    let width = image.width();
    let height = image.height();

    Ok(ScreenCapture {
        image,
        bounds: PhysicalRect {
            x: 0,
            y: 0,
            width,
            height,
        },
    })
}

/// X11 capture via ImageMagick import CLI
fn capture_via_import() -> Result<ScreenCapture, String> {
    let tmp_path = std::env::temp_dir().join(format!("pixora_cap_{}.png", std::process::id()));
    let status = Command::new("import")
        .args(["-window", "root", &tmp_path.to_string_lossy()])
        .status()
        .map_err(|e| format!("Failed to spawn import: {}", e))?;

    if !status.success() || !tmp_path.exists() {
        return Err("import command failed".to_string());
    }

    let bytes = std::fs::read(&tmp_path).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&tmp_path);

    let dyn_img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    let image = dyn_img.to_rgba8();
    let width = image.width();
    let height = image.height();

    Ok(ScreenCapture {
        image,
        bounds: PhysicalRect {
            x: 0,
            y: 0,
            width,
            height,
        },
    })
}
