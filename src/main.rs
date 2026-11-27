#![windows_subsystem = "windows"]
#![allow(dead_code)]

use gpui::{App, Application};
use pixora::app::PixoraApp;
use pixora::assets::PixoraAssets;
use pixora::logging::init_logging;
use pixora::system::dpi::init_dpi_awareness;

fn main() {
    init_logging();
    init_dpi_awareness();

    let app = PixoraApp::new();

    Application::new()
        .with_assets(PixoraAssets)
        .run(move |cx: &mut App| {
            app.start(cx);
        });
}

