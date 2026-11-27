use std::sync::Arc;

use gpui::{RenderImage, Rgba, rgb, rgba};
use image::{Frame, RgbaImage};
use smallvec::SmallVec;

const BUST_BYTES: &[u8] = include_bytes!("../assets/bust.jpg");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BustDitherMode {
    Bayer8x8,
    Halftone,
}

impl BustDitherMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bayer8x8 => "BAYER 8x8",
            Self::Halftone => "HALFTONE",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PixoraTheme {
    pub bg_dark: Rgba,
    pub bg_surface: Rgba,
    pub bg_secondary: Rgba,
    pub bg_hover: Rgba,
    pub bg_active: Rgba,

    pub border_subtle: Rgba,
    pub border_strong: Rgba,
    pub border_focus: Rgba,

    pub accent: Rgba,
    pub accent_hover: Rgba,
    pub accent_muted: Rgba,

    pub text_primary: Rgba,
    pub text_secondary: Rgba,
    pub text_muted: Rgba,
    pub text_on_accent: Rgba,

    pub sage: Rgba,
    pub amber: Rgba,
    pub crimson: Rgba,

    pub mask_overlay: Rgba,
    pub selection_border: Rgba,
    pub guide_line: Rgba,

    pub palette_red: Rgba,
    pub palette_orange: Rgba,
    pub palette_yellow: Rgba,
    pub palette_green: Rgba,
    pub palette_cyan: Rgba,
    pub palette_blue: Rgba,
    pub palette_purple: Rgba,
    pub palette_white: Rgba,
    pub palette_black: Rgba,
}

impl Default for PixoraTheme {
    fn default() -> Self {
        Self {
            bg_dark: rgb(0x0A0A0B),
            bg_surface: rgb(0x101012),
            bg_secondary: rgb(0x161618),
            bg_hover: rgb(0x1E1D1B),
            bg_active: rgb(0x262522),

            border_subtle: rgb(0x262522),
            border_strong: rgb(0x4A4740),
            border_focus: rgb(0xDDD7CA),

            accent: rgb(0xDDD7CA),
            accent_hover: rgb(0xEDE8DC),
            accent_muted: rgba(0xDDD7CA26),

            text_primary: rgb(0xDDD7CA),
            text_secondary: rgb(0x8E8A82),
            text_muted: rgb(0x5C5953),
            text_on_accent: rgb(0x0A0A0B),

            sage: rgb(0x9BB08A),
            amber: rgb(0xC79A5B),
            crimson: rgb(0xB85D53),

            mask_overlay: rgba(0x00000099),
            selection_border: rgb(0xDDD7CA),
            guide_line: rgba(0xDDD7CA66),

            palette_red: rgb(0xFF453A),
            palette_orange: rgb(0xFF9F0A),
            palette_yellow: rgb(0xFFD60A),
            palette_green: rgb(0x30D158),
            palette_cyan: rgb(0x64D2FF),
            palette_blue: rgb(0x0A84FF),
            palette_purple: rgb(0xBF5AF2),
            palette_white: rgb(0xFFFFFF),
            palette_black: rgb(0x000000),
        }
    }
}

const BAYER_8X8: [u8; 64] = [
    0, 32, 8, 40, 2, 34, 10, 42, 48, 16, 56, 24, 50, 18, 58, 26, 12, 44, 4, 36, 14, 46, 6, 38,
    60, 28, 52, 20, 62, 30, 54, 22, 3, 35, 11, 43, 1, 33, 9, 41, 51, 19, 59, 27, 49, 17, 57, 25,
    15, 47, 7, 39, 13, 45, 5, 37, 63, 31, 55, 23, 61, 29, 53, 21,
];

pub fn generate_dithered_bust(
    mode: BustDitherMode,
    dither_scale: u32,
    out_size: u32,
) -> Option<Arc<RenderImage>> {
    generate_dithered_bust_animated(mode, dither_scale, out_size, 1)
}

pub fn generate_dithered_bust_animated(
    mode: BustDitherMode,
    dither_scale: u32,
    out_size: u32,
    frame_count: usize,
) -> Option<Arc<RenderImage>> {
    let decoded = image::load_from_memory(BUST_BYTES).ok()?.to_luma8();
    let resized = image::imageops::resize(
        &decoded,
        out_size,
        out_size,
        image::imageops::FilterType::Triangle,
    );

    let frames_n = frame_count.max(1);
    let scale = dither_scale.max(1);
    let inv_size = 1.0 / out_size.max(1) as f32;
    let total_px = (out_size * out_size) as usize;

    // Precompute spatial invariants once across all frames
    let mut base_lum = vec![0.0f32; total_px];
    for y in 0..out_size {
        let v = y as f32 * inv_size;
        let bottom_fade = smoothstep(1.0, 0.80, v);
        let row = (y * out_size) as usize;
        for x in 0..out_size {
            let u = x as f32 * inv_size;
            let edge_fade = smoothstep(0.0, 0.05, u) * smoothstep(1.0, 0.95, u);
            let raw_lum = resized.get_pixel(x, y).0[0] as f32 / 255.0;
            base_lum[row + x as usize] =
                (raw_lum * bottom_fade * edge_fade).powf(1.12).clamp(0.0, 1.0);
        }
    }

    // Warm bone #DDD7CA -> (221, 215, 202) on void #0A0A0B -> (10, 10, 11)
    const BONE_R: f32 = 221.0;
    const BONE_G: f32 = 215.0;
    const BONE_B: f32 = 202.0;
    const VOID_R: f32 = 10.0;
    const VOID_G: f32 = 10.0;
    const VOID_B: f32 = 11.0;

    let mut frames = Vec::with_capacity(frames_n);

    for f in 0..frames_n {
        let phase = if frames_n > 1 {
            f as f32 / frames_n as f32
        } else {
            0.0
        };
        let tau = std::f32::consts::TAU * phase;
        let pulse = 0.5 + 0.5 * tau.sin();
        let lum_lift = if frames_n > 1 {
            0.92 + 0.14 * pulse
        } else {
            1.0
        };
        let frame_seed = (f as u32).wrapping_mul(747796405).wrapping_add(2891336453);

        let mut buf = vec![0u8; total_px * 4];

        for y in 0..out_size {
            let row = (y * out_size) as usize;
            let row_offset = row * 4;
            let scan = 0.96 + 0.04 * (y as f32 * 1.5).sin();

            for x in 0..out_size {
                let idx = row + x as usize;

                // Subtle organic dither grain shimmer across marble midtones
                let h = (x.wrapping_mul(374761393))
                    ^ (y.wrapping_mul(668265263))
                    ^ frame_seed;
                let h = (h ^ (h >> 13)).wrapping_mul(1274126177);
                let grain = (((h >> 16) & 0xFF) as f32 / 255.0 - 0.5) * 0.042;

                let lum = (base_lum[idx] * lum_lift).clamp(0.0, 1.0);
                let mask = smoothstep(0.055, 0.12, lum);

                let sx = x / scale;
                let sy = y / scale;

                let bit = match mode {
                    BustDitherMode::Bayer8x8 => {
                        let b_idx = ((sy & 7) * 8 + (sx & 7)) as usize;
                        let threshold = (BAYER_8X8[b_idx] as f32 + 0.5) / 64.0;
                        if lum + grain * 0.85 >= threshold {
                            mask
                        } else {
                            0.0
                        }
                    }
                    BustDitherMode::Halftone => {
                        let cell_x = ((x as f32 / scale as f32) / 3.0).fract() - 0.5;
                        let cell_y = ((y as f32 / scale as f32) / 3.0).fract() - 0.5;
                        let d = (cell_x * cell_x + cell_y * cell_y).sqrt() * std::f32::consts::SQRT_2;
                        let radius = lum.sqrt() * 0.88;
                        (1.0 - smoothstep(radius - 0.08, radius + 0.08, d)) * mask
                    }
                };

                let r = (VOID_R + (BONE_R * scan - VOID_R) * bit)
                    .clamp(0.0, 255.0) as u8;
                let g = (VOID_G + (BONE_G * scan - VOID_G) * bit)
                    .clamp(0.0, 255.0) as u8;
                let b = (VOID_B + (BONE_B * scan - VOID_B) * bit)
                    .clamp(0.0, 255.0) as u8;

                // RenderImage expects BGRA
                let px_idx = row_offset + x as usize * 4;
                buf[px_idx] = b;
                buf[px_idx + 1] = g;
                buf[px_idx + 2] = r;
                buf[px_idx + 3] = 255;
            }
        }

        let img = RgbaImage::from_raw(out_size, out_size, buf)?;
        frames.push(Frame::new(img));
    }

    Some(Arc::new(RenderImage::new(SmallVec::from_vec(frames))))
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() < f32::EPSILON {
        return 0.0;
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
