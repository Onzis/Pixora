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

    #[allow(unused_mut)]
    let mut initial_event: Option<pixora::app::AppEvent> = None;

    #[cfg(target_os = "linux")]
    {
        let args: Vec<String> = std::env::args().collect();
        let mut handled_flag = false;
        for arg in args.iter().skip(1) {
            match arg.as_str() {
                "--capture" | "-c" | "--region" => {
                    handled_flag = true;
                    if pixora::shortcuts::hotkeys::try_send_linux_ipc_command("region") {
                        std::process::exit(0);
                    }
                    initial_event = Some(pixora::app::AppEvent::TriggerRegionCapture);
                }
                "--fullscreen" | "-f" => {
                    handled_flag = true;
                    if pixora::shortcuts::hotkeys::try_send_linux_ipc_command("fullscreen") {
                        std::process::exit(0);
                    }
                    initial_event = Some(pixora::app::AppEvent::TriggerFullscreenCapture);
                }
                "--window" | "-w" => {
                    handled_flag = true;
                    if pixora::shortcuts::hotkeys::try_send_linux_ipc_command("window") {
                        std::process::exit(0);
                    }
                    initial_event = Some(pixora::app::AppEvent::TriggerActiveWindowCapture);
                }
                "--settings" | "-s" => {
                    handled_flag = true;
                    if pixora::shortcuts::hotkeys::try_send_linux_ipc_command("settings") {
                        std::process::exit(0);
                    }
                    initial_event = Some(pixora::app::AppEvent::OpenSettings);
                }
                _ => {}
            }
        }
        // Single-instance check when launched again from desktop menu without flags
        if !handled_flag && pixora::shortcuts::hotkeys::try_send_linux_ipc_command("settings") {
            std::process::exit(0);
        }
    }

    let app = PixoraApp::with_initial_event(initial_event);

    Application::new()
        .with_assets(PixoraAssets)
        .run(move |cx: &mut App| {
            app.start(cx);
        });
}

