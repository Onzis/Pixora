fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/icon.ico");
        println!("cargo:rerun-if-changed=assets/pixora.rc");
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let res_path = std::path::Path::new(&manifest_dir).join("assets").join("pixora.res");
        if res_path.exists() {
            println!("cargo:rustc-link-arg-bins={}", res_path.display());
        }
    }
}
