use gpui::{
    Bounds, IntoElement, Pixels, Point, Rgba, div, prelude::*, px, rgb, rgba,
};
use image::RgbaImage;

#[derive(Clone, Copy, Debug)]
pub struct MagnifierInfo {
    pub screen_x: i32,
    pub screen_y: i32,
    pub color: Rgba,
    pub hex: [char; 7],
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl MagnifierInfo {
    pub fn from_image(image: &RgbaImage, screen_x: i32, screen_y: i32, img_x: u32, img_y: u32) -> Self {
        let p = if img_x < image.width() && img_y < image.height() {
            *image.get_pixel(img_x, img_y)
        } else {
            image::Rgba([0, 0, 0, 255])
        };

        let r = p[0];
        let g = p[1];
        let b = p[2];

        let hex_str = format!("#{:02X}{:02X}{:02X}", r, g, b);
        let mut hex = ['#', '0', '0', '0', '0', '0', '0'];
        for (i, c) in hex_str.chars().take(7).enumerate() {
            hex[i] = c;
        }

        Self {
            screen_x,
            screen_y,
            color: rgb(((r as u32) << 16) | ((g as u32) << 8) | (b as u32)),
            hex,
            r,
            g,
            b,
        }
    }

    pub fn hex_string(&self) -> String {
        self.hex.iter().collect()
    }
}

pub fn render_magnifier(
    cursor: Point<Pixels>,
    info: MagnifierInfo,
    screen_bounds: Bounds<Pixels>,
) -> impl IntoElement {
    let mag_w = 140.0;
    let mag_h = 100.0;
    let offset = 20.0;

    let cur_x = f32::from(cursor.x);
    let cur_y = f32::from(cursor.y);
    let scr_w = f32::from(screen_bounds.size.width);
    let scr_h = f32::from(screen_bounds.size.height);

    let mut x = cur_x + offset;
    let mut y = cur_y + offset;

    if x + mag_w > scr_w - 10.0 {
        x = cur_x - mag_w - offset;
    }
    if y + mag_h > scr_h - 10.0 {
        y = cur_y - mag_h - offset;
    }

    let hex_label = info.hex_string();
    let rgb_label = format!("RGB({}, {}, {})", info.r, info.g, info.b);
    let pos_label = format!("X: {}  Y: {}", info.screen_x, info.screen_y);

    div()
        .absolute()
        .left(px(x))
        .top(px(y))
        .w(px(mag_w))
        .h(px(mag_h))
        .bg(rgb(0x151519))
        .border_1()
        .border_color(rgb(0x2E2E38))
        .rounded_lg()
        .shadow_lg()
        .p_2()
        .flex()
        .flex_col()
        .gap_1p5()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .size_5()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgba(0xFFFFFF40))
                        .bg(info.color),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(0xF2F2F5))
                        .child(hex_label),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0xA2A2B0))
                .child(rgb_label),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x656575))
                .child(pos_label),
        )
}
