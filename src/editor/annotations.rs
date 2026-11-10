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
