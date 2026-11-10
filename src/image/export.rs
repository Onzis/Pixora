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

pub fn auto_save_screenshot(image: &RgbaImage, config: &AppConfig) -> Result<PathBuf, String> {
    let filename = generate_timestamped_filename(&config.filename_prefix);
    let mut target = config.save_directory.join(&filename);

    let mut counter = 1;
    while target.exists() {
        let name_with_counter = format!(
            "{}_{}_{}.png",
            config.filename_prefix,
            Local::now().format("%Y-%m-%d_%H%M%S"),
            counter
        );
        target = config.save_directory.join(name_with_counter);
        counter += 1;
    }

    save_image_png(image, &target)?;
    Ok(target)
}
