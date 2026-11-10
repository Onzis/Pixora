use std::sync::Arc;
use crate::theme::PixoraTheme;
use gpui::{
    App, Bounds, ClickEvent, Context, ElementId, IntoElement, Pixels, Point, Render, RenderImage,
    Rgba, Window, div, point, prelude::*, px, rgb, rgba, svg,
};
use image::{Frame, RgbaImage};
use smallvec::smallvec;

pub const TOOLBAR_WIDTH: f32 = 750.0;
pub const TOOLBAR_MAIN_HEIGHT: f32 = 46.0;
pub const TOOLBAR_PROPS_HEIGHT: f32 = 46.0;
pub const TOOLBAR_TOTAL_HEIGHT: f32 = 98.0;

pub const PALETTE_24: [(&str, u32); 24] = [
    ("Red", 0xFF3B30),
    ("Coral", 0xFF6B4A),
    ("Orange", 0xFF9500),
    ("Gold", 0xFFCC00),
    ("Yellow", 0xFFE600),
    ("Lime", 0x8EE000),
    ("Green", 0x34C759),
    ("Mint", 0x00C7BE),
    ("Teal", 0x30B0C7),
    ("Cyan", 0x32ADE6),
    ("Sky", 0x64D2FF),
    ("Blue", 0x007AFF),
    ("Indigo", 0x5856D6),
    ("Purple", 0x8174FF),
    ("Violet", 0xAF52DE),
    ("Magenta", 0xE040FB),
    ("Pink", 0xFF2D55),
    ("Rose", 0xFF6482),
    ("White", 0xFFFFFF),
    ("Silver", 0xD1D1D6),
    ("Gray", 0x8E8E93),
    ("Slate", 0x48484A),
    ("Charcoal", 0x1C1C1E),
    ("Black", 0x000000),
];

pub struct TooltipView {
    pub text: String,
}

impl Render for TooltipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .mt(px(24.0))
            .px_2p5()
            .py_1()
            .rounded_md()
            .bg(rgb(0x18181E))
            .border_1()
            .border_color(rgb(0x323242))
            .shadow_xl()
            .text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(rgb(0xF2F2F5))
            .child(self.text.clone())
    }
}

pub const HUE_STOPS: [f32; 24] = [
    0.0, 15.0, 30.0, 45.0, 60.0, 75.0, 90.0, 105.0, 120.0, 135.0, 150.0, 165.0,
    180.0, 195.0, 210.0, 225.0, 240.0, 255.0, 270.0, 285.0, 300.0, 315.0, 330.0, 345.0,
];

pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let h_norm = (h % 360.0 + 360.0) % 360.0;
    let h_prime = h_norm / 60.0;
    let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());
    let m = v - c;

    let (r1, g1, b1) = if (0.0..1.0).contains(&h_prime) {
        (c, x, 0.0)
    } else if (1.0..2.0).contains(&h_prime) {
        (x, c, 0.0)
    } else if (2.0..3.0).contains(&h_prime) {
        (0.0, c, x)
    } else if (3.0..4.0).contains(&h_prime) {
        (0.0, x, c)
    } else if (4.0..5.0).contains(&h_prime) {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (
        ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r_f = r as f32 / 255.0;
    let g_f = g as f32 / 255.0;
    let b_f = b as f32 / 255.0;

    let max = r_f.max(g_f).max(b_f);
    let min = r_f.min(g_f).min(b_f);
    let delta = max - min;

    let h = if delta == 0.0 {
        0.0
    } else if (max - r_f).abs() < f32::EPSILON {
        60.0 * (((g_f - b_f) / delta) % 6.0)
    } else if (max - g_f).abs() < f32::EPSILON {
        60.0 * (((b_f - r_f) / delta) + 2.0)
    } else {
        60.0 * (((r_f - g_f) / delta) + 4.0)
    };
    let h = if h < 0.0 { h + 360.0 } else { h };
    let s = if max == 0.0 { 0.0 } else { delta / max };
    let v = max;

    (h, s, v)
}

pub fn parse_hex_color(hex: &str) -> Option<Rgba> {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() == 6 {
        let r = u8::from_str_radix(&clean[0..2], 16).ok()?;
        let g = u8::from_str_radix(&clean[2..4], 16).ok()?;
        let b = u8::from_str_radix(&clean[4..6], 16).ok()?;
        Some(rgb(((r as u32) << 16) | ((g as u32) << 8) | (b as u32)))
    } else {
        None
    }
}

pub fn calculate_toolbar_position(
    selection: Bounds<Pixels>,
    screen: Bounds<Pixels>,
    bar_width: f32,
    bar_height: f32,
) -> Point<Pixels> {
    let margin = 10.0;
    let sel_x = f32::from(selection.origin.x);
    let sel_y = f32::from(selection.origin.y);
    let sel_w = f32::from(selection.size.width);
    let sel_h = f32::from(selection.size.height);
    let scr_w = f32::from(screen.size.width);
    let scr_h = f32::from(screen.size.height);

    // Center toolbar horizontally relative to the selection
    let mut x = sel_x + (sel_w - bar_width) / 2.0;

    // Strict clamp to screen boundaries with 12px margin
    let min_x = 12.0;
    let max_x = (scr_w - bar_width - 12.0).max(min_x);
    x = x.clamp(min_x, max_x);

    // Vertical placement logic:
    // 1) Primary: Place below selection
    let mut y = sel_y + sel_h + margin;
    if y + bar_height > scr_h - 12.0 {
        // 2) If not enough room below, try placing above selection
        if sel_y - bar_height - margin >= 12.0 {
            y = sel_y - bar_height - margin;
        } else {
            // 3) Neither fits outside (selection covers most vertical space)
            // Place inside selection at the bottom
            if sel_h >= bar_height + 24.0 {
                y = (sel_y + sel_h - bar_height - 14.0).clamp(12.0, (scr_h - bar_height - 12.0).max(12.0));
            } else {
                y = (sel_y + 12.0).clamp(12.0, (scr_h - bar_height - 12.0).max(12.0));
            }
        }
    }

    point(px(x), px(y))
}

pub fn render_toolbar_button(
    id: ElementId,
    icon_path: &'static str,
    tooltip: &'static str,
    is_active: bool,
    theme: &PixoraTheme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let bg_color = if is_active {
        rgba(0x8174FF33)
    } else {
        theme.bg_secondary
    };

    let text_color = if is_active {
        theme.accent
    } else {
        theme.text_secondary
    };

    let border_color = if is_active {
        theme.accent
    } else {
        theme.border_subtle
    };

    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(32.0))
        .rounded_lg()
        .bg(bg_color)
        .border_1()
        .border_color(border_color)
        .hover(|s| s.bg(theme.bg_hover).border_color(theme.border_focus))
        .cursor_pointer()
        .tooltip(move |_window, cx| {
            cx.new(|_| TooltipView {
                text: tooltip.to_string(),
            })
            .into()
        })
        .on_click(on_click)
        .child(
            svg()
                .path(icon_path)
                .size(px(16.0))
                .text_color(text_color),
        )
}

pub fn render_toolbar_text_button(
    id: ElementId,
    label: &'static str,
    tooltip: &'static str,
    is_active: bool,
    theme: &PixoraTheme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let bg_color = if is_active {
        rgba(0x8174FF33)
    } else {
        theme.bg_secondary
    };

    let text_color = if is_active {
        theme.accent
    } else {
        theme.text_secondary
    };

    let border_color = if is_active {
        theme.accent
    } else {
        theme.border_subtle
    };

    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(px(32.0))
        .px_2()
        .rounded_lg()
        .bg(bg_color)
        .border_1()
        .border_color(border_color)
        .hover(|s| s.bg(theme.bg_hover).border_color(theme.border_focus))
        .cursor_pointer()
        .tooltip(move |_window, cx| {
            cx.new(|_| TooltipView {
                text: tooltip.to_string(),
            })
            .into()
        })
        .on_click(on_click)
        .child(
            div()
                .text_xs()
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(text_color)
                .child(label),
        )
}

pub fn render_toolbar_divider(theme: &PixoraTheme) -> impl IntoElement {
    div()
        .w(px(1.0))
        .h(px(18.0))
        .bg(theme.border_subtle)
        .mx_0p5()
}

pub fn render_color_swatch(
    id: ElementId,
    color: Rgba,
    is_selected: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let border = if is_selected {
        rgb(0xFFFFFF)
    } else {
        rgba(0xFFFFFF22)
    };

    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(22.0))
        .rounded_full()
        .cursor_pointer()
        .on_click(on_click)
        .child(
            div()
                .size(px(18.0))
                .rounded_full()
                .bg(color)
                .border_2()
                .border_color(border)
                .hover(|s| s.border_color(rgb(0xFFFFFF))),
        )
}

pub fn render_blur_preset(
    id: ElementId,
    label: &'static str,
    is_active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let bg = if is_active {
        rgba(0x8174FF33)
    } else {
        rgb(0x1C1C22)
    };
    let text_c = if is_active {
        rgb(0x8174FF)
    } else {
        rgb(0xA2A2B0)
    };
    let border = if is_active {
        rgb(0x8174FF)
    } else {
        rgb(0x282832)
    };

    div()
        .id(id)
        .px_2()
        .py_0p5()
        .rounded_md()
        .bg(bg)
        .border_1()
        .border_color(border)
        .hover(|s| s.bg(rgb(0x282838)))
        .cursor_pointer()
        .on_click(on_click)
        .child(
            div()
                .text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(text_c)
                .child(label),
        )
}

pub fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r_f = r as f32 / 255.0;
    let g_f = g as f32 / 255.0;
    let b_f = b as f32 / 255.0;

    let max = r_f.max(g_f).max(b_f);
    let min = r_f.min(g_f).min(b_f);
    let delta = max - min;

    let l = (max + min) / 2.0;

    let h = if delta == 0.0 {
        0.0
    } else if (max - r_f).abs() < f32::EPSILON {
        60.0 * (((g_f - b_f) / delta) % 6.0)
    } else if (max - g_f).abs() < f32::EPSILON {
        60.0 * (((b_f - r_f) / delta) + 2.0)
    } else {
        60.0 * (((r_f - g_f) / delta) + 4.0)
    };
    let h = if h < 0.0 { h + 360.0 } else { h };

    let s = if delta == 0.0 {
        0.0
    } else if l <= 0.5 {
        delta / (max + min)
    } else {
        delta / (2.0 - max - min)
    };

    (h, s, l)
}

pub fn rgb_to_cmyk(r: u8, g: u8, b: u8) -> (u8, u8, u8, u8) {
    let r_f = r as f32 / 255.0;
    let g_f = g as f32 / 255.0;
    let b_f = b as f32 / 255.0;

    let k = 1.0 - r_f.max(g_f).max(b_f);
    if (k - 1.0).abs() < f32::EPSILON {
        return (0, 0, 0, 100);
    }
    let c = ((1.0 - r_f - k) / (1.0 - k) * 100.0).round().clamp(0.0, 100.0) as u8;
    let m = ((1.0 - g_f - k) / (1.0 - k) * 100.0).round().clamp(0.0, 100.0) as u8;
    let y = ((1.0 - b_f - k) / (1.0 - k) * 100.0).round().clamp(0.0, 100.0) as u8;
    let k_pct = (k * 100.0).round().clamp(0.0, 100.0) as u8;

    (c, m, y, k_pct)
}

pub fn generate_gradient_bitmap(hue: f32, width: u32, height: u32) -> Arc<RenderImage> {
    let mut img = RgbaImage::new(width, height);
    for y in 0..height {
        let v = 1.0 - (y as f32 / (height - 1).max(1) as f32);
        for x in 0..width {
            let s = x as f32 / (width - 1).max(1) as f32;
            let (r, g, b) = hsv_to_rgb(hue, s, v);
            // GPUI RenderImage expects BGRA
            img.put_pixel(x, y, image::Rgba([b, g, r, 255]));
        }
    }
    let frame = Frame::new(img);
    Arc::new(RenderImage::new(smallvec![frame]))
}

pub fn generate_hue_strip_bitmap(width: u32, height: u32) -> Arc<RenderImage> {
    let mut img = RgbaImage::new(width, height);
    for x in 0..width {
        let h = (x as f32 / (width - 1).max(1) as f32) * 360.0;
        let (r, g, b) = hsv_to_rgb(h, 1.0, 1.0);
        for y in 0..height {
            img.put_pixel(x, y, image::Rgba([b, g, r, 255]));
        }
    }
    let frame = Frame::new(img);
    Arc::new(RenderImage::new(smallvec![frame]))
}
