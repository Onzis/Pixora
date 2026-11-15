use std::sync::Arc;
use gpui::{
    canvas, div, px, point, size, rgb, rgba, svg, Bounds, Context, Corners,
    CursorStyle, FocusHandle, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, Pixels, Point, Render, RenderImage,
    ScrollDelta, ScrollWheelEvent, Subscription, Window, WindowBackgroundAppearance, WindowBounds,
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
    pub focus_handle: FocusHandle,
    pub theme: PixoraTheme,
    pub _bounds_subscription: Option<Subscription>,
}

impl PinnedScreenshotView {
    pub fn new(image: RgbaImage, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut bgra = image.clone();
        for chunk in bgra.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }
        let frame = Frame::new(bgra);
        let render_image = Arc::new(RenderImage::new(smallvec![frame]));
        let _bounds_subscription = Some(cx.observe_window_bounds(window, |_this, _window, cx| {
            cx.notify();
        }));
        Self {
            image,
            render_image,
            zoom: 1.0,
            opacity: 1.0,
            is_hovered: false,
            focus_handle: cx.focus_handle(),
            theme: PixoraTheme::default(),
            _bounds_subscription,
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

    #[cfg(windows)]
    pub fn resize_native_window(&self, window: &mut Window, width: f32, height: f32) {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER,
        };

        let scale = window.scale_factor();
        let phys_w = (width * scale).round() as i32;
        let phys_h = (height * scale).round() as i32;

        if let Ok(handle) = window.window_handle() {
            if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
                let hwnd = HWND(win32_handle.hwnd.get() as *mut _);
                unsafe {
                    let _ = SetWindowPos(
                        hwnd,
                        None,
                        0,
                        0,
                        phys_w,
                        phys_h,
                        SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            }
        }
    }

    #[cfg(not(windows))]
    pub fn resize_native_window(&self, _window: &mut Window, _width: f32, _height: f32) {}
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
            .relative()
            .overflow_hidden()
            .bg(rgb(0x18181B))
            .cursor(CursorStyle::Arrow)
            .track_focus(&self.focus_handle)
            .on_hover(cx.listener(|this, hovered, _, cx| {
                this.is_hovered = *hovered;
                cx.notify();
            }))
            .on_key_down(cx.listener(|_, ev: &KeyDownEvent, window, _| {
                if ev.keystroke.key == "escape" {
                    window.remove_window();
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|_, _, window, cx| {
                    cx.stop_propagation();
                    window.remove_window();
                }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                    if ev.click_count >= 2 {
                        cx.stop_propagation();
                        window.remove_window();
                    } else {
                        this.start_native_drag(window);
                    }
                }),
            )
            .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, window, cx| {
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
                    let old_zoom = this.zoom;
                    if delta > 0.0 {
                        this.zoom = (this.zoom + 0.1).min(3.5);
                    } else if delta < 0.0 {
                        this.zoom = (this.zoom - 0.1).max(0.25);
                    }
                    if (this.zoom - old_zoom).abs() > 0.001 {
                        let orig_w = this.image.width() as f32;
                        let orig_h = this.image.height() as f32;
                        let cur_w = orig_w * this.zoom;
                        let cur_h = orig_h * this.zoom;
                        window.resize(size(px(cur_w), px(cur_h)));
                        this.resize_native_window(window, cur_w, cur_h);
                    }
                }
                cx.notify();
            }))
            // Image canvas explicitly sized to target zoomed dimensions - scales proportionally without cropping
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _cx| {
                        let draw_bounds = Bounds {
                            origin: bounds.origin,
                            size: size(px(cur_w), px(cur_h)),
                        };
                        let _ = window.paint_image(draw_bounds, Corners::default(), render_img.clone(), 0, false);
                    },
                )
                .w(px(cur_w))
                .h(px(cur_h))
                .opacity(opacity),
            )
            // Floating control badge (zoom percentage and close button) on hover
            .when(show_controls, |el| {
                el.child(
                    div()
                        .absolute()
                        .top(px(8.0))
                        .right(px(8.0))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2p5()
                        .py(px(4.0))
                        .rounded_md()
                        .bg(rgba(0x121216EE))
                        .border_1()
                        .border_color(rgba(0x8174FF66))
                        .shadow_lg()
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0xEDEDED))
                                .child(format!("{}%", zoom_pct)),
                        )
                        .child(
                            div()
                                .id("pin-btn-close")
                                .size(px(18.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_sm()
                                .bg(rgba(0xFFFFFF15))
                                .hover(|s| s.bg(rgba(0xFF453A88)))
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, cx.listener(|_, _, window, cx| {
                                    cx.stop_propagation();
                                    window.remove_window();
                                }))
                                .on_click(cx.listener(|_, _, window, cx| {
                                    cx.stop_propagation();
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
            })
    }
}

pub fn open_pinned_screenshot(cx: &mut gpui::App, image: RgbaImage, initial_origin: Option<Point<Pixels>>) {
    let img_w = image.width() as f32;
    let img_h = image.height() as f32;

    let origin = initial_origin.unwrap_or(point(px(120.0), px(120.0)));
    let window_options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin,
            size: size(px(img_w), px(img_h)),
        })),
        titlebar: None,
        focus: true,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: true,
        is_resizable: true,
        window_background: WindowBackgroundAppearance::Opaque,
        ..Default::default()
    };

    let _ = cx.open_window(window_options, |window, cx| {
        #[cfg(windows)]
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            use windows::Win32::Foundation::HWND;
            use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWINDOWATTRIBUTE};
            use windows::Win32::UI::WindowsAndMessaging::{
                BringWindowToTop, GetWindowLongW, SetForegroundWindow, SetWindowLongW,
                SetWindowPos, GWL_EXSTYLE, GWL_STYLE, HWND_TOPMOST, SWP_FRAMECHANGED,
                SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, WS_EX_TOPMOST, WS_POPUP,
            };

            if let Ok(handle) = window.window_handle() {
                if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
                    let hwnd = HWND(win32_handle.hwnd.get() as *mut _);
                    unsafe {
                        let style = GetWindowLongW(hwnd, GWL_STYLE);
                        let _ = SetWindowLongW(
                            hwnd,
                            GWL_STYLE,
                            (style | (WS_POPUP.0 as i32)) & !(0x00CF0000 | 0x00040000),
                        );
                        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
                        let _ = SetWindowLongW(
                            hwnd,
                            GWL_EXSTYLE,
                            ex_style | (WS_EX_TOPMOST.0 as i32),
                        );

                        // Disable Windows 11 rounded corners and borders on the pinned window
                        let no_round: u32 = 1; // DWMWCP_DONOTROUND
                        let _ = DwmSetWindowAttribute(
                            hwnd,
                            DWMWINDOWATTRIBUTE(33), // DWMWA_WINDOW_CORNER_PREFERENCE
                            &no_round as *const _ as *const _,
                            std::mem::size_of::<u32>() as u32,
                        );
                        let no_border: u32 = 0xFFFFFFFE; // DWMWA_COLOR_NONE
                        let _ = DwmSetWindowAttribute(
                            hwnd,
                            DWMWINDOWATTRIBUTE(34), // DWMWA_BORDER_COLOR
                            &no_border as *const _ as *const _,
                            std::mem::size_of::<u32>() as u32,
                        );

                        let _ = SetWindowPos(
                            hwnd,
                            Some(HWND_TOPMOST),
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED | SWP_SHOWWINDOW,
                        );

                        let _ = BringWindowToTop(hwnd);
                        let _ = SetForegroundWindow(hwnd);
                    }
                }
            }
        }

        let view = cx.new(|cx| PinnedScreenshotView::new(image, window, cx));
        window.focus(&view.read(cx).focus_handle);
        view
    });
}
