//! 前端可调用的 Tauri 命令（弹窗模式）。

use crate::app::coordinator::Coordinator;
use crate::app::AppState;
use crate::config::{AppConfig, ThemeMode};
use crate::integrations::cursor_hook;
use crate::models::{AskRequest, ChannelAction, ChannelResult, ImageAttachment};
use crate::telegram::TelegramClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

/// 弹窗初始化负载：请求内容 + 主题 + 是否置顶（前端据此套用样式、初始化导航栏）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PopupInit {
    request: AskRequest,
    theme: String,
    always_on_top: bool,
}

#[tauri::command]
pub fn popup_init(state: State<AppState>) -> PopupInit {
    PopupInit {
        request: state.request.clone(),
        theme: theme_str(state.config.general.theme),
        always_on_top: state.config.general.always_on_top,
    }
}

/// 前端提交的作答内容。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PopupSubmission {
    #[serde(default)]
    selected_options: Vec<String>,
    #[serde(default)]
    user_input: String,
    #[serde(default)]
    images: Vec<ImageAttachment>,
}

#[tauri::command]
pub fn submit_popup(app: AppHandle, submission: PopupSubmission) {
    let result = ChannelResult {
        action: ChannelAction::Send,
        selected_options: submission.selected_options,
        user_input: Some(submission.user_input),
        images: submission.images,
        source_channel_id: "popup".to_string(),
    };
    if let Some(c) = app.try_state::<Arc<Coordinator>>() {
        c.submit(result);
    }
}

#[tauri::command]
pub fn cancel_popup(app: AppHandle) {
    if let Some(c) = app.try_state::<Arc<Coordinator>>() {
        c.submit(ChannelResult::cancel("popup"));
    }
}

fn theme_str(theme: ThemeMode) -> String {
    match theme {
        ThemeMode::System => "system",
        ThemeMode::Light => "light",
        ThemeMode::Dark => "dark",
    }
    .to_string()
}

// ===== 设置页命令 =====

#[tauri::command]
pub fn get_settings() -> AppConfig {
    AppConfig::load()
}

#[tauri::command]
pub fn save_settings(config: AppConfig) -> Result<(), String> {
    config.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_prompt() -> &'static str {
    crate::prompts::CLI_REFERENCE
}

/// 实时应用主题到已打开的窗口（system→跟随系统）。
#[tauri::command]
pub fn set_theme(app: AppHandle, theme: String) {
    apply_theme_to_windows(&app, &theme);
}

/// 从弹窗导航栏切换主题：写入配置并实时应用到所有窗口。
#[tauri::command]
pub fn update_theme(app: AppHandle, theme: String) -> Result<(), String> {
    let mut cfg = AppConfig::load();
    cfg.general.theme = match theme.as_str() {
        "light" => ThemeMode::Light,
        "dark" => ThemeMode::Dark,
        _ => ThemeMode::System,
    };
    cfg.save().map_err(|e| e.to_string())?;
    apply_theme_to_windows(&app, &theme);
    Ok(())
}

fn apply_theme_to_windows(app: &AppHandle, theme: &str) {
    let t = match theme {
        "light" => Some(tauri::Theme::Light),
        "dark" => Some(tauri::Theme::Dark),
        _ => None,
    };
    for label in ["settings", "popup"] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.set_theme(t);
        }
    }
}

/// 从弹窗导航栏打开设置窗口（同进程内创建，不影响弹窗等待）。
#[tauri::command]
pub fn open_settings(app: AppHandle) -> Result<(), String> {
    crate::app::create_settings_window(&app, &AppConfig::load()).map_err(|e| e.to_string())
}

// ===== Cursor Hook =====

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookStatus {
    installed: bool,
    hooks_json_exists: bool,
    supported: bool,
}

#[tauri::command]
pub fn cursor_hook_status() -> HookStatus {
    HookStatus {
        installed: cursor_hook::is_installed(),
        hooks_json_exists: cursor_hook::hooks_json_exists(),
        supported: cursor_hook::supported(),
    }
}

#[tauri::command]
pub fn cursor_hook_install() -> Result<String, String> {
    cursor_hook::install().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cursor_hook_uninstall() -> Result<String, String> {
    cursor_hook::uninstall().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cursor_hook_reveal() {
    cursor_hook::reveal();
}

// ===== Telegram 测试连接 =====

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelegramTestArgs {
    bot_token: String,
    chat_id: String,
    api_base_url: String,
}

#[tauri::command]
pub async fn telegram_test(args: TelegramTestArgs) -> Result<String, String> {
    let client = TelegramClient::new(args.bot_token, args.chat_id, args.api_base_url)
        .map_err(|e| e.to_string())?;
    client.test_connection().await.map_err(|e| e.to_string())
}
