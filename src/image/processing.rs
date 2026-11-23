use image::{Rgba, RgbaImage};
use crate::editor::properties::SpotlightShape;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpotlightRegion {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub shape: SpotlightShape,
    pub border_color: gpui::Rgba,
    pub border_width: f32,
    pub dim_opacity: f32,
}

pub fn apply_spotlights(
    image: &mut RgbaImage,
    target_x: u32,
    target_y: u32,
    target_w: u32,
    target_h: u32,
    spotlights: &[SpotlightRegion],
    dim_opacity: f32,
    corner_radius: f32,
) {
    if spotlights.is_empty() || target_w == 0 || target_h == 0 {
        return;
    }

    let factor = (1.0 - dim_opacity.clamp(0.0, 1.0)).max(0.0);
    let max_x = (target_x + target_w).min(image.width());
    let max_y = (target_y + target_h).min(image.height());

    // 1. Dim all pixels in target area that are NOT inside any spotlight cutout
    for y in target_y..max_y {
        for x in target_x..max_x {
            let px = x as f32;
            let py = y as f32;

            let mut inside_any = false;
            for spot in spotlights {
                let min_x = spot.x;
                let min_y = spot.y;
                let max_spot_x = spot.x + spot.width;
                let max_spot_y = spot.y + spot.height;

                match spot.shape {
                    SpotlightShape::Rectangle => {
                        if px >= min_x && px <= max_spot_x && py >= min_y && py <= max_spot_y {
                            let cr = corner_radius.min(spot.width / 2.0).min(spot.height / 2.0).max(0.0);
                            if cr > 0.0 {
                                let in_top_left = px < min_x + cr && py < min_y + cr;
                                let in_top_right = px > max_spot_x - cr && py < min_y + cr;
                                let in_bottom_left = px < min_x + cr && py > max_spot_y - cr;
                                let in_bottom_right = px > max_spot_x - cr && py > max_spot_y - cr;

                                if in_top_left {
                                    let dx = px - (min_x + cr);
                                    let dy = py - (min_y + cr);
                                    if dx * dx + dy * dy <= cr * cr {
                                        inside_any = true;
                                        break;
                                    }
                                } else if in_top_right {
                                    let dx = px - (max_spot_x - cr);
                                    let dy = py - (min_y + cr);
                                    if dx * dx + dy * dy <= cr * cr {
                                        inside_any = true;
                                        break;
                                    }
                                } else if in_bottom_left {
                                    let dx = px - (min_x + cr);
                                    let dy = py - (max_spot_y - cr);
                                    if dx * dx + dy * dy <= cr * cr {
                                        inside_any = true;
                                        break;
                                    }
                                } else if in_bottom_right {
                                    let dx = px - (max_spot_x - cr);
                                    let dy = py - (max_spot_y - cr);
                                    if dx * dx + dy * dy <= cr * cr {
                                        inside_any = true;
                                        break;
                                    }
                                } else {
                                    inside_any = true;
                                    break;
                                }
                            } else {
                                inside_any = true;
                                break;
                            }
                        }
                    }
                    SpotlightShape::Ellipse => {
                        let cx = (min_x + max_spot_x) / 2.0;
                        let cy = (min_y + max_spot_y) / 2.0;
                        let rx = (max_spot_x - min_x) / 2.0;
                        let ry = (max_spot_y - min_y) / 2.0;
                        if rx > 0.0 && ry > 0.0 {
                            let norm_x = (px - cx) / rx;
                            let norm_y = (py - cy) / ry;
                            if norm_x * norm_x + norm_y * norm_y <= 1.0 {
                                inside_any = true;
                                break;
                            }
                        }
                    }
                }
            }

            if !inside_any {
                let pixel = image.get_pixel_mut(x, y);
                pixel[0] = ((pixel[0] as f32) * factor).round() as u8;
                pixel[1] = ((pixel[1] as f32) * factor).round() as u8;
                pixel[2] = ((pixel[2] as f32) * factor).round() as u8;
            }
        }
    }

    // 2. Draw crisp border around each spotlight region
    for spot in spotlights {
        if spot.border_width > 0.0 {
            match spot.shape {
                SpotlightShape::Rectangle => {
                    crate::editor::canvas::draw_rounded_rectangle(
                        image,
                        spot.x,
                        spot.y,
                        spot.width,
                        spot.height,
                        corner_radius,
                        spot.border_width,
                        spot.border_color,
                    );
                }
                SpotlightShape::Ellipse => {
                    crate::editor::canvas::draw_ellipse(
                        image,
                        spot.x,
                        spot.y,
                        spot.width,
                        spot.height,
                        spot.border_width,
                        spot.border_color,
                        false,
                    );
                }
            }
        }
    }
}

pub fn apply_pixelate(
    image: &mut RgbaImage,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    block_size: u32,
) {
    let block = block_size.max(2);
    let max_x = (x + w).min(image.width());
    let max_y = (y + h).min(image.height());

    let mut by = y;
    while by < max_y {
        let block_h = block.min(max_y - by);
        let mut bx = x;
        while bx < max_x {
            let block_w = block.min(max_x - bx);

            // Compute average color in block
            let mut r_acc: u64 = 0;
            let mut g_acc: u64 = 0;
            let mut b_acc: u64 = 0;
            let mut a_acc: u64 = 0;
            let count = (block_w * block_h) as u64;

            for dy in 0..block_h {
                for dx in 0..block_w {
                    let p = image.get_pixel(bx + dx, by + dy);
                    r_acc += p[0] as u64;
                    g_acc += p[1] as u64;
                    b_acc += p[2] as u64;
                    a_acc += p[3] as u64;
                }
            }

            let avg_p = Rgba([
                (r_acc / count) as u8,
                (g_acc / count) as u8,
                (b_acc / count) as u8,
                (a_acc / count) as u8,
            ]);

            for dy in 0..block_h {
                for dx in 0..block_w {
                    image.put_pixel(bx + dx, by + dy, avg_p);
                }
            }

            bx += block;
        }
        by += block;
    }
}

pub fn apply_blur(
    image: &mut RgbaImage,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    radius: u32,
) {
    let r = radius.clamp(1, 40) as i32;
    let max_x = (x + w).min(image.width());
    let max_y = (y + h).min(image.height());

    if max_x <= x || max_y <= y {
        return;
    }

    let sub_w = (max_x - x) as usize;
    let sub_h = (max_y - y) as usize;

    // Fast 2-pass separable box blur (simulating Gaussian blur with 3 iterations)
    let mut temp = vec![[0u8; 4]; sub_w * sub_h];
    let mut out = vec![[0u8; 4]; sub_w * sub_h];

    for sy in 0..sub_h {
        for sx in 0..sub_w {
            let p = image.get_pixel(x + sx as u32, y + sy as u32);
            temp[sy * sub_w + sx] = p.0;
        }
    }

    for _ in 0..3 {
        // Horizontal pass
        for sy in 0..sub_h {
            let row_offset = sy * sub_w;
            for sx in 0..sub_w {
                let mut r_acc = 0u32;
                let mut g_acc = 0u32;
                let mut b_acc = 0u32;
                let mut a_acc = 0u32;
                let mut count = 0u32;

                let start = (sx as i32 - r).max(0) as usize;
                let end = ((sx as i32 + r).min(sub_w as i32 - 1)) as usize;

                for kx in start..=end {
                    let p = temp[row_offset + kx];
                    r_acc += p[0] as u32;
                    g_acc += p[1] as u32;
                    b_acc += p[2] as u32;
                    a_acc += p[3] as u32;
                    count += 1;
                }

                out[row_offset + sx] = [
                    (r_acc / count) as u8,
                    (g_acc / count) as u8,
                    (b_acc / count) as u8,
                    (a_acc / count) as u8,
                ];
            }
        }

        // Vertical pass
        for sx in 0..sub_w {
            for sy in 0..sub_h {
                let mut r_acc = 0u32;
                let mut g_acc = 0u32;
                let mut b_acc = 0u32;
                let mut a_acc = 0u32;
                let mut count = 0u32;

                let start = (sy as i32 - r).max(0) as usize;
                let end = ((sy as i32 + r).min(sub_h as i32 - 1)) as usize;

                for ky in start..=end {
                    let p = out[ky * sub_w + sx];
                    r_acc += p[0] as u32;
                    g_acc += p[1] as u32;
                    b_acc += p[2] as u32;
                    a_acc += p[3] as u32;
                    count += 1;
                }

                temp[sy * sub_w + sx] = [
                    (r_acc / count) as u8,
                    (g_acc / count) as u8,
                    (b_acc / count) as u8,
                    (a_acc / count) as u8,
                ];
            }
        }
    }

    // Write blurred pixels back into the original image
    for sy in 0..sub_h {
        for sx in 0..sub_w {
            let p = temp[sy * sub_w + sx];
            image.put_pixel(x + sx as u32, y + sy as u32, Rgba(p));
        }
    }
}
