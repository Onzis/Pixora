use crate::capture::region::crop_region;
use crate::capture::screen::{ScreenCapture, capture_virtual_screen};
use crate::system::windows::{get_active_window_rect, get_window_under_cursor_rect};
use image::RgbaImage;

pub fn capture_active_window() -> Result<(RgbaImage, ScreenCapture), String> {
    let screen = capture_virtual_screen()?;
    let rect = get_active_window_rect().ok_or_else(|| "No active window found".to_string())?;
    let img = crop_region(&screen, rect).ok_or_else(|| "Failed to crop active window".to_string())?;
    Ok((img, screen))
}

pub fn capture_window_under_cursor() -> Result<(RgbaImage, ScreenCapture), String> {
    let screen = capture_virtual_screen()?;
    let rect = get_window_under_cursor_rect()
        .ok_or_else(|| "No window found under cursor".to_string())?;
    let img = crop_region(&screen, rect)
        .ok_or_else(|| "Failed to crop window under cursor".to_string())?;
    Ok((img, screen))
}
