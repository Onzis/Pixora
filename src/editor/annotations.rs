use gpui::{Bounds, Pixels, Point, Rgba};

#[derive(Clone, Debug)]
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
    NumberBadge {
        center: Point<Pixels>,
        number: u32,
        color: Rgba,
        radius: f32,
    },
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
