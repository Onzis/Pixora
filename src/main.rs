#![windows_subsystem = "windows"]
#![allow(dead_code)]

use gpui::{App, Application};
use pixora::app::PixoraApp;
use pixora::assets::PixoraAssets;
use pixora::system::dpi::init_dpi_awareness;

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let msg = format!("PANIC: {:?}\n", info);
        let _ = std::fs::write("C:\\Pixora\\crash.log", msg);
    }));

    init_dpi_awareness();

    let app = PixoraApp::new();

    Application::new()
        .with_assets(PixoraAssets)
        .run(move |cx: &mut App| {
            app.start(cx);
        });
}

