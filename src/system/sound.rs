use std::path::PathBuf;

pub const EMBEDDED_SCREENFX_MP3: &[u8] = include_bytes!("../../assets/sounds/ScreenFX.mp3");

pub fn get_or_extract_sound_path() -> Option<PathBuf> {
    // 1. Check custom path from user / config
    let user_path = PathBuf::from(r"C:\Users\Admin\Music\ScreenFX.mp3");
    if user_path.exists() {
        return Some(user_path);
    }

    // 2. Check next to executable or in assets/sounds
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let next_to_exe = parent.join("ScreenFX.mp3");
            if next_to_exe.exists() {
                return Some(next_to_exe);
            }
            let assets_sound = parent.join("assets").join("sounds").join("ScreenFX.mp3");
            if assets_sound.exists() {
                return Some(assets_sound);
            }
        }
    }

    // 3. Extract embedded sound to Pixora cache / temp dir
    let cache_dir = dirs::config_dir()
        .map(|p| p.join("Pixora"))
        .unwrap_or_else(std::env::temp_dir);
    let _ = std::fs::create_dir_all(&cache_dir);
    let cache_file = cache_dir.join("ScreenFX.mp3");

    if !cache_file.exists() || std::fs::metadata(&cache_file).map(|m| m.len()).unwrap_or(0) == 0 {
        let _ = std::fs::write(&cache_file, EMBEDDED_SCREENFX_MP3);
    }

    if cache_file.exists() {
        Some(cache_file)
    } else {
        None
    }
}

#[cfg(windows)]
mod win_audio {
    use std::path::Path;
    use std::sync::Mutex;

    #[link(name = "winmm")]
    unsafe extern "system" {
        fn mciSendStringW(
            lpszCommand: *const u16,
            lpszReturnString: *mut u16,
            cchReturn: u32,
            hwndCallback: *mut std::ffi::c_void,
        ) -> u32;
    }

    static MCI_MUTEX: Mutex<()> = Mutex::new(());

    fn send_mci(cmd: &str) -> u32 {
        let mut wide: Vec<u16> = cmd.encode_utf16().collect();
        wide.push(0);
        unsafe { mciSendStringW(wide.as_ptr(), std::ptr::null_mut(), 0, std::ptr::null_mut()) }
    }

    pub fn play_file(path: &Path) {
        let _lock = MCI_MUTEX.lock();
        let path_str = path.to_string_lossy();

        // 1. Try to replay alias directly from start
        let res = send_mci("play pixora_sfx from 0");
        if res != 0 {
            // 2. If not open yet or errored, close previous and open file
            send_mci("close pixora_sfx");
            let open_cmd = format!("open \"{}\" type mpegvideo alias pixora_sfx", path_str);
            let open_res = send_mci(&open_cmd);
            if open_res == 0 {
                send_mci("play pixora_sfx from 0");
            }
        }
    }
}

pub fn play_capture_sound() {
    std::thread::spawn(|| {
        if let Some(sound_path) = get_or_extract_sound_path() {
            #[cfg(windows)]
            win_audio::play_file(&sound_path);
        }
    });
}
