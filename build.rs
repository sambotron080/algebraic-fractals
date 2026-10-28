//! Puts the icon (assets/icon.ico, made by examples/make_icon.rs) into the Windows
//! .exe files, so Explorer and the taskbar show it. (The window's own icon is set at
//! start; macOS app bundles get theirs from build-mac.sh.)

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        if let Err(e) = res.compile() {
            // (a missing resource compiler only costs the .exe's icon)
            println!("cargo:warning=could not add the icon to the .exe: {e}");
        }
    }
}
