use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static CUSTOM_LOG_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
static LOGGING_INITIALIZED: AtomicBool = AtomicBool::new(false);

const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024; // 10 MB

pub fn set_custom_log_dir(dir: Option<PathBuf>) {
    if let Ok(mut guard) = CUSTOM_LOG_DIR.lock() {
        *guard = dir;
    }
}

pub fn get_log_dir() -> PathBuf {
    if let Ok(guard) = CUSTOM_LOG_DIR.lock() {
        if let Some(ref dir) = *guard {
            return dir.clone();
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(exe_dir) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        {
            let adj_log = exe_dir.join("log");
            if adj_log.exists()
                && std::fs::metadata(&adj_log)
                    .map(|m| !m.permissions().readonly())
                    .unwrap_or(false)
            {
                return adj_log;
            }
        }
        return dirs::state_dir()
            .or_else(dirs::data_local_dir)
            .or_else(dirs::config_dir)
            .map(|p| p.join("Pixora").join("log"))
            .unwrap_or_else(|| PathBuf::from("log"));
    }

    #[cfg(not(target_os = "linux"))]
    {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        exe_dir.join("log")
    }
}

pub fn get_log_file_path() -> PathBuf {
    get_log_dir().join("pixora.log")
}

pub fn write_to_log(content: &str) {
    let log_dir = get_log_dir();
    let log_file = get_log_file_path();

    let _ = std::fs::create_dir_all(&log_dir);

    // Rotate if file exceeds 10 MB
    if let Ok(meta) = std::fs::metadata(&log_file) {
        if meta.len() > MAX_LOG_SIZE {
            let backup = log_dir.join("pixora.log.1");
            let _ = std::fs::remove_file(&backup);
            let _ = std::fs::rename(&log_file, &backup);
        }
    }

    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
    {
        let _ = f.write_all(content.as_bytes());
        let _ = f.flush();
    }
}

pub fn log_error(module: &str, msg: impl std::fmt::Display) {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("[{}] [ERROR] [{}] {}\n", timestamp, module, msg);
    write_to_log(&line);
}

pub fn log_warn(module: &str, msg: impl std::fmt::Display) {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("[{}] [WARN] [{}] {}\n", timestamp, module, msg);
    write_to_log(&line);
}

pub fn log_info(module: &str, msg: impl std::fmt::Display) {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("[{}] [INFO] [{}] {}\n", timestamp, module, msg);
    write_to_log(&line);
}

pub fn log_panic(info: &std::panic::PanicHookInfo) {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let thread = std::thread::current();
    let thread_name = thread.name().unwrap_or("unnamed");

    let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        "Unknown panic payload".to_string()
    };

    let location = if let Some(loc) = info.location() {
        format!("{}:{}:{}", loc.file(), loc.line(), loc.column())
    } else {
        "unknown location".to_string()
    };

    let backtrace = std::backtrace::Backtrace::force_capture();

    let report = format!(
        "\n==================== PIXORA CRASH REPORT ====================\n\
         Timestamp: {}\n\
         Thread:    {}\n\
         Location:  {}\n\
         Message:   {}\n\
         Backtrace:\n{}\
         =============================================================\n\n",
        timestamp, thread_name, location, payload, backtrace
    );

    write_to_log(&report);
}

pub fn init_logging() {
    if LOGGING_INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
    }

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log_panic(info);
        default_hook(info);
    }));
}
