use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    BorderStyle, Bounds, Context, Corners, CursorStyle, ElementId, FocusHandle, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PathBuilder, Pixels, Point, Render,
    RenderImage, ScrollWheelEvent, Window, canvas, div, fill, outline, point, prelude::*, px, rgb,
    rgba, size, svg,
};
use image::{Frame, RgbaImage};
use smallvec::smallvec;

use crate::capture::region::crop_region;
use crate::capture::screen::ScreenCapture;
use crate::config::AppConfig;
use crate::editor::annotations::Annotation;
use crate::editor::canvas::render_annotations_to_image;
use crate::editor::history::EditHistory;
use crate::editor::properties::ToolProperties;
use crate::editor::tools::ToolKind;
use crate::i18n::t;
use crate::image::clipboard::{copy_image_to_clipboard, copy_text_to_clipboard};
use crate::image::export::auto_save_screenshot;
use crate::ocr::recognition::recognize_text_from_image;
use crate::overlay::magnifier::{MagnifierInfo, render_magnifier};
use crate::overlay::selection::{SelectionDragMode, SelectionState};
use crate::overlay::toolbar::{
    HoveredTooltip, TOOLBAR_MAIN_HEIGHT, TOOLBAR_TOTAL_HEIGHT, TOOLBAR_WIDTH,
    calculate_toolbar_position, generate_gradient_bitmap, generate_hue_strip_bitmap, hsv_to_rgb,
    parse_hex_color, render_blur_preset, render_color_swatch, render_toolbar_button,
    render_toolbar_divider, render_toolbar_text_button, rgb_to_cmyk, rgb_to_hsl, rgb_to_hsv,
};
use crate::system::windows::{PhysicalRect, play_capture_sound};
use crate::theme::PixoraTheme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteDragMode {
    None,
    SaturationValue,
    Hue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumericInputTarget {
    StrokeWidth,
    BlurRadius,
    PixelateBlock,
}

pub struct ScreenOverlayView {
    pub capture: ScreenCapture,
    pub render_image: Arc<RenderImage>,
    pub theme: PixoraTheme,
    pub config: AppConfig,

    pub selection: SelectionState,
    pub drag_mode: SelectionDragMode,

    pub active_tool: ToolKind,
    pub tool_props: ToolProperties,
    pub history: EditHistory,
    pub current_drawing: Option<Annotation>,
    pub drawing_start: Option<Point<Pixels>>,

    pub cursor_pos: Point<Pixels>,
    pub magnifier_info: Option<MagnifierInfo>,
    pub focus_handle: FocusHandle,

    pub toast_message: Option<(String, Instant)>,
    pub is_ocr_running: bool,

    // Text annotation editing
    pub text_input_mode: bool,
    pub text_input_pos: Point<Pixels>,
    pub text_input_buffer: String,

    // Numeric value typing for px
    pub numeric_input_mode: Option<NumericInputTarget>,
    pub numeric_input_buffer: String,

    // 2D HSV Gradient Color Studio
    pub show_color_palette: bool,
    pub custom_hex_input: String,
    pub hex_input_mode: bool,
    pub current_toolbar_bounds: Option<Bounds<Pixels>>,
    pub palette_drag_mode: PaletteDragMode,
    pub picker_hue: f32,
    pub picker_sat: f32,
    pub picker_val: f32,
    pub palette_gradient_image: Option<Arc<RenderImage>>,
    pub palette_hue_strip_image: Option<Arc<RenderImage>>,
    pub gradient_field_bounds: Option<Bounds<Pixels>>,
    pub hue_slider_bounds: Option<Bounds<Pixels>>,
    pub hovered_tooltip: Option<HoveredTooltip>,
}

impl ScreenOverlayView {
    pub fn new(capture: ScreenCapture, config: AppConfig, cx: &mut Context<Self>) -> Self {
        // GPUI RenderImage expects pixel data in BGRA format.
        // capture.image is standard RGBA, so we clone and convert byte channels (R <-> B)
        // for faithful GPUI onscreen rendering while keeping capture.image genuine RGBA.
        let mut bgra_buf = capture.image.clone();
        for chunk in bgra_buf.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }
        let frame = Frame::new(bgra_buf);
        let render_image = Arc::new(RenderImage::new(smallvec![frame]));
        let focus_handle = cx.focus_handle();

        Self {
            capture,
            render_image,
            theme: PixoraTheme::default(),
            config,

            selection: SelectionState::default(),
            drag_mode: SelectionDragMode::None,

            active_tool: ToolKind::Select,
            tool_props: ToolProperties::default(),
            history: EditHistory::new(),
            current_drawing: None,
            drawing_start: None,

            cursor_pos: point(px(0.0), px(0.0)),
            magnifier_info: None,
            focus_handle,

            toast_message: None,
            is_ocr_running: false,

            text_input_mode: false,
            text_input_pos: point(px(0.0), px(0.0)),
            text_input_buffer: String::new(),

            numeric_input_mode: None,
            numeric_input_buffer: String::new(),

            show_color_palette: false,
            custom_hex_input: "8174FF".to_string(),
            hex_input_mode: false,
            current_toolbar_bounds: None,
            palette_drag_mode: PaletteDragMode::None,
            picker_hue: 246.0,
            picker_sat: 0.55,
            picker_val: 1.0,
            palette_gradient_image: Some(generate_gradient_bitmap(246.0, 310, 150)),
            palette_hue_strip_image: Some(generate_hue_strip_bitmap(448, 12)),
            gradient_field_bounds: None,
            hue_slider_bounds: None,
            hovered_tooltip: None,
        }
    }

    pub fn set_tooltip(&mut self, text: &'static str, rel_x: f32, hovered: bool, cx: &mut Context<Self>) {
        if hovered {
            self.hovered_tooltip = Some(HoveredTooltip { text, rel_x });
        } else if self.hovered_tooltip.map(|t| t.text) == Some(text) {
            self.hovered_tooltip = None;
        }
        cx.notify();
    }

    pub fn sync_picker_from_color(&mut self) {
        let r = (self.tool_props.stroke_color.r * 255.0).round().clamp(0.0, 255.0) as u8;
        let g = (self.tool_props.stroke_color.g * 255.0).round().clamp(0.0, 255.0) as u8;
        let b = (self.tool_props.stroke_color.b * 255.0).round().clamp(0.0, 255.0) as u8;
        let (h, s, v) = rgb_to_hsv(r, g, b);
        self.picker_hue = h;
        self.picker_sat = s;
        self.picker_val = v;
        self.palette_gradient_image = Some(generate_gradient_bitmap(h, 310, 150));
        self.custom_hex_input = format!("{:02X}{:02X}{:02X}", r, g, b);
    }

    pub fn set_picker_hue(&mut self, hue: f32, cx: &mut Context<Self>) {
        self.picker_hue = hue.clamp(0.0, 360.0);
        self.palette_gradient_image = Some(generate_gradient_bitmap(self.picker_hue, 310, 150));
        self.apply_hsv_to_color(cx);
    }

    pub fn set_picker_sat_val(&mut self, sat: f32, val: f32, cx: &mut Context<Self>) {
        self.picker_sat = sat.clamp(0.0, 1.0);
        self.picker_val = val.clamp(0.0, 1.0);
        self.apply_hsv_to_color(cx);
    }

    pub fn apply_hsv_to_color(&mut self, cx: &mut Context<Self>) {
        let (r, g, b) = hsv_to_rgb(self.picker_hue, self.picker_sat, self.picker_val);
        self.tool_props.stroke_color = rgb(((r as u32) << 16) | ((g as u32) << 8) | (b as u32));
        self.custom_hex_input = format!("{:02X}{:02X}{:02X}", r, g, b);
        cx.notify();
    }

    pub fn show_toast(&mut self, msg: impl Into<String>, cx: &mut Context<Self>) {
        self.toast_message = Some((msg.into(), Instant::now()));
        cx.notify();
    }

    pub fn rebuild_render_image(&mut self) {
        let mut img = self.capture.image.clone();
        for ann in &self.history.annotations {
            match ann {
                Annotation::Blur { bounds, radius } => {
                    let bx = (f32::from(bounds.origin.x).max(0.0) as u32).min(img.width());
                    let by = (f32::from(bounds.origin.y).max(0.0) as u32).min(img.height());
                    let bw = (f32::from(bounds.size.width) as u32).min(img.width().saturating_sub(bx));
                    let bh = (f32::from(bounds.size.height) as u32).min(img.height().saturating_sub(by));
                    if bw > 0 && bh > 0 {
                        crate::image::processing::apply_blur(&mut img, bx, by, bw, bh, *radius);
                    }
                }
                Annotation::Pixelate { bounds, block_size } => {
                    let bx = (f32::from(bounds.origin.x).max(0.0) as u32).min(img.width());
                    let by = (f32::from(bounds.origin.y).max(0.0) as u32).min(img.height());
                    let bw = (f32::from(bounds.size.width) as u32).min(img.width().saturating_sub(bx));
                    let bh = (f32::from(bounds.size.height) as u32).min(img.height().saturating_sub(by));
                    if bw > 0 && bh > 0 {
                        crate::image::processing::apply_pixelate(&mut img, bx, by, bw, bh, *block_size);
                    }
                }
                _ => {}
            }
        }

        let mut bgra_buf = img;
        for chunk in bgra_buf.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }
        let frame = Frame::new(bgra_buf);
        self.render_image = Arc::new(RenderImage::new(smallvec![frame]));
    }

    pub fn do_undo(&mut self, cx: &mut Context<Self>) {
        if self.history.undo() {
            let max_badge = self
                .history
                .annotations
                .iter()
                .filter_map(|ann| {
                    if let Annotation::NumberBadge { number, .. } = ann {
                        Some(*number)
                    } else {
                        None
                    }
                })
                .max()
                .unwrap_or(0);
            self.tool_props.current_badge_number = max_badge + 1;
            self.rebuild_render_image();
            cx.notify();
        }
    }

    pub fn do_redo(&mut self, cx: &mut Context<Self>) {
        if self.history.redo() {
            let max_badge = self
                .history
                .annotations
                .iter()
                .filter_map(|ann| {
                    if let Annotation::NumberBadge { number, .. } = ann {
                        Some(*number)
                    } else {
                        None
                    }
                })
                .max()
                .unwrap_or(0);
            self.tool_props.current_badge_number = max_badge + 1;
            self.rebuild_render_image();
            cx.notify();
        }
    }

    pub fn prepare_final_image(&self) -> Option<RgbaImage> {
        if !self.selection.is_active {
            return None;
        }

        let sel_x = self.selection.x();
        let sel_y = self.selection.y();
        let sel_w = self.selection.width();
        let sel_h = self.selection.height();

        let crop_rect = PhysicalRect {
            x: sel_x.round() as i32,
            y: sel_y.round() as i32,
            width: sel_w.round().max(1.0) as u32,
            height: sel_h.round().max(1.0) as u32,
        };

        let mut cropped = crop_region(&self.capture, crop_rect)?;
        render_annotations_to_image(&mut cropped, &self.history.annotations, sel_x, sel_y);
        Some(cropped)
    }

    pub fn do_copy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let lang = self.config.language.resolve();
        if let Some(final_img) = self.prepare_final_image() {
            if let Err(e) = copy_image_to_clipboard(&final_img) {
                self.show_toast(format!("{}: {}", t(lang, "Copy failed", "Ошибка копирования"), e), cx);
            } else {
                if self.config.sound_effects {
                    play_capture_sound();
                }
                window.remove_window();
            }
        } else {
            self.show_toast(t(lang, "Select an area first", "Сначала выделите область"), cx);
        }
    }

    pub fn do_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let lang = self.config.language.resolve();
        if let Some(final_img) = self.prepare_final_image() {
            match auto_save_screenshot(&final_img, &self.config) {
                Ok(path) => {
                    if self.config.sound_effects {
                        play_capture_sound();
                    }
                    if self.config.auto_copy_to_clipboard {
                        let _ = copy_image_to_clipboard(&final_img);
                    }
                    let filename = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "screenshot.png".to_string());
                    self.show_toast(format!("{}: {}", t(lang, "Saved", "Сохранено"), filename), cx);
                    window.remove_window();
                }
                Err(e) => {
                    self.show_toast(format!("{}: {}", t(lang, "Save error", "Ошибка сохранения"), e), cx);
                }
            }
        } else {
            self.show_toast(t(lang, "Select an area first", "Сначала выделите область"), cx);
        }
    }

    pub fn do_ocr(&mut self, cx: &mut Context<Self>) {
        let lang = self.config.language.resolve();
        if let Some(img) = self.prepare_final_image() {
            self.is_ocr_running = true;
            cx.notify();

            match recognize_text_from_image(&img) {
                Ok(output) => {
                    if output.full_text.trim().is_empty() {
                        self.show_toast(t(lang, "No text found in selected area", "Текст в выбранной области не найден"), cx);
                    } else {
                        let _ = copy_text_to_clipboard(&output.full_text);
                        let preview: String = output.full_text.chars().take(28).collect();
                        self.show_toast(format!("{}: \"{}...\"", t(lang, "OCR copied", "Текст скопирован"), preview), cx);
                    }
                }
                Err(e) => {
                    self.show_toast(format!("{}: {}", t(lang, "OCR failed", "Ошибка распознавания"), e), cx);
                }
            }
            self.is_ocr_running = false;
            cx.notify();
        } else {
            self.show_toast(t(lang, "Select an area with text first", "Сначала выделите область с текстом"), cx);
        }
    }

    pub fn apply_custom_hex(&mut self, cx: &mut Context<Self>) {
        if let Some(color) = parse_hex_color(&self.custom_hex_input) {
            self.tool_props.stroke_color = color;
            self.show_toast(format!("Color set to #{}", self.custom_hex_input.to_uppercase()), cx);
        } else {
            self.show_toast("Invalid HEX format (use RRGGBB)", cx);
        }
        self.hex_input_mode = false;
        cx.notify();
    }

    pub fn update_rgb_channel(&mut self, channel: usize, delta: i16, cx: &mut Context<Self>) {
        let mut r = (self.tool_props.stroke_color.r * 255.0).round().clamp(0.0, 255.0) as i16;
        let mut g = (self.tool_props.stroke_color.g * 255.0).round().clamp(0.0, 255.0) as i16;
        let mut b = (self.tool_props.stroke_color.b * 255.0).round().clamp(0.0, 255.0) as i16;

        match channel {
            0 => r = (r + delta).clamp(0, 255),
            1 => g = (g + delta).clamp(0, 255),
            2 => b = (b + delta).clamp(0, 255),
            _ => {}
        }

        let r_u8 = r as u8;
        let g_u8 = g as u8;
        let b_u8 = b as u8;

        self.tool_props.stroke_color = rgb(((r_u8 as u32) << 16) | ((g_u8 as u32) << 8) | (b_u8 as u32));
        self.custom_hex_input = format!("{:02X}{:02X}{:02X}", r_u8, g_u8, b_u8);
        cx.notify();
    }

    pub fn set_rgb_channel(&mut self, channel: usize, val: u8, cx: &mut Context<Self>) {
        let mut r = (self.tool_props.stroke_color.r * 255.0).round().clamp(0.0, 255.0) as u8;
        let mut g = (self.tool_props.stroke_color.g * 255.0).round().clamp(0.0, 255.0) as u8;
        let mut b = (self.tool_props.stroke_color.b * 255.0).round().clamp(0.0, 255.0) as u8;

        match channel {
            0 => r = val,
            1 => g = val,
            2 => b = val,
            _ => {}
        }

        self.tool_props.stroke_color = rgb(((r as u32) << 16) | ((g as u32) << 8) | (b as u32));
        self.custom_hex_input = format!("{:02X}{:02X}{:02X}", r, g, b);
        cx.notify();
    }

    pub fn set_hue_color(&mut self, hue: f32, cx: &mut Context<Self>) {
        let (r, g, b) = hsv_to_rgb(hue, 1.0, 1.0);
        self.tool_props.stroke_color = rgb(((r as u32) << 16) | ((g as u32) << 8) | (b as u32));
        self.custom_hex_input = format!("{:02X}{:02X}{:02X}", r, g, b);
        cx.notify();
    }

    pub fn copy_hex_color(&mut self, hex: &str, cx: &mut Context<Self>) {
        let _ = copy_text_to_clipboard(hex);
        self.show_toast(format!("Copied {} to clipboard", hex), cx);
    }

    pub fn handle_mouse_down(
        &mut self,
        ev: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let p = ev.position;
        let px_f = f32::from(p.x);
        let py_f = f32::from(p.y);

        // Ignore clicks on floating toolbar / property sub-bar / color palette
        if self.selection.is_active {
            if let Some(tb) = self.current_toolbar_bounds {
                let pad = px(12.0);
                let safe_box = Bounds {
                    origin: point(tb.origin.x - pad, tb.origin.y - pad),
                    size: size(tb.size.width + pad * 2.0, tb.size.height + pad * 2.0),
                };
                if safe_box.contains(&p) {
                    if self.show_color_palette {
                        if let Some(gb) = self.gradient_field_bounds {
                            if gb.contains(&p) {
                                self.palette_drag_mode = PaletteDragMode::SaturationValue;
                                let rel_x = (px_f - f32::from(gb.origin.x)).clamp(0.0, f32::from(gb.size.width));
                                let rel_y = (py_f - f32::from(gb.origin.y)).clamp(0.0, f32::from(gb.size.height));
                                self.set_picker_sat_val(rel_x / f32::from(gb.size.width), 1.0 - (rel_y / f32::from(gb.size.height)), cx);
                                return;
                            }
                        }
                        if let Some(hb) = self.hue_slider_bounds {
                            if hb.contains(&p) {
                                self.palette_drag_mode = PaletteDragMode::Hue;
                                let rel_x = (px_f - f32::from(hb.origin.x)).clamp(0.0, f32::from(hb.size.width));
                                self.set_picker_hue(rel_x / f32::from(hb.size.width) * 360.0, cx);
                                return;
                            }
                        }
                    }
                    return;
                }
            }
        }

        // 1. Text input submission if clicking outside
        if self.text_input_mode {
            if !self.text_input_buffer.trim().is_empty() {
                let ann = Annotation::Text {
                    origin: self.text_input_pos,
                    text: self.text_input_buffer.clone(),
                    color: self.tool_props.stroke_color,
                    font_size: self.tool_props.font_size,
                };
                self.history.push_annotation(ann);
            }
            self.text_input_mode = false;
            self.text_input_buffer.clear();
            cx.notify();
        }

        // Close color palette modal if open
        if self.show_color_palette {
            self.show_color_palette = false;
            self.hex_input_mode = false;
            cx.notify();
        }

        // 2. Eyedropper mode: picks pixel color, applies it to stroke_color, copies HEX, and notifies
        if self.active_tool == ToolKind::Eyedropper {
            let img_x = (px_f.round() as u32).min(self.capture.bounds.width.saturating_sub(1));
            let img_y = (py_f.round() as u32).min(self.capture.bounds.height.saturating_sub(1));
            let pixel = self.capture.image.get_pixel(img_x, img_y);
            let picked = rgb(((pixel[0] as u32) << 16) | ((pixel[1] as u32) << 8) | (pixel[2] as u32));
            let hex = format!("#{:02X}{:02X}{:02X}", pixel[0], pixel[1], pixel[2]);
            self.tool_props.stroke_color = picked;
            self.custom_hex_input = format!("{:02X}{:02X}{:02X}", pixel[0], pixel[1], pixel[2]);
            let _ = copy_text_to_clipboard(&hex);
            self.show_toast(format!("Picked color {}", hex), cx);
            self.active_tool = ToolKind::Pencil;
            cx.notify();
            return;
        }

        // 3. Text tool placement
        if self.active_tool == ToolKind::Text {
            self.text_input_mode = true;
            self.text_input_pos = p;
            self.text_input_buffer.clear();
            cx.notify();
            return;
        }

        // 4. Number step badge placement
        if self.active_tool == ToolKind::NumberBadge {
            let badge = Annotation::NumberBadge {
                center: p,
                number: self.tool_props.current_badge_number,
                color: self.tool_props.stroke_color,
                radius: 12.0,
            };
            self.history.push_annotation(badge);
            self.tool_props.current_badge_number += 1;
            cx.notify();
            return;
        }

        // 5. Drawing tools (Pencil, Marker, Arrow, Line, Rect, Ellipse, Blur, Pixelate)
        if self.active_tool != ToolKind::Select {
            self.drawing_start = Some(p);
            match self.active_tool {
                ToolKind::Pencil => {
                    self.current_drawing = Some(Annotation::Pencil {
                        points: vec![p],
                        color: self.tool_props.stroke_color,
                        width: self.tool_props.stroke_width,
                    });
                }
                ToolKind::Marker => {
                    self.current_drawing = Some(Annotation::Marker {
                        points: vec![p],
                        color: self.tool_props.stroke_color,
                        width: self.tool_props.stroke_width,
                    });
                }
                ToolKind::Line => {
                    self.current_drawing = Some(Annotation::Line {
                        from: p,
                        to: p,
                        color: self.tool_props.stroke_color,
                        width: self.tool_props.stroke_width,
                    });
                }
                ToolKind::Arrow => {
                    self.current_drawing = Some(Annotation::Arrow {
                        from: p,
                        to: p,
                        color: self.tool_props.stroke_color,
                        width: self.tool_props.stroke_width,
                    });
                }
                ToolKind::Rectangle => {
                    self.current_drawing = Some(Annotation::Rectangle {
                        bounds: Bounds {
                            origin: p,
                            size: size(px(0.0), px(0.0)),
                        },
                        color: self.tool_props.stroke_color,
                        width: self.tool_props.stroke_width,
                        filled: self.tool_props.is_filled,
                    });
                }
                ToolKind::Ellipse => {
                    self.current_drawing = Some(Annotation::Ellipse {
                        bounds: Bounds {
                            origin: p,
                            size: size(px(0.0), px(0.0)),
                        },
                        color: self.tool_props.stroke_color,
                        width: self.tool_props.stroke_width,
                        filled: self.tool_props.is_filled,
                    });
                }
                ToolKind::Blur => {
                    self.current_drawing = Some(Annotation::Blur {
                        bounds: Bounds {
                            origin: p,
                            size: size(px(0.0), px(0.0)),
                        },
                        radius: self.tool_props.blur_radius,
                    });
                }
                ToolKind::Pixelate => {
                    self.current_drawing = Some(Annotation::Pixelate {
                        bounds: Bounds {
                            origin: p,
                            size: size(px(0.0), px(0.0)),
                        },
                        block_size: self.tool_props.pixelate_block,
                    });
                }
                _ => {}
            }
            cx.notify();
            return;
        }

        // 6. Select tool: check handles for resizing
        if let Some(handle) = self.selection.hit_test_handle(p, px(10.0)) {
            self.drag_mode = SelectionDragMode::Resizing {
                handle,
                start_mouse: p,
                orig_bounds: self.selection.bounds,
            };
            cx.notify();
            return;
        }

        // 7. Select tool: move existing selection
        if self.selection.contains(p) {
            self.drag_mode = SelectionDragMode::Moving {
                start_mouse: p,
                orig_origin: self.selection.bounds.origin,
            };
            cx.notify();
            return;
        }

        // 8. Select tool: create new selection
        self.drag_mode = SelectionDragMode::Creating { start: p };
        self.selection = SelectionState::new(px_f, py_f, 0.0, 0.0);
        cx.notify();
    }

    pub fn handle_mouse_move(
        &mut self,
        ev: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let p = ev.position;
        self.cursor_pos = p;
        let px_f = f32::from(p.x);
        let py_f = f32::from(p.y);

        if self.palette_drag_mode == PaletteDragMode::SaturationValue {
            if let Some(gb) = self.gradient_field_bounds {
                let rel_x = (px_f - f32::from(gb.origin.x)).clamp(0.0, f32::from(gb.size.width));
                let rel_y = (py_f - f32::from(gb.origin.y)).clamp(0.0, f32::from(gb.size.height));
                self.set_picker_sat_val(rel_x / f32::from(gb.size.width), 1.0 - (rel_y / f32::from(gb.size.height)), cx);
                return;
            }
        } else if self.palette_drag_mode == PaletteDragMode::Hue {
            if let Some(hb) = self.hue_slider_bounds {
                let rel_x = (px_f - f32::from(hb.origin.x)).clamp(0.0, f32::from(hb.size.width));
                self.set_picker_hue(rel_x / f32::from(hb.size.width) * 360.0, cx);
                return;
            }
        }

        // Update magnifier loupe info
        let img_x = (px_f.round() as u32).min(self.capture.bounds.width.saturating_sub(1));
        let img_y = (py_f.round() as u32).min(self.capture.bounds.height.saturating_sub(1));
        let screen_x = self.capture.bounds.x + img_x as i32;
        let screen_y = self.capture.bounds.y + img_y as i32;

        self.magnifier_info = Some(MagnifierInfo::from_image(
            &self.capture.image,
            screen_x,
            screen_y,
            img_x,
            img_y,
        ));

        // Update active drawing
        if let Some(drawing) = &mut self.current_drawing {
            if let Some(start) = self.drawing_start {
                let min_x = f32::from(start.x).min(px_f);
                let min_y = f32::from(start.y).min(py_f);
                let w = (px_f - f32::from(start.x)).abs();
                let h = (py_f - f32::from(start.y)).abs();
                let rect_bounds = Bounds {
                    origin: point(px(min_x), px(min_y)),
                    size: size(px(w), px(h)),
                };

                match drawing {
                    Annotation::Pencil { points, .. } | Annotation::Marker { points, .. } => {
                        points.push(p);
                    }
                    Annotation::Line { to, .. } | Annotation::Arrow { to, .. } => {
                        *to = p;
                    }
                    Annotation::Rectangle { bounds, .. }
                    | Annotation::Ellipse { bounds, .. }
                    | Annotation::Blur { bounds, .. }
                    | Annotation::Pixelate { bounds, .. } => {
                        *bounds = rect_bounds;
                    }
                    _ => {}
                }
                cx.notify();
                return;
            }
        }

        // Handle selection dragging
        let max_w = self.capture.bounds.width as f32;
        let max_h = self.capture.bounds.height as f32;
        match self.drag_mode {
            SelectionDragMode::Creating { start } => {
                let cur_x = px_f.clamp(0.0, max_w);
                let cur_y = py_f.clamp(0.0, max_h);
                let w = cur_x - f32::from(start.x);
                let h = cur_y - f32::from(start.y);
                self.selection = SelectionState::new(f32::from(start.x), f32::from(start.y), w, h);
                cx.notify();
            }
            SelectionDragMode::Moving {
                start_mouse,
                orig_origin,
            } => {
                let dx = px_f - f32::from(start_mouse.x);
                let dy = py_f - f32::from(start_mouse.y);
                self.selection.bounds.origin = point(
                    px(f32::from(orig_origin.x) + dx),
                    px(f32::from(orig_origin.y) + dy),
                );
                cx.notify();
            }
            SelectionDragMode::Resizing {
                handle,
                start_mouse,
                orig_bounds,
            } => {
                let cur_x = px_f.clamp(0.0, max_w);
                let cur_y = py_f.clamp(0.0, max_h);
                let delta = point(px(cur_x) - start_mouse.x, px(cur_y) - start_mouse.y);
                self.selection.update_resize(handle, orig_bounds, delta);
                cx.notify();
            }
            SelectionDragMode::None => {
                let show_mag = self.active_tool == ToolKind::Eyedropper
                    || (self.config.show_magnifier && !self.selection.is_active);
                if show_mag {
                    cx.notify();
                }
            }
        }
    }

    pub fn handle_mouse_up(
        &mut self,
        _ev: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.palette_drag_mode != PaletteDragMode::None {
            self.palette_drag_mode = PaletteDragMode::None;
            cx.notify();
            return;
        }

        self.drag_mode = SelectionDragMode::None;
        self.drawing_start = None;

        if let Some(drawing) = self.current_drawing.take() {
            let is_blur_or_pixelate = matches!(drawing, Annotation::Blur { .. } | Annotation::Pixelate { .. });
            self.history.push_annotation(drawing);
            if is_blur_or_pixelate {
                self.rebuild_render_image();
            }
            cx.notify();
            return;
        }

        // Cancel accidental tiny tap selection (< 12px)
        if self.selection.width() < 12.0 || self.selection.height() < 12.0 {
            self.selection.is_active = false;
            cx.notify();
        }
    }

    pub fn handle_scroll_wheel(
        &mut self,
        ev: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Adjust brush size via mouse wheel
        let delta = match ev.delta {
            gpui::ScrollDelta::Pixels(p) => f32::from(p.y),
            gpui::ScrollDelta::Lines(l) => l.y * 10.0,
        };

        if delta > 0.0 {
            self.tool_props.stroke_width = (self.tool_props.stroke_width + 1.0).min(48.0);
            cx.notify();
        } else if delta < 0.0 {
            self.tool_props.stroke_width = (self.tool_props.stroke_width - 1.0).max(1.0);
            cx.notify();
        }
    }

    pub fn handle_key_down(
        &mut self,
        ev: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = ev.keystroke.key.as_str();

        // 0. Handling Numeric px Input (for stroke width, blur, pixelate)
        if let Some(target) = self.numeric_input_mode {
            if key == "enter" {
                if let Ok(val) = self.numeric_input_buffer.parse::<u32>() {
                    match target {
                        NumericInputTarget::StrokeWidth => {
                            self.tool_props.stroke_width = (val as f32).clamp(1.0, 99.0);
                        }
                        NumericInputTarget::BlurRadius => {
                            self.tool_props.blur_radius = val.clamp(2, 99);
                        }
                        NumericInputTarget::PixelateBlock => {
                            self.tool_props.pixelate_block = val.clamp(2, 99);
                        }
                    }
                }
                self.numeric_input_mode = None;
                self.numeric_input_buffer.clear();
                cx.notify();
                return;
            } else if key == "escape" {
                self.numeric_input_mode = None;
                self.numeric_input_buffer.clear();
                cx.notify();
                return;
            } else if key == "backspace" {
                self.numeric_input_buffer.pop();
                cx.notify();
                return;
            } else if ev.keystroke.key.len() == 1 {
                let ch = ev.keystroke.key.chars().next().unwrap();
                if ch.is_ascii_digit() && self.numeric_input_buffer.len() < 3 {
                    self.numeric_input_buffer.push(ch);
                    cx.notify();
                    return;
                }
            }
            return;
        }

        // 1. Handling HEX input editing
        if self.hex_input_mode {
            if key == "enter" {
                self.apply_custom_hex(cx);
                return;
            } else if key == "escape" {
                self.hex_input_mode = false;
                cx.notify();
                return;
            } else if key == "backspace" {
                self.custom_hex_input.pop();
                cx.notify();
                return;
            } else if ev.keystroke.key.len() == 1 {
                let ch = ev.keystroke.key.chars().next().unwrap();
                if ch.is_ascii_hexdigit() && self.custom_hex_input.len() < 6 {
                    self.custom_hex_input.push(ch.to_ascii_uppercase());
                    cx.notify();
                    return;
                }
            }
        }

        // 2. Handling Text Annotation typing (supports Space, Russian/Cyrillic, and layout switching)
        if self.text_input_mode {
            if key == "enter" {
                if !self.text_input_buffer.trim().is_empty() {
                    let ann = Annotation::Text {
                        origin: self.text_input_pos,
                        text: self.text_input_buffer.clone(),
                        color: self.tool_props.stroke_color,
                        font_size: self.tool_props.font_size,
                    };
                    self.history.push_annotation(ann);
                }
                self.text_input_mode = false;
                self.text_input_buffer.clear();
                cx.notify();
                return;
            } else if key == "escape" {
                self.text_input_mode = false;
                self.text_input_buffer.clear();
                cx.notify();
                return;
            } else if key == "backspace" {
                self.text_input_buffer.pop();
                cx.notify();
                return;
            } else if key == "space" {
                self.text_input_buffer.push(' ');
                cx.notify();
                return;
            } else if let Some(key_char) = &ev.keystroke.key_char {
                // key_char contains the exact localized character from active layout (e.g. Russian, Cyrillic, etc.)
                if !ev.keystroke.modifiers.control && !ev.keystroke.modifiers.alt {
                    self.text_input_buffer.push_str(key_char);
                    cx.notify();
                    return;
                }
            } else if ev.keystroke.key.chars().count() == 1
                && !ev.keystroke.modifiers.control
                && !ev.keystroke.modifiers.alt
            {
                self.text_input_buffer.push_str(&ev.keystroke.key);
                cx.notify();
                return;
            }
            return;
        }

        if key == "escape" {
            window.remove_window();
            return;
        }

        // Space selects the entire screen
        if key == "space" && !self.text_input_mode && !self.hex_input_mode {
            let scr_w = f32::from(window.viewport_size().width);
            let scr_h = f32::from(window.viewport_size().height);
            self.selection = SelectionState::new(0.0, 0.0, scr_w, scr_h);
            cx.notify();
            return;
        }

        if ev.keystroke.modifiers.control {
            let is_z = key == "z" || key == "Z" || key == "я" || key == "Я"
                || ev.keystroke.key_char.as_deref() == Some("z")
                || ev.keystroke.key_char.as_deref() == Some("Z")
                || ev.keystroke.key_char.as_deref() == Some("я")
                || ev.keystroke.key_char.as_deref() == Some("Я");

            let is_y = key == "y" || key == "Y" || key == "н" || key == "Н"
                || ev.keystroke.key_char.as_deref() == Some("y")
                || ev.keystroke.key_char.as_deref() == Some("Y")
                || ev.keystroke.key_char.as_deref() == Some("н")
                || ev.keystroke.key_char.as_deref() == Some("Н");

            let is_c = key == "c" || key == "C" || key == "с" || key == "С"
                || ev.keystroke.key_char.as_deref() == Some("c")
                || ev.keystroke.key_char.as_deref() == Some("C")
                || ev.keystroke.key_char.as_deref() == Some("с")
                || ev.keystroke.key_char.as_deref() == Some("С");

            let is_s = key == "s" || key == "S" || key == "ы" || key == "Ы"
                || ev.keystroke.key_char.as_deref() == Some("s")
                || ev.keystroke.key_char.as_deref() == Some("S")
                || ev.keystroke.key_char.as_deref() == Some("ы")
                || ev.keystroke.key_char.as_deref() == Some("Ы");

            let is_a = key == "a" || key == "A" || key == "ф" || key == "Ф"
                || ev.keystroke.key_char.as_deref() == Some("a")
                || ev.keystroke.key_char.as_deref() == Some("A")
                || ev.keystroke.key_char.as_deref() == Some("ф")
                || ev.keystroke.key_char.as_deref() == Some("Ф");

            if is_c {
                self.do_copy(window, cx);
            } else if is_s {
                self.do_save(window, cx);
            } else if is_z {
                if ev.keystroke.modifiers.shift {
                    self.do_redo(cx);
                } else {
                    self.do_undo(cx);
                }
            } else if is_y {
                self.do_redo(cx);
            } else if is_a {
                let scr_w = f32::from(window.viewport_size().width);
                let scr_h = f32::from(window.viewport_size().height);
                self.selection = SelectionState::new(0.0, 0.0, scr_w, scr_h);
                cx.notify();
            }
        } else if key == "enter" {
            self.do_copy(window, cx);
        }
    }
}

impl Render for ScreenOverlayView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let screen_size = window.viewport_size();
        let screen_bounds = Bounds {
            origin: point(px(0.0), px(0.0)),
            size: screen_size,
        };

        let render_img = self.render_image.clone();
        let selection = self.selection;
        let theme = self.theme;
        let lang = self.config.language.resolve();
        let show_magnifier = self.active_tool == ToolKind::Eyedropper
            || (self.config.show_magnifier && !selection.is_active);

        let annotations = self.history.annotations.clone();
        let current_drawing = self.current_drawing.clone();

        // Determine cursor style
        let cursor_style = if self.active_tool == ToolKind::Text {
            CursorStyle::IBeam
        } else if self.active_tool == ToolKind::Select && selection.is_active {
            if selection.hit_test_handle(self.cursor_pos, px(10.0)).is_some() {
                CursorStyle::Crosshair
            } else if selection.contains(self.cursor_pos) {
                CursorStyle::ClosedHand
            } else {
                CursorStyle::Crosshair
            }
        } else {
            CursorStyle::Crosshair
        };

        let mut root = div()
            .id("overlay-root")
            .size_full()
            .cursor(cursor_style)
            .track_focus(&self.focus_handle)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev, window, cx| {
                    this.handle_mouse_down(ev, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(|this, ev, window, cx| {
                this.handle_mouse_move(ev, window, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, ev, window, cx| {
                    this.handle_mouse_up(ev, window, cx);
                }),
            )
            .on_scroll_wheel(cx.listener(|this, ev, window, cx| {
                this.handle_scroll_wheel(ev, window, cx);
            }))
            .on_key_down(cx.listener(|this, ev, window, cx| {
                this.handle_key_down(ev, window, cx);
            }));

        // Canvas layer: screen image, dark mask, selection border, handles, and shapes
        root = root.child(
            canvas(|_, _, _| (), move |bounds, _, window, _cx| {
                let _ = window.paint_image(bounds, Corners::default(), render_img, 0, false);

                if selection.is_active {
                    let sel = selection.bounds;
                    let mask_bg = theme.mask_overlay;

                    // Top mask
                    if sel.origin.y > px(0.0) {
                        window.paint_quad(fill(
                            Bounds {
                                origin: point(px(0.0), px(0.0)),
                                size: size(bounds.size.width, sel.origin.y),
                            },
                            mask_bg,
                        ));
                    }
                    // Bottom mask
                    let sel_bottom = sel.origin.y + sel.size.height;
                    if sel_bottom < bounds.size.height {
                        window.paint_quad(fill(
                            Bounds {
                                origin: point(px(0.0), sel_bottom),
                                size: size(bounds.size.width, bounds.size.height - sel_bottom),
                            },
                            mask_bg,
                        ));
                    }
                    // Left mask
                    if sel.origin.x > px(0.0) {
                        window.paint_quad(fill(
                            Bounds {
                                origin: point(px(0.0), sel.origin.y),
                                size: size(sel.origin.x, sel.size.height),
                            },
                            mask_bg,
                        ));
                    }
                    // Right mask
                    let sel_right = sel.origin.x + sel.size.width;
                    if sel_right < bounds.size.width {
                        window.paint_quad(fill(
                            Bounds {
                                origin: point(sel_right, sel.origin.y),
                                size: size(bounds.size.width - sel_right, sel.size.height),
                            },
                            mask_bg,
                        ));
                    }

                    // Selection border (CleanShot accent line)
                    window.paint_quad(outline(sel, theme.accent, BorderStyle::Solid));

                    // 8 Resize handles
                    let handle_size = size(px(8.0), px(8.0));
                    let half = px(4.0);
                    let h_pts = [
                        point(sel.origin.x - half, sel.origin.y - half),
                        point(sel.origin.x + sel.size.width / 2.0 - half, sel.origin.y - half),
                        point(sel.origin.x + sel.size.width - half, sel.origin.y - half),
                        point(sel.origin.x + sel.size.width - half, sel.origin.y + sel.size.height / 2.0 - half),
                        point(sel.origin.x + sel.size.width - half, sel.origin.y + sel.size.height - half),
                        point(sel.origin.x + sel.size.width / 2.0 - half, sel.origin.y + sel.size.height - half),
                        point(sel.origin.x - half, sel.origin.y + sel.size.height - half),
                        point(sel.origin.x - half, sel.origin.y + sel.size.height / 2.0 - half),
                    ];

                    for pt in h_pts {
                        window.paint_quad(fill(
                            Bounds { origin: pt, size: handle_size },
                            rgb(0xFFFFFF),
                        ));
                        window.paint_quad(outline(
                            Bounds { origin: pt, size: handle_size },
                            theme.accent,
                            BorderStyle::Solid,
                        ));
                    }
                } else {
                    // Fullscreen dim overlay when no selection is active yet
                    window.paint_quad(fill(bounds, rgba(0x00000055)));
                }

                // Render vector annotations
                let all_annotations = annotations.iter().chain(current_drawing.iter());
                for ann in all_annotations {
                    match ann {
                        Annotation::Line { from, to, color, width } => {
                            let mut pb = PathBuilder::stroke(px(*width));
                            pb.move_to(*from);
                            pb.line_to(*to);
                            if let Ok(p) = pb.build() {
                                window.paint_path(p, *color);
                            }
                        }
                        Annotation::Arrow { from, to, color, width } => {
                            let mut pb = PathBuilder::stroke(px(*width));
                            pb.move_to(*from);
                            pb.line_to(*to);
                            if let Ok(p) = pb.build() {
                                window.paint_path(p, *color);
                            }

                            let dx = f32::from(to.x) - f32::from(from.x);
                            let dy = f32::from(to.y) - f32::from(from.y);
                            let len = (dx * dx + dy * dy).sqrt();
                            if len > 5.0 {
                                let head_len = (*width * 3.5).clamp(12.0, 24.0);
                                let angle = dy.atan2(dx);
                                let arrow_angle = 0.5;

                                let ax1 = f32::from(to.x) - head_len * (angle - arrow_angle).cos();
                                let ay1 = f32::from(to.y) - head_len * (angle - arrow_angle).sin();
                                let ax2 = f32::from(to.x) - head_len * (angle + arrow_angle).cos();
                                let ay2 = f32::from(to.y) - head_len * (angle + arrow_angle).sin();

                                let mut head_b = PathBuilder::fill();
                                head_b.move_to(*to);
                                head_b.line_to(point(px(ax1), px(ay1)));
                                head_b.line_to(point(px(ax2), px(ay2)));
                                head_b.close();
                                if let Ok(hp) = head_b.build() {
                                    window.paint_path(hp, *color);
                                }
                            }
                        }
                        Annotation::Pencil { points, color, width } => {
                            if points.len() >= 2 {
                                let mut pb = PathBuilder::stroke(px(*width));
                                pb.move_to(points[0]);
                                for pt in &points[1..] {
                                    pb.line_to(*pt);
                                }
                                if let Ok(p) = pb.build() {
                                    window.paint_path(p, *color);
                                }
                            }
                        }
                        Annotation::Marker { points, color, width } => {
                            if points.len() >= 2 {
                                let mut marker_c = *color;
                                marker_c.a = 0.35;
                                let mut pb = PathBuilder::stroke(px(*width * 3.0));
                                pb.move_to(points[0]);
                                for pt in &points[1..] {
                                    pb.line_to(*pt);
                                }
                                if let Ok(p) = pb.build() {
                                    window.paint_path(p, marker_c);
                                }
                            }
                        }
                        Annotation::Rectangle { bounds, color, filled, width } => {
                            if *filled {
                                window.paint_quad(fill(*bounds, *color));
                            } else {
                                let mut pb = PathBuilder::stroke(px(*width));
                                let o = bounds.origin;
                                let s = bounds.size;
                                pb.move_to(o);
                                pb.line_to(point(o.x + s.width, o.y));
                                pb.line_to(point(o.x + s.width, o.y + s.height));
                                pb.line_to(point(o.x, o.y + s.height));
                                pb.close();
                                if let Ok(p) = pb.build() {
                                    window.paint_path(p, *color);
                                }
                            }
                        }
                        // Mathematically exact smooth Bézier ellipse (NOT a quad/square!)
                        Annotation::Ellipse { bounds, color, filled, width } => {
                            let cx = f32::from(bounds.origin.x) + f32::from(bounds.size.width) / 2.0;
                            let cy = f32::from(bounds.origin.y) + f32::from(bounds.size.height) / 2.0;
                            let rx = (f32::from(bounds.size.width) / 2.0).abs();
                            let ry = (f32::from(bounds.size.height) / 2.0).abs();

                            if rx > 1.0 && ry > 1.0 {
                                let kx = rx * 0.55228475;
                                let ky = ry * 0.55228475;

                                let mut pb = if *filled {
                                    PathBuilder::fill()
                                } else {
                                    PathBuilder::stroke(px(*width))
                                };

                                pb.move_to(point(px(cx), px(cy - ry)));
                                pb.cubic_bezier_to(
                                    point(px(cx + rx), px(cy)),
                                    point(px(cx + kx), px(cy - ry)),
                                    point(px(cx + rx), px(cy - ky)),
                                );
                                pb.cubic_bezier_to(
                                    point(px(cx), px(cy + ry)),
                                    point(px(cx + rx), px(cy + ky)),
                                    point(px(cx + kx), px(cy + ry)),
                                );
                                pb.cubic_bezier_to(
                                    point(px(cx - rx), px(cy)),
                                    point(px(cx - kx), px(cy + ry)),
                                    point(px(cx - rx), px(cy + ky)),
                                );
                                pb.cubic_bezier_to(
                                    point(px(cx), px(cy - ry)),
                                    point(px(cx - rx), px(cy - ky)),
                                    point(px(cx - kx), px(cy - ry)),
                                );

                                if *filled {
                                    pb.close();
                                }
                                if let Ok(path) = pb.build() {
                                    window.paint_path(path, *color);
                                }
                            }
                        }
                        Annotation::Blur { .. } | Annotation::Pixelate { .. } => {}
                        Annotation::NumberBadge { center, color, radius, .. } => {
                            let badge_bounds = Bounds {
                                origin: point(center.x - px(*radius), center.y - px(*radius)),
                                size: size(px(*radius * 2.0), px(*radius * 2.0)),
                            };
                            window.paint_quad(fill(badge_bounds, *color));
                            window.paint_quad(outline(badge_bounds, rgb(0xFFFFFF), BorderStyle::Solid));
                        }
                        _ => {}
                    }
                }

                // Live preview indicator only while actively dragging Blur or Pixelate
                if let Some(drawing) = &current_drawing {
                    match drawing {
                        Annotation::Blur { bounds, .. } => {
                            window.paint_quad(fill(*bounds, rgba(0x8174FF22)));
                            window.paint_quad(outline(*bounds, rgba(0x8174FF88), BorderStyle::Dashed));
                        }
                        Annotation::Pixelate { bounds, .. } => {
                            window.paint_quad(fill(*bounds, rgba(0x30D15822)));
                            window.paint_quad(outline(*bounds, rgba(0x30D15888), BorderStyle::Dashed));
                        }
                        _ => {}
                    }
                }
            })
            .size_full(),
        );

        // Render text annotations as crisp UI elements
        for ann in &self.history.annotations {
            if let Annotation::Text { origin, text, color, font_size } = ann {
                root = root.child(
                    div()
                        .absolute()
                        .left(origin.x)
                        .top(origin.y)
                        .text_size(px(*font_size))
                        .text_color(*color)
                        .font_weight(gpui::FontWeight::BOLD)
                        .child(text.clone()),
                );
            }
            if let Annotation::NumberBadge { center, number, radius, .. } = ann {
                root = root.child(
                    div()
                        .absolute()
                        .left(center.x - px(*radius))
                        .top(center.y - px(*radius))
                        .size(px(*radius * 2.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(rgb(0xFFFFFF))
                        .child(number.to_string()),
                );
            }
        }

        // Active text input box
        if self.text_input_mode {
            root = root.child(
                div()
                    .absolute()
                    .left(self.text_input_pos.x)
                    .top(self.text_input_pos.y)
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(rgba(0x151519EE))
                    .border_1()
                    .border_color(theme.accent)
                    .shadow_lg()
                    .child(
                        div()
                            .text_size(px(self.tool_props.font_size))
                            .text_color(self.tool_props.stroke_color)
                            .font_weight(gpui::FontWeight::BOLD)
                            .child(format!("{}▏", self.text_input_buffer)),
                    ),
            );
        }

        // Dimension badge
        if selection.is_active && self.config.show_dimensions {
            let dim_text = format!(
                "{} × {}",
                selection.width().round() as u32,
                selection.height().round() as u32
            );

            let mut badge_y = selection.y() - 28.0;
            if badge_y < 8.0 {
                badge_y = selection.y() + 8.0;
            }

            root = root.child(
                div()
                    .absolute()
                    .left(px(selection.x()))
                    .top(px(badge_y))
                    .px_2()
                    .py_0p5()
                    .rounded_md()
                    .bg(rgb(0x151519))
                    .border_1()
                    .border_color(rgb(0x282832))
                    .shadow_md()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(rgb(0xF2F2F5))
                    .child(dim_text),
            );
        }

        // Magnifier loupe
        if show_magnifier {
            if let Some(info) = self.magnifier_info {
                root = root.child(render_magnifier(self.cursor_pos, info, screen_bounds));
            }
        }

        // CleanShot instruction badge before selection
        if !selection.is_active && self.drag_mode == SelectionDragMode::None {
            let scr_w = f32::from(screen_bounds.size.width);
            let scr_h = f32::from(screen_bounds.size.height);
            root = root.child(
                div()
                    .absolute()
                    .left(px((scr_w - 420.0) / 2.0))
                    .top(px((scr_h - 44.0) / 2.0))
                    .w(px(420.0))
                    .py_2()
                    .px_4()
                    .rounded_full()
                    .bg(rgba(0x151519F0))
                    .border_1()
                    .border_color(rgba(0xFFFFFF20))
                    .shadow_2xl()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(rgb(0xD0D0D8))
                            .child(t(
                                lang,
                                "Drag to select area  •  Space for fullscreen  •  Esc to cancel",
                                "Выделите область  •  Пробел — весь экран  •  Esc — отмена",
                            )),
                    ),
            );
        }

        // CleanShot X Floating Toolbar: shown after releasing mouse (not while actively dragging creation)
        let is_creating = matches!(self.drag_mode, SelectionDragMode::Creating { .. });
        if selection.is_active && !is_creating {
            let lang = self.config.language.resolve();
            let active_tool = self.active_tool;
            let current_color = self.tool_props.stroke_color;
            let stroke_width = self.tool_props.stroke_width;
            let theme_ref = &self.theme;

            let show_props = active_tool != ToolKind::Select && active_tool != ToolKind::Eyedropper;
            let bar_height = if show_props {
                TOOLBAR_TOTAL_HEIGHT
            } else {
                TOOLBAR_MAIN_HEIGHT
            };

            let toolbar_pos = calculate_toolbar_position(
                selection.bounds,
                screen_bounds,
                TOOLBAR_WIDTH,
                bar_height,
            );

            self.current_toolbar_bounds = Some(Bounds {
                origin: toolbar_pos,
                size: size(px(TOOLBAR_WIDTH), px(bar_height)),
            });

            let mut toolbar_col = div()
                .absolute()
                .left(toolbar_pos.x)
                .top(toolbar_pos.y)
                .flex()
                .flex_col()
                .gap_1p5()
                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                })
                .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                    this.palette_drag_mode = PaletteDragMode::None;
                    cx.stop_propagation();
                    cx.notify();
                }));

            // Main Toolbar with SVG Icons
            toolbar_col = toolbar_col.child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .py_1p5()
                    .rounded_xl()
                    .bg(theme.bg_surface)
                    .border_1()
                    .border_color(theme.border_subtle)
                    .shadow_2xl()
                    // Group 1: Navigation
                    .child(render_toolbar_button(
                        "btn-select".into(),
                        "icons/select.svg",
                        active_tool == ToolKind::Select,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Select;
                            cx.notify();
                        }),
                        cx.listener(move |this, hovered, _, cx| {
                            this.set_tooltip(t(lang, "Select / Move", "Выделение / Перемещение"), 24.0, *hovered, cx);
                        }),
                    ))
                    .child(render_toolbar_divider(theme_ref))
                    // Group 2: Annotations
                    .child(render_toolbar_button(
                        "btn-pencil".into(),
                        "icons/pencil.svg",
                        active_tool == ToolKind::Pencil,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Pencil;
                            cx.notify();
                        }),
                        cx.listener(move |this, hovered, _, cx| {
                            this.set_tooltip(t(lang, "Freehand Pencil", "Карандаш"), 65.0, *hovered, cx);
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-marker".into(),
                        "icons/marker.svg",
                        active_tool == ToolKind::Marker,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Marker;
                            cx.notify();
                        }),
                        cx.listener(move |this, hovered, _, cx| {
                            this.set_tooltip(t(lang, "Highlighter Marker", "Маркер"), 101.0, *hovered, cx);
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-arrow".into(),
                        "icons/arrow.svg",
                        active_tool == ToolKind::Arrow,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Arrow;
                            cx.notify();
                        }),
                        cx.listener(move |this, hovered, _, cx| {
                            this.set_tooltip(t(lang, "Draw Arrow", "Стрелка"), 137.0, *hovered, cx);
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-line".into(),
                        "icons/line.svg",
                        active_tool == ToolKind::Line,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Line;
                            cx.notify();
                        }),
                        cx.listener(move |this, hovered, _, cx| {
                            this.set_tooltip(t(lang, "Straight Line", "Прямая линия"), 173.0, *hovered, cx);
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-rect".into(),
                        "icons/rectangle.svg",
                        active_tool == ToolKind::Rectangle,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Rectangle;
                            cx.notify();
                        }),
                        cx.listener(move |this, hovered, _, cx| {
                            this.set_tooltip(t(lang, "Rectangle", "Прямоугольник"), 209.0, *hovered, cx);
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-circle".into(),
                        "icons/circle.svg",
                        active_tool == ToolKind::Ellipse,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Ellipse;
                            cx.notify();
                        }),
                        cx.listener(move |this, hovered, _, cx| {
                            this.set_tooltip(t(lang, "Ellipse / Circle", "Эллипс / Окружность"), 245.0, *hovered, cx);
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-text".into(),
                        "icons/text.svg",
                        active_tool == ToolKind::Text,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Text;
                            cx.notify();
                        }),
                        cx.listener(move |this, hovered, _, cx| {
                            this.set_tooltip(t(lang, "Text Annotation", "Текст"), 281.0, *hovered, cx);
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-step".into(),
                        "icons/step.svg",
                        active_tool == ToolKind::NumberBadge,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::NumberBadge;
                            cx.notify();
                        }),
                        cx.listener(move |this, hovered, _, cx| {
                            this.set_tooltip(t(lang, "Step Badge Counter", "Шаги / Нумерация"), 317.0, *hovered, cx);
                        }),
                    ))
                    .child(render_toolbar_divider(theme_ref))
                    // Group 3: Privacy & Utilities
                    .child(render_toolbar_button(
                        "btn-blur".into(),
                        "icons/blur.svg",
                        active_tool == ToolKind::Blur,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Blur;
                            cx.notify();
                        }),
                        cx.listener(move |this, hovered, _, cx| {
                            this.set_tooltip(t(lang, "Blur Sensitive Region", "Размытие"), 358.0, *hovered, cx);
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-pixelate".into(),
                        "icons/pixelate.svg",
                        active_tool == ToolKind::Pixelate,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Pixelate;
                            cx.notify();
                        }),
                        cx.listener(move |this, hovered, _, cx| {
                            this.set_tooltip(t(lang, "Pixelate Mosaic", "Пикселизация"), 394.0, *hovered, cx);
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-dropper".into(),
                        "icons/eyedropper.svg",
                        active_tool == ToolKind::Eyedropper,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Eyedropper;
                            cx.notify();
                        }),
                        cx.listener(move |this, hovered, _, cx| {
                            this.set_tooltip(t(lang, "Eyedropper Color Picker", "Пипетка цвета"), 430.0, *hovered, cx);
                        }),
                    ))
                    .child(render_toolbar_divider(theme_ref))
                    // Group 4: Quick Actions
                    .child(render_toolbar_text_button(
                        "btn-ocr".into(),
                        "OCR",
                        self.is_ocr_running,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.do_ocr(cx);
                        }),
                        cx.listener(move |this, hovered, _, cx| {
                            this.set_tooltip(t(lang, "Windows WinRT Text Recognition", "Распознавание текста (OCR)"), 473.0, *hovered, cx);
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-undo".into(),
                        "icons/undo.svg",
                        false,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.do_undo(cx);
                        }),
                        cx.listener(move |this, hovered, _, cx| {
                            this.set_tooltip(t(lang, "Undo (Ctrl+Z)", "Отменить (Ctrl+Z)"), 511.0, *hovered, cx);
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-redo".into(),
                        "icons/redo.svg",
                        false,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.do_redo(cx);
                        }),
                        cx.listener(move |this, hovered, _, cx| {
                            this.set_tooltip(t(lang, "Redo (Ctrl+Y)", "Повторить (Ctrl+Y)"), 547.0, *hovered, cx);
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-save".into(),
                        "icons/save.svg",
                        false,
                        theme_ref,
                        cx.listener(|this, _, window, cx| {
                            this.do_save(window, cx);
                        }),
                        cx.listener(move |this, hovered, _, cx| {
                            this.set_tooltip(t(lang, "Save to PNG (Ctrl+S)", "Сохранить в PNG (Ctrl+S)"), 583.0, *hovered, cx);
                        }),
                    ))
                    .child(
                        div()
                            .id("btn-copy")
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .px_3()
                            .h(px(32.0))
                            .rounded_lg()
                            .bg(theme.accent)
                            .hover(|s| s.bg(theme.accent_hover))
                            .cursor_pointer()
                            .shadow_md()
                            .on_hover(cx.listener(move |this, hovered, _, cx| {
                                this.set_tooltip(t(lang, "Copy to Clipboard (Ctrl+C)", "Копировать в буфер (Ctrl+C)"), 637.0, *hovered, cx);
                            }))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.do_copy(window, cx);
                            }))
                            .child(
                                svg()
                                    .path("icons/check.svg")
                                    .size(px(14.0))
                                    .text_color(rgb(0xFFFFFF)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xFFFFFF))
                                    .child(t(lang, "Copy", "Копировать")),
                            ),
                    )
                    .child(
                        div()
                            .id("btn-close")
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(32.0))
                            .rounded_lg()
                            .bg(theme.bg_secondary)
                            .border_1()
                            .border_color(theme.border_subtle)
                            .hover(|s| s.bg(rgba(0xFF453A22)).border_color(rgb(0xFF453A)))
                            .cursor_pointer()
                            .on_hover(cx.listener(move |this, hovered, _, cx| {
                                this.set_tooltip(t(lang, "Close Overlay (Esc)", "Закрыть (Esc)"), 691.0, *hovered, cx);
                            }))
                            .on_click(cx.listener(|_, _, window, _| {
                                window.remove_window();
                            }))
                            .child(
                                svg()
                                    .path("icons/close.svg")
                                    .size(px(16.0))
                                    .text_color(theme.text_muted),
                            ),
                    ),
            );

            // Property Sub-Bar (Blur Intensity, Pixelate Block Size, or Colors & Stroke Width)
            if active_tool == ToolKind::Blur {
                toolbar_col = toolbar_col.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_1p5()
                        .rounded_xl()
                        .bg(theme.bg_surface)
                        .border_1()
                        .border_color(theme.border_subtle)
                        .shadow_xl()
                        .child(
                            svg()
                                .path("icons/blur.svg")
                                .size(px(16.0))
                                .text_color(theme.accent),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme.text_secondary)
                                .child("Blur Hardness:"),
                        )
                        // Stepper: [-] [ 12px ] [+]
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .id("btn-dec-blur")
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(22.0))
                                        .rounded_md()
                                        .bg(theme.bg_secondary)
                                        .border_1()
                                        .border_color(theme.border_subtle)
                                        .hover(|s| s.bg(theme.bg_hover))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.tool_props.blur_radius = this.tool_props.blur_radius.saturating_sub(2).max(2);
                                            cx.notify();
                                        }))
                                        .child(svg().path("icons/minus.svg").size(px(12.0)).text_color(theme.text_secondary)),
                                )
                                .child(
                                    div()
                                        .id("blur-radius-display")
                                        .px_2()
                                        .py_0p5()
                                        .min_w(px(40.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded_md()
                                        .bg(if self.numeric_input_mode == Some(NumericInputTarget::BlurRadius) {
                                            rgba(0x8174FF33)
                                        } else {
                                            theme.bg_secondary
                                        })
                                        .border_1()
                                        .border_color(if self.numeric_input_mode == Some(NumericInputTarget::BlurRadius) {
                                            theme.accent
                                        } else {
                                            theme.border_subtle
                                        })
                                        .hover(|s| s.border_color(theme.border_focus))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.numeric_input_mode = Some(NumericInputTarget::BlurRadius);
                                            this.numeric_input_buffer = this.tool_props.blur_radius.to_string();
                                            cx.notify();
                                        }))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(theme.text_primary)
                                                .child(if self.numeric_input_mode == Some(NumericInputTarget::BlurRadius) {
                                                    format!("{}▏", self.numeric_input_buffer)
                                                } else {
                                                    format!("{}px", self.tool_props.blur_radius)
                                                }),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("btn-inc-blur")
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(22.0))
                                        .rounded_md()
                                        .bg(theme.bg_secondary)
                                        .border_1()
                                        .border_color(theme.border_subtle)
                                        .hover(|s| s.bg(theme.bg_hover))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.tool_props.blur_radius = (this.tool_props.blur_radius + 2).min(48);
                                            cx.notify();
                                        }))
                                        .child(svg().path("icons/plus.svg").size(px(12.0)).text_color(theme.text_secondary)),
                                ),
                        )
                        .child(render_toolbar_divider(theme_ref))
                        // Presets: 6px Soft, 12px Medium, 24px Strong, 36px Heavy
                        .child(render_blur_preset("preset-b-6".into(), t(lang, "6px Soft", "6px Мягкое"), self.tool_props.blur_radius == 6, cx.listener(|this, _, _, cx| {
                            this.tool_props.blur_radius = 6;
                            cx.notify();
                        })))
                        .child(render_blur_preset("preset-b-12".into(), t(lang, "12px Medium", "12px Среднее"), self.tool_props.blur_radius == 12, cx.listener(|this, _, _, cx| {
                            this.tool_props.blur_radius = 12;
                            cx.notify();
                        })))
                        .child(render_blur_preset("preset-b-24".into(), t(lang, "24px Strong", "24px Сильное"), self.tool_props.blur_radius == 24, cx.listener(|this, _, _, cx| {
                            this.tool_props.blur_radius = 24;
                            cx.notify();
                        })))
                        .child(render_blur_preset("preset-b-36".into(), t(lang, "36px Heavy", "36px Максимальное"), self.tool_props.blur_radius == 36, cx.listener(|this, _, _, cx| {
                            this.tool_props.blur_radius = 36;
                            cx.notify();
                        }))),
                );
            } else if active_tool == ToolKind::Pixelate {
                toolbar_col = toolbar_col.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_1p5()
                        .rounded_xl()
                        .bg(theme.bg_surface)
                        .border_1()
                        .border_color(theme.border_subtle)
                        .shadow_xl()
                        .child(
                            svg()
                                .path("icons/pixelate.svg")
                                .size(px(16.0))
                                .text_color(theme.accent),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme.text_secondary)
                                .child("Pixel Size:"),
                        )
                        // Stepper: [-] [ 10px ] [+]
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .id("btn-dec-pixel")
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(22.0))
                                        .rounded_md()
                                        .bg(theme.bg_secondary)
                                        .border_1()
                                        .border_color(theme.border_subtle)
                                        .hover(|s| s.bg(theme.bg_hover))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.tool_props.pixelate_block = this.tool_props.pixelate_block.saturating_sub(2).max(2);
                                            cx.notify();
                                        }))
                                        .child(svg().path("icons/minus.svg").size(px(12.0)).text_color(theme.text_secondary)),
                                )
                                .child(
                                    div()
                                        .id("pixel-size-display")
                                        .px_2()
                                        .py_0p5()
                                        .min_w(px(40.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded_md()
                                        .bg(if self.numeric_input_mode == Some(NumericInputTarget::PixelateBlock) {
                                            rgba(0x8174FF33)
                                        } else {
                                            theme.bg_secondary
                                        })
                                        .border_1()
                                        .border_color(if self.numeric_input_mode == Some(NumericInputTarget::PixelateBlock) {
                                            theme.accent
                                        } else {
                                            theme.border_subtle
                                        })
                                        .hover(|s| s.border_color(theme.border_focus))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.numeric_input_mode = Some(NumericInputTarget::PixelateBlock);
                                            this.numeric_input_buffer = this.tool_props.pixelate_block.to_string();
                                            cx.notify();
                                        }))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(theme.text_primary)
                                                .child(if self.numeric_input_mode == Some(NumericInputTarget::PixelateBlock) {
                                                    format!("{}▏", self.numeric_input_buffer)
                                                } else {
                                                    format!("{}px", self.tool_props.pixelate_block)
                                                }),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("btn-inc-pixel")
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(22.0))
                                        .rounded_md()
                                        .bg(theme.bg_secondary)
                                        .border_1()
                                        .border_color(theme.border_subtle)
                                        .hover(|s| s.bg(theme.bg_hover))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.tool_props.pixelate_block = (this.tool_props.pixelate_block + 2).min(48);
                                            cx.notify();
                                        }))
                                        .child(svg().path("icons/plus.svg").size(px(12.0)).text_color(theme.text_secondary)),
                                ),
                        )
                        .child(render_toolbar_divider(theme_ref))
                        // Presets: 6px Fine, 10px Medium, 16px Coarse, 24px Heavy
                        .child(render_blur_preset("preset-p-6".into(), t(lang, "6px Fine", "6px Мелкая"), self.tool_props.pixelate_block == 6, cx.listener(|this, _, _, cx| {
                            this.tool_props.pixelate_block = 6;
                            cx.notify();
                        })))
                        .child(render_blur_preset("preset-p-10".into(), t(lang, "10px Medium", "10px Средняя"), self.tool_props.pixelate_block == 10, cx.listener(|this, _, _, cx| {
                            this.tool_props.pixelate_block = 10;
                            cx.notify();
                        })))
                        .child(render_blur_preset("preset-p-16".into(), t(lang, "16px Coarse", "16px Крупная"), self.tool_props.pixelate_block == 16, cx.listener(|this, _, _, cx| {
                            this.tool_props.pixelate_block = 16;
                            cx.notify();
                        })))
                        .child(render_blur_preset("preset-p-24".into(), t(lang, "24px Heavy", "24px Максимальная"), self.tool_props.pixelate_block == 24, cx.listener(|this, _, _, cx| {
                            this.tool_props.pixelate_block = 24;
                            cx.notify();
                        }))),
                );
            } else if show_props {
                toolbar_col = toolbar_col.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1p5()
                        .px_2p5()
                        .py_1p5()
                        .rounded_xl()
                        .bg(theme.bg_surface)
                        .border_1()
                        .border_color(theme.border_subtle)
                        .shadow_xl()
                        // 8 Quick CleanShot Colors
                        .child(render_color_swatch(
                            "swatch-red".into(),
                            theme.palette_red,
                            current_color == theme.palette_red,
                            cx.listener(|this, _, _, cx| {
                                this.tool_props.stroke_color = this.theme.palette_red;
                                cx.notify();
                            }),
                        ))
                        .child(render_color_swatch(
                            "swatch-orange".into(),
                            theme.palette_orange,
                            current_color == theme.palette_orange,
                            cx.listener(|this, _, _, cx| {
                                this.tool_props.stroke_color = this.theme.palette_orange;
                                cx.notify();
                            }),
                        ))
                        .child(render_color_swatch(
                            "swatch-yellow".into(),
                            theme.palette_yellow,
                            current_color == theme.palette_yellow,
                            cx.listener(|this, _, _, cx| {
                                this.tool_props.stroke_color = this.theme.palette_yellow;
                                cx.notify();
                            }),
                        ))
                        .child(render_color_swatch(
                            "swatch-green".into(),
                            theme.palette_green,
                            current_color == theme.palette_green,
                            cx.listener(|this, _, _, cx| {
                                this.tool_props.stroke_color = this.theme.palette_green;
                                cx.notify();
                            }),
                        ))
                        .child(render_color_swatch(
                            "swatch-cyan".into(),
                            theme.palette_cyan,
                            current_color == theme.palette_cyan,
                            cx.listener(|this, _, _, cx| {
                                this.tool_props.stroke_color = this.theme.palette_cyan;
                                cx.notify();
                            }),
                        ))
                        .child(render_color_swatch(
                            "swatch-blue".into(),
                            theme.palette_blue,
                            current_color == theme.palette_blue,
                            cx.listener(|this, _, _, cx| {
                                this.tool_props.stroke_color = this.theme.palette_blue;
                                cx.notify();
                            }),
                        ))
                        .child(render_color_swatch(
                            "swatch-purple".into(),
                            theme.palette_purple,
                            current_color == theme.palette_purple,
                            cx.listener(|this, _, _, cx| {
                                this.tool_props.stroke_color = this.theme.palette_purple;
                                cx.notify();
                            }),
                        ))
                        .child(render_color_swatch(
                            "swatch-white".into(),
                            theme.palette_white,
                            current_color == theme.palette_white,
                            cx.listener(|this, _, _, cx| {
                                this.tool_props.stroke_color = this.theme.palette_white;
                                cx.notify();
                            }),
                        ))
                        // Full Palette Button (Opens Spectrum & Custom HEX Input)
                        .child(
                            div()
                                .id("btn-open-palette")
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(24.0))
                                .rounded_full()
                                .bg(if self.show_color_palette {
                                    rgba(0x8174FF33)
                                } else {
                                    theme.bg_secondary
                                })
                                .border_1()
                                .border_color(if self.show_color_palette {
                                    theme.accent
                                } else {
                                    theme.border_subtle
                                })
                                .hover(|s| s.bg(theme.bg_hover))
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.show_color_palette = !this.show_color_palette;
                                    if this.show_color_palette {
                                        this.sync_picker_from_color();
                                    }
                                    cx.notify();
                                }))
                                .child(
                                    svg()
                                        .path("icons/palette.svg")
                                        .size(px(14.0))
                                        .text_color(if self.show_color_palette {
                                            theme.accent
                                        } else {
                                            theme.text_secondary
                                        }),
                                ),
                        )
                        .child(render_toolbar_divider(theme_ref))
                        // Dynamic Stroke Width Stepper: [-] [ 4px ] [+]
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .id("btn-decrease-width")
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(22.0))
                                        .rounded_md()
                                        .bg(theme.bg_secondary)
                                        .border_1()
                                        .border_color(theme.border_subtle)
                                        .hover(|s| s.bg(theme.bg_hover))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.tool_props.stroke_width =
                                                (this.tool_props.stroke_width - 1.0).max(1.0);
                                            cx.notify();
                                        }))
                                        .child(
                                            svg()
                                                .path("icons/minus.svg")
                                                .size(px(12.0))
                                                .text_color(theme.text_secondary),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("stroke-width-display")
                                        .px_2()
                                        .py_0p5()
                                        .min_w(px(40.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded_md()
                                        .bg(if self.numeric_input_mode == Some(NumericInputTarget::StrokeWidth) {
                                            rgba(0x8174FF33)
                                        } else {
                                            theme.bg_secondary
                                        })
                                        .border_1()
                                        .border_color(if self.numeric_input_mode == Some(NumericInputTarget::StrokeWidth) {
                                            theme.accent
                                        } else {
                                            theme.border_subtle
                                        })
                                        .hover(|s| s.border_color(theme.border_focus))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.numeric_input_mode = Some(NumericInputTarget::StrokeWidth);
                                            this.numeric_input_buffer = (this.tool_props.stroke_width as u32).to_string();
                                            cx.notify();
                                        }))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(theme.text_primary)
                                                .child(if self.numeric_input_mode == Some(NumericInputTarget::StrokeWidth) {
                                                    format!("{}▏", self.numeric_input_buffer)
                                                } else {
                                                    format!("{}px", stroke_width as u32)
                                                }),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("btn-increase-width")
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(22.0))
                                        .rounded_md()
                                        .bg(theme.bg_secondary)
                                        .border_1()
                                        .border_color(theme.border_subtle)
                                        .hover(|s| s.bg(theme.bg_hover))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.tool_props.stroke_width =
                                                (this.tool_props.stroke_width + 1.0).min(48.0);
                                            cx.notify();
                                        }))
                                        .child(
                                            svg()
                                                .path("icons/plus.svg")
                                                .size(px(12.0))
                                                .text_color(theme.text_secondary),
                                        ),
                                ),
                        )
                        // Fill toggle for Rect & Ellipse
                        .when(active_tool == ToolKind::Rectangle || active_tool == ToolKind::Ellipse, |p| {
                            p.child(render_toolbar_divider(theme_ref)).child(
                                div()
                                    .id("toggle-fill")
                                    .px_2()
                                    .py_0p5()
                                    .rounded_md()
                                    .bg(if self.tool_props.is_filled {
                                        rgba(0x8174FF33)
                                    } else {
                                        theme.bg_secondary
                                    })
                                    .border_1()
                                    .border_color(if self.tool_props.is_filled {
                                        theme.accent
                                    } else {
                                        theme.border_subtle
                                    })
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.tool_props.is_filled = !this.tool_props.is_filled;
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .text_color(theme.text_primary)
                                            .child(if self.tool_props.is_filled {
                                                "Fill: ON"
                                            } else {
                                                "Outline"
                                            }),
                                    ),
                            )
                        })
                        // Font size for Text tool
                        .when(active_tool == ToolKind::Text, |p| {
                            p.child(render_toolbar_divider(theme_ref)).child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .id("font-14px")
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded_md()
                                            .bg(if (self.tool_props.font_size - 14.0).abs() < 0.1 {
                                                rgba(0x8174FF33)
                                            } else {
                                                theme.bg_secondary
                                            })
                                            .border_1()
                                            .border_color(if (self.tool_props.font_size - 14.0).abs() < 0.1 {
                                                theme.accent
                                            } else {
                                                theme.border_subtle
                                            })
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.tool_props.font_size = 14.0;
                                                cx.notify();
                                            }))
                                            .child(div().text_xs().text_color(theme.text_primary).child("14")),
                                    )
                                    .child(
                                        div()
                                            .id("font-18px")
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded_md()
                                            .bg(if (self.tool_props.font_size - 18.0).abs() < 0.1 {
                                                rgba(0x8174FF33)
                                            } else {
                                                theme.bg_secondary
                                            })
                                            .border_1()
                                            .border_color(if (self.tool_props.font_size - 18.0).abs() < 0.1 {
                                                theme.accent
                                            } else {
                                                theme.border_subtle
                                            })
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.tool_props.font_size = 18.0;
                                                cx.notify();
                                            }))
                                            .child(div().text_xs().text_color(theme.text_primary).child("18")),
                                    )
                                    .child(
                                        div()
                                            .id("font-24px")
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded_md()
                                            .bg(if (self.tool_props.font_size - 24.0).abs() < 0.1 {
                                                rgba(0x8174FF33)
                                            } else {
                                                theme.bg_secondary
                                            })
                                            .border_1()
                                            .border_color(if (self.tool_props.font_size - 24.0).abs() < 0.1 {
                                                theme.accent
                                            } else {
                                                theme.border_subtle
                                            })
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.tool_props.font_size = 24.0;
                                                cx.notify();
                                            }))
                                            .child(div().text_xs().text_color(theme.text_primary).child("24")),
                                    ),
                            )
                        }),
                );
            }

            root = root.child(toolbar_col);

            // Floating Tooltip: dynamically positioned strictly ABOVE or BELOW toolbar icons depending on situation
            if let Some(tip) = self.hovered_tooltip {
                let scr_w = f32::from(screen_bounds.size.width);
                let scr_h = f32::from(screen_bounds.size.height);
                let bar_y = f32::from(toolbar_pos.y);
                let sel_y = f32::from(selection.bounds.origin.y);
                let sel_h = f32::from(selection.bounds.size.height);

                // Determine whether toolbar is placed above selection, below selection, or inside
                let show_above = if bar_y + bar_height <= sel_y + 4.0 {
                    // Toolbar is positioned ABOVE selection:
                    // Show tooltip ABOVE toolbar (away from selection), unless screen top prevents it
                    bar_y - 34.0 >= 8.0
                } else if bar_y >= sel_y + sel_h - 4.0 {
                    // Toolbar is positioned BELOW selection:
                    // Show tooltip BELOW toolbar (away from selection), unless screen bottom prevents it
                    bar_y + bar_height + 34.0 > scr_h - 8.0
                } else {
                    // Toolbar is positioned INSIDE selection (e.g. nearly full-screen selection):
                    // Show above if toolbar is in lower half of screen, otherwise below
                    bar_y + bar_height + 34.0 > scr_h - 8.0 || bar_y > scr_h / 2.0
                };

                let tip_y = if show_above {
                    (bar_y - 34.0).max(8.0)
                } else {
                    (bar_y + bar_height + 8.0).min(scr_h - 34.0)
                };

                let approx_w = (tip.text.chars().count() as f32 * 7.5 + 20.0).max(56.0);
                let tip_x = (f32::from(toolbar_pos.x) + tip.rel_x - approx_w / 2.0)
                    .clamp(12.0, (scr_w - approx_w - 12.0).max(12.0));

                root = root.child(
                    div()
                        .absolute()
                        .left(px(tip_x))
                        .top(px(tip_y))
                        .px_2p5()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(0x18181E))
                        .border_1()
                        .border_color(rgb(0x323242))
                        .shadow_2xl()
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(rgb(0xF2F2F5))
                        .child(tip.text),
                );
            }

            // Expanded Google-Style 2D HSV Gradient Color Studio Dropdown
            if self.show_color_palette {
                let cur_r = (current_color.r * 255.0).round().clamp(0.0, 255.0) as u8;
                let cur_g = (current_color.g * 255.0).round().clamp(0.0, 255.0) as u8;
                let cur_b = (current_color.b * 255.0).round().clamp(0.0, 255.0) as u8;
                let hex_code = format!("#{:02x}{:02x}{:02x}", cur_r, cur_g, cur_b);
                let (h_deg, s_hsl, l_hsl) = rgb_to_hsl(cur_r, cur_g, cur_b);
                let (c_cmyk, m_cmyk, y_cmyk, k_cmyk) = rgb_to_cmyk(cur_r, cur_g, cur_b);

                let hsv_h = self.picker_hue.round() as u32;
                let hsv_s = (self.picker_sat * 100.0).round() as u32;
                let hsv_v = (self.picker_val * 100.0).round() as u32;
                let hsl_h = h_deg.round() as u32;
                let hsl_s = (s_hsl * 100.0).round() as u32;
                let hsl_l = (l_hsl * 100.0).round() as u32;

                let grad_img = self.palette_gradient_image.clone();
                let hue_img = self.palette_hue_strip_image.clone();
                let sat = self.picker_sat;
                let val = self.picker_val;
                let hue = self.picker_hue;

                let scr_h = f32::from(screen_bounds.size.height);
                let palette_w = 480.0;
                let palette_h = 360.0;
                let top_section_h = 150.0;
                let left_preview_w = 130.0;
                let grad_w = 448.0 - left_preview_w;
                let hue_strip_w = 448.0;

                let palette_drop_up = f32::from(toolbar_pos.y) + bar_height + palette_h + 16.0 > scr_h - 12.0;
                let palette_top = if palette_drop_up {
                    (f32::from(toolbar_pos.y) - palette_h - 8.0).max(12.0)
                } else {
                    f32::from(toolbar_pos.y) + bar_height + 8.0
                };
                let palette_left = f32::from(toolbar_pos.x) + (TOOLBAR_WIDTH - palette_w) / 2.0;

                let grad_top = palette_top + 16.0 + 34.0;
                let grad_left = palette_left + 16.0 + left_preview_w;

                self.gradient_field_bounds = Some(Bounds {
                    origin: point(px(grad_left), px(grad_top)),
                    size: size(px(grad_w), px(top_section_h)),
                });

                let hue_top = grad_top + top_section_h + 12.0;
                let hue_left = palette_left + 16.0;

                self.hue_slider_bounds = Some(Bounds {
                    origin: point(px(hue_left), px(hue_top)),
                    size: size(px(hue_strip_w), px(20.0)),
                });

                let render_format_card = |label: &'static str, val_str: String, cx: &mut Context<Self>| {
                    let copy_str = val_str.clone();
                    div()
                        .id(ElementId::NamedInteger("card-format".into(), label.as_ptr() as u64))
                        .flex_1()
                        .px_2()
                        .py_1p5()
                        .rounded_md()
                        .bg(theme.bg_secondary)
                        .border_1()
                        .border_color(theme.border_subtle)
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .cursor_pointer()
                        .hover(|s| s.border_color(theme.border_focus))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let _ = copy_text_to_clipboard(&copy_str);
                            this.show_toast(format!("Copied {}: {}", label, copy_str), cx);
                        }))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(theme.text_muted)
                                .child(label),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme.text_primary)
                                .child(val_str),
                        )
                };

                root = root.child(
                    div()
                        .id("palette-dropdown-panel")
                        .absolute()
                        .left(px(palette_left))
                        .top(px(palette_top))
                        .w(px(palette_w))
                        .p_4()
                        .rounded_xl()
                        .bg(theme.bg_surface)
                        .border_1()
                        .border_color(theme.border_subtle)
                        .shadow_2xl()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                            cx.stop_propagation();
                        })
                        .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                            this.palette_drag_mode = PaletteDragMode::None;
                            cx.stop_propagation();
                            cx.notify();
                        }))
                        // 1. Header: "Палитра" + Close Button
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(theme.text_primary)
                                        .child("Палитра"),
                                )
                                .child(
                                    div()
                                        .id("btn-close-palette-box")
                                        .size(px(22.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded_md()
                                        .bg(theme.bg_secondary)
                                        .border_1()
                                        .border_color(theme.border_subtle)
                                        .hover(|s| s.bg(rgba(0xFF453A22)).border_color(rgb(0xFF453A)))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.show_color_palette = false;
                                            this.hex_input_mode = false;
                                            cx.notify();
                                        }))
                                        .child(
                                            svg()
                                                .path("icons/close.svg")
                                                .size(px(12.0))
                                                .text_color(theme.text_muted),
                                        ),
                                ),
                        )
                        // 2. Top section: Left preview block + Right 2D Saturation-Value gradient
                        .child(
                            div()
                                .flex()
                                .h(px(top_section_h))
                                .rounded_lg()
                                .border_1()
                                .border_color(theme.border_subtle)
                                .overflow_hidden()
                                // Left solid preview
                                .child(
                                    div()
                                        .w(px(left_preview_w))
                                        .h_full()
                                        .bg(current_color)
                                        .border_r_1()
                                        .border_color(rgba(0xFFFFFF18)),
                                )
                                // Right 2D gradient canvas
                                .child(
                                    div()
                                        .id("palette-2d-canvas-wrap")
                                        .flex_1()
                                        .h_full()
                                        .cursor_crosshair()
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                                                cx.stop_propagation();
                                                this.palette_drag_mode = PaletteDragMode::SaturationValue;
                                                if let Some(gb) = this.gradient_field_bounds {
                                                    let px_f = f32::from(ev.position.x);
                                                    let py_f = f32::from(ev.position.y);
                                                    let rel_x = (px_f - f32::from(gb.origin.x)).clamp(0.0, f32::from(gb.size.width));
                                                    let rel_y = (py_f - f32::from(gb.origin.y)).clamp(0.0, f32::from(gb.size.height));
                                                    this.set_picker_sat_val(rel_x / f32::from(gb.size.width), 1.0 - (rel_y / f32::from(gb.size.height)), cx);
                                                }
                                            }),
                                        )
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.palette_drag_mode = PaletteDragMode::None;
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        )
                                        .child(
                                            canvas(|_, _, _| (), move |bounds, _, window, _cx| {
                                                if let Some(img) = &grad_img {
                                                    let _ = window.paint_image(bounds, Corners::default(), img.clone(), 0, false);
                                                }
                                                let center_x = bounds.origin.x + px(sat * f32::from(bounds.size.width));
                                                let center_y = bounds.origin.y + px((1.0 - val) * f32::from(bounds.size.height));
                                                let r = px(7.0);
                                                let circle_bounds = Bounds {
                                                    origin: point(center_x - r, center_y - r),
                                                    size: size(r * 2.0, r * 2.0),
                                                };
                                                window.paint_quad(outline(
                                                    Bounds {
                                                        origin: point(circle_bounds.origin.x + px(0.5), circle_bounds.origin.y + px(0.5)),
                                                        size: circle_bounds.size,
                                                    },
                                                    rgba(0x00000088),
                                                    BorderStyle::Solid,
                                                ));
                                                window.paint_quad(outline(circle_bounds, rgb(0xFFFFFF), BorderStyle::Solid));
                                            })
                                            .size_full(),
                                        ),
                                ),
                        )
                        // 3. Continuous Rainbow Hue Slider Bar
                        .child(
                            div()
                                .id("palette-hue-slider-wrap")
                                .w_full()
                                .h(px(20.0))
                                .flex()
                                .items_center()
                                .cursor_pointer()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                                        cx.stop_propagation();
                                        this.palette_drag_mode = PaletteDragMode::Hue;
                                        if let Some(hb) = this.hue_slider_bounds {
                                            let px_f = f32::from(ev.position.x);
                                            let rel_x = (px_f - f32::from(hb.origin.x)).clamp(0.0, f32::from(hb.size.width));
                                            this.set_picker_hue(rel_x / f32::from(hb.size.width) * 360.0, cx);
                                        }
                                    }),
                                )
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.palette_drag_mode = PaletteDragMode::None;
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                )
                                .child(
                                    canvas(|_, _, _| (), move |bounds, _, window, _cx| {
                                        if let Some(img) = &hue_img {
                                            let _ = window.paint_image(bounds, Corners::all(px(5.0)), img.clone(), 0, false);
                                        }
                                        let center_x = bounds.origin.x + px((hue / 360.0) * f32::from(bounds.size.width));
                                        let center_y = bounds.origin.y + bounds.size.height / 2.0;
                                        let r = px(8.0);
                                        let thumb_bounds = Bounds {
                                            origin: point(center_x - r, center_y - r),
                                            size: size(r * 2.0, r * 2.0),
                                        };
                                        let (hr, hg, hb) = hsv_to_rgb(hue, 1.0, 1.0);
                                        let thumb_col = rgb(((hr as u32) << 16) | ((hg as u32) << 8) | (hb as u32));
                                        window.paint_quad(fill(thumb_bounds, thumb_col));
                                        window.paint_quad(outline(thumb_bounds, rgb(0xFFFFFF), BorderStyle::Solid));
                                    })
                                    .w_full()
                                    .h(px(12.0)),
                                ),
                        )
                        // 4. Wide HEX input bar with Copy Button
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .bg(theme.bg_secondary)
                                .border_1()
                                .border_color(if self.hex_input_mode {
                                    theme.accent
                                } else {
                                    theme.border_subtle
                                })
                                .child(
                                    div()
                                        .id("hex-text-click")
                                        .flex()
                                        .flex_col()
                                        .gap_0p5()
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.hex_input_mode = true;
                                            cx.notify();
                                        }))
                                        .child(
                                            div()
                                                .text_size(px(10.0))
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(theme.text_muted)
                                                .child("HEX"),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(theme.text_primary)
                                                .child(if self.hex_input_mode {
                                                    format!("{}▏", self.custom_hex_input)
                                                } else {
                                                    hex_code.clone()
                                                }),
                                        ),
                                )
                                .child({
                                    let to_copy = hex_code.clone();
                                    div()
                                        .id("btn-copy-hex-google")
                                        .p_1p5()
                                        .rounded_md()
                                        .bg(theme.bg_surface)
                                        .border_1()
                                        .border_color(theme.border_subtle)
                                        .hover(|s| s.bg(theme.bg_hover).border_color(theme.border_focus))
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.copy_hex_color(&to_copy, cx);
                                        }))
                                        .child(
                                            svg()
                                                .path("icons/copy.svg")
                                                .size(px(16.0))
                                                .text_color(theme.text_secondary),
                                        )
                                }),
                        )
                        // 5. Four format cards: RGB, CMYK, HSV, HSL
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(render_format_card(
                                    "RGB",
                                    format!("{}, {}, {}", cur_r, cur_g, cur_b),
                                    cx,
                                ))
                                .child(render_format_card(
                                    "CMYK",
                                    format!("{}%, {}%, {}%, {}%", c_cmyk, m_cmyk, y_cmyk, k_cmyk),
                                    cx,
                                ))
                                .child(render_format_card(
                                    "HSV",
                                    format!("{}°, {}%, {}%", hsv_h, hsv_s, hsv_v),
                                    cx,
                                ))
                                .child(render_format_card(
                                    "HSL",
                                    format!("{}°, {}%, {}%", hsl_h, hsl_s, hsl_l),
                                    cx,
                                )),
                        ),
                );
            }
        }

        // CleanShot toast feedback
        if let Some((msg, created)) = &self.toast_message {
            if created.elapsed() < Duration::from_secs(3) {
                root = root.child(
                    div()
                        .absolute()
                        .bottom(px(32.0))
                        .left(px((f32::from(screen_size.width) - 340.0) / 2.0))
                        .w(px(340.0))
                        .py_2()
                        .px_4()
                        .rounded_lg()
                        .bg(theme.bg_surface)
                        .border_1()
                        .border_color(theme.accent)
                        .shadow_2xl()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(theme.text_primary)
                                .child(msg.clone()),
                        ),
                );
            }
        }

        root
    }
}

impl Drop for ScreenOverlayView {
    fn drop(&mut self) {
        crate::app::IS_OVERLAY_OPEN.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}
