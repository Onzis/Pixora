use image::{Rgba, RgbaImage};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeautifyPreset {
    Sunset,      // Coral orange to magenta
    Ocean,       // Deep ocean teal to cyan
    Cosmic,      // Neon purple to violet
    StudioDark,  // Sleek graphite to deep slate
    Emerald,     // Lush mint to deep emerald
    Aurora,      // Northern lights cyan to lavender
    Transparent, // Pure shadow with alpha channel
}

impl BeautifyPreset {
    pub fn all() -> &'static [BeautifyPreset] {
        &[
            BeautifyPreset::Sunset,
            BeautifyPreset::Ocean,
            BeautifyPreset::Cosmic,
            BeautifyPreset::StudioDark,
            BeautifyPreset::Emerald,
            BeautifyPreset::Aurora,
            BeautifyPreset::Transparent,
        ]
    }

    pub fn display_name(&self, is_ru: bool) -> &'static str {
        match self {
            BeautifyPreset::Sunset => if is_ru { "Закат" } else { "Sunset" },
            BeautifyPreset::Ocean => if is_ru { "Океан" } else { "Ocean" },
            BeautifyPreset::Cosmic => if is_ru { "Космос" } else { "Cosmic" },
            BeautifyPreset::StudioDark => if is_ru { "Студийный" } else { "Studio Dark" },
            BeautifyPreset::Emerald => if is_ru { "Изумруд" } else { "Emerald" },
            BeautifyPreset::Aurora => if is_ru { "Аврора" } else { "Aurora" },
            BeautifyPreset::Transparent => if is_ru { "Прозрачный" } else { "Transparent" },
        }
    }

    pub fn gradient_colors(&self) -> (Rgba<u8>, Rgba<u8>) {
        match self {
            BeautifyPreset::Sunset => (
                Rgba([255, 94, 98, 255]),   // Warm coral
                Rgba([138, 35, 135, 255]),  // Deep magenta-purple
            ),
            BeautifyPreset::Ocean => (
                Rgba([33, 147, 176, 255]),  // Steel cyan
                Rgba([109, 213, 237, 255]), // Bright lagoon
            ),
            BeautifyPreset::Cosmic => (
                Rgba([142, 45, 226, 255]),  // Electric purple
                Rgba([74, 0, 224, 255]),    // Indigo violet
            ),
            BeautifyPreset::StudioDark => (
                Rgba([36, 40, 50, 255]),    // Graphite
                Rgba([18, 20, 26, 255]),    // Charcoal black
            ),
            BeautifyPreset::Emerald => (
                Rgba([16, 185, 129, 255]),  // Lush mint
                Rgba([5, 150, 105, 255]),   // Deep emerald
            ),
            BeautifyPreset::Aurora => (
                Rgba([0, 198, 255, 255]),   // Cyan
                Rgba([155, 89, 182, 255]),  // Lavender
            ),
            BeautifyPreset::Transparent => (
                Rgba([0, 0, 0, 0]),
                Rgba([0, 0, 0, 0]),
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeautifyOptions {
    pub preset: BeautifyPreset,
    pub padding: u32,
    pub corner_radius: f32,
    pub shadow_blur: f32,
    pub shadow_offset_y: f32,
    pub shadow_opacity: f32,
}

impl Default for BeautifyOptions {
    fn default() -> Self {
        Self {
            preset: BeautifyPreset::Sunset,
            padding: 44,
            corner_radius: 14.0,
            shadow_blur: 24.0,
            shadow_offset_y: 10.0,
            shadow_opacity: 0.40,
        }
    }
}

/// Computes the signed distance from point (x, y) to a rounded rectangle
/// centered at (cx, cy) with half-extents (hx, hy) and corner radius r.
#[inline(always)]
fn sdf_rounded_box(x: f32, y: f32, cx: f32, cy: f32, hx: f32, hy: f32, r: f32) -> f32 {
    let px = (x - cx).abs();
    let py = (y - cy).abs();
    let qx = px - (hx - r);
    let qy = py - (hy - r);
    let ox = qx.max(0.0);
    let oy = qy.max(0.0);
    let outside = (ox * ox + oy * oy).sqrt();
    let inside = qx.max(qy).min(0.0);
    outside + inside - r
}

/// Blends source RGBA over destination RGBA in place
#[inline(always)]
fn blend_over(dst: &mut Rgba<u8>, src: Rgba<u8>) {
    let src_a = src[3] as f32 / 255.0;
    if src_a <= 0.0 {
        return;
    }
    if src_a >= 1.0 {
        *dst = src;
        return;
    }

    let dst_a = dst[3] as f32 / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a <= 0.0 {
        *dst = Rgba([0, 0, 0, 0]);
        return;
    }

    let r = (src[0] as f32 * src_a + dst[0] as f32 * dst_a * (1.0 - src_a)) / out_a;
    let g = (src[1] as f32 * src_a + dst[1] as f32 * dst_a * (1.0 - src_a)) / out_a;
    let b = (src[2] as f32 * src_a + dst[2] as f32 * dst_a * (1.0 - src_a)) / out_a;

    *dst = Rgba([
        r.round().clamp(0.0, 255.0) as u8,
        g.round().clamp(0.0, 255.0) as u8,
        b.round().clamp(0.0, 255.0) as u8,
        (out_a * 255.0).round().clamp(0.0, 255.0) as u8,
    ]);
}

/// Creates a beautified screenshot with decorative background, rounded corners,
/// and smooth anti-aliased drop shadow.
pub fn apply_beautify(image: &RgbaImage, options: &BeautifyOptions) -> RgbaImage {
    let src_w = image.width();
    let src_h = image.height();
    if src_w == 0 || src_h == 0 {
        return image.clone();
    }

    let pad = options.padding;
    let out_w = src_w + pad * 2;
    let out_h = src_h + pad * 2;
    let mut out = RgbaImage::new(out_w, out_h);

    let (c1, c2) = options.preset.gradient_colors();
    let is_transparent = options.preset == BeautifyPreset::Transparent;

    // Center coordinates for screenshot
    let inner_cx = pad as f32 + (src_w as f32) / 2.0;
    let inner_cy = pad as f32 + (src_h as f32) / 2.0;
    let inner_hx = (src_w as f32) / 2.0;
    let inner_hy = (src_h as f32) / 2.0;
    let r = options.corner_radius.clamp(0.0, inner_hx.min(inner_hy));

    // Shadow center and properties
    let shadow_cx = inner_cx;
    let shadow_cy = inner_cy + options.shadow_offset_y;
    let shadow_blur = options.shadow_blur.max(1.0);
    let shadow_op = options.shadow_opacity.clamp(0.0, 1.0);

    // Step 1: Render background and shadow
    for y in 0..out_h {
        let yf = y as f32;
        let ty = yf / (out_h as f32).max(1.0);

        for x in 0..out_w {
            let xf = x as f32;
            let tx = xf / (out_w as f32).max(1.0);

            // Compute background color
            let mut bg_pixel = if is_transparent {
                Rgba([0, 0, 0, 0])
            } else {
                let t = (tx * 0.6 + ty * 0.4).clamp(0.0, 1.0);
                let r_col = ((1.0 - t) * c1[0] as f32 + t * c2[0] as f32).round() as u8;
                let g_col = ((1.0 - t) * c1[1] as f32 + t * c2[1] as f32).round() as u8;
                let b_col = ((1.0 - t) * c1[2] as f32 + t * c2[2] as f32).round() as u8;
                Rgba([r_col, g_col, b_col, 255])
            };

            // Compute smooth drop shadow
            let s_dist = sdf_rounded_box(xf, yf, shadow_cx, shadow_cy, inner_hx, inner_hy, r);
            if s_dist < shadow_blur * 2.0 {
                let shadow_factor = if s_dist <= 0.0 {
                    1.0
                } else {
                    let d = s_dist / shadow_blur;
                    (-0.5 * d * d).exp()
                };
                let s_alpha = (shadow_op * shadow_factor * 255.0).round().clamp(0.0, 255.0) as u8;
                if s_alpha > 0 {
                    let shadow_pixel = Rgba([0, 0, 0, s_alpha]);
                    blend_over(&mut bg_pixel, shadow_pixel);
                }
            }

            out.put_pixel(x, y, bg_pixel);
        }
    }

    // Step 2: Overlay screenshot with rounded corners and anti-aliasing
    for sy in 0..src_h {
        let dy = pad + sy;
        let yf = dy as f32;

        for sx in 0..src_w {
            let dx = pad + sx;
            let xf = dx as f32;

            let dist = sdf_rounded_box(xf, yf, inner_cx, inner_cy, inner_hx, inner_hy, r);
            if dist > 0.5 {
                // Outside rounded corners
                continue;
            }

            let mut src_pixel = *image.get_pixel(sx, sy);
            if dist > -0.5 {
                // Anti-aliased corner edge
                let coverage = (0.5 - dist).clamp(0.0, 1.0);
                let current_a = src_pixel[3] as f32 / 255.0;
                src_pixel[3] = (current_a * coverage * 255.0).round() as u8;
            }

            let dst_pixel = out.get_pixel_mut(dx, dy);
            blend_over(dst_pixel, src_pixel);
        }
    }

    out
}
