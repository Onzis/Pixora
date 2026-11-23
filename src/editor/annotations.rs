use gpui::{Bounds, Pixels, Point, Rgba, point, px, size};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnotationHandle {
    Endpoint(bool), // true = start (from), false = end (to)
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
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
        ' ' => 0.28,
        'i' | 'j' | 'l' | '!' | '|' | ':' | ';' | '\'' | '.' | ',' => 0.26,
        'f' | 'r' | 't' | '(' | ')' | '[' | ']' => 0.35,
        'm' | 'w' | 'M' | 'W' | '—' | 'Ж' | 'Ш' | 'Щ' | 'Ю' => 0.85,
        'A'..='Z' | 'А'..='Я' => 0.65,
        _ => 0.52,
    }
}

pub fn string_width(s: &str, font_size: f32) -> f32 {
    let mut total = 0.0f32;
    for ch in s.chars() {
        total += char_width(ch);
    }
    total * font_size
}

pub fn text_bounds_dimensions(text: &str, font_size: f32) -> (f32, f32) {
    let lines: Vec<&str> = text.lines().collect();
    let mut max_line_w = 0.0f32;
    for line in &lines {
        let lw = string_width(line, font_size);
        if lw > max_line_w {
            max_line_w = lw;
        }
    }
    let w = max_line_w.max(font_size * 0.5);
    let line_count = lines.len().max(1) as f32;
    let h = line_count * font_size * 1.25;
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
            Annotation::Arrow { from, to, width, .. } | Annotation::Line { from, to, width, .. } => {
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
            Annotation::Text { origin, text, font_size, .. } => {
                let (w, h) = text_bounds_dimensions(text, *font_size);
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
            Annotation::Text { origin, font_size, text, .. } => {
                let (orig_w, orig_h) = text_bounds_dimensions(text, *font_size);
                let new_w = f32::from(norm.size.width);
                let new_h = f32::from(norm.size.height);
                let rw = new_w / orig_w.max(1.0);
                let rh = new_h / orig_h.max(1.0);
                let ratio = if (rw - 1.0).abs() >= (rh - 1.0).abs() { rw } else { rh };
                if ratio > 0.05 && ratio < 20.0 {
                    *font_size = (*font_size * ratio).clamp(10.0, 96.0);
                }
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
            Annotation::Text { origin, font_size, .. } => {
                let orig_fs = *font_size;
                match handle {
                    AnnotationHandle::Right => {
                        let ratio = ((ow + dx) / ow.max(1.0)).clamp(0.1, 8.0);
                        *font_size = (orig_fs * ratio).clamp(10.0, 96.0);
                        *origin = point(px(ox), px(oy));
                    }
                    AnnotationHandle::Left => {
                        let ratio = ((ow - dx) / ow.max(1.0)).clamp(0.1, 8.0);
                        let target_fs = (orig_fs * ratio).clamp(10.0, 96.0);
                        let actual_ratio = target_fs / orig_fs;
                        let new_x = (ox + ow) - ow * actual_ratio;
                        *font_size = target_fs;
                        *origin = point(px(new_x), px(oy));
                    }
                    AnnotationHandle::Bottom => {
                        let ratio = ((oh + dy) / oh.max(1.0)).clamp(0.1, 8.0);
                        *font_size = (orig_fs * ratio).clamp(10.0, 96.0);
                        *origin = point(px(ox), px(oy));
                    }
                    AnnotationHandle::Top => {
                        let ratio = ((oh - dy) / oh.max(1.0)).clamp(0.1, 8.0);
                        let target_fs = (orig_fs * ratio).clamp(10.0, 96.0);
                        let actual_ratio = target_fs / orig_fs;
                        let new_y = (oy + oh) - oh * actual_ratio;
                        *font_size = target_fs;
                        *origin = point(px(ox), px(new_y));
                    }
                    AnnotationHandle::BottomRight => {
                        let ratio_w = (ow + dx) / ow.max(1.0);
                        let ratio_h = (oh + dy) / oh.max(1.0);
                        let ratio = if dx.abs() >= dy.abs() { ratio_w } else { ratio_h }.clamp(0.1, 8.0);
                        *font_size = (orig_fs * ratio).clamp(10.0, 96.0);
                        *origin = point(px(ox), px(oy));
                    }
                    AnnotationHandle::BottomLeft => {
                        let ratio_w = (ow - dx) / ow.max(1.0);
                        let ratio_h = (oh + dy) / oh.max(1.0);
                        let ratio = if dx.abs() >= dy.abs() { ratio_w } else { ratio_h }.clamp(0.1, 8.0);
                        let target_fs = (orig_fs * ratio).clamp(10.0, 96.0);
                        let actual_ratio = target_fs / orig_fs;
                        let new_x = (ox + ow) - ow * actual_ratio;
                        *font_size = target_fs;
                        *origin = point(px(new_x), px(oy));
                    }
                    AnnotationHandle::TopRight => {
                        let ratio_w = (ow + dx) / ow.max(1.0);
                        let ratio_h = (oh - dy) / oh.max(1.0);
                        let ratio = if dx.abs() >= dy.abs() { ratio_w } else { ratio_h }.clamp(0.1, 8.0);
                        let target_fs = (orig_fs * ratio).clamp(10.0, 96.0);
                        let actual_ratio = target_fs / orig_fs;
                        let new_y = (oy + oh) - oh * actual_ratio;
                        *font_size = target_fs;
                        *origin = point(px(ox), px(new_y));
                    }
                    AnnotationHandle::TopLeft => {
                        let ratio_w = (ow - dx) / ow.max(1.0);
                        let ratio_h = (oh - dy) / oh.max(1.0);
                        let ratio = if dx.abs() >= dy.abs() { ratio_w } else { ratio_h }.clamp(0.1, 8.0);
                        let target_fs = (orig_fs * ratio).clamp(10.0, 96.0);
                        let actual_ratio = target_fs / orig_fs;
                        let new_x = (ox + ow) - ow * actual_ratio;
                        let new_y = (oy + oh) - oh * actual_ratio;
                        *font_size = target_fs;
                        *origin = point(px(new_x), px(new_y));
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
            Annotation::Arrow { from, to, width, .. } => {
                let a_f = (f32::from(from.x), f32::from(from.y));
                let b_f = (f32::from(to.x), f32::from(to.y));
                let tol = tolerance + *width / 2.0;
                let tol_sq = tol * tol;
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
                false
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
            Annotation::Text { origin, text, font_size, .. } => {
                let (w, h) = text_bounds_dimensions(text, *font_size);
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
        }
    }

    pub fn hit_test_handle(&self, p: Point<Pixels>, radius: Pixels) -> Option<AnnotationHandle> {
        let r_f32 = f32::from(radius);
        let r_sq = r_f32 * r_f32;
        match self {
            Annotation::Arrow { from, to, .. } | Annotation::Line { from, to, .. } => {
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
            | Annotation::NumberBadge { .. } => {
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
            Annotation::Arrow { from, to, .. } | Annotation::Line { from, to, .. } => {
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

    pub fn is_filled(&self) -> Option<bool> {
        match self {
            Annotation::Rectangle { filled, .. }
            | Annotation::Ellipse { filled, .. }
            | Annotation::Triangle { filled, .. }
            | Annotation::Star { filled, .. }
            | Annotation::Diamond { filled, .. } => Some(*filled),
            _ => None,
        }
    }

    pub fn set_filled(&mut self, new_filled: bool) {
        match self {
            Annotation::Rectangle { filled, .. }
            | Annotation::Ellipse { filled, .. }
            | Annotation::Triangle { filled, .. }
            | Annotation::Star { filled, .. }
            | Annotation::Diamond { filled, .. } => *filled = new_filled,
            _ => {}
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
