use gpui::{Rgba, rgb};

#[derive(Clone, Debug)]
pub struct ToolProperties {
    pub stroke_color: Rgba,
    pub stroke_width: f32,
    pub font_size: f32,
    pub blur_radius: u32,
    pub pixelate_block: u32,
    pub is_filled: bool,
    pub current_badge_number: u32,
}

impl Default for ToolProperties {
    fn default() -> Self {
        Self {
            stroke_color: rgb(0xFF453A),
            stroke_width: 3.5,
            font_size: 18.0,
            blur_radius: 12,
            pixelate_block: 10,
            is_filled: false,
            current_badge_number: 1,
        }
    }
}
