fn main() {
    // macOS 原生 QuickLook 预览面板（QLPreviewPanel）位于 Quartz 框架（QuickLookUI）。
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-lib=framework=Quartz");
    }
    tauri_build::build()
}
