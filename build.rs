fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/icon.ico");
        println!("cargo:rerun-if-changed=assets/pixora.rc");
        println!("cargo:rerun-if-changed=Cargo.toml");

        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let assets_dir = std::path::Path::new(&manifest_dir).join("assets");
        let res_path = assets_dir.join("pixora.res");
        let rc_path = assets_dir.join("pixora.rc");

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
