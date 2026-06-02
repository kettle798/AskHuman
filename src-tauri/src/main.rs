// HumanInLoop / AskHuman —— Rust + Tauri 跨平台实现入口。
//
// 注意：本程序既是 CLI（向 stdout 输出结果）又会按需弹出 GUI 窗口。
// 因此不设置 `windows_subsystem = "windows"`，以保证 Windows 上也能向终端写 stdout。
// （代价是 GUI 模式在 Windows 上可能伴随控制台窗口，后续单独处理。）

mod app;
mod cli;

fn main() {
    cli::dispatch();
}
