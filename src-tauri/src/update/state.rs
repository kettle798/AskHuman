//! 自更新状态 `~/.askhuman/update.json`：缓存最新版本/日志/检查时间、用户忽略集合、
//! 以及「盘上已是新版、待 drain 生效」标记。读写均 best-effort（缺失/损坏回默认）。

use crate::paths;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdateState {
    /// 上次检查到的最新正式版（规范化版本号）。
    pub latest_version: String,
    /// 最新版日志摘要（markdown），可空。
    pub release_notes: String,
    /// 上次检查时间（unix 秒）。
    pub checked_at: u64,
    /// 用户已忽略、不再主动提示的版本号集合。
    pub dismissed_versions: Vec<String>,
    /// 盘上二进制已是新版、等待 daemon drain 换新生效。
    pub pending: bool,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 读取状态（缺失 / 解析失败 → 默认）。
pub fn load() -> UpdateState {
    std::fs::read(paths::update_state_file())
        .ok()
        .and_then(|d| serde_json::from_slice(&d).ok())
        .unwrap_or_default()
}

/// 原子落盘（best-effort；不存在配置目录则创建）。
pub fn save(state: &UpdateState) {
    let path = paths::update_state_file();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let Ok(data) = serde_json::to_vec_pretty(state) else {
        return;
    };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, &data).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

/// 记录一次检查结果（更新最新版本 / 日志 / 时间）。
pub fn record_check(latest_version: &str, release_notes: &str) {
    let mut s = load();
    s.latest_version = latest_version.to_string();
    s.release_notes = release_notes.to_string();
    s.checked_at = now_secs();
    save(&s);
}

/// 该版本是否已被用户忽略。
pub fn is_dismissed(version: &str) -> bool {
    load().dismissed_versions.iter().any(|v| v == version)
}

/// 忽略某版本（不再主动弹该版本提示）。
pub fn dismiss(version: &str) {
    let mut s = load();
    if !s.dismissed_versions.iter().any(|v| v == version) {
        s.dismissed_versions.push(version.to_string());
        save(&s);
    }
}

/// 清空忽略集合（用户主动「检查更新」时调用）。
pub fn clear_dismissed() {
    let mut s = load();
    if !s.dismissed_versions.is_empty() {
        s.dismissed_versions.clear();
        save(&s);
    }
}

/// 设置「待生效」标记。
pub fn set_pending(pending: bool) {
    let mut s = load();
    if s.pending != pending {
        s.pending = pending;
        save(&s);
    }
}
