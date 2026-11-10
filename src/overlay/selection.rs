use gpui::{Bounds, Pixels, Point, point, px, size};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeHandle {
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
pub enum SelectionDragMode {
    None,
    Creating { start: Point<Pixels> },
    Moving { start_mouse: Point<Pixels>, orig_origin: Point<Pixels> },
    Resizing {
        handle: ResizeHandle,
        start_mouse: Point<Pixels>,
        orig_bounds: Bounds<Pixels>,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SelectionState {
    pub bounds: Bounds<Pixels>,
    pub is_active: bool,
}

impl SelectionState {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        let (norm_x, norm_w) = if w < 0.0 { (x + w, -w) } else { (x, w) };
        let (norm_y, norm_h) = if h < 0.0 { (y + h, -h) } else { (y, h) };
        Self {
            bounds: Bounds {
                origin: point(px(norm_x), px(norm_y)),
                size: size(px(norm_w), px(norm_h)),
            },
            is_active: norm_w > 8.0 && norm_h > 8.0,
        }
    }

    pub fn width(&self) -> f32 {
        f32::from(self.bounds.size.width)
    }

    pub fn height(&self) -> f32 {
        f32::from(self.bounds.size.height)
    }

    pub fn x(&self) -> f32 {
        f32::from(self.bounds.origin.x)
    }

    pub fn y(&self) -> f32 {
        f32::from(self.bounds.origin.y)
    }

    pub fn contains(&self, p: Point<Pixels>) -> bool {
        if !self.is_active {
            return false;
        }
        p.x >= self.bounds.origin.x
            && p.x <= self.bounds.origin.x + self.bounds.size.width
            && p.y >= self.bounds.origin.y
            && p.y <= self.bounds.origin.y + self.bounds.size.height
    }

    pub fn hit_test_handle(&self, p: Point<Pixels>, radius: Pixels) -> Option<ResizeHandle> {
        if !self.is_active {
            return None;
        }

        let x = self.bounds.origin.x;
        let y = self.bounds.origin.y;
        let w = self.bounds.size.width;
        let h = self.bounds.size.height;
        let mid_x = x + w / 2.0;
        let mid_y = y + h / 2.0;

        let handles = [
            (ResizeHandle::TopLeft, point(x, y)),
            (ResizeHandle::Top, point(mid_x, y)),
            (ResizeHandle::TopRight, point(x + w, y)),
            (ResizeHandle::Right, point(x + w, mid_y)),
            (ResizeHandle::BottomRight, point(x + w, y + h)),
            (ResizeHandle::Bottom, point(mid_x, y + h)),
            (ResizeHandle::BottomLeft, point(x, y + h)),
            (ResizeHandle::Left, point(x, mid_y)),
        ];

        let r_f32 = f32::from(radius);
        for (handle, pt) in handles {
            let dx = f32::from(p.x - pt.x).abs();
            let dy = f32::from(p.y - pt.y).abs();
            if dx <= r_f32 && dy <= r_f32 {
                return Some(handle);
            }
        }

        None
    }

    pub fn update_resize(
        &mut self,
        handle: ResizeHandle,
        orig_bounds: Bounds<Pixels>,
        delta: Point<Pixels>,
    ) {
        let mut x = f32::from(orig_bounds.origin.x);
        let mut y = f32::from(orig_bounds.origin.y);
        let mut w = f32::from(orig_bounds.size.width);
        let mut h = f32::from(orig_bounds.size.height);
        let dx = f32::from(delta.x);
        let dy = f32::from(delta.y);

        match handle {
            ResizeHandle::TopLeft => {
                x += dx;
                y += dy;
                w -= dx;
                h -= dy;
            }
            ResizeHandle::Top => {
                y += dy;
                h -= dy;
            }
            ResizeHandle::TopRight => {
                y += dy;
                w += dx;
                h -= dy;
            }
            ResizeHandle::Right => {
                w += dx;
            }
            ResizeHandle::BottomRight => {
                w += dx;
                h += dy;
            }
            ResizeHandle::Bottom => {
                h += dy;
            }
            ResizeHandle::BottomLeft => {
                x += dx;
                w -= dx;
                h += dy;
            }
            ResizeHandle::Left => {
                x += dx;
                w -= dx;
            }
        }

        *self = Self::new(x, y, w, h);
    }
}
