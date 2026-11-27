use std::sync::Mutex;

pub const GITHUB_REPO_URL: &str = "https://github.com/Onzis/Pixora";
pub const GITHUB_RELEASES_URL: &str = "https://github.com/Onzis/Pixora/releases";
pub const GITHUB_LATEST_RELEASE_URL: &str = "https://github.com/Onzis/Pixora/releases/latest";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate { version: String },
    Available { current: String, latest: String, release_url: String },
    Error(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub is_update_available: bool,
    pub release_url: String,
}

static GLOBAL_UPDATE_STATE: Mutex<UpdateStatus> = Mutex::new(UpdateStatus::Idle);

pub fn get_global_update_status() -> UpdateStatus {
    GLOBAL_UPDATE_STATE.lock().map(|g| g.clone()).unwrap_or(UpdateStatus::Idle)
}

pub fn set_global_update_status(status: UpdateStatus) {
    if let Ok(mut g) = GLOBAL_UPDATE_STATE.lock() {
        *g = status;
    }
}

pub fn parse_version_tuple(v: &str) -> Vec<u64> {
    let clean = v.trim().trim_start_matches(|c| c == 'v' || c == 'V');
    clean
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<u64>().unwrap_or(0))
        .collect()
}

pub fn is_newer_version(current: &str, candidate: &str) -> bool {
    let cur = parse_version_tuple(current);
    let cand = parse_version_tuple(candidate);
    cand > cur
}

pub fn check_latest_release() -> Result<UpdateInfo, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();

    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    #[cfg(windows)]
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let mut cmd = std::process::Command::new("curl.exe");
    cmd.args(["-s", "-I", GITHUB_LATEST_RELEASE_URL]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().map_err(|e| format!("Failed to execute curl: {}", e))?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut location = None;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.to_lowercase().starts_with("location:") {
            let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
            if parts.len() == 2 {
                location = Some(parts[1].trim().to_string());
                break;
            }
        }
    }

    let release_url = match location {
        Some(loc) => loc,
        None => {
            return Err("No redirection Location header found in GitHub response".to_string());
        }
    };

    let latest_version = if let Some(tag_idx) = release_url.rfind("/tag/") {
        let tag = &release_url[tag_idx + 5..];
        tag.trim().trim_start_matches(|c| c == 'v' || c == 'V').to_string()
    } else {
        return Err(format!("Could not parse release tag from URL: {}", release_url));
    };

    let is_update_available = is_newer_version(&current_version, &latest_version);

    Ok(UpdateInfo {
        current_version,
        latest_version,
        is_update_available,
        release_url,
    })
}

pub fn check_latest_release_status() -> UpdateStatus {
    match check_latest_release() {
        Ok(info) => {
            if info.is_update_available {
                UpdateStatus::Available {
                    current: info.current_version,
                    latest: info.latest_version,
                    release_url: info.release_url,
                }
            } else {
                UpdateStatus::UpToDate {
                    version: info.latest_version,
                }
            }
        }
        Err(e) => UpdateStatus::Error(e),
    }
}

pub fn open_url(url: &str) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer.exe")
            .arg(url)
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(url)
            .spawn();
    }
}
