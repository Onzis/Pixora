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
use crate::image::clipboard::{copy_image_to_clipboard, copy_text_to_clipboard};
use crate::image::export::auto_save_screenshot;
use crate::ocr::recognition::recognize_text_from_image;
use crate::overlay::magnifier::{MagnifierInfo, render_magnifier};
use crate::overlay::selection::{SelectionDragMode, SelectionState};
use crate::overlay::toolbar::{
    HUE_STOPS, TOOLBAR_MAIN_HEIGHT, TOOLBAR_TOTAL_HEIGHT, TOOLBAR_WIDTH, TooltipView,
    calculate_toolbar_position, hsv_to_rgb, parse_hex_color, render_blur_preset,
    render_color_swatch, render_toolbar_button, render_toolbar_divider,
    render_toolbar_text_button, rgb_to_hsv,
};
use crate::system::windows::{PhysicalRect, play_capture_sound};
use crate::theme::PixoraTheme;

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

    // Extended Color Palette & Custom Hex
    pub show_color_palette: bool,
    pub custom_hex_input: String,
    pub hex_input_mode: bool,
    pub current_toolbar_bounds: Option<Bounds<Pixels>>,
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

            show_color_palette: false,
            custom_hex_input: "8174FF".to_string(),
            hex_input_mode: false,
            current_toolbar_bounds: None,
        }
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
        if let Some(final_img) = self.prepare_final_image() {
            if let Err(e) = copy_image_to_clipboard(&final_img) {
                self.show_toast(format!("Copy failed: {}", e), cx);
            } else {
                if self.config.sound_effects {
                    play_capture_sound();
                }
                window.remove_window();
            }
        } else {
            self.show_toast("Select an area first", cx);
        }
    }

    pub fn do_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                    self.show_toast(format!("Saved: {}", filename), cx);
                    window.remove_window();
                }
                Err(e) => {
                    self.show_toast(format!("Save error: {}", e), cx);
                }
            }
        } else {
            self.show_toast("Select an area first", cx);
        }
    }

    pub fn do_ocr(&mut self, cx: &mut Context<Self>) {
        if let Some(img) = self.prepare_final_image() {
            self.is_ocr_running = true;
            cx.notify();

            match recognize_text_from_image(&img) {
                Ok(output) => {
                    if output.full_text.trim().is_empty() {
                        self.show_toast("No text found in selected area", cx);
                    } else {
                        let _ = copy_text_to_clipboard(&output.full_text);
                        let preview: String = output.full_text.chars().take(28).collect();
                        self.show_toast(format!("OCR copied: \"{}...\"", preview), cx);
                    }
                }
                Err(e) => {
                    self.show_toast(format!("OCR failed: {}", e), cx);
                }
            }
            self.is_ocr_running = false;
            cx.notify();
        } else {
            self.show_toast("Select an area with text first", cx);
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
                let pad = px(8.0);
                let safe_box = Bounds {
                    origin: point(tb.origin.x - pad, tb.origin.y - pad),
                    size: size(tb.size.width + pad * 2.0, tb.size.height + pad * 2.0),
                };
                if safe_box.contains(&p) {
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
        match self.drag_mode {
            SelectionDragMode::Creating { start } => {
                let w = px_f - f32::from(start.x);
                let h = py_f - f32::from(start.y);
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
                let delta = point(p.x - start_mouse.x, p.y - start_mouse.y);
                self.selection.update_resize(handle, orig_bounds, delta);
                cx.notify();
            }
            SelectionDragMode::None => {
                if !self.selection.is_active || self.active_tool == ToolKind::Eyedropper {
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
            match key {
                "c" => self.do_copy(window, cx),
                "s" => self.do_save(window, cx),
                "z" => {
                    self.history.undo();
                    cx.notify();
                }
                "y" => {
                    self.history.redo();
                    cx.notify();
                }
                _ => {}
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
        let show_magnifier = self.config.show_magnifier
            && (!selection.is_active || self.active_tool == ToolKind::Eyedropper);

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
                        Annotation::Blur { bounds, .. } => {
                            window.paint_quad(fill(*bounds, rgba(0x8174FF22)));
                            window.paint_quad(outline(*bounds, rgba(0x8174FF88), BorderStyle::Dashed));
                        }
                        Annotation::Pixelate { bounds, .. } => {
                            window.paint_quad(fill(*bounds, rgba(0x30D15822)));
                            window.paint_quad(outline(*bounds, rgba(0x30D15888), BorderStyle::Dashed));
                        }
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
                            .child("Drag to select area  •  Space for fullscreen  •  Esc to cancel"),
                    ),
            );
        }

        // CleanShot X Floating Toolbar
        if selection.is_active {
            let active_tool = self.active_tool;
            let current_color = self.tool_props.stroke_color;
            let stroke_width = self.tool_props.stroke_width;
            let theme_ref = &self.theme;

            let show_props = active_tool != ToolKind::Select && active_tool != ToolKind::Eyedropper;
            let bar_height = if self.show_color_palette {
                440.0
            } else if show_props {
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
                .on_mouse_up(MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                });

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
                        "Select / Move",
                        active_tool == ToolKind::Select,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Select;
                            cx.notify();
                        }),
                    ))
                    .child(render_toolbar_divider(theme_ref))
                    // Group 2: Annotations
                    .child(render_toolbar_button(
                        "btn-pencil".into(),
                        "icons/pencil.svg",
                        "Freehand Pencil",
                        active_tool == ToolKind::Pencil,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Pencil;
                            cx.notify();
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-marker".into(),
                        "icons/marker.svg",
                        "Highlighter Marker",
                        active_tool == ToolKind::Marker,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Marker;
                            cx.notify();
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-arrow".into(),
                        "icons/arrow.svg",
                        "Draw Arrow",
                        active_tool == ToolKind::Arrow,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Arrow;
                            cx.notify();
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-line".into(),
                        "icons/line.svg",
                        "Straight Line",
                        active_tool == ToolKind::Line,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Line;
                            cx.notify();
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-rect".into(),
                        "icons/rectangle.svg",
                        "Rectangle",
                        active_tool == ToolKind::Rectangle,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Rectangle;
                            cx.notify();
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-circle".into(),
                        "icons/circle.svg",
                        "Ellipse / Circle",
                        active_tool == ToolKind::Ellipse,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Ellipse;
                            cx.notify();
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-text".into(),
                        "icons/text.svg",
                        "Text Annotation",
                        active_tool == ToolKind::Text,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Text;
                            cx.notify();
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-step".into(),
                        "icons/step.svg",
                        "Step Badge Counter",
                        active_tool == ToolKind::NumberBadge,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::NumberBadge;
                            cx.notify();
                        }),
                    ))
                    .child(render_toolbar_divider(theme_ref))
                    // Group 3: Privacy & Utilities
                    .child(render_toolbar_button(
                        "btn-blur".into(),
                        "icons/blur.svg",
                        "Blur Sensitive Region",
                        active_tool == ToolKind::Blur,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Blur;
                            cx.notify();
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-pixelate".into(),
                        "icons/pixelate.svg",
                        "Pixelate Mosaic",
                        active_tool == ToolKind::Pixelate,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Pixelate;
                            cx.notify();
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-dropper".into(),
                        "icons/eyedropper.svg",
                        "Eyedropper Color Picker",
                        active_tool == ToolKind::Eyedropper,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Eyedropper;
                            cx.notify();
                        }),
                    ))
                    .child(render_toolbar_divider(theme_ref))
                    // Group 4: Quick Actions
                    .child(render_toolbar_text_button(
                        "btn-ocr".into(),
                        "OCR",
                        "Windows WinRT Text Recognition",
                        self.is_ocr_running,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.do_ocr(cx);
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-undo".into(),
                        "icons/undo.svg",
                        "Undo (Ctrl+Z)",
                        false,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.history.undo();
                            this.rebuild_render_image();
                            cx.notify();
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-redo".into(),
                        "icons/redo.svg",
                        "Redo (Ctrl+Y)",
                        false,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.history.redo();
                            this.rebuild_render_image();
                            cx.notify();
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-save".into(),
                        "icons/save.svg",
                        "Save to PNG (Ctrl+S)",
                        false,
                        theme_ref,
                        cx.listener(|this, _, window, cx| {
                            this.do_save(window, cx);
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
                            .tooltip(move |_window, cx| {
                                cx.new(|_| TooltipView {
                                    text: "Copy to Clipboard (Ctrl+C)".to_string(),
                                })
                                .into()
                            })
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
                                    .child("Copy"),
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
                            .tooltip(move |_window, cx| {
                                cx.new(|_| TooltipView {
                                    text: "Close Overlay (Esc)".to_string(),
                                })
                                .into()
                            })
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
                                        .bg(theme.bg_secondary)
                                        .border_1()
                                        .border_color(theme.border_subtle)
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(theme.text_primary)
                                                .child(format!("{}px", self.tool_props.blur_radius)),
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
                        .child(render_blur_preset("preset-b-6".into(), "6px Soft", self.tool_props.blur_radius == 6, cx.listener(|this, _, _, cx| {
                            this.tool_props.blur_radius = 6;
                            cx.notify();
                        })))
                        .child(render_blur_preset("preset-b-12".into(), "12px Medium", self.tool_props.blur_radius == 12, cx.listener(|this, _, _, cx| {
                            this.tool_props.blur_radius = 12;
                            cx.notify();
                        })))
                        .child(render_blur_preset("preset-b-24".into(), "24px Strong", self.tool_props.blur_radius == 24, cx.listener(|this, _, _, cx| {
                            this.tool_props.blur_radius = 24;
                            cx.notify();
                        })))
                        .child(render_blur_preset("preset-b-36".into(), "36px Heavy", self.tool_props.blur_radius == 36, cx.listener(|this, _, _, cx| {
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
                                        .bg(theme.bg_secondary)
                                        .border_1()
                                        .border_color(theme.border_subtle)
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(theme.text_primary)
                                                .child(format!("{}px", self.tool_props.pixelate_block)),
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
                        .child(render_blur_preset("preset-p-6".into(), "6px Fine", self.tool_props.pixelate_block == 6, cx.listener(|this, _, _, cx| {
                            this.tool_props.pixelate_block = 6;
                            cx.notify();
                        })))
                        .child(render_blur_preset("preset-p-10".into(), "10px Medium", self.tool_props.pixelate_block == 10, cx.listener(|this, _, _, cx| {
                            this.tool_props.pixelate_block = 10;
                            cx.notify();
                        })))
                        .child(render_blur_preset("preset-p-16".into(), "16px Coarse", self.tool_props.pixelate_block == 16, cx.listener(|this, _, _, cx| {
                            this.tool_props.pixelate_block = 16;
                            cx.notify();
                        })))
                        .child(render_blur_preset("preset-p-24".into(), "24px Heavy", self.tool_props.pixelate_block == 24, cx.listener(|this, _, _, cx| {
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
                                        .min_w(px(36.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded_md()
                                        .bg(theme.bg_secondary)
                                        .border_1()
                                        .border_color(theme.border_subtle)
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(theme.text_primary)
                                                .child(format!("{}px", stroke_width as u32)),
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

            // Expanded True RGB Color Studio Popup
            if self.show_color_palette {
                let cur_r = (current_color.r * 255.0).round().clamp(0.0, 255.0) as u8;
                let cur_g = (current_color.g * 255.0).round().clamp(0.0, 255.0) as u8;
                let cur_b = (current_color.b * 255.0).round().clamp(0.0, 255.0) as u8;
                let hex_str = format!("#{:02X}{:02X}{:02X}", cur_r, cur_g, cur_b);
                let (cur_h, _, _) = rgb_to_hsv(cur_r, cur_g, cur_b);

                // 1. Hue Rainbow Spectrum Bar (0° - 360°)
                let mut hue_row = div()
                    .flex()
                    .items_center()
                    .gap_1();

                for hue in HUE_STOPS {
                    let (hr, hg, hb) = hsv_to_rgb(hue, 1.0, 1.0);
                    let h_col = rgb(((hr as u32) << 16) | ((hg as u32) << 8) | (hb as u32));
                    let is_active_hue = (cur_h - hue).abs() < 8.0 || (cur_h - hue).abs() > 352.0;

                    hue_row = hue_row.child(
                        div()
                            .id(ElementId::NamedInteger("hue-stop".into(), hue as u64))
                            .flex_1()
                            .h(px(16.0))
                            .rounded_sm()
                            .bg(h_col)
                            .border_1()
                            .border_color(if is_active_hue {
                                rgb(0xFFFFFF)
                            } else {
                                rgba(0x00000033)
                            })
                            .hover(|s| s.border_color(rgb(0xFFFFFF)))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_hue_color(hue, cx);
                            })),
                    );
                }

                // Helper for rendering an RGB channel row
                let render_channel_row = |ch_idx: usize,
                                          ch_label: &'static str,
                                          ch_val: u8,
                                          ch_color: gpui::Rgba,
                                          badge_bg: gpui::Rgba,
                                          theme: PixoraTheme,
                                          cx: &mut Context<Self>| {
                    let bar_w = (ch_val as f32 / 255.0 * 140.0).clamp(2.0, 140.0);
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        // Channel Badge (R, G, B)
                        .child(
                            div()
                                .size(px(20.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_md()
                                .bg(badge_bg)
                                .border_1()
                                .border_color(ch_color)
                                .text_xs()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(ch_color)
                                .child(ch_label),
                        )
                        // -10 Stepper
                        .child(
                            div()
                                .id(ElementId::NamedInteger("ch-dec10".into(), (ch_idx * 10 + 1) as u64))
                                .px_1p5()
                                .py_0p5()
                                .rounded_md()
                                .bg(theme.bg_secondary)
                                .border_1()
                                .border_color(theme.border_subtle)
                                .hover(|s| s.bg(theme.bg_hover))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.update_rgb_channel(ch_idx, -10, cx);
                                }))
                                .text_xs()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(theme.text_secondary)
                                .child("-10"),
                        )
                        // -1 Stepper
                        .child(
                            div()
                                .id(ElementId::NamedInteger("ch-dec1".into(), (ch_idx * 10 + 2) as u64))
                                .size(px(20.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_md()
                                .bg(theme.bg_secondary)
                                .border_1()
                                .border_color(theme.border_subtle)
                                .hover(|s| s.bg(theme.bg_hover))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.update_rgb_channel(ch_idx, -1, cx);
                                }))
                                .child(
                                    svg()
                                        .path("icons/minus.svg")
                                        .size(px(10.0))
                                        .text_color(theme.text_secondary),
                                ),
                        )
                        // Interactive Visual Channel Track
                        .child(
                            div()
                                .id(ElementId::NamedInteger("ch-track".into(), (ch_idx * 10 + 3) as u64))
                                .w(px(140.0))
                                .h(px(12.0))
                                .rounded_full()
                                .bg(rgb(0x181820))
                                .border_1()
                                .border_color(theme.border_subtle)
                                .overflow_hidden()
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let new_v = if ch_val > 128 { 0 } else { 255 };
                                    this.set_rgb_channel(ch_idx, new_v, cx);
                                }))
                                .child(
                                    div()
                                        .w(px(bar_w))
                                        .h_full()
                                        .rounded_full()
                                        .bg(ch_color),
                                ),
                        )
                        // +1 Stepper
                        .child(
                            div()
                                .id(ElementId::NamedInteger("ch-inc1".into(), (ch_idx * 10 + 4) as u64))
                                .size(px(20.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_md()
                                .bg(theme.bg_secondary)
                                .border_1()
                                .border_color(theme.border_subtle)
                                .hover(|s| s.bg(theme.bg_hover))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.update_rgb_channel(ch_idx, 1, cx);
                                }))
                                .child(
                                    svg()
                                        .path("icons/plus.svg")
                                        .size(px(10.0))
                                        .text_color(theme.text_secondary),
                                ),
                        )
                        // +10 Stepper
                        .child(
                            div()
                                .id(ElementId::NamedInteger("ch-inc10".into(), (ch_idx * 10 + 5) as u64))
                                .px_1p5()
                                .py_0p5()
                                .rounded_md()
                                .bg(theme.bg_secondary)
                                .border_1()
                                .border_color(theme.border_subtle)
                                .hover(|s| s.bg(theme.bg_hover))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.update_rgb_channel(ch_idx, 10, cx);
                                }))
                                .text_xs()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(theme.text_secondary)
                                .child("+10"),
                        )
                        // Numeric Value Box (0..255)
                        .child(
                            div()
                                .min_w(px(36.0))
                                .px_1p5()
                                .py_0p5()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_md()
                                .bg(theme.bg_secondary)
                                .border_1()
                                .border_color(theme.border_subtle)
                                .text_xs()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(theme.text_primary)
                                .child(format!("{}", ch_val)),
                        )
                };

                let r_row = render_channel_row(
                    0,
                    "R",
                    cur_r,
                    rgb(0xFF453A),
                    rgba(0xFF3B3022),
                    theme,
                    cx,
                );
                let g_row = render_channel_row(
                    1,
                    "G",
                    cur_g,
                    rgb(0x30D158),
                    rgba(0x34C75922),
                    theme,
                    cx,
                );
                let b_row = render_channel_row(
                    2,
                    "B",
                    cur_b,
                    rgb(0x0A84FF),
                    rgba(0x007AFF22),
                    theme,
                    cx,
                );

                // Quick Presets Row
                let mut presets_row = div()
                    .flex()
                    .items_center()
                    .gap_1p5();

                let quick_presets: [u32; 12] = [
                    0xFF3B30, 0xFF9500, 0xFFCC00, 0x34C759, 0x00C7BE, 0x32ADE6,
                    0x007AFF, 0x8174FF, 0xFF2D55, 0xFFFFFF, 0x8E8E93, 0x000000,
                ];

                for hex_val in quick_presets {
                    let c = rgb(hex_val);
                    let is_sel = current_color == c;
                    presets_row = presets_row.child(render_color_swatch(
                        ElementId::NamedInteger("preset-swatch".into(), hex_val as u64),
                        c,
                        is_sel,
                        cx.listener(move |this, _, _, cx| {
                            this.tool_props.stroke_color = c;
                            this.custom_hex_input = format!("{:06X}", hex_val);
                            cx.notify();
                        }),
                    ));
                }

                toolbar_col = toolbar_col.child(
                    div()
                        .w(px(460.0))
                        .px_3p5()
                        .py_3()
                        .rounded_xl()
                        .bg(theme.bg_surface)
                        .border_1()
                        .border_color(theme.border_subtle)
                        .shadow_2xl()
                        .flex()
                        .flex_col()
                        .gap_2p5()
                        // 1. Header with Preview Swatch, Color Values, Copy HEX & Eyedropper
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2p5()
                                        .child(
                                            div()
                                                .w(px(38.0))
                                                .h(px(26.0))
                                                .rounded_md()
                                                .bg(current_color)
                                                .border_2()
                                                .border_color(rgb(0xFFFFFF))
                                                .shadow_md(),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_0p5()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(gpui::FontWeight::BOLD)
                                                        .text_color(theme.text_primary)
                                                        .child(hex_str.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(10.0))
                                                        .text_color(theme.text_muted)
                                                        .child(format!("rgb({}, {}, {})", cur_r, cur_g, cur_b)),
                                                ),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1p5()
                                        // Copy HEX button
                                        .child({
                                            let hx = hex_str.clone();
                                            div()
                                                .id("btn-copy-hex-palette")
                                                .flex()
                                                .items_center()
                                                .gap_1()
                                                .px_2()
                                                .py_1()
                                                .rounded_md()
                                                .bg(theme.bg_secondary)
                                                .border_1()
                                                .border_color(theme.border_subtle)
                                                .hover(|s| s.bg(theme.bg_hover))
                                                .cursor_pointer()
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.copy_hex_color(&hx, cx);
                                                }))
                                                .child(
                                                    svg()
                                                        .path("icons/copy.svg")
                                                        .size(px(12.0))
                                                        .text_color(theme.text_secondary),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(theme.text_secondary)
                                                        .child("Copy HEX"),
                                                )
                                        })
                                        // Pick from Screen (Eyedropper)
                                        .child(
                                            div()
                                                .id("btn-eyedropper-palette")
                                                .flex()
                                                .items_center()
                                                .gap_1()
                                                .px_2()
                                                .py_1()
                                                .rounded_md()
                                                .bg(theme.bg_secondary)
                                                .border_1()
                                                .border_color(theme.border_subtle)
                                                .hover(|s| s.bg(theme.bg_hover))
                                                .cursor_pointer()
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.active_tool = ToolKind::Eyedropper;
                                                    this.show_color_palette = false;
                                                    cx.notify();
                                                }))
                                                .child(
                                                    svg()
                                                        .path("icons/eyedropper.svg")
                                                        .size(px(12.0))
                                                        .text_color(theme.accent),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(theme.accent)
                                                        .child("Pick"),
                                                ),
                                        )
                                        // Close button
                                        .child(
                                            div()
                                                .id("btn-close-palette")
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
                                ),
                        )
                        .child(
                            div()
                                .w_full()
                                .h(px(1.0))
                                .bg(theme.border_subtle),
                        )
                        // 2. Hue Rainbow Spectrum Bar (0° - 360°)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(theme.text_muted)
                                        .child("Hue Spectrum (0° – 360°):"),
                                )
                                .child(hue_row),
                        )
                        // 3. RGB Channels: Red, Green, Blue
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1p5()
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(theme.text_muted)
                                        .child("RGB Channels (0 – 255):"),
                                )
                                .child(r_row)
                                .child(g_row)
                                .child(b_row),
                        )
                        .child(
                            div()
                                .w_full()
                                .h(px(1.0))
                                .bg(theme.border_subtle),
                        )
                        // 4. Custom HEX Direct Input & Quick Presets
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    // HEX input field
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1p5()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(theme.text_muted)
                                                .child("#"),
                                        )
                                        .child(
                                            div()
                                                .id("hex-input-box")
                                                .px_2()
                                                .py_1()
                                                .min_w(px(72.0))
                                                .rounded_md()
                                                .bg(theme.bg_secondary)
                                                .border_1()
                                                .border_color(if self.hex_input_mode {
                                                    theme.accent
                                                } else {
                                                    theme.border_subtle
                                                })
                                                .cursor_pointer()
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.hex_input_mode = true;
                                                    cx.notify();
                                                }))
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(gpui::FontWeight::MEDIUM)
                                                        .text_color(theme.text_primary)
                                                        .child(if self.hex_input_mode {
                                                            format!("{}▏", self.custom_hex_input)
                                                        } else {
                                                            self.custom_hex_input.clone()
                                                        }),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .id("btn-apply-hex")
                                                .px_2()
                                                .py_1()
                                                .rounded_md()
                                                .bg(theme.accent)
                                                .hover(|s| s.bg(theme.accent_hover))
                                                .cursor_pointer()
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.apply_custom_hex(cx);
                                                }))
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                                        .text_color(rgb(0xFFFFFF))
                                                        .child("Apply"),
                                                ),
                                        ),
                                )
                                .child(presets_row),
                        ),
                );
            }

            root = root.child(toolbar_col);
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
