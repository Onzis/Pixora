use crate::editor::annotations::Annotation;
use crate::image::processing::{apply_blur, apply_pixelate};
use gpui::Rgba;
use image::{Rgba as ImageRgba, RgbaImage};

/// Rasterize all vector annotations and filters onto an RgbaImage in-place.
pub fn render_annotations_to_image(
    image: &mut RgbaImage,
    annotations: &[Annotation],
    offset_x: f32,
    offset_y: f32,
) {
    for ann in annotations {
        match ann {
            Annotation::Blur { bounds, radius } => {
                let bx = ((f32::from(bounds.origin.x) - offset_x).max(0.0) as u32).min(image.width());
                let by = ((f32::from(bounds.origin.y) - offset_y).max(0.0) as u32).min(image.height());
                let bw = (f32::from(bounds.size.width) as u32).min(image.width() - bx);
                let bh = (f32::from(bounds.size.height) as u32).min(image.height() - by);
                if bw > 0 && bh > 0 {
                    apply_blur(image, bx, by, bw, bh, *radius);
                }
            }
            Annotation::Pixelate { bounds, block_size } => {
                let bx = ((f32::from(bounds.origin.x) - offset_x).max(0.0) as u32).min(image.width());
                let by = ((f32::from(bounds.origin.y) - offset_y).max(0.0) as u32).min(image.height());
                let bw = (f32::from(bounds.size.width) as u32).min(image.width() - bx);
                let bh = (f32::from(bounds.size.height) as u32).min(image.height() - by);
                if bw > 0 && bh > 0 {
                    apply_pixelate(image, bx, by, bw, bh, *block_size);
                }
            }
            Annotation::Line {
                from,
                to,
                color,
                width,
            } => {
                draw_line(
                    image,
                    f32::from(from.x) - offset_x,
                    f32::from(from.y) - offset_y,
                    f32::from(to.x) - offset_x,
                    f32::from(to.y) - offset_y,
                    *width,
                    *color,
                    false,
                );
            }
            Annotation::Arrow {
                from,
                to,
                color,
                width,
            } => {
                let x1 = f32::from(from.x) - offset_x;
                let y1 = f32::from(from.y) - offset_y;
                let x2 = f32::from(to.x) - offset_x;
                let y2 = f32::from(to.y) - offset_y;

                draw_line(image, x1, y1, x2, y2, *width, *color, false);

                // Draw arrowhead
                let dx = x2 - x1;
                let dy = y2 - y1;
                let len = (dx * dx + dy * dy).sqrt();
                if len > 5.0 {
                    let head_len = (*width * 4.0).clamp(14.0, 32.0);
                    let angle = dy.atan2(dx);
                    let arrow_angle = 0.5;

                    let ax1 = x2 - head_len * (angle - arrow_angle).cos();
                    let ay1 = y2 - head_len * (angle - arrow_angle).sin();
                    let ax2 = x2 - head_len * (angle + arrow_angle).cos();
                    let ay2 = y2 - head_len * (angle + arrow_angle).sin();

                    draw_line(image, x2, y2, ax1, ay1, *width, *color, false);
                    draw_line(image, x2, y2, ax2, ay2, *width, *color, false);
                }
            }
            Annotation::Pencil {
                points,
                color,
                width,
            } => {
                for pair in points.windows(2) {
                    draw_line(
                        image,
                        f32::from(pair[0].x) - offset_x,
                        f32::from(pair[0].y) - offset_y,
                        f32::from(pair[1].x) - offset_x,
                        f32::from(pair[1].y) - offset_y,
                        *width,
                        *color,
                        false,
                    );
                }
            }
            Annotation::Marker {
                points,
                color,
                width,
            } => {
                let mut marker_color = *color;
                marker_color.a = 0.35;
                for pair in points.windows(2) {
                    draw_line(
                        image,
                        f32::from(pair[0].x) - offset_x,
                        f32::from(pair[0].y) - offset_y,
                        f32::from(pair[1].x) - offset_x,
                        f32::from(pair[1].y) - offset_y,
                        *width * 3.0,
                        marker_color,
                        true,
                    );
                }
            }
            Annotation::Rectangle {
                bounds,
                color,
                width,
                filled,
            } => {
                let rx = f32::from(bounds.origin.x) - offset_x;
                let ry = f32::from(bounds.origin.y) - offset_y;
                let rw = f32::from(bounds.size.width);
                let rh = f32::from(bounds.size.height);
                draw_rectangle(image, rx, ry, rw, rh, *width, *color, *filled);
            }
            Annotation::Ellipse {
                bounds,
                color,
                width,
                filled,
            } => {
                let rx = f32::from(bounds.origin.x) - offset_x;
                let ry = f32::from(bounds.origin.y) - offset_y;
                let rw = f32::from(bounds.size.width);
                let rh = f32::from(bounds.size.height);
                draw_ellipse(image, rx, ry, rw, rh, *width, *color, *filled);
            }
            Annotation::NumberBadge {
                center,
                number,
                color,
                radius,
            } => {
                let cx = f32::from(center.x) - offset_x;
                let cy = f32::from(center.y) - offset_y;
                let r = *radius;

                draw_circle(image, cx, cy, r, *color, true, 1.0);
                draw_circle(
                    image,
                    cx,
                    cy,
                    r,
                    gpui::rgb(0xFFFFFF),
                    false,
                    2.0,
                );

                let num_str = number.to_string();
                let text_x = cx - (num_str.len() as f32 * 4.5);
                let text_y = cy - 6.0;
                draw_simple_text(image, text_x, text_y, &num_str, gpui::rgb(0xFFFFFF), 2);
            }
            Annotation::Text {
                origin,
                text,
                color,
                font_size,
            } => {
                let scale = ((*font_size / 9.0).round() as usize).max(1);
                draw_simple_text(
                    image,
                    f32::from(origin.x) - offset_x,
                    f32::from(origin.y) - offset_y,
                    text,
                    *color,
                    scale,
                );
            }
        }
    }
}

fn blend_pixel(dst: ImageRgba<u8>, color: Rgba, alpha_mult: f32) -> ImageRgba<u8> {
    let sa = (color.a * alpha_mult).clamp(0.0, 1.0);
    let da = (dst[3] as f32) / 255.0;

    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0 {
        return dst;
    }

    let sr = color.r * 255.0;
    let sg = color.g * 255.0;
    let sb = color.b * 255.0;

    let dr = dst[0] as f32;
    let dg = dst[1] as f32;
    let db = dst[2] as f32;

    let out_r = ((sr * sa + dr * da * (1.0 - sa)) / out_a).round() as u8;
    let out_g = ((sg * sa + dg * da * (1.0 - sa)) / out_a).round() as u8;
    let out_b = ((sb * sa + db * da * (1.0 - sa)) / out_a).round() as u8;

    ImageRgba([out_r, out_g, out_b, (out_a * 255.0).round() as u8])
}

fn put_pixel_blended(image: &mut RgbaImage, x: i32, y: i32, color: Rgba, alpha: f32) {
    if x >= 0 && y >= 0 && (x as u32) < image.width() && (y as u32) < image.height() {
        let cur = image.get_pixel(x as u32, y as u32);
        let blended = blend_pixel(*cur, color, alpha);
        image.put_pixel(x as u32, y as u32, blended);
    }
}

fn draw_line(
    image: &mut RgbaImage,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    width: f32,
    color: Rgba,
    is_marker: bool,
) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let dist = (dx * dx + dy * dy).sqrt();
    let steps = (dist * 2.0).ceil() as usize;
    let r = (width / 2.0).max(0.5);

    if steps == 0 {
        draw_circle(image, x0, y0, r, color, true, 1.0);
        return;
    }

    let x_step = dx / steps as f32;
    let y_step = dy / steps as f32;

    let mut cx = x0;
    let mut cy = y0;

    for _ in 0..=steps {
        let min_x = (cx - r).floor() as i32;
        let max_x = (cx + r).ceil() as i32;
        let min_y = (cy - r).floor() as i32;
        let max_y = (cy + r).ceil() as i32;

        for py in min_y..=max_y {
            for px in min_x..=max_x {
                let d = ((px as f32 - cx).powi(2) + (py as f32 - cy).powi(2)).sqrt();
                if d <= r {
                    let a = if is_marker {
                        0.35
                    } else if d > r - 1.0 {
                        r - d
                    } else {
                        1.0
                    };
                    put_pixel_blended(image, px, py, color, a);
                }
            }
        }

        cx += x_step;
        cy += y_step;
    }
}

fn draw_circle(
    image: &mut RgbaImage,
    cx: f32,
    cy: f32,
    radius: f32,
    color: Rgba,
    filled: bool,
    stroke_w: f32,
) {
    let r_outer = radius + stroke_w / 2.0;
    let r_inner = if filled { 0.0 } else { (radius - stroke_w / 2.0).max(0.0) };

    let min_x = (cx - r_outer).floor() as i32;
    let max_x = (cx + r_outer).ceil() as i32;
    let min_y = (cy - r_outer).floor() as i32;
    let max_y = (cy + r_outer).ceil() as i32;

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            let d = ((px as f32 - cx).powi(2) + (py as f32 - cy).powi(2)).sqrt();
            if filled {
                if d <= r_outer {
                    let a = if d > r_outer - 1.0 { r_outer - d } else { 1.0 };
                    put_pixel_blended(image, px, py, color, a);
                }
            } else {
                if d <= r_outer && d >= r_inner {
                    put_pixel_blended(image, px, py, color, 1.0);
                }
            }
        }
    }
}

fn draw_rectangle(
    image: &mut RgbaImage,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    width: f32,
    color: Rgba,
    filled: bool,
) {
    if filled {
        let min_x = x.round() as i32;
        let min_y = y.round() as i32;
        let max_x = (x + w).round() as i32;
        let max_y = (y + h).round() as i32;
        for py in min_y..max_y {
            for px in min_x..max_x {
                put_pixel_blended(image, px, py, color, 1.0);
            }
        }
    } else {
        draw_line(image, x, y, x + w, y, width, color, false);
        draw_line(image, x + w, y, x + w, y + h, width, color, false);
        draw_line(image, x + w, y + h, x, y + h, width, color, false);
        draw_line(image, x, y + h, x, y, width, color, false);
    }
}

fn draw_ellipse(
    image: &mut RgbaImage,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    width: f32,
    color: Rgba,
    filled: bool,
) {
    let rx = w / 2.0;
    let ry = h / 2.0;
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let cx = x + rx;
    let cy = y + ry;

    let min_x = (x - width).floor() as i32;
    let max_x = (x + w + width).ceil() as i32;
    let min_y = (y - width).floor() as i32;
    let max_y = (y + h + width).ceil() as i32;

    let half_w = width / 2.0;

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            let dx = (px as f32 - cx) / rx;
            let dy = (py as f32 - cy) / ry;
            let val = dx * dx + dy * dy;

            if filled {
                if val <= 1.0 {
                    put_pixel_blended(image, px, py, color, 1.0);
                }
            } else {
                let dist_from_edge = (val.sqrt() - 1.0).abs() * ((rx + ry) / 2.0);
                if dist_from_edge <= half_w {
                    put_pixel_blended(image, px, py, color, 1.0);
                }
            }
        }
    }
}

static FONT_5X7: [[u8; 5]; 96] = [
    [0x00, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x5F, 0x00, 0x00],
    [0x00, 0x07, 0x00, 0x07, 0x00],
    [0x14, 0x7F, 0x14, 0x7F, 0x14],
    [0x24, 0x2A, 0x7F, 0x2A, 0x12],
    [0x23, 0x13, 0x08, 0x64, 0x62],
    [0x36, 0x49, 0x55, 0x22, 0x50],
    [0x00, 0x05, 0x03, 0x00, 0x00],
    [0x00, 0x1C, 0x22, 0x41, 0x00],
    [0x00, 0x41, 0x22, 0x1C, 0x00],
    [0x08, 0x2A, 0x1C, 0x2A, 0x08],
    [0x08, 0x08, 0x3E, 0x08, 0x08],
    [0x00, 0x50, 0x30, 0x00, 0x00],
    [0x08, 0x08, 0x08, 0x08, 0x08],
    [0x00, 0x60, 0x60, 0x00, 0x00],
    [0x20, 0x10, 0x08, 0x04, 0x02],
    [0x3E, 0x51, 0x49, 0x45, 0x3E],
    [0x00, 0x42, 0x7F, 0x40, 0x00],
    [0x42, 0x61, 0x51, 0x49, 0x46],
    [0x21, 0x41, 0x45, 0x4B, 0x31],
    [0x18, 0x14, 0x12, 0x7F, 0x10],
    [0x27, 0x45, 0x45, 0x45, 0x39],
    [0x3C, 0x4A, 0x49, 0x49, 0x30],
    [0x01, 0x71, 0x09, 0x05, 0x03],
    [0x36, 0x49, 0x49, 0x49, 0x36],
    [0x06, 0x49, 0x49, 0x29, 0x1E],
    [0x00, 0x36, 0x36, 0x00, 0x00],
    [0x00, 0x56, 0x36, 0x00, 0x00],
    [0x00, 0x08, 0x14, 0x22, 0x41],
    [0x14, 0x14, 0x14, 0x14, 0x14],
    [0x41, 0x22, 0x14, 0x08, 0x00],
    [0x02, 0x01, 0x51, 0x09, 0x06],
    [0x32, 0x49, 0x79, 0x41, 0x3E],
    [0x7E, 0x11, 0x11, 0x11, 0x7E],
    [0x7F, 0x49, 0x49, 0x49, 0x36],
    [0x3E, 0x41, 0x41, 0x41, 0x22],
    [0x7F, 0x41, 0x41, 0x22, 0x1C],
    [0x7F, 0x49, 0x49, 0x49, 0x41],
    [0x7F, 0x09, 0x09, 0x01, 0x01],
    [0x3E, 0x41, 0x41, 0x51, 0x32],
    [0x7F, 0x08, 0x08, 0x08, 0x7F],
    [0x00, 0x41, 0x7F, 0x41, 0x00],
    [0x20, 0x40, 0x41, 0x3F, 0x01],
    [0x7F, 0x08, 0x14, 0x22, 0x41],
    [0x7F, 0x40, 0x40, 0x40, 0x40],
    [0x7F, 0x02, 0x04, 0x02, 0x7F],
    [0x7F, 0x04, 0x08, 0x10, 0x7F],
    [0x3E, 0x41, 0x41, 0x41, 0x3E],
    [0x7F, 0x09, 0x09, 0x09, 0x06],
    [0x3E, 0x41, 0x51, 0x21, 0x5E],
    [0x7F, 0x09, 0x19, 0x29, 0x46],
    [0x46, 0x49, 0x49, 0x49, 0x31],
    [0x01, 0x01, 0x7F, 0x01, 0x01],
    [0x3F, 0x40, 0x40, 0x40, 0x3F],
    [0x1F, 0x20, 0x40, 0x20, 0x1F],
    [0x7F, 0x20, 0x18, 0x20, 0x7F],
    [0x63, 0x14, 0x08, 0x14, 0x63],
    [0x03, 0x04, 0x78, 0x04, 0x03],
    [0x61, 0x51, 0x49, 0x45, 0x43],
    [0x00, 0x7F, 0x41, 0x41, 0x00],
    [0x02, 0x04, 0x08, 0x10, 0x20],
    [0x00, 0x41, 0x41, 0x7F, 0x00],
    [0x04, 0x02, 0x01, 0x02, 0x04],
    [0x40, 0x40, 0x40, 0x40, 0x40],
    [0x00, 0x01, 0x02, 0x04, 0x00],
    [0x20, 0x54, 0x54, 0x54, 0x78],
    [0x7F, 0x48, 0x44, 0x44, 0x38],
    [0x38, 0x44, 0x44, 0x44, 0x20],
    [0x38, 0x44, 0x44, 0x48, 0x7F],
    [0x38, 0x54, 0x54, 0x54, 0x18],
    [0x08, 0x7E, 0x09, 0x01, 0x02],
    [0x08, 0x14, 0x54, 0x54, 0x3C],
    [0x7F, 0x08, 0x04, 0x04, 0x78],
    [0x00, 0x44, 0x7D, 0x40, 0x00],
    [0x20, 0x40, 0x44, 0x3D, 0x00],
    [0x00, 0x7F, 0x10, 0x28, 0x44],
    [0x00, 0x41, 0x7F, 0x40, 0x00],
    [0x7C, 0x04, 0x18, 0x04, 0x78],
    [0x7C, 0x08, 0x04, 0x04, 0x78],
    [0x38, 0x44, 0x44, 0x44, 0x38],
    [0x7C, 0x14, 0x14, 0x14, 0x08],
    [0x08, 0x14, 0x14, 0x18, 0x7C],
    [0x7C, 0x08, 0x04, 0x04, 0x08],
    [0x48, 0x54, 0x54, 0x54, 0x20],
    [0x04, 0x3F, 0x44, 0x40, 0x20],
    [0x3C, 0x40, 0x40, 0x20, 0x7C],
    [0x1C, 0x20, 0x40, 0x20, 0x1C],
    [0x3C, 0x40, 0x30, 0x40, 0x3C],
    [0x44, 0x28, 0x10, 0x28, 0x44],
    [0x0C, 0x50, 0x50, 0x50, 0x3C],
    [0x44, 0x64, 0x54, 0x4C, 0x44],
    [0x00, 0x08, 0x36, 0x41, 0x00],
    [0x00, 0x00, 0x7F, 0x00, 0x00],
    [0x00, 0x41, 0x36, 0x08, 0x00],
    [0x08, 0x08, 0x2A, 0x1C, 0x08],
    [0x00, 0x00, 0x00, 0x00, 0x00],
];

fn draw_simple_text(
    image: &mut RgbaImage,
    start_x: f32,
    start_y: f32,
    text: &str,
    color: Rgba,
    scale: usize,
) {
    let mut cursor_x = start_x as i32;
    let base_y = start_y as i32;
    let s = scale.max(1) as i32;

    for ch in text.chars() {
        if ch == '\n' {
            continue;
        }

        let code = ch as usize;
        let glyph_idx = if code >= 32 && code < 128 {
            code - 32
        } else {
            0
        };

        let glyph = FONT_5X7[glyph_idx];
        for (col, &col_bits) in glyph.iter().enumerate() {
            for row in 0..7 {
                if (col_bits & (1 << row)) != 0 {
                    for sy in 0..s {
                        for sx in 0..s {
                            let px = cursor_x + (col as i32 * s) + sx;
                            let py = base_y + (row * s) + sy;
                            put_pixel_blended(image, px, py, color, 1.0);
                        }
                    }
                }
            }
        }

        cursor_x += (6 * s) as i32;
    }
}
