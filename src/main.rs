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

    #[cfg(target_os = "linux")]
    {
        let args: Vec<String> = std::env::args().collect();
        for arg in args.iter().skip(1) {
            match arg.as_str() {
                "--capture" | "-c" | "--region" => {
                    if pixora::shortcuts::hotkeys::try_send_linux_ipc_command("region") {
                        std::process::exit(0);
                    }
                }
                "--fullscreen" | "-f" => {
                    if pixora::shortcuts::hotkeys::try_send_linux_ipc_command("fullscreen") {
                        std::process::exit(0);
                    }
                }
                "--window" | "-w" => {
                    if pixora::shortcuts::hotkeys::try_send_linux_ipc_command("window") {
                        std::process::exit(0);
                    }
                }
                _ => {}
            }
        }
    }

    let app = PixoraApp::new();

    Application::new()
        .with_assets(PixoraAssets)
        .run(move |cx: &mut App| {
            app.start(cx);
        });
}

