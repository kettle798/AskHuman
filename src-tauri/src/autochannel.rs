//! 「IM 会话期自动激活」的与传输无关的小逻辑：活跃槽持久化、入站 slash 命令解析、
//! `/status` 文本组装、激活回执文案。
//!
//! 设计见 `docs/plans/im-channel-activation.md`。活跃槽（当前用哪个 IM 接收提问）持久化到
//! `~/.askhuman/state/auto-channel.json`，跨 daemon 重启保留，仅由「用户在某渠道的入站消息」更新。

use crate::i18n::{self, Lang};
use crate::paths;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 持久化的活跃槽。
#[derive(Default, Serialize, Deserialize)]
struct Persisted {
    /// 当前活跃渠道 id（"feishu" / "dingding" / "telegram" / "slack" / "popup"）；
    /// None / "popup" = 不向任何 IM 发卡片（只弹窗）。在哪个渠道作答 / 说话就更新为哪个。
    #[serde(default)]
    channel: Option<String>,
    /// 最近一次更新时间（unix 秒，仅诊断用）。
    #[serde(default)]
    updated_at: u64,
}

/// 读取持久化的活跃槽（缺失 / 解析失败 → None）。
pub fn load_active() -> Option<String> {
    let text = std::fs::read_to_string(paths::auto_channel_file()).ok()?;
    let parsed: Persisted = serde_json::from_str(&text).ok()?;
    parsed.channel.filter(|s| !s.is_empty())
}

/// 原子写入活跃槽（临时文件 + rename）。best-effort，失败静默。
pub fn save_active(channel: Option<&str>) {
    let data = Persisted {
        channel: channel.map(|s| s.to_string()),
        updated_at: now_secs(),
    };
    let Ok(json) = serde_json::to_string_pretty(&data) else {
        return;
    };
    let path = paths::auto_channel_file();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension(format!("json.tmp-{}", uuid::Uuid::new_v4()));
    if std::fs::write(&tmp, json.as_bytes()).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

/// 入站内置命令（带 `/` 前缀才算命令；可扩展）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// `/here`、`/这里`：把此渠道设为活跃槽 + 补推在途 + 必回执。
    Here,
    /// `/status`、`/状态`：返回工作中/空闲 agent 的状态文本。
    Status,
}

/// 解析入站文本：`trim` 后**以 `/` 开头**才进命令分派，取首个 token（大小写不敏感）匹配。
/// 非命令返回 None（调用方按「普通消息」处理）。
pub fn parse_command(text: &str) -> Option<Command> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') {
        return None;
    }
    let token = trimmed.split_whitespace().next().unwrap_or("");
    match token.to_ascii_lowercase().as_str() {
        "/here" | "/这里" => Some(Command::Here),
        "/status" | "/状态" => Some(Command::Status),
        _ => None,
    }
}

/// 激活回执文案：基础确认句 +（补推了 N>0 条在途时）追加补推后缀。
pub fn activated_receipt(pending: usize, lang: Lang) -> String {
    let mut s = i18n::tr(lang, "autoChannel.activated").to_string();
    if pending > 0 {
        s.push_str(&i18n::tr(lang, "autoChannel.pending").replace("{n}", &pending.to_string()));
    }
    s
}

/// 反激活提示：活跃槽切到别处时发给**旧**渠道，明确告知切到了哪个渠道（`new_id`，含 "popup"），
/// 后续提问不再走此渠道、可发 `/here` 重新激活。
pub fn deactivated_receipt(new_id: &str, lang: Lang) -> String {
    i18n::tr(lang, "autoChannel.deactivated").replace("{target}", &channel_label(new_id, lang))
}

/// 渠道 id → 展示名（复用「回复来源」文案）。未知 id 原样返回。
pub fn channel_label(id: &str, lang: Lang) -> String {
    let key = match id {
        "popup" => "channel.sourcePopup",
        "telegram" => "channel.sourceTelegram",
        "dingding" => "channel.sourceDingTalk",
        "feishu" => "channel.sourceFeishu",
        "slack" => "channel.sourceSlack",
        other => return other.to_string(),
    };
    i18n::tr(lang, key).to_string()
}

/// 由 agent 注册表快照（`AgentRegistry::snapshot()` 的 Value 数组）组装 `/status` 文本：
/// 仅列「工作中 / 空闲」（已结束不列），工作中在前；空则给「需开启生命周期追踪」提示。
pub fn status_text(snapshot: &Value, lang: Lang) -> String {
    let empty = Vec::new();
    let list = snapshot.as_array().unwrap_or(&empty);

    let mut working: Vec<String> = Vec::new();
    let mut idle: Vec<String> = Vec::new();
    for rec in list {
        let state = rec.get("state").and_then(|v| v.as_str()).unwrap_or("");
        let line = match state {
            "working" => &mut working,
            "idle" => &mut idle,
            _ => continue, // ended / 未知：不列
        };
        line.push(format_line(rec, lang));
    }

    if working.is_empty() && idle.is_empty() {
        return i18n::tr(lang, "autoChannel.statusEmpty").to_string();
    }

    let mut out = String::new();
    if !working.is_empty() {
        out.push_str(i18n::tr(lang, "autoChannel.statusWorking"));
        out.push('\n');
        out.push_str(&working.join("\n"));
    }
    if !idle.is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(i18n::tr(lang, "autoChannel.statusIdle"));
        out.push('\n');
        out.push_str(&idle.join("\n"));
    }
    out
}

/// 单行：`• 类型 — 标题（项目）`。
fn format_line(rec: &Value, lang: Lang) -> String {
    let kind = rec.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    let kind_label = crate::agents::AgentKind::parse(kind)
        .map(|k| k.label())
        .unwrap_or(kind);

    let title = rec
        .get("title")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| i18n::tr(lang, "autoChannel.noTitle").to_string());

    let project = rec
        .get("cwd")
        .and_then(|v| v.as_str())
        .and_then(project_name)
        .unwrap_or_else(|| i18n::tr(lang, "autoChannel.noProject").to_string());

    format!("• {} — {}（{}）", kind_label, title, project)
}

/// 取工作目录的末段作为项目名（空 → None）。
fn project_name(cwd: &str) -> Option<String> {
    let trimmed = cwd.trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    std::path::Path::new(trimmed)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
