use gpui::{Rgba, rgb};
use crate::editor::annotations::{BadgeMode, BadgeShape, StickerKind};

#[derive(Clone, Debug)]
pub struct ToolProperties {
    pub stroke_color: Rgba,
    pub stroke_width: f32,
    pub font_size: f32,
    pub blur_radius: u32,
    pub pixelate_block: u32,
    pub is_filled: bool,
    pub current_badge_number: u32,
    pub spotlight_dim: f32,
    pub arrow_two_headed: bool,
    pub arrow_curved: bool,
    pub badge_shape: BadgeShape,
    pub badge_mode: BadgeMode,
    pub badge_filled: bool,
    pub sticker_kind: StickerKind,
    pub sticker_size: f32,
}

impl Default for ToolProperties {
    fn default() -> Self {
        Self {
            stroke_color: rgb(0xFF453A),
            stroke_width: 6.0,
            font_size: 18.0,
            blur_radius: 12,
            pixelate_block: 10,
            is_filled: false,
            current_badge_number: 1,
            spotlight_dim: 0.60,
            arrow_two_headed: false,
            arrow_curved: false,
            badge_shape: BadgeShape::Circle,
            badge_mode: BadgeMode::Numbers,
            badge_filled: true,
            sticker_kind: StickerKind::Checkmark,
            sticker_size: 40.0,
        }
    }
}
