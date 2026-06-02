//! Tauri 运行时：创建窗口并运行事件循环。
//! Step 1 仅打通空窗口；提问/设置的完整流程在后续步骤实现。

use tauri::{WebviewUrl, WebviewWindowBuilder};

/// 启动 Tauri 并按视图模式（"popup" / "settings"）创建一个窗口。
/// 阻塞直到窗口关闭。
pub fn run_window(view: &str) {
    let view = view.to_string();

    tauri::Builder::default()
        .setup(move |app| {
            let (width, height, title) = match view.as_str() {
                "settings" => (560.0_f64, 640.0_f64, "HumanInLoop 设置"),
                _ => (560.0_f64, 620.0_f64, "HumanInLoop"),
            };

            let url = WebviewUrl::App(format!("index.html?view={view}").into());
            WebviewWindowBuilder::new(app, "main", url)
                .title(title)
                .inner_size(width, height)
                .center()
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("启动 Tauri 失败");
}
