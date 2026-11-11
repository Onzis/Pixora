use gpui::{
    BorderStyle, Bounds, IntoElement, Pixels, Point, Rgba, canvas, div, fill, outline, point,
    prelude::*, px, rgb, rgba, size,
};
use image::RgbaImage;

use crate::i18n::{AppLanguage, t};

pub const LOUPE_RADIUS: i32 = 5;
pub const LOUPE_GRID_SIZE: usize = 11;
pub const LOUPE_TOTAL_CELLS: usize = LOUPE_GRID_SIZE * LOUPE_GRID_SIZE;
pub const LOUPE_CELL_SIZE: f32 = 12.0;
pub const LOUPE_GRID_PX: f32 = (LOUPE_GRID_SIZE as f32) * LOUPE_CELL_SIZE; // 132.0 px

#[derive(Clone, Copy, Debug)]
pub struct MagnifierInfo {
    pub screen_x: i32,
    pub screen_y: i32,
    pub color: Rgba,
    pub hex: [char; 7],
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub grid_colors: [Rgba; LOUPE_TOTAL_CELLS],
}

impl MagnifierInfo {
    pub fn from_image(
        image: &RgbaImage,
        screen_x: i32,
        screen_y: i32,
        img_x: u32,
        img_y: u32,
    ) -> Self {
        let w = image.width() as i32;
        let h = image.height() as i32;
        let mut grid_colors = [rgba(0x000000FF); LOUPE_TOTAL_CELLS];

        for dy in -LOUPE_RADIUS..=LOUPE_RADIUS {
            for dx in -LOUPE_RADIUS..=LOUPE_RADIUS {
                let sx = (img_x as i32 + dx).clamp(0, (w - 1).max(0)) as u32;
                let sy = (img_y as i32 + dy).clamp(0, (h - 1).max(0)) as u32;
                let p = if sx < image.width() && sy < image.height() {
                    *image.get_pixel(sx, sy)
                } else {
                    image::Rgba([0, 0, 0, 255])
                };
                let c = rgb(((p[0] as u32) << 16) | ((p[1] as u32) << 8) | (p[2] as u32));
                let idx = ((dy + LOUPE_RADIUS) * (LOUPE_GRID_SIZE as i32) + (dx + LOUPE_RADIUS)) as usize;
                grid_colors[idx] = c;
            }
        }

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
            grid_colors,
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
    is_eyedropper: bool,
    lang: AppLanguage,
) -> impl IntoElement {
    let padding = 8.0;
    let mag_w = LOUPE_GRID_PX + padding * 2.0; // 148.0
    let mag_h = LOUPE_GRID_PX + padding * 2.0 + 44.0; // 192.0
    let offset = 22.0;

    let cur_x = f32::from(cursor.x);
    let cur_y = f32::from(cursor.y);
    let scr_w = f32::from(screen_bounds.size.width);
    let scr_h = f32::from(screen_bounds.size.height);

    let mut x = cur_x + offset;
    let mut y = cur_y + offset;

    if x + mag_w > scr_w - 12.0 {
        x = cur_x - mag_w - offset;
    }
    if y + mag_h > scr_h - 12.0 {
        y = cur_y - mag_h - offset;
    }

    x = x.clamp(8.0, (scr_w - mag_w - 8.0).max(8.0));
    y = y.clamp(8.0, (scr_h - mag_h - 8.0).max(8.0));

    let hex_label = info.hex_string();
    let pos_label = format!("X: {}  Y: {}", info.screen_x, info.screen_y);

    div()
        .absolute()
        .left(px(x))
        .top(px(y))
        .w(px(mag_w))
        .h(px(mag_h))
        .bg(rgba(0x131317FA))
        .border_1()
        .border_color(rgba(0xFFFFFF28))
        .rounded_xl()
        .shadow_2xl()
        .p(px(padding))
        .flex()
        .flex_col()
        .items_center()
        .gap_1p5()
        // 1. Magnified Pixel Grid (Canvas)
        .child(
            div()
                .w(px(LOUPE_GRID_PX))
                .h(px(LOUPE_GRID_PX))
                .rounded_md()
                .border_1()
                .border_color(rgba(0xFFFFFF35))
                .child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, _, window, _cx| {
                            let cell_sz = px(LOUPE_CELL_SIZE);
                            let origin_x = bounds.origin.x;
                            let origin_y = bounds.origin.y;

                            // Draw each magnified pixel cell
                            for row in 0..LOUPE_GRID_SIZE {
                                let y_pos = origin_y + px(row as f32 * LOUPE_CELL_SIZE);
                                for col in 0..LOUPE_GRID_SIZE {
                                    let x_pos = origin_x + px(col as f32 * LOUPE_CELL_SIZE);
                                    let cell_bounds = Bounds {
                                        origin: point(x_pos, y_pos),
                                        size: size(cell_sz, cell_sz),
                                    };
                                    let color = info.grid_colors[row * LOUPE_GRID_SIZE + col];
                                    window.paint_quad(fill(cell_bounds, color));
                                    window.paint_quad(outline(
                                        cell_bounds,
                                        rgba(0x00000028),
                                        BorderStyle::Solid,
                                    ));
                                }
                            }

                            // Center target reticle
                            let center_col = LOUPE_RADIUS as f32;
                            let center_row = LOUPE_RADIUS as f32;
                            let cx_left = origin_x + px(center_col * LOUPE_CELL_SIZE);
                            let cy_top = origin_y + px(center_row * LOUPE_CELL_SIZE);

                            // Crosshair lines through center
                            window.paint_quad(fill(
                                Bounds {
                                    origin: point(cx_left + px(5.5), origin_y),
                                    size: size(px(1.0), px(center_row * LOUPE_CELL_SIZE)),
                                },
                                rgba(0xFFFFFF38),
                            ));
                            window.paint_quad(fill(
                                Bounds {
                                    origin: point(cx_left + px(5.5), cy_top + px(LOUPE_CELL_SIZE)),
                                    size: size(px(1.0), px(center_row * LOUPE_CELL_SIZE)),
                                },
                                rgba(0xFFFFFF38),
                            ));
                            window.paint_quad(fill(
                                Bounds {
                                    origin: point(origin_x, cy_top + px(5.5)),
                                    size: size(px(center_col * LOUPE_CELL_SIZE), px(1.0)),
                                },
                                rgba(0xFFFFFF38),
                            ));
                            window.paint_quad(fill(
                                Bounds {
                                    origin: point(cx_left + px(LOUPE_CELL_SIZE), cy_top + px(5.5)),
                                    size: size(px(center_col * LOUPE_CELL_SIZE), px(1.0)),
                                },
                                rgba(0xFFFFFF38),
                            ));

                            let center_bounds = Bounds {
                                origin: point(cx_left, cy_top),
                                size: size(cell_sz, cell_sz),
                            };

                            let outer_center = Bounds {
                                origin: point(cx_left - px(1.0), cy_top - px(1.0)),
                                size: size(cell_sz + px(2.0), cell_sz + px(2.0)),
                            };

                            // Double outline for visibility on any background (light or dark)
                            window.paint_quad(outline(
                                outer_center,
                                rgba(0x000000B8),
                                BorderStyle::Solid,
                            ));
                            window.paint_quad(outline(
                                center_bounds,
                                rgb(0xFFFFFF),
                                BorderStyle::Solid,
                            ));
                        },
                    )
                    .size_full(),
                ),
        )
        // 2. Info readout: Color swatch, HEX, RGB, and coordinates
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1p5()
                                .child(
                                    div()
                                        .w(px(14.0))
                                        .h(px(14.0))
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(rgba(0xFFFFFF50))
                                        .bg(info.color),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(rgb(0xF2F2F5))
                                        .child(hex_label),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(rgb(0xA2A2B0))
                                .child(format!("{},{},{}", info.r, info.g, info.b)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x757585))
                                .child(pos_label),
                        )
                        .when(is_eyedropper, |d| {
                            d.child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(rgb(0x9D8CFF))
                                    .child(t(lang, "Pick", "Клик")),
                            )
                        }),
                ),
        )
}
