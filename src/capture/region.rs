use crate::capture::screen::ScreenCapture;
use crate::system::windows::PhysicalRect;
use image::{RgbaImage, imageops};

pub fn crop_region(
    capture: &ScreenCapture,
    region: PhysicalRect,
) -> Option<RgbaImage> {
    if region.width == 0 || region.height == 0 {
        return None;
    }

    // Convert virtual screen coordinates to image-local pixel coordinates
    let local_x = (region.x - capture.bounds.x).max(0) as u32;
    let local_y = (region.y - capture.bounds.y).max(0) as u32;

    if local_x >= capture.bounds.width || local_y >= capture.bounds.height {
        return None;
    }

    let clamped_w = region.width.min(capture.bounds.width - local_x);
    let clamped_h = region.height.min(capture.bounds.height - local_y);

    if clamped_w == 0 || clamped_h == 0 {
        return None;
    }

    let cropped = imageops::crop_imm(&capture.image, local_x, local_y, clamped_w, clamped_h).to_image();
    Some(cropped)
}
