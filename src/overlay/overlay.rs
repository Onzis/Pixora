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
use crate::editor::annotations::{
    Annotation, AnnotationHandle, BadgeMode, BadgeShape,
    calculate_arrow_geometry, calculate_arrowhead, normalize_bounds, number_to_badge_label,
    sample_quadratic_bezier, text_bounds_dimensions, wrap_text_to_lines,
};
use crate::editor::canvas::render_annotations_to_image;
use crate::editor::history::EditHistory;
use crate::editor::properties::ToolProperties;
use crate::editor::tools::ToolKind;
use crate::i18n::t;
use crate::image::clipboard::{copy_image_to_clipboard, copy_text_to_clipboard};
use crate::image::export::auto_save_screenshot;
use crate::ocr::qrcode::decode_qr_from_image;
use crate::ocr::recognition::recognize_text_from_image;
use crate::overlay::magnifier::{MagnifierInfo, render_magnifier};
use crate::overlay::selection::{ResizeHandle, SelectionDragMode, SelectionState};
use crate::overlay::toolbar::{
    HoveredTooltip, TOOLBAR_MAIN_HEIGHT, TOOLBAR_TOTAL_HEIGHT, TOOLBAR_WIDTH,
    calculate_toolbar_position, generate_gradient_bitmap, generate_hue_strip_bitmap, hsv_to_rgb,
    parse_hex_color, render_color_swatch, render_toolbar_button,
    render_toolbar_divider, render_toolbar_dropdown_button, render_toolbar_text_button, rgb_to_cmyk, rgb_to_hsl, rgb_to_hsv,
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
    FontSize,
    SpotlightDim,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowSnapCandidate {
    pub bounds: Bounds<Pixels>,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AnnotationDragMode {
    None,
    Moving {
        start_mouse: Point<Pixels>,
        orig_annotation: Annotation,
    },
    DraggingEndpoint {
        is_start: bool,
        orig_annotation: Annotation,
    },
    DraggingBezierControl {
        orig_annotation: Annotation,
    },
    Resizing {
        handle: AnnotationHandle,
        start_mouse: Point<Pixels>,
        orig_annotation: Annotation,
        orig_bounds: Bounds<Pixels>,
    },
}

pub struct ScreenOverlayView {
    pub capture: ScreenCapture,
    pub render_image: Arc<RenderImage>,
    pub theme: PixoraTheme,
    pub config: AppConfig,

    pub selection: SelectionState,
    pub drag_mode: SelectionDragMode,

    pub selected_annotation: Option<usize>,
    pub annotation_drag_mode: AnnotationDragMode,

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

    // Window snapping
    pub detected_windows: Vec<WindowSnapCandidate>,
    pub hovered_window: Option<WindowSnapCandidate>,
    pub snap_candidate: Option<WindowSnapCandidate>,

    // Text annotation editing
    pub text_input_mode: bool,
    pub text_input_pos: Point<Pixels>,
    pub text_input_width: Option<f32>,
    pub text_input_height: Option<f32>,
    pub text_input_buffer: String,

    // Numeric value typing for px
    pub numeric_input_mode: Option<NumericInputTarget>,
    pub numeric_input_buffer: String,

    // 2D HSV Gradient Color Studio
    pub show_color_palette: bool,
    pub custom_hex_input: String,
    pub hex_input_mode: bool,
    pub current_toolbar_bounds: Option<Bounds<Pixels>>,
    pub palette_panel_bounds: Option<Bounds<Pixels>>,
    pub shapes_panel_bounds: Option<Bounds<Pixels>>,
    pub beautify_panel_bounds: Option<Bounds<Pixels>>,
    pub palette_drag_mode: PaletteDragMode,
    pub picker_hue: f32,
    pub picker_sat: f32,
    pub picker_val: f32,
    pub palette_gradient_image: Option<Arc<RenderImage>>,
    pub palette_hue_strip_image: Option<Arc<RenderImage>>,
    pub gradient_field_bounds: Option<Bounds<Pixels>>,
    pub hue_slider_bounds: Option<Bounds<Pixels>>,
    pub hovered_tooltip: Option<HoveredTooltip>,

    // Beautify & Pin
    pub is_beautify_active: bool,
    pub beautify_preset_idx: usize,
    pub show_shapes_menu: bool,
    pub show_beautify_menu: bool,
    pub shapes_btn_hovered: bool,
    pub shapes_menu_hovered: bool,
    pub shapes_hover_token: u32,
    pub beautify_btn_hovered: bool,
    pub beautify_menu_hovered: bool,
    pub beautify_hover_token: u32,
    pub last_used_shape: ToolKind,
    pub preset_thumbnails: Vec<(crate::image::beautify::BeautifyPreset, Arc<RenderImage>)>,
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

        let mut initial_cursor = point(px(0.0), px(0.0));
        let mut initial_magnifier = None;
        #[cfg(windows)]
        {
            use windows::Win32::Foundation::POINT;
            use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
            let mut pt = POINT { x: 0, y: 0 };
            if unsafe { GetCursorPos(&mut pt) }.is_ok() {
                let rx = (pt.x - capture.bounds.x).max(0) as u32;
                let ry = (pt.y - capture.bounds.y).max(0) as u32;
                initial_cursor = point(px(rx as f32), px(ry as f32));
                initial_magnifier = Some(MagnifierInfo::from_image(
                    &capture.image,
                    pt.x,
                    pt.y,
                    rx,
                    ry,
                ));
            }
        }

        let initial_stroke_color = parse_hex_color(&config.default_color_hex).unwrap_or(rgb(0xFF453A));
        let tool_props = ToolProperties {
            stroke_color: initial_stroke_color,
            stroke_width: config.default_stroke_width.clamp(1.0, 99.0),
            font_size: config.default_font_size.clamp(10.0, 96.0),
            blur_radius: config.blur_radius.clamp(2, 99),
            pixelate_block: config.pixelate_block.clamp(2, 99),
            spotlight_dim: config.spotlight_dim.clamp(0.10, 0.95),
            is_filled: false,
            current_badge_number: 1,
            ..ToolProperties::default()
        };
        let custom_hex = config.default_color_hex.trim_start_matches('#').to_string();

        let mut detected_windows = Vec::new();
        if config.window_snapping {
            let max_w = capture.bounds.width as f32;
            let max_h = capture.bounds.height as f32;
            let raw_windows = crate::system::windows::enumerate_visible_windows();
            for win in raw_windows {
                let win_x = (win.bounds.x - capture.bounds.x) as f32;
                let win_y = (win.bounds.y - capture.bounds.y) as f32;
                let win_w = win.bounds.width as f32;
                let win_h = win.bounds.height as f32;

                let cl_x = win_x.max(0.0);
                let cl_y = win_y.max(0.0);
                let cl_r = (win_x + win_w).min(max_w);
                let cl_b = (win_y + win_h).min(max_h);

                let cl_w = cl_r - cl_x;
                let cl_h = cl_b - cl_y;

                if cl_w >= 40.0 && cl_h >= 40.0 {
                    detected_windows.push(WindowSnapCandidate {
                        bounds: Bounds {
                            origin: point(px(cl_x), px(cl_y)),
                            size: size(px(cl_w), px(cl_h)),
                        },
                        title: win.title,
                    });
                }
            }
        }

        let hovered_window = if config.window_snapping {
            detected_windows.iter().find(|w| w.bounds.contains(&initial_cursor)).cloned()
        } else {
            None
        };

        Self {
            capture,
            render_image,
            theme: PixoraTheme::default(),
            config,

            selection: SelectionState::default(),
            drag_mode: SelectionDragMode::None,

            selected_annotation: None,
            annotation_drag_mode: AnnotationDragMode::None,

            active_tool: ToolKind::Select,
            tool_props,
            history: EditHistory::new(),
            current_drawing: None,
            drawing_start: None,

            cursor_pos: initial_cursor,
            magnifier_info: initial_magnifier,
            focus_handle,

            toast_message: None,
            is_ocr_running: false,

            detected_windows,
            hovered_window,
            snap_candidate: None,

            text_input_mode: false,
            text_input_pos: point(px(0.0), px(0.0)),
            text_input_width: None,
            text_input_height: None,
            text_input_buffer: String::new(),

            numeric_input_mode: None,
            numeric_input_buffer: String::new(),

            show_color_palette: false,
            custom_hex_input: custom_hex,
            hex_input_mode: false,
            current_toolbar_bounds: None,
            palette_panel_bounds: None,
            shapes_panel_bounds: None,
            beautify_panel_bounds: None,
            palette_drag_mode: PaletteDragMode::None,
            picker_hue: 246.0,
            picker_sat: 0.55,
            picker_val: 1.0,
            palette_gradient_image: Some(generate_gradient_bitmap(246.0, 160, 80)),
            palette_hue_strip_image: Some(generate_hue_strip_bitmap(448, 12)),
            gradient_field_bounds: None,
            hue_slider_bounds: None,
            hovered_tooltip: None,
            is_beautify_active: false,
            beautify_preset_idx: 0,
            show_shapes_menu: false,
            show_beautify_menu: false,
            shapes_btn_hovered: false,
            shapes_menu_hovered: false,
            shapes_hover_token: 0,
            beautify_btn_hovered: false,
            beautify_menu_hovered: false,
            beautify_hover_token: 0,
            last_used_shape: ToolKind::Rectangle,
            preset_thumbnails: crate::image::beautify::BeautifyPreset::all()
                .iter()
                .map(|&p| (p, crate::overlay::toolbar::generate_preset_thumbnail(p, 88, 54)))
                .collect(),
        }
    }

    pub fn schedule_check_close_shapes_menu(&mut self, cx: &mut Context<Self>) {
        self.shapes_hover_token = self.shapes_hover_token.wrapping_add(1);
        let token = self.shapes_hover_token;
        cx.spawn(move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
            let cx = cx.clone();
            async move {
                cx.background_executor().timer(Duration::from_millis(250)).await;
                let _ = cx.update(|app| {
                    let _ = this.update(app, |view, cx| {
                        if view.shapes_hover_token == token
                            && !view.shapes_btn_hovered
                            && !view.shapes_menu_hovered
                        {
                            view.show_shapes_menu = false;
                            cx.notify();
                        }
                    });
                });
            }
        })
        .detach();
    }

    pub fn schedule_check_close_beautify_menu(&mut self, cx: &mut Context<Self>) {
        self.beautify_hover_token = self.beautify_hover_token.wrapping_add(1);
        let token = self.beautify_hover_token;
        cx.spawn(move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
            let cx = cx.clone();
            async move {
                cx.background_executor().timer(Duration::from_millis(250)).await;
                let _ = cx.update(|app| {
                    let _ = this.update(app, |view, cx| {
                        if view.beautify_hover_token == token
                            && !view.beautify_btn_hovered
                            && !view.beautify_menu_hovered
                        {
                            view.show_beautify_menu = false;
                            cx.notify();
                        }
                    });
                });
            }
        })
        .detach();
    }

    pub fn set_tooltip(&mut self, text: &'static str, rel_x: f32, hovered: bool, cx: &mut Context<Self>) {
        if hovered {
            self.hovered_tooltip = Some(HoveredTooltip { text, rel_x });
        } else if self.hovered_tooltip.map(|t| t.text) == Some(text) {
            self.hovered_tooltip = None;
        }
        cx.notify();
    }

    pub fn sync_tool_props(&mut self) {
        self.config.default_stroke_width = self.tool_props.stroke_width;
        self.config.blur_radius = self.tool_props.blur_radius;
        self.config.pixelate_block = self.tool_props.pixelate_block;
        self.config.spotlight_dim = self.tool_props.spotlight_dim;
        self.config.default_font_size = self.tool_props.font_size;

        let r = (self.tool_props.stroke_color.r * 255.0).round().clamp(0.0, 255.0) as u8;
        let g = (self.tool_props.stroke_color.g * 255.0).round().clamp(0.0, 255.0) as u8;
        let b = (self.tool_props.stroke_color.b * 255.0).round().clamp(0.0, 255.0) as u8;
        self.config.default_color_hex = format!("#{:02X}{:02X}{:02X}", r, g, b);
    }

    pub fn update_selected_annotation_from_props(&mut self) {
        if let Some(idx) = self.selected_annotation {
            if idx < self.history.annotations.len() {
                self.history.snapshot();
                let ann = &mut self.history.annotations[idx];
                ann.set_color(self.tool_props.stroke_color);
                ann.set_stroke_width(self.tool_props.stroke_width);
                ann.set_font_size(self.tool_props.font_size);
                ann.set_filled(self.tool_props.is_filled);
                if let Annotation::Arrow { .. } = ann {
                    ann.set_two_headed(self.tool_props.arrow_two_headed);
                    ann.set_curved(self.tool_props.arrow_curved);
                }
                if let Annotation::NumberBadge { .. } = ann {
                    ann.set_badge_shape(self.tool_props.badge_shape);
                    ann.set_badge_mode(self.tool_props.badge_mode);
                    ann.set_filled(self.tool_props.badge_filled);
                }
                match ann {
                    Annotation::Blur { radius, .. } => *radius = self.tool_props.blur_radius,
                    Annotation::Pixelate { block_size, .. } => *block_size = self.tool_props.pixelate_block,
                    Annotation::Spotlight { dim_opacity, .. } => *dim_opacity = self.tool_props.spotlight_dim,
                    _ => {}
                }
                let is_filter_or_spotlight = matches!(
                    ann,
                    Annotation::Blur { .. } | Annotation::Pixelate { .. } | Annotation::Spotlight { .. }
                );
                if is_filter_or_spotlight {
                    self.rebuild_render_image();
                }
            }
        }
    }

    pub fn sync_and_save_tool_props(&mut self) {
        self.sync_tool_props();
        self.update_selected_annotation_from_props();
        let _ = self.config.save();
    }

    pub fn sync_picker_from_color(&mut self) {
        let r = (self.tool_props.stroke_color.r * 255.0).round().clamp(0.0, 255.0) as u8;
        let g = (self.tool_props.stroke_color.g * 255.0).round().clamp(0.0, 255.0) as u8;
        let b = (self.tool_props.stroke_color.b * 255.0).round().clamp(0.0, 255.0) as u8;
        let (h, s, v) = rgb_to_hsv(r, g, b);
        self.picker_hue = h;
        self.picker_sat = s;
        self.picker_val = v;
        self.palette_gradient_image = Some(generate_gradient_bitmap(h, 160, 80));
        self.custom_hex_input = format!("{:02X}{:02X}{:02X}", r, g, b);
    }

    pub fn set_picker_hue(&mut self, hue: f32, cx: &mut Context<Self>) {
        let hue = hue.clamp(0.0, 360.0);
        if (self.picker_hue - hue).abs() < 0.2 {
            return;
        }
        self.picker_hue = hue;
        self.palette_gradient_image = Some(generate_gradient_bitmap(self.picker_hue, 160, 80));
        self.apply_hsv_to_color(cx);
    }

    pub fn set_picker_sat_val(&mut self, sat: f32, val: f32, cx: &mut Context<Self>) {
        let sat = sat.clamp(0.0, 1.0);
        let val = val.clamp(0.0, 1.0);
        if (self.picker_sat - sat).abs() < 0.002 && (self.picker_val - val).abs() < 0.002 {
            return;
        }
        self.picker_sat = sat;
        self.picker_val = val;
        self.apply_hsv_to_color(cx);
    }

    pub fn apply_hsv_to_color(&mut self, cx: &mut Context<Self>) {
        let (r, g, b) = hsv_to_rgb(self.picker_hue, self.picker_sat, self.picker_val);
        self.tool_props.stroke_color = rgb(((r as u32) << 16) | ((g as u32) << 8) | (b as u32));
        self.custom_hex_input = format!("{:02X}{:02X}{:02X}", r, g, b);
        self.sync_tool_props();
        cx.notify();
    }

    pub fn show_toast(&mut self, msg: impl Into<String>, cx: &mut Context<Self>) {
        self.toast_message = Some((msg.into(), Instant::now()));
        cx.notify();
    }

    pub fn rebuild_render_image(&mut self) {
        let mut img = self.capture.image.clone();

        // 1. Process Spotlight annotations first so dark mask covers the selection area and cutouts are created
        let (sel_x, sel_y, sel_w, sel_h) = if self.selection.is_active {
            (
                (self.selection.x().max(0.0) as u32).min(img.width()),
                (self.selection.y().max(0.0) as u32).min(img.height()),
                (self.selection.width() as u32).min(img.width().saturating_sub(self.selection.x().max(0.0) as u32)),
                (self.selection.height() as u32).min(img.height().saturating_sub(self.selection.y().max(0.0) as u32)),
            )
        } else {
            (0, 0, img.width(), img.height())
        };

        let spotlights: Vec<crate::image::processing::SpotlightRegion> = self.history.annotations
            .iter()
            .filter_map(|ann| {
                if let Annotation::Spotlight { bounds, color, width, dim_opacity } = ann {
                    let sx = f32::from(bounds.origin.x);
                    let sy = f32::from(bounds.origin.y);
                    let sw = f32::from(bounds.size.width);
                    let sh = f32::from(bounds.size.height);
                    let (norm_x, norm_w) = if sw < 0.0 { (sx + sw, -sw) } else { (sx, sw) };
                    let (norm_y, norm_h) = if sh < 0.0 { (sy + sh, -sh) } else { (sy, sh) };
                    Some(crate::image::processing::SpotlightRegion {
                        x: norm_x,
                        y: norm_y,
                        width: norm_w,
                        height: norm_h,
                        border_color: *color,
                        border_width: *width,
                        dim_opacity: *dim_opacity,
                    })
                } else {
                    None
                }
            })
            .collect();

        if !spotlights.is_empty() && sel_w > 0 && sel_h > 0 {
            let max_dim = spotlights.iter().map(|s| s.dim_opacity).fold(0.0f32, f32::max);
            crate::image::processing::apply_spotlights(
                &mut img,
                sel_x,
                sel_y,
                sel_w,
                sel_h,
                &spotlights,
                max_dim,
                0.0,
            );
        }

        // 2. Process Blur and Pixelate annotations
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

    pub fn cancel_selection(&mut self, cx: &mut Context<Self>) {
        self.selection.is_active = false;
        self.selection = SelectionState::new(0.0, 0.0, 0.0, 0.0);
        self.drag_mode = SelectionDragMode::None;
        self.selected_annotation = None;
        self.annotation_drag_mode = AnnotationDragMode::None;
        self.active_tool = ToolKind::Select;
        self.history = EditHistory::new();
        self.rebuild_render_image();
        self.current_drawing = None;
        self.drawing_start = None;
        self.snap_candidate = None;
        self.text_input_mode = false;
        self.text_input_buffer.clear();
        self.text_input_width = None;
        self.text_input_height = None;
        self.hex_input_mode = false;
        self.show_color_palette = false;
        self.show_shapes_menu = false;
        self.show_beautify_menu = false;
        self.sync_and_save_tool_props();
        self.numeric_input_mode = None;
        self.numeric_input_buffer.clear();
        self.tool_props.current_badge_number = 1;

        if self.config.window_snapping {
            self.hovered_window = self.detected_windows.iter().find(|w| w.bounds.contains(&self.cursor_pos)).cloned();
        }
        cx.notify();
    }

    pub fn commit_text_input(&mut self) {
        if self.text_input_mode {
            if !self.text_input_buffer.trim().is_empty() {
                let ann = Annotation::Text {
                    origin: self.text_input_pos,
                    text: self.text_input_buffer.clone(),
                    color: self.tool_props.stroke_color,
                    font_size: self.tool_props.font_size,
                    box_width: self.text_input_width,
                };
                self.history.push_annotation(ann);
                self.selected_annotation = None;
            }
            self.text_input_mode = false;
            self.text_input_buffer.clear();
            self.text_input_width = None;
        self.text_input_height = None;
        }
    }

    pub fn do_undo(&mut self, cx: &mut Context<Self>) {
        self.selected_annotation = None;
        self.annotation_drag_mode = AnnotationDragMode::None;
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
        self.selected_annotation = None;
        self.annotation_drag_mode = AnnotationDragMode::None;
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

        if self.is_beautify_active {
            let presets = crate::image::beautify::BeautifyPreset::all();
            let preset = presets[self.beautify_preset_idx % presets.len()];
            let opts = crate::image::beautify::BeautifyOptions {
                preset,
                ..Default::default()
            };
            Some(crate::image::beautify::apply_beautify(&cropped, &opts))
        } else {
            Some(cropped)
        }
    }

    pub fn do_copy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_text_input();
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
        self.commit_text_input();
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

    pub fn toggle_beautify(&mut self, cx: &mut Context<Self>) {
        let lang = self.config.language.resolve();
        let is_ru = lang.is_ru();
        let presets = crate::image::beautify::BeautifyPreset::all();
        if !self.is_beautify_active {
            self.is_beautify_active = true;
            self.beautify_preset_idx = 0;
            let name = presets[0].display_name(is_ru);
            self.show_toast(format!("{}: {}", t(lang, "Beautify enabled", "Фон подложки включён"), name), cx);
        } else {
            self.beautify_preset_idx += 1;
            if self.beautify_preset_idx >= presets.len() {
                self.is_beautify_active = false;
                self.beautify_preset_idx = 0;
                self.show_toast(t(lang, "Beautify disabled", "Фон подложки выключен"), cx);
            } else {
                let name = presets[self.beautify_preset_idx].display_name(is_ru);
                self.show_toast(format!("{}: {}", t(lang, "Beautify", "Фон подложки"), name), cx);
            }
        }
        cx.notify();
    }

    pub fn do_pin(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_text_input();
        let lang = self.config.language.resolve();
        if let Some(final_img) = self.prepare_final_image() {
            let origin = point(px(self.selection.x()), px(self.selection.y()));
            window.remove_window();
            cx.spawn(move |_view, cx: &mut gpui::AsyncApp| {
                let cx = cx.clone();
                async move {
                    let _ = cx.update(|app| {
                        crate::pin::open_pinned_screenshot(app, final_img, Some(origin));
                    });
                }
            })
            .detach();
        } else {
            self.show_toast(t(lang, "Select an area first", "Сначала выделите область"), cx);
        }
    }

    pub fn do_ocr(&mut self, cx: &mut Context<Self>) {
        self.commit_text_input();
        let lang = self.config.language.resolve();
        let ocr_pref = self.config.ocr_language.clone();
        if let Some(img) = self.prepare_final_image() {
            self.is_ocr_running = true;
            cx.notify();

            match recognize_text_from_image(&img, Some(&ocr_pref)) {
                Ok(output) => {
                    if output.full_text.trim().is_empty() {
                        self.show_toast(t(lang, "No text found in selected area", "Текст в выбранной области не найден"), cx);
                    } else {
                        let _ = copy_text_to_clipboard(&output.full_text);
                        let first_line = output.full_text.lines().next().unwrap_or(&output.full_text).trim();
                        let char_count = first_line.chars().count();
                        let preview: String = if char_count > 36 {
                            format!("{}...", first_line.chars().take(36).collect::<String>())
                        } else {
                            first_line.to_string()
                        };
                        self.show_toast(format!("{}: \"{}\"", t(lang, "OCR copied", "Текст скопирован"), preview), cx);
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

    pub fn do_qr_scan(&mut self, cx: &mut Context<Self>) {
        self.commit_text_input();
        let lang = self.config.language.resolve();
        let img_opt = if self.selection.is_active {
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
            crop_region(&self.capture, crop_rect).or_else(|| self.prepare_final_image())
        } else {
            self.prepare_final_image()
        };

        if let Some(img) = img_opt {
            match decode_qr_from_image(&img) {
                Ok(result) => {
                    let _ = copy_text_to_clipboard(&result.content);
                    if self.config.sound_effects {
                        play_capture_sound();
                    }
                    let first_line = result.content.lines().next().unwrap_or(&result.content).trim();
                    let char_count = first_line.chars().count();
                    let preview: String = if char_count > 36 {
                        format!("{}...", first_line.chars().take(36).collect::<String>())
                    } else {
                        first_line.to_string()
                    };
                    let label = match result.format.as_deref() {
                        Some("QR_CODE") | None => t(lang, "QR Code copied", "QR-код скопирован"),
                        Some(_) => t(lang, "Code copied", "Код скопирован"),
                    };
                    self.show_toast(format!("{}: \"{}\"", label, preview), cx);
                }
                Err(e) => {
                    self.show_toast(format!("{}: {}", t(lang, "QR scan failed", "QR-код не найден"), e), cx);
                }
            }
        } else {
            self.show_toast(t(lang, "Select an area with QR code first", "Сначала выделите область с QR-кодом"), cx);
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
        self.sync_and_save_tool_props();
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
        self.sync_and_save_tool_props();
        cx.notify();
    }

    pub fn set_hue_color(&mut self, hue: f32, cx: &mut Context<Self>) {
        let (r, g, b) = hsv_to_rgb(hue, 1.0, 1.0);
        self.tool_props.stroke_color = rgb(((r as u32) << 16) | ((g as u32) << 8) | (b as u32));
        self.custom_hex_input = format!("{:02X}{:02X}{:02X}", r, g, b);
        self.sync_and_save_tool_props();
        cx.notify();
    }

    pub fn copy_hex_color(&mut self, hex: &str, cx: &mut Context<Self>) {
        let _ = copy_text_to_clipboard(hex);
        self.show_toast(format!("Copied {} to clipboard", hex), cx);
    }

    pub fn handle_right_mouse_down(
        &mut self,
        _ev: &MouseDownEvent,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        // Intentionally absorbed on mouse down.
        // Actions are executed on mouse up to prevent the subsequent mouse-up event
        // from leaking through to the underlying desktop/browser window and triggering
        // its native context menu.
    }

    pub fn handle_right_mouse_up(
        &mut self,
        _ev: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.show_shapes_menu {
            self.show_shapes_menu = false;
            cx.notify();
            return;
        }

        if self.show_beautify_menu {
            self.show_beautify_menu = false;
            cx.notify();
            return;
        }

        if self.show_color_palette {
            self.show_color_palette = false;
            self.hex_input_mode = false;
            self.sync_and_save_tool_props();
            cx.notify();
            return;
        }

        if self.numeric_input_mode.is_some() {
            self.numeric_input_mode = None;
            self.numeric_input_buffer.clear();
            cx.notify();
            return;
        }

        if self.text_input_mode {
            self.text_input_mode = false;
            self.text_input_buffer.clear();
            self.text_input_width = None;
        self.text_input_height = None;
            cx.notify();
            return;
        }

        if self.selection.is_active {
            self.cancel_selection(cx);
            return;
        }

        window.remove_window();
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

        // 1. Floating panels (toolbar, palette, shapes dropdown, beautify dropdown) ALWAYS take precedence!
        // Never let selection resize handles intercept clicks meant for floating menus or toolbar.
        if self.selection.is_active {
            let in_floating = self.current_toolbar_bounds.map_or(false, |b| b.contains(&p))
                || (self.show_color_palette && self.palette_panel_bounds.map_or(false, |b| b.contains(&p)))
                || (self.show_shapes_menu && self.shapes_panel_bounds.map_or(false, |b| b.contains(&p)))
                || (self.show_beautify_menu && self.beautify_panel_bounds.map_or(false, |b| b.contains(&p)));

            if in_floating {
                if self.show_color_palette {
                    if let Some(gb) = self.gradient_field_bounds {
                        if gb.contains(&p) {
                            self.commit_text_input();
                            self.palette_drag_mode = PaletteDragMode::SaturationValue;
                            let rel_x = (px_f - f32::from(gb.origin.x)).clamp(0.0, f32::from(gb.size.width));
                            let rel_y = (py_f - f32::from(gb.origin.y)).clamp(0.0, f32::from(gb.size.height));
                            self.set_picker_sat_val(rel_x / f32::from(gb.size.width), 1.0 - (rel_y / f32::from(gb.size.height)), cx);
                            return;
                        }
                    }
                    if let Some(hb) = self.hue_slider_bounds {
                        if hb.contains(&p) {
                            self.commit_text_input();
                            self.palette_drag_mode = PaletteDragMode::Hue;
                            let rel_x = (px_f - f32::from(hb.origin.x)).clamp(0.0, f32::from(hb.size.width));
                            self.set_picker_hue(rel_x / f32::from(hb.size.width) * 360.0, cx);
                            return;
                        }
                    }
                }
                self.commit_text_input();
                return;
            }
        }

        // 2. Annotation handle hit testing (takes priority if an annotation is already selected)
        if self.selection.is_active {
            if let Some(sel_idx) = self.selected_annotation {
                if sel_idx < self.history.annotations.len() {
                    if let Some(h) = self.history.annotations[sel_idx].hit_test_handle(p, px(14.0)) {
                        self.commit_text_input();
                        self.history.snapshot();
                        let ann = self.history.annotations[sel_idx].clone();
                        match h {
                            AnnotationHandle::Endpoint(is_start) => {
                                self.annotation_drag_mode = AnnotationDragMode::DraggingEndpoint {
                                    is_start,
                                    orig_annotation: ann,
                                };
                            }
                            AnnotationHandle::BezierControl => {
                                self.annotation_drag_mode = AnnotationDragMode::DraggingBezierControl {
                                    orig_annotation: ann,
                                };
                            }
                            _ => {
                                let orig_bounds = self.history.annotations[sel_idx].bounds();
                                self.annotation_drag_mode = AnnotationDragMode::Resizing {
                                    handle: h,
                                    start_mouse: p,
                                    orig_annotation: ann,
                                    orig_bounds,
                                };
                            }
                        }
                        cx.notify();
                        return;
                    }
                }
            }
        }

        // 3. Selection resize handles take priority over canvas!
        // When clicking on or near any of the 8 handles (radius 12px), enter Resizing mode immediately.
        if self.selection.is_active {
            if let Some(handle) = self.selection.hit_test_handle(p, px(12.0)) {
                self.commit_text_input();
                self.drag_mode = SelectionDragMode::Resizing {
                    handle,
                    start_mouse: p,
                    orig_bounds: self.selection.bounds,
                };
                cx.notify();
                return;
            }
        }

        // 4. In Select tool mode: clicking on any existing annotation selects it for transform/move/deletion
        if self.selection.is_active && self.active_tool == ToolKind::Select {
            let hit_idx = self.history.annotations.iter().enumerate().rev().find_map(|(i, ann)| {
                if ann.hit_test(p, 8.0) {
                    Some(i)
                } else {
                    None
                }
            });

            if let Some(idx) = hit_idx {
                self.commit_text_input();
                let ann = self.history.annotations[idx].clone();
                if ev.click_count == 2 {
                    if let Annotation::Text { origin, text, font_size, box_width, .. } = &ann {
                        self.history.annotations.remove(idx);
                        self.selected_annotation = None;
                        self.text_input_mode = true;
                        self.text_input_pos = *origin;
                        self.text_input_width = *box_width;
                        self.text_input_height = None;
                        self.text_input_buffer = text.clone();
                        self.tool_props.font_size = *font_size;
                        cx.notify();
                        return;
                    }
                }
                self.selected_annotation = Some(idx);
                if let Some(c) = ann.color() {
                    self.tool_props.stroke_color = c;
                }
                if let Some(w) = ann.stroke_width() {
                    self.tool_props.stroke_width = w;
                }
                if let Some(fs) = ann.font_size() {
                    self.tool_props.font_size = fs;
                }
                if let Some(f) = ann.is_filled() {
                    self.tool_props.is_filled = f;
                }
                if let Some(th) = ann.is_two_headed() {
                    self.tool_props.arrow_two_headed = th;
                }
                if let Some(curved) = ann.is_curved() {
                    self.tool_props.arrow_curved = curved;
                }
                if let Some(shape) = ann.badge_shape() {
                    self.tool_props.badge_shape = shape;
                }
                if let Some(mode) = ann.badge_mode() {
                    self.tool_props.badge_mode = mode;
                }
                match ann {
                    Annotation::Blur { radius, .. } => self.tool_props.blur_radius = radius,
                    Annotation::Pixelate { block_size, .. } => self.tool_props.pixelate_block = block_size,
                    Annotation::Spotlight { dim_opacity, .. } => self.tool_props.spotlight_dim = dim_opacity,
                    _ => {}
                }
                self.history.snapshot();
                if let Some(h) = ann.hit_test_handle(p, px(14.0)) {
                    match h {
                        AnnotationHandle::Endpoint(is_start) => {
                            self.annotation_drag_mode = AnnotationDragMode::DraggingEndpoint {
                                is_start,
                                orig_annotation: ann,
                            };
                        }
                        AnnotationHandle::BezierControl => {
                            self.annotation_drag_mode = AnnotationDragMode::DraggingBezierControl {
                                orig_annotation: ann,
                            };
                        }
                        _ => {
                            let orig_bounds = ann.bounds();
                            self.annotation_drag_mode = AnnotationDragMode::Resizing {
                                handle: h,
                                start_mouse: p,
                                orig_annotation: ann,
                                orig_bounds,
                            };
                        }
                    }
                } else {
                    self.annotation_drag_mode = AnnotationDragMode::Moving {
                        start_mouse: p,
                        orig_annotation: ann,
                    };
                }
                cx.notify();
                return;
            } else {
                let had_selected = self.selected_annotation.is_some();
                self.selected_annotation = None;
                if had_selected {
                    self.commit_text_input();
                    cx.notify();
                    return;
                }
            }
        } else if self.selected_annotation.is_some() {
            // Deselect previous annotation when clicking on empty space
            self.selected_annotation = None;
            self.commit_text_input();
            cx.notify();
            return;
        }

        // 3. Text input submission if clicking on empty space / outside
        if self.text_input_mode {
            self.commit_text_input();
            cx.notify();
            return;
        }

        // Close open dropdowns if clicking outside on canvas
        if self.show_shapes_menu {
            self.show_shapes_menu = false;
            cx.notify();
        }
        if self.show_beautify_menu {
            self.show_beautify_menu = false;
            cx.notify();
        }
        if self.show_color_palette {
            self.show_color_palette = false;
            self.hex_input_mode = false;
            self.sync_and_save_tool_props();
            cx.notify();
        }

        // 4. When no selection is active, any click MUST create a selection or snap to window!
        if !self.selection.is_active {
            self.active_tool = ToolKind::Select;
            if self.config.window_snapping {
                if let Some(hovered) = &self.hovered_window {
                    if hovered.bounds.contains(&p) {
                        self.snap_candidate = Some(hovered.clone());
                    }
                } else if let Some(found) = self.detected_windows.iter().find(|w| w.bounds.contains(&p)) {
                    self.snap_candidate = Some(found.clone());
                }
            }

            self.drag_mode = SelectionDragMode::Creating { start: p };
            self.selection = SelectionState::new(px_f, py_f, 0.0, 0.0);
            cx.notify();
            return;
        }

        // 5. Active selection mode tools:
        // Eyedropper mode: picks pixel color, applies it to stroke_color, copies HEX, and notifies
        if self.active_tool == ToolKind::Eyedropper {
            let img_x = (px_f.round() as u32).min(self.capture.bounds.width.saturating_sub(1));
            let img_y = (py_f.round() as u32).min(self.capture.bounds.height.saturating_sub(1));
            let pixel = self.capture.image.get_pixel(img_x, img_y);
            let picked = rgb(((pixel[0] as u32) << 16) | ((pixel[1] as u32) << 8) | (pixel[2] as u32));
            let hex = format!("#{:02X}{:02X}{:02X}", pixel[0], pixel[1], pixel[2]);
            self.tool_props.stroke_color = picked;
            self.custom_hex_input = format!("{:02X}{:02X}{:02X}", pixel[0], pixel[1], pixel[2]);
            self.sync_and_save_tool_props();
            let lang = self.config.language.resolve();
            let _ = copy_text_to_clipboard(&hex);
            self.show_toast(format!("{} {}", t(lang, "Picked color:", "Цвет скопирован:"), hex), cx);
            self.active_tool = ToolKind::Pencil;
            cx.notify();
            return;
        }

        // Text tool placement
        if self.active_tool == ToolKind::Text {
            self.commit_text_input();
            self.selected_annotation = None;
            self.drawing_start = Some(p);
            cx.notify();
            return;
        }

        // Number step badge placement
        if self.active_tool == ToolKind::NumberBadge {
            let badge = Annotation::NumberBadge {
                center: p,
                number: self.tool_props.current_badge_number,
                color: self.tool_props.stroke_color,
                radius: 14.0,
                shape: self.tool_props.badge_shape,
                mode: self.tool_props.badge_mode,
                filled: self.tool_props.badge_filled,
            };
            self.history.push_annotation(badge);
            self.tool_props.current_badge_number += 1;
            cx.notify();
            return;
        }

        // Drawing tools (Pencil, Marker, Arrow, Line, Rect, Ellipse, Blur, Pixelate)
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
                    let control = if self.tool_props.arrow_curved {
                        Some(p)
                    } else {
                        None
                    };
                    self.current_drawing = Some(Annotation::Arrow {
                        from: p,
                        to: p,
                        control,
                        two_headed: self.tool_props.arrow_two_headed,
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
                ToolKind::Triangle => {
                    self.current_drawing = Some(Annotation::Triangle {
                        bounds: Bounds {
                            origin: p,
                            size: size(px(0.0), px(0.0)),
                        },
                        color: self.tool_props.stroke_color,
                        width: self.tool_props.stroke_width,
                        filled: self.tool_props.is_filled,
                    });
                }
                ToolKind::Star => {
                    self.current_drawing = Some(Annotation::Star {
                        bounds: Bounds {
                            origin: p,
                            size: size(px(0.0), px(0.0)),
                        },
                        color: self.tool_props.stroke_color,
                        width: self.tool_props.stroke_width,
                        filled: self.tool_props.is_filled,
                    });
                }
                ToolKind::Diamond => {
                    self.current_drawing = Some(Annotation::Diamond {
                        bounds: Bounds {
                            origin: p,
                            size: size(px(0.0), px(0.0)),
                        },
                        color: self.tool_props.stroke_color,
                        width: self.tool_props.stroke_width,
                        filled: self.tool_props.is_filled,
                    });
                }
                ToolKind::Heart => {
                    self.current_drawing = Some(Annotation::Heart {
                        bounds: Bounds {
                            origin: p,
                            size: size(px(0.0), px(0.0)),
                        },
                        color: self.tool_props.stroke_color,
                        width: self.tool_props.stroke_width,
                        filled: self.tool_props.is_filled,
                    });
                }
                ToolKind::Cross => {
                    self.current_drawing = Some(Annotation::Cross {
                        bounds: Bounds {
                            origin: p,
                            size: size(px(0.0), px(0.0)),
                        },
                        color: self.tool_props.stroke_color,
                        width: self.tool_props.stroke_width,
                        filled: self.tool_props.is_filled,
                    });
                }
                ToolKind::Checkmark => {
                    self.current_drawing = Some(Annotation::Checkmark {
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
                ToolKind::Spotlight => {
                    self.current_drawing = Some(Annotation::Spotlight {
                        bounds: Bounds {
                            origin: p,
                            size: size(px(0.0), px(0.0)),
                        },
                        color: gpui::rgba(0x00000000),
                        width: 0.0,
                        dim_opacity: self.tool_props.spotlight_dim,
                    });
                }
                _ => {}
            }
            cx.notify();
            return;
        }

        // 6. Select tool: move existing selection
        if self.selection.contains(p) {
            self.drag_mode = SelectionDragMode::Moving {
                start_mouse: p,
                orig_origin: self.selection.bounds.origin,
            };
            cx.notify();
            return;
        }

        // 7. Select tool: clicking outside existing selection cancels old selection and starts a new one!
        self.cancel_selection(cx);
        if self.config.window_snapping {
            if let Some(hovered) = &self.hovered_window {
                if hovered.bounds.contains(&p) {
                    self.snap_candidate = Some(hovered.clone());
                }
            } else if let Some(found) = self.detected_windows.iter().find(|w| w.bounds.contains(&p)) {
                self.snap_candidate = Some(found.clone());
            }
        }

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

        // Annotation dragging / resizing / endpoint transform
        if self.annotation_drag_mode != AnnotationDragMode::None {
            if let Some(idx) = self.selected_annotation {
                if idx < self.history.annotations.len() {
                    match &self.annotation_drag_mode {
                        AnnotationDragMode::Moving { start_mouse, orig_annotation } => {
                            let delta = point(p.x - start_mouse.x, p.y - start_mouse.y);
                            let mut moved = orig_annotation.clone();
                            moved.translate(delta);
                            self.history.annotations[idx] = moved.clone();
                            if matches!(moved, Annotation::Blur { .. } | Annotation::Pixelate { .. } | Annotation::Spotlight { .. }) {
                                self.rebuild_render_image();
                            }
                            cx.notify();
                            return;
                        }
                        AnnotationDragMode::DraggingEndpoint { is_start, orig_annotation } => {
                            let mut modified = orig_annotation.clone();
                            match &mut modified {
                                Annotation::Arrow { from, to, .. } | Annotation::Line { from, to, .. } => {
                                    if *is_start {
                                        *from = p;
                                    } else {
                                        *to = p;
                                    }
                                }
                                _ => {}
                            }
                            self.history.annotations[idx] = modified;
                            cx.notify();
                            return;
                        }
                        AnnotationDragMode::DraggingBezierControl { orig_annotation } => {
                            let mut modified = orig_annotation.clone();
                            if let Annotation::Arrow { control, .. } = &mut modified {
                                *control = Some(p);
                            }
                            self.history.annotations[idx] = modified;
                            cx.notify();
                            return;
                        }
                        AnnotationDragMode::Resizing { handle, start_mouse, orig_annotation, orig_bounds } => {
                            let mut modified = orig_annotation.clone();
                            modified.resize_with_handle(*orig_bounds, *handle, *start_mouse, p);
                            if let Some(fs) = modified.font_size() {
                                self.tool_props.font_size = fs;
                            }
                            self.history.annotations[idx] = modified.clone();
                            if matches!(modified, Annotation::Blur { .. } | Annotation::Pixelate { .. } | Annotation::Spotlight { .. }) {
                                self.rebuild_render_image();
                            }
                            cx.notify();
                            return;
                        }
                        _ => {}
                    }
                }
            }
        }

        if self.active_tool == ToolKind::Text && self.drawing_start.is_some() {
            cx.notify();
            return;
        }

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
                    Annotation::Line { to, .. } => {
                        *to = p;
                    }
                    Annotation::Arrow { to, from, control, .. } => {
                        *to = p;
                        if self.tool_props.arrow_curved {
                            let mx = (f32::from(from.x) + f32::from(to.x)) / 2.0;
                            let my = (f32::from(from.y) + f32::from(to.y)) / 2.0;
                            let dx = f32::from(to.x) - f32::from(from.x);
                            let dy = f32::from(to.y) - f32::from(from.y);
                            let len = (dx * dx + dy * dy).sqrt();
                            if len > 10.0 {
                                let offset = (len * 0.25).max(25.0);
                                let nx = -dy / len;
                                let ny = dx / len;
                                *control = Some(point(px(mx + nx * offset), px(my + ny * offset)));
                            }
                        }
                    }
                    Annotation::Rectangle { bounds, .. }
                    | Annotation::Ellipse { bounds, .. }
                    | Annotation::Triangle { bounds, .. }
                    | Annotation::Star { bounds, .. }
                    | Annotation::Diamond { bounds, .. }
                    | Annotation::Heart { bounds, .. }
                    | Annotation::Cross { bounds, .. }
                    | Annotation::Checkmark { bounds, .. }
                    | Annotation::Blur { bounds, .. }
                    | Annotation::Pixelate { bounds, .. }
                    | Annotation::Spotlight { bounds, .. } => {
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
                let drag_dist = ((px_f - f32::from(start.x)).powi(2) + (py_f - f32::from(start.y)).powi(2)).sqrt();
                if drag_dist > 5.0 {
                    self.snap_candidate = None;
                    if self.hovered_window.is_some() {
                        self.hovered_window = None;
                    }
                }
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
                let mut should_notify = false;
                if self.config.window_snapping && !self.selection.is_active {
                    let prev_hover = self.hovered_window.as_ref().map(|w| w.bounds);
                    let new_hover = self.detected_windows.iter().find(|w| w.bounds.contains(&p)).cloned();
                    let new_bounds = new_hover.as_ref().map(|w| w.bounds);
                    if prev_hover != new_bounds {
                        self.hovered_window = new_hover;
                        should_notify = true;
                    }
                }
                let show_mag = self.active_tool == ToolKind::Eyedropper
                    || (self.config.show_magnifier && !self.selection.is_active);
                if show_mag || should_notify {
                    cx.notify();
                }
            }
        }
    }

    pub fn handle_mouse_up(
        &mut self,
        ev: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let p = ev.position;
        if self.palette_drag_mode != PaletteDragMode::None {
            self.palette_drag_mode = PaletteDragMode::None;
            self.sync_and_save_tool_props();
            cx.notify();
            return;
        }

        if self.annotation_drag_mode != AnnotationDragMode::None {
            self.history.pop_last_snapshot_if_unchanged();
            self.annotation_drag_mode = AnnotationDragMode::None;
            if let Some(idx) = self.selected_annotation {
                if idx < self.history.annotations.len() {
                    let ann = &self.history.annotations[idx];
                    if matches!(ann, Annotation::Blur { .. } | Annotation::Pixelate { .. } | Annotation::Spotlight { .. }) {
                        self.rebuild_render_image();
                    }
                }
            }
            cx.notify();
            return;
        }

        if self.active_tool == ToolKind::Text {
            if let Some(start) = self.drawing_start.take() {
                let px_f = f32::from(p.x);
                let py_f = f32::from(p.y);
                let min_x = f32::from(start.x).min(px_f);
                let min_y = f32::from(start.y).min(py_f);
                let w = (px_f - f32::from(start.x)).abs();
                let h = (py_f - f32::from(start.y)).abs();

                let (pos, box_w, box_h) = if w >= 24.0 && h >= 16.0 {
                    (point(px(min_x), px(min_y)), w, Some(h))
                } else {
                    let remaining_w = if self.selection.is_active {
                        (self.selection.x() + self.selection.width() - f32::from(start.x)).max(180.0)
                    } else {
                        (self.capture.bounds.width as f32 - f32::from(start.x)).max(180.0)
                    };
                    (start, remaining_w.clamp(200.0, 380.0), None)
                };

                self.text_input_mode = true;
                self.text_input_pos = pos;
                self.text_input_width = Some(box_w);
                self.text_input_height = box_h;
                self.text_input_buffer.clear();
                cx.notify();
                return;
            }
        }

        let was_moving_or_resizing = matches!(self.drag_mode, SelectionDragMode::Moving { .. } | SelectionDragMode::Resizing { .. });
        self.drag_mode = SelectionDragMode::None;
        self.drawing_start = None;

        if let Some(drawing) = self.current_drawing.take() {
            let b = drawing.bounds();
            let is_too_small = match &drawing {
                Annotation::Pencil { points, .. } | Annotation::Marker { points, .. } => {
                    points.len() < 2
                }
                Annotation::Line { from, to, .. } | Annotation::Arrow { from, to, .. } => {
                    let dx = f32::from(to.x - from.x);
                    let dy = f32::from(to.y - from.y);
                    (dx * dx + dy * dy).sqrt() < 4.0
                }
                _ => f32::from(b.size.width).abs() < 4.0 && f32::from(b.size.height).abs() < 4.0,
            };
            if is_too_small {
                cx.notify();
                return;
            }

            let is_filter_or_spotlight = matches!(
                drawing,
                Annotation::Blur { .. } | Annotation::Pixelate { .. } | Annotation::Spotlight { .. }
            );
            self.history.push_annotation(drawing);
            self.selected_annotation = None;
            if is_filter_or_spotlight {
                self.rebuild_render_image();
            }
            cx.notify();
            return;
        }

        if was_moving_or_resizing && self.history.annotations.iter().any(|a| matches!(a, Annotation::Spotlight { .. })) {
            self.rebuild_render_image();
        }

        // Check if an instant window snap was clicked
        if let Some(snapped) = self.snap_candidate.take() {
            let b = snapped.bounds;
            self.selection = SelectionState::new(
                f32::from(b.origin.x),
                f32::from(b.origin.y),
                f32::from(b.size.width),
                f32::from(b.size.height),
            );
            self.selection.is_active = true;
            self.hovered_window = None;
            self.drag_mode = SelectionDragMode::None;
            cx.notify();
            return;
        }

        // Cancel accidental tiny tap selection (< 12px)
        if self.selection.width() < 12.0 || self.selection.height() < 12.0 {
            self.cancel_selection(cx);
            return;
        }
    }

    pub fn handle_scroll_wheel(
        &mut self,
        ev: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta = match ev.delta {
            gpui::ScrollDelta::Pixels(p) => f32::from(p.y),
            gpui::ScrollDelta::Lines(l) => l.y * 10.0,
        };

        if delta == 0.0 {
            return;
        }

        let is_up = delta > 0.0;

        match self.active_tool {
            ToolKind::Blur => {
                if is_up {
                    self.tool_props.blur_radius = (self.tool_props.blur_radius + 2).min(99);
                } else {
                    self.tool_props.blur_radius = self.tool_props.blur_radius.saturating_sub(2).max(2);
                }
            }
            ToolKind::Pixelate => {
                if is_up {
                    self.tool_props.pixelate_block = (self.tool_props.pixelate_block + 2).min(99);
                } else {
                    self.tool_props.pixelate_block = self.tool_props.pixelate_block.saturating_sub(2).max(2);
                }
            }
            ToolKind::Spotlight => {
                if is_up {
                    self.tool_props.spotlight_dim = ((self.tool_props.spotlight_dim + 0.05) * 100.0).round() / 100.0;
                } else {
                    self.tool_props.spotlight_dim = ((self.tool_props.spotlight_dim - 0.05) * 100.0).round() / 100.0;
                }
                self.tool_props.spotlight_dim = self.tool_props.spotlight_dim.clamp(0.10, 0.95);
            }
            ToolKind::Text => {
                if is_up {
                    self.tool_props.font_size = (self.tool_props.font_size + 2.0).min(96.0);
                } else {
                    self.tool_props.font_size = (self.tool_props.font_size - 2.0).max(10.0);
                }
            }
            _ => {
                // Pencil, Marker, Arrow, Line, Rectangle, Ellipse, Select
                if is_up {
                    self.tool_props.stroke_width = (self.tool_props.stroke_width + 1.0).min(99.0);
                } else {
                    self.tool_props.stroke_width = (self.tool_props.stroke_width - 1.0).max(1.0);
                }
            }
        }

        self.sync_and_save_tool_props();
        cx.notify();
    }

    pub fn handle_key_down(
        &mut self,
        ev: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = ev.keystroke.key.as_str();

        // 0. Handling Numeric px Input (for stroke width, blur, pixelate, font size)
        if let Some(target) = self.numeric_input_mode {
            if key == "enter" {
                if let Ok(val) = self.numeric_input_buffer.parse::<u32>() {
                    match target {
                        NumericInputTarget::StrokeWidth => {
                            self.tool_props.stroke_width = (val as f32).clamp(0.0, 99.0);
                        }
                        NumericInputTarget::BlurRadius => {
                            self.tool_props.blur_radius = val.clamp(2, 99);
                        }
                        NumericInputTarget::PixelateBlock => {
                            self.tool_props.pixelate_block = val.clamp(2, 99);
                        }
                        NumericInputTarget::FontSize => {
                            self.tool_props.font_size = (val as f32).clamp(10.0, 96.0);
                        }
                        NumericInputTarget::SpotlightDim => {
                            let pct = val.clamp(10, 95);
                            self.tool_props.spotlight_dim = pct as f32 / 100.0;
                        }
                    }
                    self.sync_and_save_tool_props();
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
                if ev.keystroke.modifiers.shift {
                    self.text_input_buffer.push('\n');
                    cx.notify();
                    return;
                } else {
                    self.commit_text_input();
                    cx.notify();
                    return;
                }
            } else if key == "escape" {
                self.text_input_mode = false;
                self.text_input_buffer.clear();
                self.text_input_width = None;
        self.text_input_height = None;
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

        // Delete or Backspace removes currently selected vector/filter annotation
        if (key == "delete" || key == "backspace")
            && !self.text_input_mode
            && !self.hex_input_mode
            && self.numeric_input_mode.is_none()
        {
            if let Some(idx) = self.selected_annotation.take() {
                if idx < self.history.annotations.len() {
                    self.history.snapshot();
                    let removed = self.history.annotations.remove(idx);
                    if matches!(
                        removed,
                        Annotation::Blur { .. } | Annotation::Pixelate { .. } | Annotation::Spotlight { .. }
                    ) {
                        self.rebuild_render_image();
                    }
                    cx.notify();
                    return;
                }
            }
        }

        if key == "escape" {
            if self.show_shapes_menu {
                self.show_shapes_menu = false;
                cx.notify();
                return;
            }
            if self.show_beautify_menu {
                self.show_beautify_menu = false;
                cx.notify();
                return;
            }
            if self.show_color_palette {
                self.show_color_palette = false;
                self.hex_input_mode = false;
                self.sync_and_save_tool_props();
                cx.notify();
                return;
            }
            if self.selected_annotation.is_some() {
                self.selected_annotation = None;
                cx.notify();
                return;
            }
            if self.selection.is_active {
                self.cancel_selection(cx);
                return;
            }
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
            || (self.config.show_magnifier && (
                !selection.is_active
                || matches!(self.drag_mode, SelectionDragMode::Creating { .. } | SelectionDragMode::Resizing { .. })
            ));

        let annotations = self.history.annotations.clone();
        let current_drawing = self.current_drawing.clone();
        let selected_annotation = self.selected_annotation;
        let text_drag_start = if self.active_tool == ToolKind::Text { self.drawing_start } else { None };
        let active_tool = self.active_tool;
        let cursor_pos = self.cursor_pos;
        let text_input_mode = self.text_input_mode;
        let text_input_pos = self.text_input_pos;
        let text_input_width = self.text_input_width;
        let text_input_height = self.text_input_height;
        let text_input_buffer = self.text_input_buffer.clone();
        let tool_font_size = self.tool_props.font_size;

        // Determine cursor style
        let is_over_floating_panel = self.current_toolbar_bounds.map_or(false, |b| b.contains(&self.cursor_pos))
            || (self.show_color_palette && self.palette_panel_bounds.map_or(false, |b| b.contains(&self.cursor_pos)))
            || (self.show_shapes_menu && self.shapes_panel_bounds.map_or(false, |b| b.contains(&self.cursor_pos)))
            || (self.show_beautify_menu && self.beautify_panel_bounds.map_or(false, |b| b.contains(&self.cursor_pos)));

        let cursor_style = if is_over_floating_panel {
            CursorStyle::Arrow
        } else if self.active_tool == ToolKind::Text && !selection.is_active {
            CursorStyle::IBeam
        } else if selection.is_active {
            if let Some(sel_idx) = self.selected_annotation {
                if sel_idx < self.history.annotations.len() {
                    let ann = &self.history.annotations[sel_idx];
                    if let Some(h) = ann.hit_test_handle(self.cursor_pos, px(14.0)) {
                        match h {
                            AnnotationHandle::Endpoint(_) | AnnotationHandle::BezierControl => CursorStyle::Crosshair,
                            AnnotationHandle::Top | AnnotationHandle::Bottom => CursorStyle::ResizeUpDown,
                            AnnotationHandle::Left | AnnotationHandle::Right => CursorStyle::ResizeLeftRight,
                            AnnotationHandle::TopLeft | AnnotationHandle::BottomRight => CursorStyle::ResizeUpLeftDownRight,
                            AnnotationHandle::TopRight | AnnotationHandle::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
                        }
                    } else if ann.hit_test(self.cursor_pos, 8.0) {
                        CursorStyle::PointingHand
                    } else if let Some(handle) = selection.hit_test_handle(self.cursor_pos, px(12.0)) {
                        match handle {
                            ResizeHandle::Top | ResizeHandle::Bottom => CursorStyle::ResizeUpDown,
                            ResizeHandle::Left | ResizeHandle::Right => CursorStyle::ResizeLeftRight,
                            ResizeHandle::TopLeft | ResizeHandle::BottomRight => CursorStyle::ResizeUpLeftDownRight,
                            ResizeHandle::TopRight | ResizeHandle::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
                        }
                    } else if self.active_tool == ToolKind::Select && selection.contains(self.cursor_pos) {
                        CursorStyle::ClosedHand
                    } else {
                        CursorStyle::Crosshair
                    }
                } else {
                    CursorStyle::Crosshair
                }
            } else if let Some(handle) = selection.hit_test_handle(self.cursor_pos, px(12.0)) {
                match handle {
                    ResizeHandle::Top | ResizeHandle::Bottom => CursorStyle::ResizeUpDown,
                    ResizeHandle::Left | ResizeHandle::Right => CursorStyle::ResizeLeftRight,
                    ResizeHandle::TopLeft | ResizeHandle::BottomRight => CursorStyle::ResizeUpLeftDownRight,
                    ResizeHandle::TopRight | ResizeHandle::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
                }
            } else if self.active_tool == ToolKind::Select && selection.contains(self.cursor_pos) {
                if self.history.annotations.iter().any(|a| a.hit_test(self.cursor_pos, 8.0)) {
                    CursorStyle::PointingHand
                } else {
                    CursorStyle::ClosedHand
                }
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
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, ev, window, cx| {
                    this.handle_right_mouse_down(ev, window, cx);
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
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(|this, ev, window, cx| {
                    this.handle_right_mouse_up(ev, window, cx);
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

                // Render vector annotations only when a selection is active
                if selection.is_active {
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
                            Annotation::Arrow { from, to, control, two_headed, color, width } => {
                                let x1 = f32::from(from.x);
                                let y1 = f32::from(from.y);
                                let x2 = f32::from(to.x);
                                let y2 = f32::from(to.y);

                                if let Some(cp) = control {
                                    let cx_f = f32::from(cp.x);
                                    let cy_f = f32::from(cp.y);
                                    let steps = 24;
                                    let pts = sample_quadratic_bezier((x1, y1), (cx_f, cy_f), (x2, y2), steps);

                                    let mut pb = PathBuilder::stroke(px(*width));
                                    if !pts.is_empty() {
                                        pb.move_to(point(px(pts[0].0), px(pts[0].1)));
                                        for pt in &pts[1..] {
                                            pb.line_to(point(px(pt.0), px(pt.1)));
                                        }
                                        if let Ok(p) = pb.build() {
                                            window.paint_path(p, *color);
                                        }
                                    }

                                    if let Some(head) = calculate_arrowhead((x2, y2), (cx_f, cy_f), *width, 100.0) {
                                        let mut head_b = PathBuilder::fill();
                                        head_b.move_to(point(px(head.tip.0), px(head.tip.1)));
                                        head_b.line_to(point(px(head.left_wing.0), px(head.left_wing.1)));
                                        head_b.line_to(point(px(head.right_wing.0), px(head.right_wing.1)));
                                        head_b.close();
                                        if let Ok(hp) = head_b.build() {
                                            window.paint_path(hp, *color);
                                        }
                                    }

                                    if *two_headed {
                                        if let Some(head) = calculate_arrowhead((x1, y1), (cx_f, cy_f), *width, 100.0) {
                                            let mut head_b = PathBuilder::fill();
                                            head_b.move_to(point(px(head.tip.0), px(head.tip.1)));
                                            head_b.line_to(point(px(head.left_wing.0), px(head.left_wing.1)));
                                            head_b.line_to(point(px(head.right_wing.0), px(head.right_wing.1)));
                                            head_b.close();
                                            if let Ok(hp) = head_b.build() {
                                                window.paint_path(hp, *color);
                                            }
                                        }
                                    }
                                } else {
                                    let geom = calculate_arrow_geometry((x1, y1), (x2, y2), *width);

                                    let shaft_start = if *two_headed {
                                        let geom_rev = calculate_arrow_geometry((x2, y2), (x1, y1), *width);
                                        geom_rev.shaft_to
                                    } else {
                                        geom.shaft_from
                                    };

                                    let mut pb = PathBuilder::stroke(px(*width));
                                    pb.move_to(point(px(shaft_start.0), px(shaft_start.1)));
                                    pb.line_to(point(px(geom.shaft_to.0), px(geom.shaft_to.1)));
                                    if let Ok(p) = pb.build() {
                                        window.paint_path(p, *color);
                                    }

                                    if geom.has_head {
                                        let mut head_b = PathBuilder::fill();
                                        head_b.move_to(point(px(geom.tip.0), px(geom.tip.1)));
                                        head_b.line_to(point(px(geom.left_wing.0), px(geom.left_wing.1)));
                                        head_b.line_to(point(px(geom.right_wing.0), px(geom.right_wing.1)));
                                        head_b.close();
                                        if let Ok(hp) = head_b.build() {
                                            window.paint_path(hp, *color);
                                        }
                                    }

                                    if *two_headed {
                                        let geom_rev = calculate_arrow_geometry((x2, y2), (x1, y1), *width);
                                        if geom_rev.has_head {
                                            let mut head_b = PathBuilder::fill();
                                            head_b.move_to(point(px(geom_rev.tip.0), px(geom_rev.tip.1)));
                                            head_b.line_to(point(px(geom_rev.left_wing.0), px(geom_rev.left_wing.1)));
                                            head_b.line_to(point(px(geom_rev.right_wing.0), px(geom_rev.right_wing.1)));
                                            head_b.close();
                                            if let Ok(hp) = head_b.build() {
                                                window.paint_path(hp, *color);
                                            }
                                        }
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
                            Annotation::Triangle { bounds, color, filled, width } => {
                                let ox = bounds.origin.x;
                                let oy = bounds.origin.y;
                                let w = bounds.size.width;
                                let h = bounds.size.height;
                                let mut pb = if *filled {
                                    PathBuilder::fill()
                                } else {
                                    PathBuilder::stroke(px(*width))
                                };
                                pb.move_to(point(ox + px(f32::from(w) / 2.0), oy));
                                pb.line_to(point(ox + w, oy + h));
                                pb.line_to(point(ox, oy + h));
                                pb.close();
                                if let Ok(path) = pb.build() {
                                    window.paint_path(path, *color);
                                }
                            }
                            Annotation::Diamond { bounds, color, filled, width } => {
                                let ox = bounds.origin.x;
                                let oy = bounds.origin.y;
                                let w = bounds.size.width;
                                let h = bounds.size.height;
                                let mut pb = if *filled {
                                    PathBuilder::fill()
                                } else {
                                    PathBuilder::stroke(px(*width))
                                };
                                pb.move_to(point(ox + px(f32::from(w) / 2.0), oy));
                                pb.line_to(point(ox + w, oy + px(f32::from(h) / 2.0)));
                                pb.line_to(point(ox + px(f32::from(w) / 2.0), oy + h));
                                pb.line_to(point(ox, oy + px(f32::from(h) / 2.0)));
                                pb.close();
                                if let Ok(path) = pb.build() {
                                    window.paint_path(path, *color);
                                }
                            }
                            Annotation::Star { bounds, color, filled, width } => {
                                let cx = f32::from(bounds.origin.x) + f32::from(bounds.size.width) / 2.0;
                                let cy = f32::from(bounds.origin.y) + f32::from(bounds.size.height) / 2.0;
                                let r_out = (f32::from(bounds.size.width).abs().min(f32::from(bounds.size.height).abs())) / 2.0;
                                let r_in = r_out * 0.42;
                                if r_out > 1.0 {
                                    let mut pb = if *filled {
                                        PathBuilder::fill()
                                    } else {
                                        PathBuilder::stroke(px(*width))
                                    };
                                    for i in 0..10 {
                                        let angle = -std::f32::consts::FRAC_PI_2 + (i as f32) * std::f32::consts::PI / 5.0;
                                        let r = if i % 2 == 0 { r_out } else { r_in };
                                        let pt = point(px(cx + r * angle.cos()), px(cy + r * angle.sin()));
                                        if i == 0 {
                                            pb.move_to(pt);
                                        } else {
                                            pb.line_to(pt);
                                        }
                                    }
                                    pb.close();
                                    if let Ok(path) = pb.build() {
                                        window.paint_path(path, *color);
                                    }
                                }
                            }
                            Annotation::Heart { bounds, color, filled, width } => {
                                let ox = f32::from(bounds.origin.x);
                                let oy = f32::from(bounds.origin.y);
                                let w = f32::from(bounds.size.width);
                                let h = f32::from(bounds.size.height);
                                if w.abs() > 2.0 && h.abs() > 2.0 {
                                    let mut pb = if *filled {
                                        PathBuilder::fill()
                                    } else {
                                        PathBuilder::stroke(px(*width))
                                    };
                                    let tip = point(px(ox + w / 2.0), px(oy + h));
                                    pb.move_to(tip);
                                    pb.cubic_bezier_to(
                                        point(px(ox + w * 0.25), px(oy + h * 0.05)),
                                        point(px(ox), px(oy + h * 0.6)),
                                        point(px(ox), px(oy + h * 0.15)),
                                    );
                                    pb.cubic_bezier_to(
                                        point(px(ox + w * 0.5), px(oy + h * 0.3)),
                                        point(px(ox + w * 0.4), px(oy)),
                                        point(px(ox + w * 0.5), px(oy + h * 0.25)),
                                    );
                                    pb.cubic_bezier_to(
                                        point(px(ox + w * 0.75), px(oy + h * 0.05)),
                                        point(px(ox + w * 0.5), px(oy + h * 0.25)),
                                        point(px(ox + w * 0.6), px(oy)),
                                    );
                                    pb.cubic_bezier_to(
                                        tip,
                                        point(px(ox + w), px(oy + h * 0.15)),
                                        point(px(ox + w), px(oy + h * 0.6)),
                                    );
                                    pb.close();
                                    if let Ok(path) = pb.build() {
                                        window.paint_path(path, *color);
                                    }
                                }
                            }
                            Annotation::Cross { bounds, color, filled, width } => {
                                let ox = bounds.origin.x;
                                let oy = bounds.origin.y;
                                let w = bounds.size.width;
                                let h = bounds.size.height;
                                let stroke_w = if *filled {
                                    (f32::from(w).min(f32::from(h)) * 0.22).max(*width * 2.0)
                                } else {
                                    *width
                                };
                                let mut pb = PathBuilder::stroke(px(stroke_w));
                                pb.move_to(point(ox, oy));
                                pb.line_to(point(ox + w, oy + h));
                                pb.move_to(point(ox + w, oy));
                                pb.line_to(point(ox, oy + h));
                                if let Ok(path) = pb.build() {
                                    window.paint_path(path, *color);
                                }
                            }
                            Annotation::Checkmark { bounds, color, filled, width } => {
                                let ox = f32::from(bounds.origin.x);
                                let oy = f32::from(bounds.origin.y);
                                let w = f32::from(bounds.size.width);
                                let h = f32::from(bounds.size.height);
                                let stroke_w = if *filled {
                                    (w.min(h) * 0.22).max(*width * 2.0)
                                } else {
                                    *width
                                };
                                let mut pb = PathBuilder::stroke(px(stroke_w));
                                pb.move_to(point(px(ox), px(oy + h * 0.55)));
                                pb.line_to(point(px(ox + w * 0.38), px(oy + h * 0.9)));
                                pb.line_to(point(px(ox + w), px(oy + h * 0.15)));
                                if let Ok(path) = pb.build() {
                                    window.paint_path(path, *color);
                                }
                            }
                            Annotation::Blur { .. } | Annotation::Pixelate { .. } | Annotation::Spotlight { .. } => {}
                            Annotation::NumberBadge { center, color, radius, shape, mode: _, filled, .. } => {
                                let badge_bounds = Bounds {
                                    origin: point(center.x - px(*radius), center.y - px(*radius)),
                                    size: size(px(*radius * 2.0), px(*radius * 2.0)),
                                };
                                match shape {
                                    BadgeShape::Circle => {
                                        let cx = f32::from(center.x);
                                        let cy = f32::from(center.y);
                                        let r = *radius;
                                        let k = r * 0.55228475;
                                        let mut pb = if *filled { PathBuilder::fill() } else { PathBuilder::stroke(px(2.5)) };
                                        pb.move_to(point(px(cx), px(cy - r)));
                                        pb.cubic_bezier_to(point(px(cx + r), px(cy)), point(px(cx + k), px(cy - r)), point(px(cx + r), px(cy - k)));
                                        pb.cubic_bezier_to(point(px(cx), px(cy + r)), point(px(cx + r), px(cy + k)), point(px(cx + k), px(cy + r)));
                                        pb.cubic_bezier_to(point(px(cx - r), px(cy)), point(px(cx - k), px(cy + r)), point(px(cx - r), px(cy + k)));
                                        pb.cubic_bezier_to(point(px(cx), px(cy - r)), point(px(cx - r), px(cy - k)), point(px(cx - k), px(cy - r)));
                                        if *filled { pb.close(); }
                                        if let Ok(path) = pb.build() {
                                            window.paint_path(path, *color);
                                        }
                                        if *filled {
                                            let mut outline_pb = PathBuilder::stroke(px(2.0));
                                            outline_pb.move_to(point(px(cx), px(cy - r)));
                                            outline_pb.cubic_bezier_to(point(px(cx + r), px(cy)), point(px(cx + k), px(cy - r)), point(px(cx + r), px(cy - k)));
                                            outline_pb.cubic_bezier_to(point(px(cx), px(cy + r)), point(px(cx + r), px(cy + k)), point(px(cx + k), px(cy + r)));
                                            outline_pb.cubic_bezier_to(point(px(cx - r), px(cy)), point(px(cx - k), px(cy + r)), point(px(cx - r), px(cy + k)));
                                            outline_pb.cubic_bezier_to(point(px(cx), px(cy - r)), point(px(cx - r), px(cy - k)), point(px(cx - k), px(cy - r)));
                                            if let Ok(outline_path) = outline_pb.build() {
                                                window.paint_path(outline_path, rgb(0xFFFFFF));
                                            }
                                        }
                                    }
                                    BadgeShape::RoundedRect => {
                                        if *filled {
                                            window.paint_quad(fill(badge_bounds, *color));
                                            window.paint_quad(outline(badge_bounds, rgb(0xFFFFFF), BorderStyle::Solid));
                                        } else {
                                            window.paint_quad(outline(badge_bounds, *color, BorderStyle::Solid));
                                        }
                                    }
                                }
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
                            Annotation::Spotlight { bounds, color, width, .. } => {
                                let norm_bounds = normalize_bounds(*bounds);
                                window.paint_quad(fill(norm_bounds, rgba(0x8174FF22)));
                                if *width > 0.0 {
                                    window.paint_quad(outline(norm_bounds, *color, BorderStyle::Solid));
                                } else {
                                    window.paint_quad(outline(norm_bounds, rgba(0x8174FFAA), BorderStyle::Dashed));
                                }
                            }
                            _ => {}
                        }
                    }

                    if active_tool == ToolKind::Text {
                        if let Some(start) = text_drag_start {
                            let min_x = f32::from(start.x).min(f32::from(cursor_pos.x));
                            let min_y = f32::from(start.y).min(f32::from(cursor_pos.y));
                            let w = (f32::from(cursor_pos.x) - f32::from(start.x)).abs();
                            let h = (f32::from(cursor_pos.y) - f32::from(start.y)).abs();
                            if w > 4.0 || h > 4.0 {
                                let preview_b = Bounds {
                                    origin: point(px(min_x), px(min_y)),
                                    size: size(px(w), px(h)),
                                };
                                window.paint_quad(outline(preview_b, rgba(0x8174FFAA), BorderStyle::Dashed));
                            }
                        }
                    }

                    if text_input_mode {
                        let font_size = tool_font_size;
                        let (calc_w, calc_h) = text_bounds_dimensions(&text_input_buffer, font_size, text_input_width);
                        let frame_w = text_input_width.unwrap_or(calc_w);
                        let frame_h = text_input_height.unwrap_or(calc_h).max(calc_h);

                        let b = Bounds {
                            origin: text_input_pos,
                            size: size(px(frame_w), px(frame_h)),
                        };
                        window.paint_quad(outline(b, rgba(0x8174FFAA), BorderStyle::Dashed));
                        let x = b.origin.x;
                        let y = b.origin.y;
                        let w = b.size.width;
                        let h = b.size.height;
                        let mid_x = x + w / 2.0;
                        let mid_y = y + h / 2.0;
                        let pts = [
                            point(x, y), point(mid_x, y), point(x + w, y),
                            point(x + w, mid_y), point(x + w, y + h), point(mid_x, y + h),
                            point(x, y + h), point(x, mid_y),
                        ];
                        for pt in pts {
                            let h_bounds = Bounds {
                                origin: point(pt.x - px(4.0), pt.y - px(4.0)),
                                size: size(px(8.0), px(8.0)),
                            };
                            window.paint_quad(fill(h_bounds, rgb(0xFFFFFF)));
                            window.paint_quad(outline(h_bounds, rgb(0x8174FF), BorderStyle::Solid));
                        }
                    }

                    // Render selection box and control handles for currently selected annotation
                    if let Some(idx) = selected_annotation {
                        if let Some(ann) = annotations.get(idx) {
                            match ann {
                                Annotation::Arrow { from, to, control, .. } => {
                                    for pt in [*from, *to] {
                                        let h_bounds = Bounds {
                                            origin: point(pt.x - px(5.0), pt.y - px(5.0)),
                                            size: size(px(10.0), px(10.0)),
                                        };
                                        window.paint_quad(fill(h_bounds, rgb(0xFFFFFF)));
                                        window.paint_quad(outline(h_bounds, rgb(0x8174FF), BorderStyle::Solid));
                                    }
                                    if let Some(cp) = control {
                                        let h_bounds = Bounds {
                                            origin: point(cp.x - px(5.5), cp.y - px(5.5)),
                                            size: size(px(11.0), px(11.0)),
                                        };
                                        window.paint_quad(fill(h_bounds, rgb(0xFFCC00)));
                                        window.paint_quad(outline(h_bounds, rgb(0x8174FF), BorderStyle::Solid));
                                    }
                                }
                                Annotation::Line { from, to, .. } => {
                                    for pt in [*from, *to] {
                                        let h_bounds = Bounds {
                                            origin: point(pt.x - px(5.0), pt.y - px(5.0)),
                                            size: size(px(10.0), px(10.0)),
                                        };
                                        window.paint_quad(fill(h_bounds, rgb(0xFFFFFF)));
                                        window.paint_quad(outline(h_bounds, rgb(0x8174FF), BorderStyle::Solid));
                                    }
                                }
                                _ => {
                                    let b = ann.bounds();
                                    window.paint_quad(outline(b, rgba(0x8174FFAA), BorderStyle::Dashed));
                                    let x = b.origin.x;
                                    let y = b.origin.y;
                                    let w = b.size.width;
                                    let h = b.size.height;
                                    let mid_x = x + w / 2.0;
                                    let mid_y = y + h / 2.0;
                                    let pts = [
                                        point(x, y), point(mid_x, y), point(x + w, y),
                                        point(x + w, mid_y), point(x + w, y + h), point(mid_x, y + h),
                                        point(x, y + h), point(x, mid_y),
                                    ];
                                    for pt in pts {
                                        let h_bounds = Bounds {
                                            origin: point(pt.x - px(4.0), pt.y - px(4.0)),
                                            size: size(px(8.0), px(8.0)),
                                        };
                                        window.paint_quad(fill(h_bounds, rgb(0xFFFFFF)));
                                        window.paint_quad(outline(h_bounds, rgb(0x8174FF), BorderStyle::Solid));
                                    }
                                }
                            }
                        }
                    }
                }
            })
            .size_full(),
        );

        // Render text annotations as crisp UI elements only when selection is active
        if self.selection.is_active {
            for ann in &self.history.annotations {
                if let Annotation::Text { origin, text, color, font_size, box_width } = ann {
                    let lines = wrap_text_to_lines(text, *font_size, *box_width);
                    let pad = (*font_size * 0.12).clamp(6.0, 14.0);
                    let mut container = div()
                        .absolute()
                        .left(origin.x + px(pad))
                        .top(origin.y + px(pad - 0.20 * font_size))
                        .text_size(px(*font_size))
                        .text_color(*color)
                        .font_weight(gpui::FontWeight::BOLD)
                        .line_height(px(*font_size * 1.20))
                        .flex()
                        .flex_col();

                    if let Some(bw) = box_width {
                        let inner_w = (*bw - 2.0 * pad).max(10.0);
                        container = container.w(px(inner_w));
                    }

                    for line in lines {
                        container = container.child(
                            div().whitespace_nowrap().child(line)
                        );
                    }

                    root = root.child(container);
                }
                if let Annotation::NumberBadge { center, number, radius, mode, filled, color, .. } = ann {
                    let label = number_to_badge_label(*number, *mode);
                    let text_c = if *filled { rgb(0xFFFFFF) } else { *color };
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
                            .text_color(text_c)
                            .child(label),
                    );
                }
            }
        }

        // Active text input box
        if self.text_input_mode {
            let font_size = self.tool_props.font_size;
            let pad = (font_size * 0.12).clamp(6.0, 14.0);
            let lines = wrap_text_to_lines(&self.text_input_buffer, font_size, self.text_input_width);
            let box_w = self.text_input_width.unwrap_or(360.0);

            let inner_w = (box_w - 2.0 * pad).max(10.0);

            let mut text_lines_col = div()
                .flex()
                .flex_col()
                .w(px(inner_w))
                .text_size(px(font_size))
                .text_color(self.tool_props.stroke_color)
                .font_weight(gpui::FontWeight::BOLD)
                .line_height(px(font_size * 1.20));

            let total_lines = lines.len();
            for (idx, line) in lines.iter().enumerate() {
                let is_last = idx == total_lines - 1;
                let display_str = if is_last {
                    format!("{}▏", line)
                } else {
                    line.clone()
                };
                text_lines_col = text_lines_col.child(
                    div().whitespace_nowrap().child(display_str)
                );
            }

            root = root.child(
                div()
                    .absolute()
                    .left(self.text_input_pos.x + px(pad))
                    .top(self.text_input_pos.y + px(pad - 0.20 * font_size))
                    .w(px(inner_w))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .child(text_lines_col),
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
                root = root.child(render_magnifier(
                    self.cursor_pos,
                    info,
                    screen_bounds,
                    self.active_tool == ToolKind::Eyedropper,
                    lang,
                ));
            }
        }

        // CleanShot instruction badge or window snap highlight before selection
        let is_creating = matches!(self.drag_mode, SelectionDragMode::Creating { .. });
        if !selection.is_active && !is_creating {
            let scr_w = f32::from(screen_bounds.size.width);
            let scr_h = f32::from(screen_bounds.size.height);

            if let Some(win) = &self.hovered_window {
                let wb = win.bounds;
                // 1. Semi-transparent accent tint & highlight border over the detected window
                root = root.child(
                    div()
                        .absolute()
                        .left(wb.origin.x)
                        .top(wb.origin.y)
                        .w(wb.size.width)
                        .h(wb.size.height)
                        .bg(rgba(0x8174FF16))
                        .border_2()
                        .border_color(rgba(0x8174FFCC))
                        .rounded_lg(),
                );

                // 2. Window info pill badge (Window name + Dimensions + Click to snap hint)
                let badge_w = 520.0;
                let win_cx = f32::from(wb.origin.x) + f32::from(wb.size.width) / 2.0;
                let badge_x = (win_cx - badge_w / 2.0).clamp(16.0, (scr_w - badge_w - 16.0).max(16.0));
                let badge_y = (f32::from(wb.origin.y) + 16.0).clamp(16.0, (scr_h - 48.0).max(16.0));

                let win_title = if !win.title.trim().is_empty() {
                    let mut s = win.title.trim().to_string();
                    if s.chars().count() > 24 {
                        s = s.chars().take(22).collect::<String>() + "…";
                    }
                    format!("{} • ", s)
                } else {
                    String::new()
                };

                let dim_text = format!("{} × {}", f32::from(wb.size.width) as u32, f32::from(wb.size.height) as u32);
                let hint_text = t(
                    lang,
                    "Click to select window",
                    "Клик — выбрать окно",
                );

                root = root.child(
                    div()
                        .absolute()
                        .top(px(badge_y))
                        .left(px(badge_x))
                        .w(px(badge_w))
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .py_1p5()
                                .px_4()
                                .rounded_full()
                                .bg(rgba(0x131317FA))
                                .border_1()
                                .border_color(rgba(0x8174FFA0))
                                .shadow_2xl()
                                .flex()
                                .items_center()
                                .gap_2p5()
                                .flex_shrink_0()
                                .child(
                                    svg()
                                        .path("icons/window.svg")
                                        .size_3p5()
                                        .text_color(rgb(0x8174FF))
                                        .flex_shrink_0(),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .flex_shrink_0()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                .text_color(rgb(0xF2F2F5))
                                                .child(win_title),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(rgb(0x9D8CFF))
                                                .child(dim_text),
                                        ),
                                )
                                .child(
                                    div()
                                        .w(px(1.0))
                                        .h(px(12.0))
                                        .bg(rgba(0xFFFFFF30))
                                        .flex_shrink_0(),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0xA2A2B0))
                                        .flex_shrink_0()
                                        .child(hint_text),
                                ),
                        ),
                );
            } else {
                root = root.child(
                    div()
                        .absolute()
                        .top(px((scr_h - 44.0) / 2.0))
                        .left(px(0.0))
                        .w(screen_bounds.size.width)
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .py_2()
                                .px_5()
                                .rounded_full()
                                .bg(rgba(0x151519F0))
                                .border_1()
                                .border_color(rgba(0xFFFFFF20))
                                .shadow_2xl()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(rgb(0xD0D0D8))
                                        .child(t(
                                            lang,
                                            "Drag to select area  •  Space for fullscreen  •  Esc / Right Click to cancel",
                                            "Выделите область  •  Пробел — весь экран  •  Esc / ПКМ — отмена",
                                        )),
                                ),
                        ),
                );
            }
        }

        // CleanShot X Floating Toolbar: shown after releasing mouse (not while actively dragging creation)
        let is_creating = matches!(self.drag_mode, SelectionDragMode::Creating { .. });
        if selection.is_active && !is_creating {
            let lang = self.config.language.resolve();
            let active_tool = self.active_tool;
            let effective_tool = if active_tool == ToolKind::Select {
                if let Some(idx) = self.selected_annotation {
                    if idx < self.history.annotations.len() {
                        match &self.history.annotations[idx] {
                            Annotation::Pencil { .. } => ToolKind::Pencil,
                            Annotation::Marker { .. } => ToolKind::Marker,
                            Annotation::Arrow { .. } => ToolKind::Arrow,
                            Annotation::Line { .. } => ToolKind::Line,
                            Annotation::Rectangle { .. } => ToolKind::Rectangle,
                            Annotation::Ellipse { .. } => ToolKind::Ellipse,
                            Annotation::Triangle { .. } => ToolKind::Triangle,
                            Annotation::Star { .. } => ToolKind::Star,
                            Annotation::Diamond { .. } => ToolKind::Diamond,
                            Annotation::Heart { .. } => ToolKind::Heart,
                            Annotation::Cross { .. } => ToolKind::Cross,
                            Annotation::Checkmark { .. } => ToolKind::Checkmark,
                            Annotation::Text { .. } => ToolKind::Text,
                            Annotation::Blur { .. } => ToolKind::Blur,
                            Annotation::Pixelate { .. } => ToolKind::Pixelate,
                            Annotation::Spotlight { .. } => ToolKind::Spotlight,
                            Annotation::NumberBadge { .. } => ToolKind::NumberBadge,
                        }
                    } else {
                        ToolKind::Select
                    }
                } else {
                    ToolKind::Select
                }
            } else {
                active_tool
            };

            let show_props = effective_tool != ToolKind::Select && effective_tool != ToolKind::Eyedropper;
            let current_color = self.tool_props.stroke_color;
            let stroke_width = self.tool_props.stroke_width;
            let theme_ref = &self.theme;
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
                    if this.palette_drag_mode != PaletteDragMode::None {
                        this.palette_drag_mode = PaletteDragMode::None;
                        this.sync_and_save_tool_props();
                    }
                    cx.stop_propagation();
                    cx.notify();
                }));

            // Main Toolbar with SVG Icons
            toolbar_col = toolbar_col.child(
                div()
                    .id("main-toolbar-panel")
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
                    .cursor_default()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .on_mouse_move(|_, _, cx| {
                        cx.stop_propagation();
                    })
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
                    // Shapes dropdown button (Rectangle, Ellipse, Triangle, Star, Diamond, Arrow, Line)
                    .child({
                        let is_shape_active = active_tool.is_shape();
                        let current_shape_icon = self.last_used_shape.icon_path();
                        let is_menu_open = self.show_shapes_menu;
                        render_toolbar_dropdown_button(
                            "btn-shapes-dropdown".into(),
                            current_shape_icon,
                            is_shape_active || is_menu_open,
                            theme_ref,
                            cx.listener(|this, _, _, cx| {
                                this.show_shapes_menu = !this.show_shapes_menu;
                                if this.show_shapes_menu {
                                    this.show_beautify_menu = false;
                                    this.show_color_palette = false;
                                    if !this.active_tool.is_shape() {
                                        this.active_tool = this.last_used_shape;
                                    }
                                }
                                cx.notify();
                            }),
                            cx.listener(move |this, hovered, _, cx| {
                                this.set_tooltip(t(lang, "Shapes", "Фигуры"), 147.0, *hovered, cx);
                            }),
                        )
                    })
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
                            this.set_tooltip(t(lang, "Text Annotation", "Текст"), 183.0, *hovered, cx);
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
                            this.set_tooltip(t(lang, "Step Badge Counter", "Шаги / Нумерация"), 219.0, *hovered, cx);
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
                            this.set_tooltip(t(lang, "Blur Sensitive Region", "Размытие"), 260.0, *hovered, cx);
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
                            this.set_tooltip(t(lang, "Pixelate Mosaic", "Пикселизация"), 296.0, *hovered, cx);
                        }),
                    ))
                    .child(render_toolbar_button(
                        "btn-spotlight".into(),
                        "icons/spotlight.svg",
                        active_tool == ToolKind::Spotlight,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.active_tool = ToolKind::Spotlight;
                            cx.notify();
                        }),
                        cx.listener(move |this, hovered, _, cx| {
                            this.set_tooltip(t(lang, "Spotlight Focus / Dim", "Фокус / Затемнение"), 332.0, *hovered, cx);
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
                            this.set_tooltip(t(lang, "Eyedropper Color Picker", "Пипетка цвета"), 368.0, *hovered, cx);
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
                            this.set_tooltip(t(lang, "Windows WinRT Text Recognition", "Распознавание текста (OCR)"), 411.0, *hovered, cx);
                        }),
                    ))
                    .child(render_toolbar_text_button(
                        "btn-qr".into(),
                        "QR",
                        false,
                        theme_ref,
                        cx.listener(|this, _, _, cx| {
                            this.do_qr_scan(cx);
                        }),
                        cx.listener(move |this, hovered, _, cx| {
                            this.set_tooltip(t(lang, "QR Code Scanner", "Сканировать QR-код"), 447.0, *hovered, cx);
                        }),
                    ))
                    .child({
                        let is_beautify = self.is_beautify_active;
                        let is_menu_open = self.show_beautify_menu;
                        render_toolbar_dropdown_button(
                            "btn-beautify".into(),
                            "icons/sparkles.svg",
                            is_beautify || is_menu_open,
                            theme_ref,
                            cx.listener(|this, _, _, cx| {
                                this.show_beautify_menu = !this.show_beautify_menu;
                                if this.show_beautify_menu {
                                    this.show_shapes_menu = false;
                                    this.show_color_palette = false;
                                }
                                cx.notify();
                            }),
                            cx.listener(move |this, hovered, _, cx| {
                                this.set_tooltip(t(lang, "Beautify & Presets", "Оформление и пресеты"), 481.0, *hovered, cx);
                            }),
                        )
                    })
                    .child(render_toolbar_button(
                        "btn-pin".into(),
                        "icons/pin.svg",
                        false,
                        theme_ref,
                        cx.listener(|this, _, window, cx| {
                            this.do_pin(window, cx);
                        }),
                        cx.listener(move |this, hovered, _, cx| {
                            this.set_tooltip(t(lang, "Pin to Screen (Always on Top)", "Закрепить поверх окон"), 529.0, *hovered, cx);
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
                            this.set_tooltip(t(lang, "Undo (Ctrl+Z)", "Отменить (Ctrl+Z)"), 565.0, *hovered, cx);
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
                            this.set_tooltip(t(lang, "Redo (Ctrl+Y)", "Повторить (Ctrl+Y)"), 601.0, *hovered, cx);
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
                            this.set_tooltip(t(lang, "Save to PNG (Ctrl+S)", "Сохранить в PNG (Ctrl+S)"), 637.0, *hovered, cx);
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
                                this.set_tooltip(t(lang, "Copy to Clipboard (Ctrl+C)", "Копировать в буфер (Ctrl+C)"), 696.0, *hovered, cx);
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
                                this.set_tooltip(t(lang, "Close Overlay (Esc)", "Закрыть (Esc)"), 761.0, *hovered, cx);
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
            if effective_tool == ToolKind::Blur {
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
                                .child(t(lang, "Blur Hardness:", "Интенсивность:")),
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
                                            this.sync_and_save_tool_props();
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
                                            this.tool_props.blur_radius = (this.tool_props.blur_radius + 2).min(99);
                                            this.sync_and_save_tool_props();
                                            cx.notify();
                                        }))
                                        .child(svg().path("icons/plus.svg").size(px(12.0)).text_color(theme.text_secondary)),
                                ),
                        ),
                );
            } else if effective_tool == ToolKind::Pixelate {
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
                                .child(t(lang, "Pixel Size:", "Размер пикселей:")),
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
                                            this.sync_and_save_tool_props();
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
                                            this.tool_props.pixelate_block = (this.tool_props.pixelate_block + 2).min(99);
                                            this.sync_and_save_tool_props();
                                            cx.notify();
                                        }))
                                        .child(svg().path("icons/plus.svg").size(px(12.0)).text_color(theme.text_secondary)),
                                ),
                        ),
                );
            } else if effective_tool == ToolKind::Spotlight {
                let dim_pct = (self.tool_props.spotlight_dim * 100.0).round() as u32;
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
                                .path("icons/spotlight.svg")
                                .size(px(16.0))
                                .text_color(theme.accent),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme.text_secondary)
                                .child(t(lang, "Dimming:", "Затемнение:")),
                        )
                        // Stepper: [-] [ 60% ] [+]
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .id("btn-dec-spotlight-dim")
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
                                            this.tool_props.spotlight_dim = ((this.tool_props.spotlight_dim - 0.05) * 100.0).round() / 100.0;
                                            this.tool_props.spotlight_dim = this.tool_props.spotlight_dim.clamp(0.10, 0.95);
                                            this.sync_and_save_tool_props();
                                            cx.notify();
                                        }))
                                        .child(svg().path("icons/minus.svg").size(px(12.0)).text_color(theme.text_secondary)),
                                )
                                .child(
                                    div()
                                        .id("spotlight-dim-display")
                                        .px_2()
                                        .py_0p5()
                                        .min_w(px(46.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded_md()
                                        .bg(if self.numeric_input_mode == Some(NumericInputTarget::SpotlightDim) {
                                            rgba(0x8174FF33)
                                        } else {
                                            theme.bg_secondary
                                        })
                                        .border_1()
                                        .border_color(if self.numeric_input_mode == Some(NumericInputTarget::SpotlightDim) {
                                            theme.accent
                                        } else {
                                            theme.border_subtle
                                        })
                                        .hover(|s| s.border_color(theme.border_focus))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.numeric_input_mode = Some(NumericInputTarget::SpotlightDim);
                                            this.numeric_input_buffer = ((this.tool_props.spotlight_dim * 100.0).round() as u32).to_string();
                                            cx.notify();
                                        }))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(theme.text_primary)
                                                .child(if self.numeric_input_mode == Some(NumericInputTarget::SpotlightDim) {
                                                    format!("{}%▏", self.numeric_input_buffer)
                                                } else {
                                                    format!("{}%", dim_pct)
                                                }),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("btn-inc-spotlight-dim")
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
                                            this.tool_props.spotlight_dim = ((this.tool_props.spotlight_dim + 0.05) * 100.0).round() / 100.0;
                                            this.tool_props.spotlight_dim = this.tool_props.spotlight_dim.clamp(0.10, 0.95);
                                            this.sync_and_save_tool_props();
                                            cx.notify();
                                        }))
                                        .child(svg().path("icons/plus.svg").size(px(12.0)).text_color(theme.text_secondary)),
                                ),
                        ),
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
                        // Quick Color Swatches: White, Black, Red
                        .child(render_color_swatch(
                            "swatch-white".into(),
                            theme.palette_white,
                            current_color == theme.palette_white,
                            cx.listener(|this, _, _, cx| {
                                this.tool_props.stroke_color = this.theme.palette_white;
                                this.sync_and_save_tool_props();
                                cx.notify();
                            }),
                        ))
                        .child(render_color_swatch(
                            "swatch-black".into(),
                            theme.palette_black,
                            current_color == theme.palette_black,
                            cx.listener(|this, _, _, cx| {
                                this.tool_props.stroke_color = this.theme.palette_black;
                                this.sync_and_save_tool_props();
                                cx.notify();
                            }),
                        ))
                        .child(render_color_swatch(
                            "swatch-red".into(),
                            theme.palette_red,
                            current_color == theme.palette_red,
                            cx.listener(|this, _, _, cx| {
                                this.tool_props.stroke_color = this.theme.palette_red;
                                this.sync_and_save_tool_props();
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
                        .when(effective_tool != ToolKind::Text, |p| {
                            p.child(
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
                                            this.sync_and_save_tool_props();
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
                                                (this.tool_props.stroke_width + 1.0).min(99.0);
                                            this.sync_and_save_tool_props();
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
                        })
                        // Fill toggle for closed shapes (Rect, Ellipse, Triangle, Star, Diamond)
                        .when(
                            effective_tool == ToolKind::Rectangle
                                || effective_tool == ToolKind::Ellipse
                                || effective_tool == ToolKind::Triangle
                                || effective_tool == ToolKind::Star
                                || effective_tool == ToolKind::Diamond
                                || effective_tool == ToolKind::Heart
                                || effective_tool == ToolKind::Cross
                                || effective_tool == ToolKind::Checkmark,
                            |p| {
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
                                        this.sync_and_save_tool_props();
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
                        // Font size stepper for Text tool
                        .when(effective_tool == ToolKind::Text, |p| {
                            p.child(render_toolbar_divider(theme_ref)).child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .id("btn-dec-font")
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
                                                this.tool_props.font_size = (this.tool_props.font_size - 2.0).max(10.0);
                                                this.sync_and_save_tool_props();
                                                cx.notify();
                                            }))
                                            .child(svg().path("icons/minus.svg").size(px(12.0)).text_color(theme.text_secondary)),
                                    )
                                    .child(
                                        div()
                                            .id("font-size-display")
                                            .px_2()
                                            .py_0p5()
                                            .min_w(px(40.0))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .rounded_md()
                                            .bg(if self.numeric_input_mode == Some(NumericInputTarget::FontSize) {
                                                rgba(0x8174FF33)
                                            } else {
                                                theme.bg_secondary
                                            })
                                            .border_1()
                                            .border_color(if self.numeric_input_mode == Some(NumericInputTarget::FontSize) {
                                                theme.accent
                                            } else {
                                                theme.border_subtle
                                            })
                                            .hover(|s| s.border_color(theme.border_focus))
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.numeric_input_mode = Some(NumericInputTarget::FontSize);
                                                this.numeric_input_buffer = (this.tool_props.font_size as u32).to_string();
                                                cx.notify();
                                            }))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(theme.text_primary)
                                                    .child(if self.numeric_input_mode == Some(NumericInputTarget::FontSize) {
                                                        format!("{}▏", self.numeric_input_buffer)
                                                    } else {
                                                        format!("{}px", self.tool_props.font_size as u32)
                                                    }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("btn-inc-font")
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
                                                this.tool_props.font_size = (this.tool_props.font_size + 2.0).min(96.0);
                                                this.sync_and_save_tool_props();
                                                cx.notify();
                                            }))
                                            .child(svg().path("icons/plus.svg").size(px(12.0)).text_color(theme.text_secondary)),
                                    ),
                            )
                        })
                        // Arrow controls (Curved Bezier & Double-headed)
                        .when(effective_tool == ToolKind::Arrow, |p| {
                            p.child(render_toolbar_divider(theme_ref))
                                .child(
                                    div()
                                        .id("toggle-arrow-curved")
                                        .px_2()
                                        .py_0p5()
                                        .rounded_md()
                                        .bg(if self.tool_props.arrow_curved {
                                            rgba(0x8174FF33)
                                        } else {
                                            theme.bg_secondary
                                        })
                                        .border_1()
                                        .border_color(if self.tool_props.arrow_curved {
                                            theme.accent
                                        } else {
                                            theme.border_subtle
                                        })
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.tool_props.arrow_curved = !this.tool_props.arrow_curved;
                                            this.sync_and_save_tool_props();
                                            cx.notify();
                                        }))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(theme.text_primary)
                                                .child(if self.tool_props.arrow_curved {
                                                    t(lang, "Curved", "Изгиб")
                                                } else {
                                                    t(lang, "Straight", "Прямая")
                                                }),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("toggle-arrow-double")
                                        .px_2()
                                        .py_0p5()
                                        .rounded_md()
                                        .bg(if self.tool_props.arrow_two_headed {
                                            rgba(0x8174FF33)
                                        } else {
                                            theme.bg_secondary
                                        })
                                        .border_1()
                                        .border_color(if self.tool_props.arrow_two_headed {
                                            theme.accent
                                        } else {
                                            theme.border_subtle
                                        })
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.tool_props.arrow_two_headed = !this.tool_props.arrow_two_headed;
                                            this.sync_and_save_tool_props();
                                            cx.notify();
                                        }))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(theme.text_primary)
                                                .child(if self.tool_props.arrow_two_headed {
                                                    "<--->"
                                                } else {
                                                    "--->"
                                                }),
                                        ),
                                )
                        })
                        // Step badge controls (Shape, Mode, Fill, Reset)
                        .when(effective_tool == ToolKind::NumberBadge, |p| {
                            p.child(render_toolbar_divider(theme_ref))
                                .child(
                                    div()
                                        .id("toggle-badge-shape")
                                        .px_2()
                                        .py_0p5()
                                        .rounded_md()
                                        .bg(if self.tool_props.badge_shape == BadgeShape::RoundedRect {
                                            rgba(0x8174FF33)
                                        } else {
                                            theme.bg_secondary
                                        })
                                        .border_1()
                                        .border_color(if self.tool_props.badge_shape == BadgeShape::RoundedRect {
                                            theme.accent
                                        } else {
                                            theme.border_subtle
                                        })
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.tool_props.badge_shape = match this.tool_props.badge_shape {
                                                BadgeShape::Circle => BadgeShape::RoundedRect,
                                                BadgeShape::RoundedRect => BadgeShape::Circle,
                                            };
                                            this.sync_and_save_tool_props();
                                            cx.notify();
                                        }))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(theme.text_primary)
                                                .child(if self.tool_props.badge_shape == BadgeShape::Circle {
                                                    t(lang, "Circle", "Круг")
                                                } else {
                                                    t(lang, "Square", "Квадрат")
                                                }),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("toggle-badge-mode")
                                        .px_2()
                                        .py_0p5()
                                        .rounded_md()
                                        .bg(if self.tool_props.badge_mode == BadgeMode::Alphabet {
                                            rgba(0x8174FF33)
                                        } else {
                                            theme.bg_secondary
                                        })
                                        .border_1()
                                        .border_color(if self.tool_props.badge_mode == BadgeMode::Alphabet {
                                            theme.accent
                                        } else {
                                            theme.border_subtle
                                        })
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.tool_props.badge_mode = match this.tool_props.badge_mode {
                                                BadgeMode::Numbers => BadgeMode::Alphabet,
                                                BadgeMode::Alphabet => BadgeMode::Numbers,
                                            };
                                            this.sync_and_save_tool_props();
                                            cx.notify();
                                        }))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(theme.text_primary)
                                                .child(if self.tool_props.badge_mode == BadgeMode::Numbers {
                                                    "123"
                                                } else {
                                                    "ABC"
                                                }),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("toggle-badge-fill")
                                        .px_2()
                                        .py_0p5()
                                        .rounded_md()
                                        .bg(if self.tool_props.badge_filled {
                                            rgba(0x8174FF33)
                                        } else {
                                            theme.bg_secondary
                                        })
                                        .border_1()
                                        .border_color(if self.tool_props.badge_filled {
                                            theme.accent
                                        } else {
                                            theme.border_subtle
                                        })
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.tool_props.badge_filled = !this.tool_props.badge_filled;
                                            this.sync_and_save_tool_props();
                                            cx.notify();
                                        }))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(theme.text_primary)
                                                .child(if self.tool_props.badge_filled {
                                                    t(lang, "Fill", "Заливка")
                                                } else {
                                                    t(lang, "Outline", "Контур")
                                                }),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("btn-reset-badge-counter")
                                        .px_2()
                                        .py_0p5()
                                        .rounded_md()
                                        .bg(theme.bg_secondary)
                                        .border_1()
                                        .border_color(theme.border_subtle)
                                        .hover(|s| s.bg(theme.bg_hover))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.tool_props.current_badge_number = 1;
                                            this.sync_and_save_tool_props();
                                            cx.notify();
                                        }))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(theme.text_secondary)
                                                .child(format!("↺ 1 ({})", self.tool_props.current_badge_number)),
                                        ),
                                )
                        })
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

                self.palette_panel_bounds = Some(Bounds {
                    origin: point(px(palette_left), px(palette_top)),
                    size: size(px(palette_w), px(palette_h)),
                });

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
                            if this.palette_drag_mode != PaletteDragMode::None {
                                this.palette_drag_mode = PaletteDragMode::None;
                                this.sync_and_save_tool_props();
                            }
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
                                            this.sync_and_save_tool_props();
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
                                                this.sync_and_save_tool_props();
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
                                        this.sync_and_save_tool_props();
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

            // Shapes Dropdown Menu Popover
            if self.show_shapes_menu {
                let is_ru = lang.is_ru();
                let scr_h = f32::from(screen_bounds.size.height);
                let shapes_w = 230.0;
                let shapes_h = 420.0;
                let shapes_drop_up = f32::from(toolbar_pos.y) + bar_height + shapes_h + 16.0 > scr_h - 12.0;
                let shapes_top = if shapes_drop_up {
                    (f32::from(toolbar_pos.y) - shapes_h - 8.0).max(12.0)
                } else {
                    f32::from(toolbar_pos.y) + bar_height + 8.0
                };
                let shapes_left = (f32::from(toolbar_pos.x) + 80.0).clamp(12.0, f32::from(screen_bounds.size.width) - shapes_w - 12.0);

                let shapes_list = [
                    (ToolKind::Rectangle, "icons/rectangle.svg", "Прямоугольник", "Rectangle"),
                    (ToolKind::Ellipse, "icons/circle.svg", "Эллипс", "Ellipse"),
                    (ToolKind::Triangle, "icons/triangle.svg", "Треугольник", "Triangle"),
                    (ToolKind::Star, "icons/star.svg", "Звезда", "Star"),
                    (ToolKind::Heart, "icons/heart.svg", "Сердце", "Heart"),
                    (ToolKind::Cross, "icons/cross.svg", "Крест", "Cross"),
                    (ToolKind::Checkmark, "icons/checkmark.svg", "Галочка", "Checkmark"),
                    (ToolKind::Diamond, "icons/diamond.svg", "Ромб", "Diamond"),
                    (ToolKind::Arrow, "icons/arrow.svg", "Стрелка", "Arrow"),
                    (ToolKind::Line, "icons/line.svg", "Линия", "Line"),
                ];

                let shapes_bounds = Bounds {
                    origin: point(px(shapes_left), px(shapes_top)),
                    size: size(px(shapes_w), px(shapes_h)),
                };
                self.shapes_panel_bounds = Some(shapes_bounds);

                let mut shapes_col = div()
                    .id("shapes-dropdown-panel")
                    .absolute()
                    .left(px(shapes_left))
                    .top(px(shapes_top))
                    .w(px(shapes_w))
                    .p_2()
                    .rounded_xl()
                    .bg(theme.bg_surface)
                    .border_1()
                    .border_color(theme.border_subtle)
                    .shadow_2xl()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .on_mouse_move(|_, _, cx| {
                        cx.stop_propagation();
                    })
                    // Header
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .pb_1p5()
                            .px_1()
                            .border_b_1()
                            .border_color(theme.border_subtle)
                            .cursor_default()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(theme.text_primary)
                                    .child(if is_ru { "Фигуры" } else { "Shapes" }),
                            )
                            .child(
                                div()
                                    .id("btn-close-shapes")
                                    .p_1()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.bg_hover))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.show_shapes_menu = false;
                                        this.shapes_btn_hovered = false;
                                        this.shapes_menu_hovered = false;
                                        this.shapes_hover_token = this.shapes_hover_token.wrapping_add(1);
                                        cx.notify();
                                    }))
                                    .child(
                                        svg()
                                            .path("icons/close.svg")
                                            .size(px(14.0))
                                            .text_color(theme.text_muted),
                                    ),
                            ),
                    );

                for (kind, icon, ru_name, en_name) in shapes_list {
                    let is_selected = active_tool == kind;
                    let display_text = if is_ru { ru_name } else { en_name };
                    shapes_col = shapes_col.child(
                        div()
                            .id(ElementId::NamedInteger("shape-item".into(), kind as usize as u64))
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_2p5()
                            .py_1p5()
                            .rounded_lg()
                            .cursor_pointer()
                            .bg(if is_selected {
                                rgba(0x8174FF22)
                            } else {
                                rgba(0x00000000)
                            })
                            .border_1()
                            .border_color(if is_selected {
                                theme.accent
                            } else {
                                rgba(0x00000000)
                            })
                            .hover(|s| s.bg(theme.bg_hover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.active_tool = kind;
                                this.last_used_shape = kind;
                                this.show_shapes_menu = false;
                                this.shapes_btn_hovered = false;
                                this.shapes_menu_hovered = false;
                                this.shapes_hover_token = this.shapes_hover_token.wrapping_add(1);
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        svg()
                                            .path(icon)
                                            .size(px(16.0))
                                            .text_color(if is_selected {
                                                theme.accent
                                            } else {
                                                theme.text_secondary
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(if is_selected {
                                                gpui::FontWeight::SEMIBOLD
                                            } else {
                                                gpui::FontWeight::NORMAL
                                            })
                                            .text_color(if is_selected {
                                                theme.accent
                                            } else {
                                                theme.text_primary
                                            })
                                            .child(display_text),
                                    ),
                            )
                            .child(
                                if is_selected {
                                    div().child(
                                        svg()
                                            .path("icons/check.svg")
                                            .size(px(12.0))
                                            .text_color(theme.accent),
                                    )
                                } else {
                                    div()
                                },
                            ),
                    );
                }

                root = root.child(shapes_col);
            }

            // Background & Substrate Dropdown Menu with Live Thumbnails
            if self.show_beautify_menu {
                let is_ru = lang.is_ru();
                let scr_h = f32::from(screen_bounds.size.height);
                let b_w = 460.0;
                let b_h = 270.0;
                let b_drop_up = f32::from(toolbar_pos.y) + bar_height + b_h + 16.0 > scr_h - 12.0;
                let b_top = if b_drop_up {
                    (f32::from(toolbar_pos.y) - b_h - 8.0).max(12.0)
                } else {
                    f32::from(toolbar_pos.y) + bar_height + 8.0
                };
                let b_left = (f32::from(toolbar_pos.x) + 290.0).clamp(12.0, f32::from(screen_bounds.size.width) - b_w - 12.0);

                let is_active = self.is_beautify_active;
                let current_idx = self.beautify_preset_idx;

                let b_bounds = Bounds {
                    origin: point(px(b_left), px(b_top)),
                    size: size(px(b_w), px(b_h)),
                };
                self.beautify_panel_bounds = Some(b_bounds);

                let mut b_panel = div()
                    .id("beautify-dropdown-panel")
                    .absolute()
                    .left(px(b_left))
                    .top(px(b_top))
                    .w(px(b_w))
                    .p_3()
                    .rounded_xl()
                    .bg(theme.bg_surface)
                    .border_1()
                    .border_color(theme.border_subtle)
                    .shadow_2xl()
                    .flex()
                    .flex_col()
                    .gap_2p5()
                    .cursor_default()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .on_mouse_move(|_, _, cx| {
                        cx.stop_propagation();
                    })
                    // Header: Title + Toggle Switch + Close
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .pb_2()
                            .border_b_1()
                            .border_color(theme.border_subtle)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        svg()
                                            .path("icons/sparkles.svg")
                                            .size(px(16.0))
                                            .text_color(theme.accent),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(theme.text_primary)
                                            .child(if is_ru { "Фон и подложка" } else { "Background & Substrate" }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("toggle-beautify-switch")
                                            .px_2p5()
                                            .py_0p5()
                                            .rounded_full()
                                            .cursor_pointer()
                                            .bg(if is_active {
                                                theme.accent
                                            } else {
                                                theme.bg_secondary
                                            })
                                            .border_1()
                                            .border_color(if is_active {
                                                theme.accent
                                            } else {
                                                theme.border_subtle
                                            })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.is_beautify_active = !this.is_beautify_active;
                                                cx.notify();
                                            }))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(if is_active {
                                                        rgb(0xFFFFFF)
                                                    } else {
                                                        theme.text_secondary
                                                    })
                                                    .child(if is_active {
                                                        if is_ru { "ВКЛ" } else { "ON" }
                                                    } else {
                                                        if is_ru { "ВЫКЛ" } else { "OFF" }
                                                    }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("btn-close-beautify")
                                            .p_1()
                                            .rounded_md()
                                            .cursor_pointer()
                                            .hover(|s| s.bg(theme.bg_hover))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.show_beautify_menu = false;
                                                this.beautify_btn_hovered = false;
                                                this.beautify_menu_hovered = false;
                                                this.beautify_hover_token = this.beautify_hover_token.wrapping_add(1);
                                                cx.notify();
                                            }))
                                            .child(
                                                svg()
                                                    .path("icons/close.svg")
                                                    .size(px(14.0))
                                                    .text_color(theme.text_muted),
                                            ),
                                    ),
                            ),
                    );

                // Grid of Thumbnail Cards
                let mut thumb_grid = div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .justify_start();

                for (idx, (preset, render_img)) in self.preset_thumbnails.iter().enumerate() {
                    let is_selected = is_active && (current_idx == idx);
                    let display_name = preset.display_name(is_ru);
                    let img_clone = render_img.clone();

                    thumb_grid = thumb_grid.child(
                        div()
                            .id(ElementId::NamedInteger("preset-card".into(), idx as u64))
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_1()
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.beautify_preset_idx = idx;
                                this.is_beautify_active = true;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .relative()
                                    .w(px(98.0))
                                    .h(px(58.0))
                                    .rounded_lg()
                                    .overflow_hidden()
                                    .border_2()
                                    .border_color(if is_selected {
                                        theme.accent
                                    } else {
                                        rgba(0xFFFFFF18)
                                    })
                                    .hover(|s| s.border_color(theme.border_focus))
                                    .child(
                                        canvas(|_, _, _| (), move |bounds, _, window, _cx| {
                                            let _ = window.paint_image(bounds, Corners::default(), img_clone.clone(), 0, false);
                                            // Miniature preview frame representing the screenshot in the center
                                            let pad_x = px(18.0);
                                            let pad_y = px(12.0);
                                            let inner = Bounds {
                                                origin: point(bounds.origin.x + pad_x, bounds.origin.y + pad_y),
                                                size: size(bounds.size.width - pad_x * 2.0, bounds.size.height - pad_y * 2.0),
                                            };
                                            // Subtle shadow
                                            let shadow_b = Bounds {
                                                origin: point(inner.origin.x + px(1.0), inner.origin.y + px(2.0)),
                                                size: inner.size,
                                            };
                                            window.paint_quad(fill(shadow_b, rgba(0x00000044)));
                                            window.paint_quad(fill(inner, rgba(0xFFFFFFEE)));
                                            window.paint_quad(outline(inner, rgba(0x00000022), BorderStyle::Solid));
                                        })
                                        .size_full(),
                                    )
                                    .child(
                                        if is_selected {
                                            div()
                                                .absolute()
                                                .top(px(3.0))
                                                .right(px(3.0))
                                                .size(px(16.0))
                                                .rounded_full()
                                                .bg(theme.accent)
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child(
                                                    svg()
                                                        .path("icons/check.svg")
                                                        .size(px(10.0))
                                                        .text_color(rgb(0xFFFFFF)),
                                                )
                                        } else {
                                            div()
                                        },
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(if is_selected {
                                        gpui::FontWeight::BOLD
                                    } else {
                                        gpui::FontWeight::NORMAL
                                    })
                                    .text_color(if is_selected {
                                        theme.accent
                                    } else {
                                        theme.text_secondary
                                    })
                                    .child(display_name),
                            ),
                    );
                }

                b_panel = b_panel.child(thumb_grid);
                root = root.child(b_panel);
            }
        }

        // CleanShot toast feedback
        if let Some((msg, created)) = &self.toast_message {
            if created.elapsed() < Duration::from_secs(3) {
                let is_err = msg.to_lowercase().contains("failed") || msg.to_lowercase().contains("ошибка");
                let icon_path = if is_err { "icons/close.svg" } else { "icons/check.svg" };
                let accent_color = if is_err { rgb(0xFF453A) } else { theme.accent };

                root = root.child(
                    div()
                        .absolute()
                        .bottom(px(32.0))
                        .left(px(0.0))
                        .w(screen_size.width)
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .max_w(px((f32::from(screen_size.width) - 64.0).min(760.0)))
                                .py_2p5()
                                .px_5()
                                .rounded_xl()
                                .bg(theme.bg_surface)
                                .border_1()
                                .border_color(accent_color)
                                .shadow_2xl()
                                .flex()
                                .items_center()
                                .gap_2p5()
                                .child(
                                    svg()
                                        .path(icon_path)
                                        .size(px(14.0))
                                        .text_color(accent_color),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(theme.text_primary)
                                        .child(msg.clone()),
                                ),
                        ),
                );
            }
        }

        root
    }
}

impl Drop for ScreenOverlayView {
    fn drop(&mut self) {
        self.sync_and_save_tool_props();
        crate::app::IS_OVERLAY_OPEN.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}
