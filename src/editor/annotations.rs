use gpui::{Bounds, Pixels, Point, Rgba, point, px, size};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnotationHandle {
    Endpoint(bool), // true = start (from), false = end (to)
    BezierControl,
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeShape {
    Circle,
    RoundedRect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeMode {
    Numbers,
    Alphabet,
}

pub fn number_to_badge_label(number: u32, mode: BadgeMode) -> String {
    match mode {
        BadgeMode::Numbers => number.to_string(),
        BadgeMode::Alphabet => {
            let mut n = number;
            let mut s = String::new();
            while n > 0 {
                let rem = ((n - 1) % 26) as u8;
                s.insert(0, (b'A' + rem) as char);
                n = (n - 1) / 26;
            }
            if s.is_empty() {
                "A".to_string()
            } else {
                s
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StickerKind {
    Checkmark,
    Cross,
    Warning,
    Cursor,
    Star,
    Heart,
    Question,
    Lightbulb,
}

impl StickerKind {
    pub fn icon_path(&self) -> &'static str {
        match self {
            StickerKind::Checkmark => "icons/sticker-check.svg",
            StickerKind::Cross => "icons/sticker-cross.svg",
            StickerKind::Warning => "icons/sticker-warning.svg",
            StickerKind::Cursor => "icons/sticker-cursor.svg",
            StickerKind::Star => "icons/sticker-star.svg",
            StickerKind::Heart => "icons/sticker-heart.svg",
            StickerKind::Question => "icons/sticker-question.svg",
            StickerKind::Lightbulb => "icons/sticker-bulb.svg",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Annotation {
    Pencil {
        points: Vec<Point<Pixels>>,
        color: Rgba,
        width: f32,
    },
    Marker {
        points: Vec<Point<Pixels>>,
        color: Rgba,
        width: f32,
    },
    Arrow {
        from: Point<Pixels>,
        to: Point<Pixels>,
        control: Option<Point<Pixels>>,
        two_headed: bool,
        color: Rgba,
        width: f32,
    },
    Line {
        from: Point<Pixels>,
        to: Point<Pixels>,
        color: Rgba,
        width: f32,
    },
    Rectangle {
        bounds: Bounds<Pixels>,
        color: Rgba,
        width: f32,
        filled: bool,
    },
    Ellipse {
        bounds: Bounds<Pixels>,
        color: Rgba,
        width: f32,
        filled: bool,
    },
    Triangle {
        bounds: Bounds<Pixels>,
        color: Rgba,
        width: f32,
        filled: bool,
    },
    Star {
        bounds: Bounds<Pixels>,
        color: Rgba,
        width: f32,
        filled: bool,
    },
    Diamond {
        bounds: Bounds<Pixels>,
        color: Rgba,
        width: f32,
        filled: bool,
    },
    Text {
        origin: Point<Pixels>,
        text: String,
        color: Rgba,
        font_size: f32,
        box_width: Option<f32>,
    },
    Blur {
        bounds: Bounds<Pixels>,
        radius: u32,
    },
    Pixelate {
        bounds: Bounds<Pixels>,
        block_size: u32,
    },
    Spotlight {
        bounds: Bounds<Pixels>,
        color: Rgba,
        width: f32,
        dim_opacity: f32,
    },
    NumberBadge {
        center: Point<Pixels>,
        number: u32,
        color: Rgba,
        radius: f32,
        shape: BadgeShape,
        mode: BadgeMode,
        filled: bool,
    },
    Sticker {
        center: Point<Pixels>,
        kind: StickerKind,
        size: f32,
        color: Option<Rgba>,
    },
}

pub fn normalize_bounds(b: Bounds<Pixels>) -> Bounds<Pixels> {
    let x = f32::from(b.origin.x);
    let y = f32::from(b.origin.y);
    let w = f32::from(b.size.width);
    let h = f32::from(b.size.height);
    let (norm_x, norm_w) = if w < 0.0 { (x + w, -w) } else { (x, w) };
    let (norm_y, norm_h) = if h < 0.0 { (y + h, -h) } else { (y, h) };
    Bounds {
        origin: point(px(norm_x), px(norm_y)),
        size: size(px(norm_w), px(norm_h)),
    }
}

pub fn point_in_triangle(p: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)) -> bool {
    let sign = |a: (f32, f32), b: (f32, f32), c: (f32, f32)| -> f32 {
        (a.0 - c.0) * (b.1 - c.1) - (b.0 - c.0) * (a.1 - c.1)
    };
    let d1 = sign(p, p1, p2);
    let d2 = sign(p, p2, p3);
    let d3 = sign(p, p3, p1);
    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
    !(has_neg && has_pos)
}

pub fn dist_to_segment_sq(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-6 {
        let ex = p.0 - a.0;
        let ey = p.1 - a.1;
        return ex * ex + ey * ey;
    }
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len_sq).clamp(0.0, 1.0);
    let proj_x = a.0 + t * dx;
    let proj_y = a.1 + t * dy;
    let ex = p.0 - proj_x;
    let ey = p.1 - proj_y;
    ex * ex + ey * ey
}

pub fn sample_quadratic_bezier(
    p0: (f32, f32),
    p1: (f32, f32),
    p2: (f32, f32),
    steps: usize,
) -> Vec<(f32, f32)> {
    let mut pts = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let u = 1.0 - t;
        let x = u * u * p0.0 + 2.0 * u * t * p1.0 + t * t * p2.0;
        let y = u * u * p0.1 + 2.0 * u * t * p1.1 + t * t * p2.1;
        pts.push((x, y));
    }
    pts
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArrowHeadGeometry {
    pub tip: (f32, f32),
    pub left_wing: (f32, f32),
    pub right_wing: (f32, f32),
    pub base_center: (f32, f32),
    pub shaft_attachment: (f32, f32),
}

pub fn calculate_arrowhead(
    tip: (f32, f32),
    dir_from: (f32, f32),
    width: f32,
    total_len: f32,
) -> Option<ArrowHeadGeometry> {
    let dx = tip.0 - dir_from.0;
    let dy = tip.1 - dir_from.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 2.0 {
        return None;
    }
    let ux = dx / len;
    let uy = dy / len;
    let nx = -uy;
    let ny = ux;

    let base_head_len = width * 2.5 + 12.0;
    let head_len = base_head_len.min(total_len * 0.80);
    let half_width = (head_len * 0.48).max(width * 0.95 + 4.0);

    let base_x = tip.0 - head_len * ux;
    let base_y = tip.1 - head_len * uy;

    let left_wing = (base_x + half_width * nx, base_y + half_width * ny);
    let right_wing = (base_x - half_width * nx, base_y - half_width * ny);

    let overlap = (width * 0.4).min(head_len * 0.35);
    let shaft_attachment = (base_x + overlap * ux, base_y + overlap * uy);

    Some(ArrowHeadGeometry {
        tip,
        left_wing,
        right_wing,
        base_center: (base_x, base_y),
        shaft_attachment,
    })
}

pub fn resize_bounds_with_handle(
    orig_bounds: Bounds<Pixels>,
    handle: AnnotationHandle,
    start_mouse: Point<Pixels>,
    cur_mouse: Point<Pixels>,
) -> Bounds<Pixels> {
    let dx = f32::from(cur_mouse.x - start_mouse.x);
    let dy = f32::from(cur_mouse.y - start_mouse.y);
    let ox = f32::from(orig_bounds.origin.x);
    let oy = f32::from(orig_bounds.origin.y);
    let ow = f32::from(orig_bounds.size.width);
    let oh = f32::from(orig_bounds.size.height);

    let (mut x, mut y, mut w, mut h) = (ox, oy, ow, oh);
    match handle {
        AnnotationHandle::TopLeft => {
            x += dx; y += dy; w -= dx; h -= dy;
        }
        AnnotationHandle::Top => {
            y += dy; h -= dy;
        }
        AnnotationHandle::TopRight => {
            y += dy; w += dx; h -= dy;
        }
        AnnotationHandle::Right => {
            w += dx;
        }
        AnnotationHandle::BottomRight => {
            w += dx; h += dy;
        }
        AnnotationHandle::Bottom => {
            h += dy;
        }
        AnnotationHandle::BottomLeft => {
            x += dx; w -= dx; h += dy;
        }
        AnnotationHandle::Left => {
            x += dx; w -= dx;
        }
        _ => {}
    }

    let (norm_x, norm_w) = if w < 0.0 { (x + w, -w) } else { (x, w) };
    let (norm_y, norm_h) = if h < 0.0 { (y + h, -h) } else { (y, h) };
    Bounds {
        origin: point(px(norm_x), px(norm_y)),
        size: size(px(norm_w.max(4.0)), px(norm_h.max(4.0))),
    }
}

pub fn char_width(ch: char) -> f32 {
    match ch {
        '.' | ',' | ':' | ';' | '\'' | ' ' => 0.28,
        'i' | 'j' | 'l' | '!' | '|' | 'I' => 0.30,
        '(' | ')' | '[' | ']' | '{' | '}' | 'f' | 't' | 'r' => 0.38,
        '-' | '—' | 'г' | '?' | '/' | 's' => 0.44,
        'т' | '*' | 'z' | 'э' | 'c' | 'з' | 'с' => 0.48,
        'a' | 'e' | 'y' | 'v' | 'x' | 'а' | 'е' | 'ё' | 'у' | 'х' => 0.54,
        '0'..='9' | 'k' | 'ь' | 'я' | 'к' => 0.58,
        'в' | 'T' | 'Т' | '#' | 'ч' | 'h' | 'л' | 'n' | 'u' | 'Y' | 'Z' | 'б' | 'o' | 'о' | 'н' | 'п'
        | 'P' | 'Р' | 'У' | 'd' | 'g' | 'q' | 'b' | 'p' | 'д' | 'р' | 'Ь' | 'C' | 'С' | 'Э' | 'и' | 'й' => 0.62,
        'B' | 'В' | 'Б' | 'ъ' | 'K' | 'К' | 'ц' | 'R' | 'X' | 'Х' | 'V' => 0.66,
        'A' | 'А' | '<' | '>' | '+' | '=' | 'G' | 'U' | 'Ч' | 'Л' | 'D' | 'Ъ' | 'O' | 'Q' | 'О' | 'П' | 'H' | 'Н' | 'Д' => 0.74,
        'м' | 'ф' | 'N' | 'И' | 'Й' | 'w' | 'ы' | 'Ц' | 'Ф' => 0.80,
        'ж' | '&' | 'ш' | '%' | 'ю' | 'Ы' | 'щ' => 0.88,
        'm' | '@' | 'M' | 'М' | 'Ж' => 0.96,
        'W' | 'Ш' | 'Ю' | 'Щ' => 1.05,
        _ => 0.62,
    }
}

pub fn string_width(s: &str, font_size: f32) -> f32 {
    let mut total = 0.0f32;
    for ch in s.chars() {
        total += char_width(ch);
    }
    total * font_size
}

#[cfg(windows)]
pub fn measure_text_width(s: &str, font_size: f32) -> f32 {
    if s.is_empty() {
        return 0.0;
    }
    unsafe {
        use windows::Win32::Graphics::Gdi::*;
        use windows::Win32::Foundation::SIZE;
        use windows::core::w;

        let hdc = GetDC(None);
        if !hdc.is_invalid() {
            let font_h = -(font_size.round() as i32);
            let hfont = CreateFontW(
                font_h, 0, 0, 0, 700, 0, 0, 0,
                FONT_CHARSET(1),
                FONT_OUTPUT_PRECISION(0),
                FONT_CLIP_PRECISION(0),
                FONT_QUALITY(0),
                0,
                w!("Segoe UI"),
            );
            if !hfont.is_invalid() {
                let old_font = SelectObject(hdc, hfont.into());
                let wide: Vec<u16> = s.encode_utf16().collect();
                let mut sz = SIZE::default();
                let ok = GetTextExtentPoint32W(hdc, &wide, &mut sz).as_bool();
                SelectObject(hdc, old_font);
                let _ = DeleteObject(hfont.into());
                ReleaseDC(None, hdc);

                if ok && sz.cx > 0 {
                    return sz.cx as f32;
                }
            } else {
                ReleaseDC(None, hdc);
            }
        }
    }
    string_width(s, font_size)
}

#[cfg(not(windows))]
pub fn measure_text_width(s: &str, font_size: f32) -> f32 {
    string_width(s, font_size)
}

pub fn wrap_text_to_lines(text: &str, font_size: f32, box_width: Option<f32>) -> Vec<String> {
    let max_w = match box_width {
        Some(w) if w > 10.0 => w,
        _ => return text.lines().map(|s| s.to_string()).collect(),
    };

    let pad = (font_size * 0.12).clamp(6.0, 14.0);
    let avail_w = (max_w - 2.0 * pad - 4.0).max(font_size * 0.5);
    let mut result = Vec::new();

    for raw_line in text.split('\n') {
        if raw_line.is_empty() {
            result.push(String::new());
            continue;
        }

        let words: Vec<&str> = raw_line.split(' ').collect();
        let mut cur = String::new();

        for word in words {
            if word.is_empty() {
                if !cur.is_empty() {
                    cur.push(' ');
                }
                continue;
            }

            let test_str = if cur.is_empty() {
                word.to_string()
            } else {
                format!("{} {}", cur, word)
            };

            if measure_text_width(&test_str, font_size) <= avail_w {
                cur = test_str;
            } else {
                if !cur.is_empty() {
                    result.push(cur);
                }

                if measure_text_width(word, font_size) <= avail_w {
                    cur = word.to_string();
                } else {
                    let mut chunk = String::new();
                    for ch in word.chars() {
                        let mut test_chunk = chunk.clone();
                        test_chunk.push(ch);
                        if measure_text_width(&test_chunk, font_size) <= avail_w {
                            chunk = test_chunk;
                        } else {
                            if !chunk.is_empty() {
                                result.push(chunk);
                            }
                            chunk = ch.to_string();
                        }
                    }
                    cur = chunk;
                }
            }
        }

        if !cur.is_empty() {
            result.push(cur);
        }
    }

    if result.is_empty() {
        result.push(String::new());
    }

    result
}

pub fn text_bounds_dimensions(text: &str, font_size: f32, box_width: Option<f32>) -> (f32, f32) {
    let lines = wrap_text_to_lines(text, font_size, box_width);
    let mut max_line_w = 0.0f32;
    for line in &lines {
        let lw = measure_text_width(line, font_size);
        if lw > max_line_w {
            max_line_w = lw;
        }
    }
    let pad = (font_size * 0.12).clamp(6.0, 14.0);
    let w = if let Some(bw) = box_width {
        bw.max(max_line_w + 2.0 * pad + 4.0)
    } else {
        max_line_w.max(font_size * 0.5) + 2.0 * pad + 4.0
    };
    let line_count = lines.len().max(1) as f32;
    let line_step = font_size * 1.20;
    let text_h = (line_count - 1.0) * line_step + font_size * 1.15;
    let h = text_h + 2.0 * pad;
    (w, h)
}

impl Annotation {
    pub fn bounds(&self) -> Bounds<Pixels> {
        match self {
            Annotation::Pencil { points, width, .. } | Annotation::Marker { points, width, .. } => {
                if points.is_empty() {
                    return Bounds { origin: point(px(0.0), px(0.0)), size: size(px(0.0), px(0.0)) };
                }
                let mut min_x = f32::from(points[0].x);
                let mut max_x = min_x;
                let mut min_y = f32::from(points[0].y);
                let mut max_y = min_y;
                for pt in &points[1..] {
                    let x = f32::from(pt.x);
                    let y = f32::from(pt.y);
                    if x < min_x { min_x = x; }
                    if x > max_x { max_x = x; }
                    if y < min_y { min_y = y; }
                    if y > max_y { max_y = y; }
                }
                let pad = *width / 2.0;
                Bounds {
                    origin: point(px(min_x - pad), px(min_y - pad)),
                    size: size(px((max_x - min_x) + *width), px((max_y - min_y) + *width)),
                }
            }
            Annotation::Arrow { from, to, control, width, .. } => {
                let x1 = f32::from(from.x);
                let y1 = f32::from(from.y);
                let x2 = f32::from(to.x);
                let y2 = f32::from(to.y);
                let mut min_x = x1.min(x2);
                let mut max_x = x1.max(x2);
                let mut min_y = y1.min(y2);
                let mut max_y = y1.max(y2);
                if let Some(cp) = control {
                    let cx = f32::from(cp.x);
                    let cy = f32::from(cp.y);
                    min_x = min_x.min(cx);
                    max_x = max_x.max(cx);
                    min_y = min_y.min(cy);
                    max_y = max_y.max(cy);
                }
                let pad = (*width * 2.5).max(10.0);
                Bounds {
                    origin: point(px(min_x - pad), px(min_y - pad)),
                    size: size(px((max_x - min_x) + pad * 2.0), px((max_y - min_y) + pad * 2.0)),
                }
            }
            Annotation::Line { from, to, width, .. } => {
                let x1 = f32::from(from.x);
                let y1 = f32::from(from.y);
                let x2 = f32::from(to.x);
                let y2 = f32::from(to.y);
                let min_x = x1.min(x2);
                let max_x = x1.max(x2);
                let min_y = y1.min(y2);
                let max_y = y1.max(y2);
                let pad = (*width * 2.5).max(10.0);
                Bounds {
                    origin: point(px(min_x - pad), px(min_y - pad)),
                    size: size(px((max_x - min_x) + pad * 2.0), px((max_y - min_y) + pad * 2.0)),
                }
            }
            Annotation::Rectangle { bounds, .. }
            | Annotation::Ellipse { bounds, .. }
            | Annotation::Triangle { bounds, .. }
            | Annotation::Star { bounds, .. }
            | Annotation::Diamond { bounds, .. }
            | Annotation::Blur { bounds, .. }
            | Annotation::Pixelate { bounds, .. }
            | Annotation::Spotlight { bounds, .. } => normalize_bounds(*bounds),
            Annotation::Text { origin, text, font_size, box_width, .. } => {
                let (w, h) = text_bounds_dimensions(text, *font_size, *box_width);
                Bounds {
                    origin: *origin,
                    size: size(px(w), px(h)),
                }
            }
            Annotation::NumberBadge { center, radius, .. } => {
                Bounds {
                    origin: point(center.x - px(*radius), center.y - px(*radius)),
                    size: size(px(*radius * 2.0), px(*radius * 2.0)),
                }
            }
            Annotation::Sticker { center, size: sticker_size, .. } => {
                let half = *sticker_size / 2.0;
                Bounds {
                    origin: point(center.x - px(half), center.y - px(half)),
                    size: size(px(*sticker_size), px(*sticker_size)),
                }
            }
        }
    }

    pub fn set_bounds(&mut self, new_bounds: Bounds<Pixels>) {
        let norm = normalize_bounds(new_bounds);
        match self {
            Annotation::Rectangle { bounds, .. }
            | Annotation::Ellipse { bounds, .. }
            | Annotation::Triangle { bounds, .. }
            | Annotation::Star { bounds, .. }
            | Annotation::Diamond { bounds, .. }
            | Annotation::Blur { bounds, .. }
            | Annotation::Pixelate { bounds, .. }
            | Annotation::Spotlight { bounds, .. } => {
                *bounds = norm;
            }
            Annotation::Text { origin, box_width, .. } => {
                let new_w = f32::from(norm.size.width);
                *box_width = Some(new_w);
                *origin = norm.origin;
            }
            Annotation::NumberBadge { center, radius, .. } => {
                let new_r = (f32::from(norm.size.width) / 2.0).max(f32::from(norm.size.height) / 2.0);
                *radius = new_r.clamp(8.0, 64.0);
                *center = point(
                    norm.origin.x + px(*radius),
                    norm.origin.y + px(*radius),
                );
            }
            Annotation::Sticker { center, size: sticker_size, .. } => {
                let new_s = f32::from(norm.size.width).max(f32::from(norm.size.height)).clamp(16.0, 256.0);
                *sticker_size = new_s;
                *center = point(
                    norm.origin.x + px(new_s / 2.0),
                    norm.origin.y + px(new_s / 2.0),
                );
            }
            _ => {}
        }
    }

    pub fn resize_with_handle(
        &mut self,
        orig_bounds: Bounds<Pixels>,
        handle: AnnotationHandle,
        start_mouse: Point<Pixels>,
        cur_mouse: Point<Pixels>,
    ) {
        let dx = f32::from(cur_mouse.x - start_mouse.x);
        let dy = f32::from(cur_mouse.y - start_mouse.y);
        let ox = f32::from(orig_bounds.origin.x);
        let oy = f32::from(orig_bounds.origin.y);
        let ow = f32::from(orig_bounds.size.width);
        let oh = f32::from(orig_bounds.size.height);

        match self {
            Annotation::Text { origin, font_size, box_width, text, .. } => {
                let orig_fs = *font_size;
                match handle {
                    AnnotationHandle::Right => {
                        let new_w = (ow + dx).max(50.0);
                        *box_width = Some(new_w);
                        *origin = point(px(ox), px(oy));
                    }
                    AnnotationHandle::Left => {
                        let new_w = (ow - dx).max(50.0);
                        let new_x = (ox + ow) - new_w;
                        *box_width = Some(new_w);
                        *origin = point(px(new_x), px(oy));
                    }
                    AnnotationHandle::Bottom => {
                        let ratio = ((oh + dy) / oh.max(1.0)).clamp(0.2, 5.0);
                        *font_size = (orig_fs * ratio).clamp(10.0, 96.0);
                        *origin = point(px(ox), px(oy));
                    }
                    AnnotationHandle::Top => {
                        let ratio = ((oh - dy) / oh.max(1.0)).clamp(0.2, 5.0);
                        let target_fs = (orig_fs * ratio).clamp(10.0, 96.0);
                        let (_, new_h) = text_bounds_dimensions(text, target_fs, *box_width);
                        let new_y = (oy + oh) - new_h;
                        *font_size = target_fs;
                        *origin = point(px(ox), px(new_y));
                    }
                    AnnotationHandle::BottomRight => {
                        let new_w = (ow + dx).max(50.0);
                        *box_width = Some(new_w);
                        *origin = point(px(ox), px(oy));
                    }
                    AnnotationHandle::BottomLeft => {
                        let new_w = (ow - dx).max(50.0);
                        let new_x = (ox + ow) - new_w;
                        *box_width = Some(new_w);
                        *origin = point(px(new_x), px(oy));
                    }
                    AnnotationHandle::TopRight => {
                        let new_w = (ow + dx).max(50.0);
                        *box_width = Some(new_w);
                        *origin = point(px(ox), px(oy));
                    }
                    AnnotationHandle::TopLeft => {
                        let new_w = (ow - dx).max(50.0);
                        let new_x = (ox + ow) - new_w;
                        *box_width = Some(new_w);
                        *origin = point(px(new_x), px(oy));
                    }
                    _ => {}
                }
            }
            Annotation::NumberBadge { radius, .. } => {
                let orig_r = *radius;
                let ratio_w = (ow + dx.abs()) / ow.max(1.0);
                let ratio_h = (oh + dy.abs()) / oh.max(1.0);
                let ratio = ratio_w.max(ratio_h).clamp(0.2, 5.0);
                *radius = (orig_r * ratio).clamp(8.0, 64.0);
            }
            Annotation::Sticker { size: sticker_size, .. } => {
                let orig_s = *sticker_size;
                let ratio_w = (ow + dx.abs()) / ow.max(1.0);
                let ratio_h = (oh + dy.abs()) / oh.max(1.0);
                let ratio = ratio_w.max(ratio_h).clamp(0.2, 5.0);
                *sticker_size = (orig_s * ratio).clamp(16.0, 256.0);
            }
            _ => {
                let new_bounds = resize_bounds_with_handle(orig_bounds, handle, start_mouse, cur_mouse);
                self.set_bounds(new_bounds);
            }
        }
    }

    pub fn hit_test(&self, p: Point<Pixels>, tolerance: f32) -> bool {
        let px_f = f32::from(p.x);
        let py_f = f32::from(p.y);
        let p_f = (px_f, py_f);

        match self {
            Annotation::Line { from, to, width, .. } => {
                let a_f = (f32::from(from.x), f32::from(from.y));
                let b_f = (f32::from(to.x), f32::from(to.y));
                let tol = tolerance + *width / 2.0;
                dist_to_segment_sq(p_f, a_f, b_f) <= tol * tol
            }
            Annotation::Arrow { from, to, control, two_headed, width, .. } => {
                let a_f = (f32::from(from.x), f32::from(from.y));
                let b_f = (f32::from(to.x), f32::from(to.y));
                let tol = tolerance + *width / 2.0;
                let tol_sq = tol * tol;

                if let Some(cp) = control {
                    let c_f = (f32::from(cp.x), f32::from(cp.y));
                    let pts = sample_quadratic_bezier(a_f, c_f, b_f, 24);
                    for pair in pts.windows(2) {
                        if dist_to_segment_sq(p_f, pair[0], pair[1]) <= tol_sq {
                            return true;
                        }
                    }
                    let head_to_geom = calculate_arrowhead(b_f, c_f, *width, 100.0);
                    if let Some(geom) = head_to_geom {
                        if dist_to_segment_sq(p_f, geom.tip, geom.left_wing) <= tol_sq
                            || dist_to_segment_sq(p_f, geom.tip, geom.right_wing) <= tol_sq
                            || dist_to_segment_sq(p_f, geom.left_wing, geom.right_wing) <= tol_sq
                            || point_in_triangle(p_f, geom.tip, geom.left_wing, geom.right_wing)
                        {
                            return true;
                        }
                    }
                    if *two_headed {
                        let head_from_geom = calculate_arrowhead(a_f, c_f, *width, 100.0);
                        if let Some(geom) = head_from_geom {
                            if dist_to_segment_sq(p_f, geom.tip, geom.left_wing) <= tol_sq
                                || dist_to_segment_sq(p_f, geom.tip, geom.right_wing) <= tol_sq
                                || dist_to_segment_sq(p_f, geom.left_wing, geom.right_wing) <= tol_sq
                                || point_in_triangle(p_f, geom.tip, geom.left_wing, geom.right_wing)
                            {
                                return true;
                            }
                        }
                    }
                    false
                } else {
                    if dist_to_segment_sq(p_f, a_f, b_f) <= tol_sq {
                        return true;
                    }
                    let geom = calculate_arrow_geometry(a_f, b_f, *width);
                    if geom.has_head {
                        if dist_to_segment_sq(p_f, geom.tip, geom.left_wing) <= tol_sq
                            || dist_to_segment_sq(p_f, geom.tip, geom.right_wing) <= tol_sq
                            || dist_to_segment_sq(p_f, geom.left_wing, geom.right_wing) <= tol_sq
                            || point_in_triangle(p_f, geom.tip, geom.left_wing, geom.right_wing)
                        {
                            return true;
                        }
                    }
                    if *two_headed {
                        let geom_rev = calculate_arrow_geometry(b_f, a_f, *width);
                        if geom_rev.has_head {
                            if dist_to_segment_sq(p_f, geom_rev.tip, geom_rev.left_wing) <= tol_sq
                                || dist_to_segment_sq(p_f, geom_rev.tip, geom_rev.right_wing) <= tol_sq
                                || dist_to_segment_sq(p_f, geom_rev.left_wing, geom_rev.right_wing) <= tol_sq
                                || point_in_triangle(p_f, geom_rev.tip, geom_rev.left_wing, geom_rev.right_wing)
                            {
                                return true;
                            }
                        }
                    }
                    false
                }
            }
            Annotation::Rectangle { bounds, filled, width, .. } => {
                let norm = normalize_bounds(*bounds);
                let left = f32::from(norm.origin.x);
                let top = f32::from(norm.origin.y);
                let w = f32::from(norm.size.width);
                let h = f32::from(norm.size.height);
                let right = left + w;
                let bottom = top + h;
                let tol = tolerance + *width / 2.0;

                if *filled || (w < 32.0 && h < 32.0) {
                    px_f >= left - tol && px_f <= right + tol && py_f >= top - tol && py_f <= bottom + tol
                } else {
                    let tol_sq = tol * tol;
                    dist_to_segment_sq(p_f, (left, top), (right, top)) <= tol_sq
                        || dist_to_segment_sq(p_f, (right, top), (right, bottom)) <= tol_sq
                        || dist_to_segment_sq(p_f, (right, bottom), (left, bottom)) <= tol_sq
                        || dist_to_segment_sq(p_f, (left, bottom), (left, top)) <= tol_sq
                }
            }
            Annotation::Ellipse { bounds, filled, width, .. } => {
                let norm = normalize_bounds(*bounds);
                let w = f32::from(norm.size.width);
                let h = f32::from(norm.size.height);
                let rx = w / 2.0;
                let ry = h / 2.0;
                if rx < 1.0 || ry < 1.0 {
                    return false;
                }
                let cx = f32::from(norm.origin.x) + rx;
                let cy = f32::from(norm.origin.y) + ry;
                let dx = (px_f - cx) / rx;
                let dy = (py_f - cy) / ry;
                let dist_norm = (dx * dx + dy * dy).sqrt();
                let tol = (tolerance + *width / 2.0) / rx.min(ry);

                if *filled || rx < 20.0 || ry < 20.0 {
                    dist_norm <= 1.0 + tol
                } else {
                    (dist_norm - 1.0).abs() <= tol
                }
            }
            Annotation::Triangle { bounds, filled, width, .. } => {
                let norm = normalize_bounds(*bounds);
                let ox = f32::from(norm.origin.x);
                let oy = f32::from(norm.origin.y);
                let w = f32::from(norm.size.width);
                let h = f32::from(norm.size.height);
                let p1 = (ox + w / 2.0, oy);
                let p2 = (ox + w, oy + h);
                let p3 = (ox, oy + h);
                let tol = tolerance + *width / 2.0;

                if *filled || (w < 32.0 && h < 32.0) {
                    point_in_triangle(p_f, p1, p2, p3)
                } else {
                    let tol_sq = tol * tol;
                    dist_to_segment_sq(p_f, p1, p2) <= tol_sq
                        || dist_to_segment_sq(p_f, p2, p3) <= tol_sq
                        || dist_to_segment_sq(p_f, p3, p1) <= tol_sq
                }
            }
            Annotation::Diamond { bounds, filled, width, .. } => {
                let norm = normalize_bounds(*bounds);
                let ox = f32::from(norm.origin.x);
                let oy = f32::from(norm.origin.y);
                let w = f32::from(norm.size.width);
                let h = f32::from(norm.size.height);
                let cx = ox + w / 2.0;
                let cy = oy + h / 2.0;
                let p1 = (cx, oy);
                let p2 = (ox + w, cy);
                let p3 = (cx, oy + h);
                let p4 = (ox, cy);
                let tol = tolerance + *width / 2.0;

                if *filled || (w < 32.0 && h < 32.0) {
                    (px_f - cx).abs() / (w / 2.0).max(1.0) + (py_f - cy).abs() / (h / 2.0).max(1.0) <= 1.0 + tolerance / 10.0
                } else {
                    let tol_sq = tol * tol;
                    dist_to_segment_sq(p_f, p1, p2) <= tol_sq
                        || dist_to_segment_sq(p_f, p2, p3) <= tol_sq
                        || dist_to_segment_sq(p_f, p3, p4) <= tol_sq
                        || dist_to_segment_sq(p_f, p4, p1) <= tol_sq
                }
            }
            Annotation::Star { bounds, .. } => {
                let norm = normalize_bounds(*bounds);
                let left = f32::from(norm.origin.x);
                let top = f32::from(norm.origin.y);
                let right = left + f32::from(norm.size.width);
                let bottom = top + f32::from(norm.size.height);
                px_f >= left - tolerance && px_f <= right + tolerance && py_f >= top - tolerance && py_f <= bottom + tolerance
            }
            Annotation::Text { origin, text, font_size, box_width, .. } => {
                let (w, h) = text_bounds_dimensions(text, *font_size, *box_width);
                let x = f32::from(origin.x);
                let y = f32::from(origin.y);
                px_f >= x - tolerance && px_f <= x + w + tolerance && py_f >= y - tolerance && py_f <= y + h + tolerance
            }
            Annotation::NumberBadge { center, radius, .. } => {
                let dx = px_f - f32::from(center.x);
                let dy = py_f - f32::from(center.y);
                let tol = radius + tolerance;
                dx * dx + dy * dy <= tol * tol
            }
            Annotation::Pencil { points, width, .. } | Annotation::Marker { points, width, .. } => {
                let tol = tolerance + *width;
                let tol_sq = tol * tol;
                for w in points.windows(2) {
                    let a_f = (f32::from(w[0].x), f32::from(w[0].y));
                    let b_f = (f32::from(w[1].x), f32::from(w[1].y));
                    if dist_to_segment_sq(p_f, a_f, b_f) <= tol_sq {
                        return true;
                    }
                }
                false
            }
            Annotation::Blur { bounds, .. }
            | Annotation::Pixelate { bounds, .. }
            | Annotation::Spotlight { bounds, .. } => {
                let norm = normalize_bounds(*bounds);
                let left = f32::from(norm.origin.x);
                let top = f32::from(norm.origin.y);
                let right = left + f32::from(norm.size.width);
                let bottom = top + f32::from(norm.size.height);
                px_f >= left - tolerance && px_f <= right + tolerance && py_f >= top - tolerance && py_f <= bottom + tolerance
            }
            Annotation::Sticker { center, size, .. } => {
                let half = size / 2.0;
                let cx = f32::from(center.x);
                let cy = f32::from(center.y);
                (px_f - cx).abs() <= half + tolerance && (py_f - cy).abs() <= half + tolerance
            }
        }
    }

    pub fn hit_test_handle(&self, p: Point<Pixels>, radius: Pixels) -> Option<AnnotationHandle> {
        let r_f32 = f32::from(radius);
        let r_sq = r_f32 * r_f32;
        match self {
            Annotation::Arrow { from, to, control, .. } => {
                if let Some(cp) = control {
                    let d_cp_sq = f32::from(p.x - cp.x).powi(2) + f32::from(p.y - cp.y).powi(2);
                    if d_cp_sq <= r_sq {
                        return Some(AnnotationHandle::BezierControl);
                    }
                }
                let d_from_sq = f32::from(p.x - from.x).powi(2) + f32::from(p.y - from.y).powi(2);
                let d_to_sq = f32::from(p.x - to.x).powi(2) + f32::from(p.y - to.y).powi(2);
                if d_from_sq <= r_sq && d_from_sq <= d_to_sq {
                    return Some(AnnotationHandle::Endpoint(true));
                }
                if d_to_sq <= r_sq {
                    return Some(AnnotationHandle::Endpoint(false));
                }
                None
            }
            Annotation::Line { from, to, .. } => {
                let d_from_sq = f32::from(p.x - from.x).powi(2) + f32::from(p.y - from.y).powi(2);
                let d_to_sq = f32::from(p.x - to.x).powi(2) + f32::from(p.y - to.y).powi(2);
                if d_from_sq <= r_sq && d_from_sq <= d_to_sq {
                    return Some(AnnotationHandle::Endpoint(true));
                }
                if d_to_sq <= r_sq {
                    return Some(AnnotationHandle::Endpoint(false));
                }
                None
            }
            Annotation::Rectangle { .. }
            | Annotation::Ellipse { .. }
            | Annotation::Triangle { .. }
            | Annotation::Star { .. }
            | Annotation::Diamond { .. }
            | Annotation::Blur { .. }
            | Annotation::Pixelate { .. }
            | Annotation::Spotlight { .. }
            | Annotation::Text { .. }
            | Annotation::NumberBadge { .. }
            | Annotation::Sticker { .. } => {
                let b = self.bounds();
                let x = b.origin.x;
                let y = b.origin.y;
                let w = b.size.width;
                let h = b.size.height;
                let mid_x = x + w / 2.0;
                let mid_y = y + h / 2.0;

                let handles = [
                    (AnnotationHandle::TopLeft, point(x, y)),
                    (AnnotationHandle::Top, point(mid_x, y)),
                    (AnnotationHandle::TopRight, point(x + w, y)),
                    (AnnotationHandle::Right, point(x + w, mid_y)),
                    (AnnotationHandle::BottomRight, point(x + w, y + h)),
                    (AnnotationHandle::Bottom, point(mid_x, y + h)),
                    (AnnotationHandle::BottomLeft, point(x, y + h)),
                    (AnnotationHandle::Left, point(x, mid_y)),
                ];

                let mut best_handle = None;
                let mut min_dist_sq = r_sq;
                for (handle, pt) in handles {
                    let dx = f32::from(p.x - pt.x);
                    let dy = f32::from(p.y - pt.y);
                    let d_sq = dx * dx + dy * dy;
                    if d_sq <= min_dist_sq {
                        min_dist_sq = d_sq;
                        best_handle = Some(handle);
                    }
                }
                best_handle
            }
            _ => None,
        }
    }

    pub fn translate(&mut self, delta: Point<Pixels>) {
        match self {
            Annotation::Pencil { points, .. } | Annotation::Marker { points, .. } => {
                for pt in points {
                    *pt = point(pt.x + delta.x, pt.y + delta.y);
                }
            }
            Annotation::Arrow { from, to, control, .. } => {
                *from = point(from.x + delta.x, from.y + delta.y);
                *to = point(to.x + delta.x, to.y + delta.y);
                if let Some(cp) = control {
                    *cp = point(cp.x + delta.x, cp.y + delta.y);
                }
            }
            Annotation::Line { from, to, .. } => {
                *from = point(from.x + delta.x, from.y + delta.y);
                *to = point(to.x + delta.x, to.y + delta.y);
            }
            Annotation::Rectangle { bounds, .. }
            | Annotation::Ellipse { bounds, .. }
            | Annotation::Triangle { bounds, .. }
            | Annotation::Star { bounds, .. }
            | Annotation::Diamond { bounds, .. }
            | Annotation::Blur { bounds, .. }
            | Annotation::Pixelate { bounds, .. }
            | Annotation::Spotlight { bounds, .. } => {
                bounds.origin = point(bounds.origin.x + delta.x, bounds.origin.y + delta.y);
            }
            Annotation::Text { origin, .. } => {
                *origin = point(origin.x + delta.x, origin.y + delta.y);
            }
            Annotation::NumberBadge { center, .. } => {
                *center = point(center.x + delta.x, center.y + delta.y);
            }
            Annotation::Sticker { center, .. } => {
                *center = point(center.x + delta.x, center.y + delta.y);
            }
        }
    }

    pub fn color(&self) -> Option<Rgba> {
        match self {
            Annotation::Pencil { color, .. }
            | Annotation::Marker { color, .. }
            | Annotation::Arrow { color, .. }
            | Annotation::Line { color, .. }
            | Annotation::Rectangle { color, .. }
            | Annotation::Ellipse { color, .. }
            | Annotation::Triangle { color, .. }
            | Annotation::Star { color, .. }
            | Annotation::Diamond { color, .. }
            | Annotation::Text { color, .. }
            | Annotation::Spotlight { color, .. }
            | Annotation::NumberBadge { color, .. } => Some(*color),
            Annotation::Sticker { color, .. } => *color,
            _ => None,
        }
    }

    pub fn set_color(&mut self, new_color: Rgba) {
        match self {
            Annotation::Pencil { color, .. }
            | Annotation::Marker { color, .. }
            | Annotation::Arrow { color, .. }
            | Annotation::Line { color, .. }
            | Annotation::Rectangle { color, .. }
            | Annotation::Ellipse { color, .. }
            | Annotation::Triangle { color, .. }
            | Annotation::Star { color, .. }
            | Annotation::Diamond { color, .. }
            | Annotation::Text { color, .. }
            | Annotation::Spotlight { color, .. }
            | Annotation::NumberBadge { color, .. } => *color = new_color,
            Annotation::Sticker { color, .. } => *color = Some(new_color),
            _ => {}
        }
    }

    pub fn stroke_width(&self) -> Option<f32> {
        match self {
            Annotation::Pencil { width, .. }
            | Annotation::Marker { width, .. }
            | Annotation::Arrow { width, .. }
            | Annotation::Line { width, .. }
            | Annotation::Rectangle { width, .. }
            | Annotation::Ellipse { width, .. }
            | Annotation::Triangle { width, .. }
            | Annotation::Star { width, .. }
            | Annotation::Diamond { width, .. }
            | Annotation::Spotlight { width, .. } => Some(*width),
            _ => None,
        }
    }

    pub fn set_stroke_width(&mut self, new_width: f32) {
        match self {
            Annotation::Pencil { width, .. }
            | Annotation::Marker { width, .. }
            | Annotation::Arrow { width, .. }
            | Annotation::Line { width, .. }
            | Annotation::Rectangle { width, .. }
            | Annotation::Ellipse { width, .. }
            | Annotation::Triangle { width, .. }
            | Annotation::Star { width, .. }
            | Annotation::Diamond { width, .. }
            | Annotation::Spotlight { width, .. } => *width = new_width,
            _ => {}
        }
    }

    pub fn font_size(&self) -> Option<f32> {
        match self {
            Annotation::Text { font_size, .. } => Some(*font_size),
            _ => None,
        }
    }

    pub fn set_font_size(&mut self, new_size: f32) {
        if let Annotation::Text { font_size, .. } = self {
            *font_size = new_size;
        }
    }

    pub fn box_width(&self) -> Option<f32> {
        match self {
            Annotation::Text { box_width, .. } => *box_width,
            _ => None,
        }
    }

    pub fn set_box_width(&mut self, new_width: Option<f32>) {
        if let Annotation::Text { box_width, .. } = self {
            *box_width = new_width;
        }
    }

    pub fn is_filled(&self) -> Option<bool> {
        match self {
            Annotation::Rectangle { filled, .. }
            | Annotation::Ellipse { filled, .. }
            | Annotation::Triangle { filled, .. }
            | Annotation::Star { filled, .. }
            | Annotation::Diamond { filled, .. }
            | Annotation::NumberBadge { filled, .. } => Some(*filled),
            _ => None,
        }
    }

    pub fn set_filled(&mut self, new_filled: bool) {
        match self {
            Annotation::Rectangle { filled, .. }
            | Annotation::Ellipse { filled, .. }
            | Annotation::Triangle { filled, .. }
            | Annotation::Star { filled, .. }
            | Annotation::Diamond { filled, .. }
            | Annotation::NumberBadge { filled, .. } => *filled = new_filled,
            _ => {}
        }
    }

    pub fn badge_shape(&self) -> Option<BadgeShape> {
        match self {
            Annotation::NumberBadge { shape, .. } => Some(*shape),
            _ => None,
        }
    }

    pub fn set_badge_shape(&mut self, new_shape: BadgeShape) {
        if let Annotation::NumberBadge { shape, .. } = self {
            *shape = new_shape;
        }
    }

    pub fn badge_mode(&self) -> Option<BadgeMode> {
        match self {
            Annotation::NumberBadge { mode, .. } => Some(*mode),
            _ => None,
        }
    }

    pub fn set_badge_mode(&mut self, new_mode: BadgeMode) {
        if let Annotation::NumberBadge { mode, .. } = self {
            *mode = new_mode;
        }
    }

    pub fn is_two_headed(&self) -> Option<bool> {
        match self {
            Annotation::Arrow { two_headed, .. } => Some(*two_headed),
            _ => None,
        }
    }

    pub fn set_two_headed(&mut self, th: bool) {
        if let Annotation::Arrow { two_headed, .. } = self {
            *two_headed = th;
        }
    }

    pub fn is_curved(&self) -> Option<bool> {
        match self {
            Annotation::Arrow { control, .. } => Some(control.is_some()),
            _ => None,
        }
    }

    pub fn set_curved(&mut self, curved: bool) {
        if let Annotation::Arrow { from, to, control, .. } = self {
            if curved {
                if control.is_none() {
                    let mx = (f32::from(from.x) + f32::from(to.x)) / 2.0;
                    let my = (f32::from(from.y) + f32::from(to.y)) / 2.0;
                    let dx = f32::from(to.x) - f32::from(from.x);
                    let dy = f32::from(to.y) - f32::from(from.y);
                    let len = (dx * dx + dy * dy).sqrt().max(1.0);
                    let nx = -dy / len;
                    let ny = dx / len;
                    let offset = (len * 0.25).max(30.0);
                    *control = Some(point(px(mx + nx * offset), px(my + ny * offset)));
                }
            } else {
                *control = None;
            }
        }
    }

    pub fn sticker_kind(&self) -> Option<StickerKind> {
        match self {
            Annotation::Sticker { kind, .. } => Some(*kind),
            _ => None,
        }
    }

    pub fn set_sticker_kind(&mut self, new_kind: StickerKind) {
        if let Annotation::Sticker { kind, .. } = self {
            *kind = new_kind;
        }
    }

    pub fn sticker_size(&self) -> Option<f32> {
        match self {
            Annotation::Sticker { size, .. } => Some(*size),
            _ => None,
        }
    }

    pub fn set_sticker_size(&mut self, new_size: f32) {
        if let Annotation::Sticker { size, .. } = self {
            *size = new_size;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArrowGeometry {
    pub shaft_from: (f32, f32),
    pub shaft_to: (f32, f32),
    pub tip: (f32, f32),
    pub left_wing: (f32, f32),
    pub right_wing: (f32, f32),
    pub has_head: bool,
}

pub fn calculate_arrow_geometry(
    from: (f32, f32),
    to: (f32, f32),
    width: f32,
) -> ArrowGeometry {
    let (x1, y1) = from;
    let (x2, y2) = to;
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();

    if len < 3.0 {
        return ArrowGeometry {
            shaft_from: from,
            shaft_to: to,
            tip: to,
            left_wing: to,
            right_wing: to,
            has_head: false,
        };
    }

    let ux = dx / len;
    let uy = dy / len;
    let nx = -uy;
    let ny = ux;

    // Head length scales smoothly with stroke width, clamped to at most 80% of total arrow length
    let base_head_len = width * 2.5 + 12.0;
    let head_len = base_head_len.min(len * 0.80);

    // Half width of the arrowhead base must always be wider than the stroke thickness
    // so the shaft never peeks through or exceeds the arrowhead width.
    let half_width = (head_len * 0.48).max(width * 0.95 + 4.0);

    // Tip is exactly at (x2, y2)
    let tip = (x2, y2);

    // Base center of the arrowhead
    let base_x = x2 - head_len * ux;
    let base_y = y2 - head_len * uy;

    let left_wing = (base_x + half_width * nx, base_y + half_width * ny);
    let right_wing = (base_x - half_width * nx, base_y - half_width * ny);

    // The shaft ends slightly inside the arrowhead base so there is no hairline gap,
    // but stops far enough from the tip that the shaft never pokes out.
    let overlap = (width * 0.4).min(head_len * 0.35);
    let shaft_to = (base_x + overlap * ux, base_y + overlap * uy);

    ArrowGeometry {
        shaft_from: from,
        shaft_to,
        tip,
        left_wing,
        right_wing,
        has_head: true,
    }
}
