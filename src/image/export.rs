use crate::config::AppConfig;
use chrono::Local;
use image::{ImageEncoder, RgbaImage, codecs::png::PngEncoder};
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

pub fn generate_timestamped_filename(prefix: &str) -> String {
    let now = Local::now();
    format!("{}_{}.png", prefix, now.format("%Y-%m-%d_%H%M%S"))
}

pub fn generate_pattern_filename(pattern: &str, format_ext: &str) -> String {
    let now = Local::now();
    let formatted = now.format(pattern).to_string();
    format!("{}.{}", formatted, format_ext.trim_start_matches('.'))
}

pub fn save_image_png(image: &RgbaImage, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let file = File::create(path).map_err(|e| format!("Failed to create file: {}", e))?;
    let writer = BufWriter::new(file);

    let encoder = PngEncoder::new(writer);
    encoder
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("PNG encoding failed: {}", e))?;

    Ok(())
}

pub fn save_image_jpeg(image: &RgbaImage, path: &Path, quality: u8) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let file = File::create(path).map_err(|e| format!("Failed to create file: {}", e))?;
    let writer = BufWriter::new(file);

    let dyn_img = image::DynamicImage::ImageRgba8(image.clone());
    let rgb = dyn_img.into_rgb8();

    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(writer, quality.clamp(1, 100));
    encoder
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| format!("JPEG encoding failed: {}", e))?;

    Ok(())
}

pub fn auto_save_screenshot(image: &RgbaImage, config: &AppConfig) -> Result<PathBuf, String> {
    let ext = if config.image_format.eq_ignore_ascii_case("jpeg")
        || config.image_format.eq_ignore_ascii_case("jpg")
    {
        "jpg"
    } else {
        "png"
    };

    let filename = if !config.filename_pattern.trim().is_empty() {
        generate_pattern_filename(&config.filename_pattern, ext)
    } else {
        generate_timestamped_filename(&config.filename_prefix)
    };

    let mut target = config.save_directory.join(&filename);

    let mut counter = 1;
    let base_stem = target
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "screenshot".to_string());

    while target.exists() {
        let name_with_counter = format!("{}_{}.{}", base_stem, counter, ext);
        target = config.save_directory.join(name_with_counter);
        counter += 1;
    }

    if ext == "jpg" {
        save_image_jpeg(image, &target, config.jpeg_quality)?;
    } else {
        save_image_png(image, &target)?;
    }

    Ok(target)
}
