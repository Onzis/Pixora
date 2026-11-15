use std::path::Path;

#[cfg(windows)]
pub fn set_run_at_startup(enabled: bool) -> Result<(), String> {
    use windows::core::w;
    use windows::Win32::System::Registry::{
        HKEY_CURRENT_USER, KEY_SET_VALUE, RegCloseKey, RegDeleteValueW, RegOpenKeyExW,
        RegSetValueExW, REG_SZ,
    };

    unsafe {
        let subkey = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
        let mut hkey = windows::Win32::System::Registry::HKEY::default();

        let res = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            subkey,
            Some(0),
            KEY_SET_VALUE,
            &mut hkey,
        );
        if res.is_err() {
            return Err("Failed to open startup registry key".to_string());
        }

        let app_name = w!("Pixora");
        if enabled {
            if let Ok(exe_path) = std::env::current_exe() {
                let wide_path: Vec<u16> = exe_path
                    .to_string_lossy()
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();

                let _ = RegSetValueExW(
                    hkey,
                    app_name,
                    Some(0),
                    REG_SZ,
                    Some(std::slice::from_raw_parts(
                        wide_path.as_ptr() as *const u8,
                        wide_path.len() * 2,
                    )),
                );
            }
        } else {
            let _ = RegDeleteValueW(hkey, app_name);
        }

        let _ = RegCloseKey(hkey);
        Ok(())
    }
}

#[cfg(not(windows))]
pub fn set_run_at_startup(_enabled: bool) -> Result<(), String> {
    Ok(())
}

pub fn open_folder_in_explorer(path: &Path) {
    let _ = std::process::Command::new("explorer.exe")
        .arg(path)
        .spawn();
}

pub fn choose_folder_dialog(initial_dir: &Path) -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    {
        use std::process::Command;
        let script = format!(
            "Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.FolderBrowserDialog; $f.SelectedPath = '{}'; if ($f.ShowDialog() -eq 'OK') {{ Write-Output $f.SelectedPath }}",
            initial_dir.display().to_string().replace('\'', "''")
        );
        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
            .ok()?;
        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path_str.is_empty() {
                let p = std::path::PathBuf::from(path_str);
                if p.is_dir() {
                    return Some(p);
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = initial_dir;
    }
    None
}
