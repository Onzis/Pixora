use image::{RgbaImage, imageops};

pub fn crop_rgba(
    image: &RgbaImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Option<RgbaImage> {
    if width == 0 || height == 0 || x >= image.width() || y >= image.height() {
        return None;
    }
    let actual_w = width.min(image.width() - x);
    let actual_h = height.min(image.height() - y);
    Some(imageops::crop_imm(image, x, y, actual_w, actual_h).to_image())
}
