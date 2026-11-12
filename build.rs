fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/icon.ico");
        println!("cargo:rerun-if-changed=assets/pixora.rc");
        println!("cargo:rerun-if-changed=Cargo.toml");

        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.17.1".to_string());
        let version_parts: Vec<u32> = version
            .split('.')
            .map(|s| s.parse::<u32>().unwrap_or(0))
            .collect();
        let major = version_parts.first().copied().unwrap_or(0);
        let minor = version_parts.get(1).copied().unwrap_or(0);
        let patch = version_parts.get(2).copied().unwrap_or(0);

        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let assets_dir = std::path::Path::new(&manifest_dir).join("assets");
        let res_path = assets_dir.join("pixora.res");
        let rc_path = assets_dir.join("pixora.rc");

        let rc_content = format!(
r#"1 ICON "icon.ico"

1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "041904b0"
        BEGIN
            VALUE "CompanyName", "Onzi"
            VALUE "FileDescription", "Pixora Screen Capture"
            VALUE "FileVersion", "{major}.{minor}.{patch}.0"
            VALUE "InternalName", "Pixora.exe"
            VALUE "LegalCopyright", "Copyright (c) 2026 Onzi"
            VALUE "OriginalFilename", "Pixora.exe"
            VALUE "ProductName", "Pixora"
            VALUE "ProductVersion", "{version}"
        END
        BLOCK "040904b0"
        BEGIN
            VALUE "CompanyName", "Onzi"
            VALUE "FileDescription", "Pixora Screen Capture"
            VALUE "FileVersion", "{major}.{minor}.{patch}.0"
            VALUE "InternalName", "Pixora.exe"
            VALUE "LegalCopyright", "Copyright (c) 2026 Onzi"
            VALUE "OriginalFilename", "Pixora.exe"
            VALUE "ProductName", "Pixora"
            VALUE "ProductVersion", "{version}"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0419, 1200, 0x0409, 1200
    END
END
"#
        );
        let _ = std::fs::write(&rc_path, rc_content);

        let rc_compilers = [
            r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\rc.exe",
            r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\rc.exe",
            r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\rc.exe",
            "rc.exe",
        ];

        for compiler in &rc_compilers {
            if std::path::Path::new(compiler).exists() || *compiler == "rc.exe" {
                if let Ok(st) = std::process::Command::new(compiler)
                    .arg(format!("/fo{}", res_path.display()))
                    .arg(&rc_path)
                    .status()
                {
                    if st.success() {
                        break;
                    }
                }
            }
        }

        if res_path.exists() {
            println!("cargo:rustc-link-arg-bins={}", res_path.display());
        }
    }
}
