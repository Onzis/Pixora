use std::sync::Arc;
use gpui::{
    canvas, div, px, point, size, rgb, rgba, svg, Bounds, Context, Corners,
    CursorStyle, IntoElement, MouseButton, Pixels, Point, Render, RenderImage,
    ScrollDelta, ScrollWheelEvent, Window, WindowBackgroundAppearance, WindowBounds,
    WindowKind, WindowOptions, prelude::*,
};
use image::{Frame, RgbaImage};
use smallvec::smallvec;
use crate::theme::PixoraTheme;

pub struct PinnedScreenshotView {
    pub image: RgbaImage,
    pub render_image: Arc<RenderImage>,
    pub zoom: f32,
    pub opacity: f32,
    pub is_hovered: bool,
    pub theme: PixoraTheme,
}

impl PinnedScreenshotView {
    pub fn new(image: RgbaImage) -> Self {
        let frame = Frame::new(image.clone());
        let render_image = Arc::new(RenderImage::new(smallvec![frame]));
        Self {
            image,
            render_image,
            zoom: 1.0,
            opacity: 1.0,
            is_hovered: false,
            theme: PixoraTheme::default(),
        }
    }

    #[cfg(windows)]
    pub fn start_native_drag(&self, window: &mut Window) {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
        use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
        use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, HTCAPTION, WM_NCLBUTTONDOWN};

        if let Ok(handle) = window.window_handle() {
            if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
                let hwnd = HWND(win32_handle.hwnd.get() as *mut _);
                unsafe {
                    let _ = ReleaseCapture();
                    let _ = SendMessageW(
                        hwnd,
                        WM_NCLBUTTONDOWN,
                        Some(WPARAM(HTCAPTION as usize)),
                        Some(LPARAM(0)),
                    );
                }
            }
        }
    }

    #[cfg(not(windows))]
    pub fn start_native_drag(&self, _window: &mut Window) {}
}

impl Render for PinnedScreenshotView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let orig_w = self.image.width() as f32;
        let orig_h = self.image.height() as f32;
        let cur_w = orig_w * self.zoom;
        let cur_h = orig_h * self.zoom;

        let render_img = self.render_image.clone();
        let opacity = self.opacity;
        let zoom_pct = (self.zoom * 100.0).round() as u32;
        let show_controls = self.is_hovered;

        div()
            .id("pinned-window-root")
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(rgba(0x00000001)) // Non-zero alpha so hit test catches mouse events
            .cursor(CursorStyle::Arrow)
            .on_hover(cx.listener(|this, hovered, _, cx| {
                this.is_hovered = *hovered;
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|_, _, window, _| {
                    window.remove_window();
                }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, _| {
                    this.start_native_drag(window);
                }),
            )
            .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _, cx| {
                let delta = match ev.delta {
                    ScrollDelta::Lines(lines) => lines.y,
                    ScrollDelta::Pixels(pixels) => f32::from(pixels.y) / 20.0,
                };
                if ev.modifiers.control {
                    if delta > 0.0 {
                        this.opacity = (this.opacity + 0.05).min(1.0);
                    } else if delta < 0.0 {
                        this.opacity = (this.opacity - 0.05).max(0.20);
                    }
                } else {
                    if delta > 0.0 {
                        this.zoom = (this.zoom + 0.1).min(3.5);
                    } else if delta < 0.0 {
                        this.zoom = (this.zoom - 0.1).max(0.25);
                    }
                }
                cx.notify();
            }))
            .child(
                div()
                    .relative()
                    .w(px(cur_w))
                    .h(px(cur_h))
                    .rounded_md()
                    .shadow_lg()
                    .border_1()
                    .border_color(rgba(0xFFFFFF33))
                    .overflow_hidden()
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _cx| {
                                let _ = window.paint_image(bounds, Corners::default(), render_img.clone(), 0, false);
                            },
                        )
                        .size_full()
                        .opacity(opacity),
                    )
                    // Floating badge with zoom and close button on hover
                    .when(show_controls, |el| {
                        el.child(
                            div()
                                .absolute()
                                .top(px(6.0))
                                .right(px(6.0))
                                .flex()
                                .items_center()
                                .gap_1p5()
                                .px_2()
                                .py(px(3.0))
                                .rounded_md()
                                .bg(rgba(0x18181FEE))
                                .border_1()
                                .border_color(rgba(0x8174FF66))
                                .shadow_md()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0xEDEDED))
                                        .child(format!("{}%", zoom_pct)),
                                )
                                .child(
                                    div()
                                        .id("pin-btn-close")
                                        .size(px(16.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded_sm()
                                        .hover(|s| s.bg(rgba(0xFF453A44)))
                                        .cursor_pointer()
                                        .on_mouse_down(MouseButton::Left, |_, _, _| {})
                                        .on_click(cx.listener(|_, _, window, _| {
                                            window.remove_window();
                                        }))
                                        .child(
                                            svg()
                                                .path("icons/close.svg")
                                                .size(px(10.0))
                                                .text_color(rgb(0xFFFFFF)),
                                        ),
                                ),
                        )
                    }),
            )
    }
}

pub fn open_pinned_screenshot(cx: &mut gpui::App, image: RgbaImage, initial_origin: Option<Point<Pixels>>) {
    let img_w = image.width() as f32;
    let img_h = image.height() as f32;

    let origin = initial_origin.unwrap_or(point(px(120.0), px(120.0)));
    let window_options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin,
            size: size(px(img_w + 16.0), px(img_h + 16.0)),
        })),
        titlebar: None,
        focus: true,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: true,
        is_resizable: true,
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };

    let _ = cx.open_window(window_options, |window, cx| {
        #[cfg(windows)]
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            use windows::Win32::Foundation::HWND;
            use windows::Win32::UI::WindowsAndMessaging::{
                SetWindowPos, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
            };

            if let Ok(handle) = window.window_handle() {
                if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
                    let hwnd = HWND(win32_handle.hwnd.get() as *mut _);
                    unsafe {
                        let _ = SetWindowPos(
                            hwnd,
                            Some(HWND_TOPMOST),
                            0, 0, 0, 0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
                        );
                    }
                }
            }
        }

        cx.new(|_| PinnedScreenshotView::new(image))
    });
}
