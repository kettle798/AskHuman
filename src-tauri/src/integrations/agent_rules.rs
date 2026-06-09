//! Agent 全局提示词（Rules）安装/卸载/状态：Cursor / Claude Code / Codex。
//!
//! 三者共用同一份提示词正文（`prompts::cli_reference()`），但落点不同：
//! - Cursor：本应用**独占文件** `~/.cursor/rules/askhuman.mdc`（frontmatter + 头标记 + 正文）。
//! - Claude Code：**共享文件** `~/.claude/CLAUDE.md` 内的托管区块。
//! - Codex：**共享文件** `~/.codex/AGENTS.md` 内的托管区块。
//!
//! 写共享文件时只在自有 `begin/end` 区块内增删，绝不动用户其它内容；写独占文件时整文件由本
//! 应用拥有，卸载仅在文件含头标记时删除。区块增删为纯函数，便于单测（幂等、保留他人内容）。

use crate::paths;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// 共享文件托管区块起始标记。
pub const BLOCK_BEGIN: &str = "<!-- AskHuman:begin DO NOT EDIT (managed by AskHuman) -->";
/// 共享文件托管区块结束标记。
pub const BLOCK_END: &str = "<!-- AskHuman:end -->";
/// 独占文件头标记（用于识别本应用拥有的文件，防止误删用户同名文件）。
pub const MANAGED_FILE_MARK: &str = "<!-- AskHuman:managed-file DO NOT EDIT (managed by AskHuman) -->";

/// 目标 Agent。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AgentTarget {
    Cursor,
    ClaudeCode,
    Codex,
}

impl AgentTarget {
    /// 由前端传入的字符串解析（未知值返回 None）。
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "cursor" => Some(AgentTarget::Cursor),
            "claude" => Some(AgentTarget::ClaudeCode),
            "codex" => Some(AgentTarget::Codex),
            _ => None,
        }
    }

    /// 目标规则文件路径。
    fn file(self) -> PathBuf {
        match self {
            AgentTarget::Cursor => paths::cursor_rule_file(),
            AgentTarget::ClaudeCode => paths::claude_md(),
            AgentTarget::Codex => paths::codex_agents_md(),
        }
    }

    /// 是否为「独占文件」模式（Cursor 为整文件拥有；其余为共享文件托管区块）。
    fn is_owned_file(self) -> bool {
        matches!(self, AgentTarget::Cursor)
    }
}

// MARK: - 纯函数（可测试）

/// 共享文件是否已含本应用托管区块。
pub fn has_block(text: &str) -> bool {
    text.contains(BLOCK_BEGIN)
}

/// 在共享文件文本中插入/更新托管区块：已存在→替换其内部；不存在→追加到末尾（前置空行）。
/// 绝不改动区块以外的内容。
pub fn upsert_block(text: &str, body: &str) -> String {
    let block = format!("{BLOCK_BEGIN}\n{body}\n{BLOCK_END}");
    if let Some((start, end)) = block_span(text) {
        let mut out = String::with_capacity(text.len() + block.len());
        out.push_str(&text[..start]);
        out.push_str(&block);
        out.push_str(&text[end..]);
        return out;
    }
    let base = text.trim_end();
    if base.is_empty() {
        format!("{block}\n")
    } else {
        format!("{base}\n\n{block}\n")
    }
}

/// 从共享文件文本中删除托管区块（含两行标记），并清理多余空行。不存在则原样返回。
pub fn remove_block(text: &str) -> String {
    if let Some((start, end)) = block_span(text) {
        let mut out = String::with_capacity(text.len());
        out.push_str(&text[..start]);
        out.push_str(&text[end..]);
        return tidy(&out);
    }
    text.to_string()
}

/// 定位托管区块的字节区间 `[start, end)`（含 begin/end 两行标记本身）。
fn block_span(text: &str) -> Option<(usize, usize)> {
    let start = text.find(BLOCK_BEGIN)?;
    let end_marker = text[start..].find(BLOCK_END)? + start;
    Some((start, end_marker + BLOCK_END.len()))
}

/// 折叠连续空行（最多保留一行空行）、去除尾部空白，非空时保留单个结尾换行。
fn tidy(s: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut prev_empty = false;
    for line in s.split('\n') {
        let is_empty = line.trim().is_empty();
        if is_empty && prev_empty {
            continue;
        }
        out.push(line);
        prev_empty = is_empty;
    }
    let trimmed = out.join("\n").trim_end().to_string();
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}\n")
    }
}

/// 组装 Cursor 独占规则文件内容：`alwaysApply:true` frontmatter + 头标记 + 正文。
pub fn build_cursor_rule(body: &str) -> String {
    format!("---\nalwaysApply: true\n---\n{MANAGED_FILE_MARK}\n\n{body}\n")
}

/// 独占文件是否由本应用拥有（含头标记）。
pub fn is_managed_cursor_file(text: &str) -> bool {
    text.contains(MANAGED_FILE_MARK)
}

// MARK: - 状态 / 路径

/// 该 Agent 的规则是否已安装。
pub fn is_installed(agent: AgentTarget) -> bool {
    let path = agent.file();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return false;
    };
    if agent.is_owned_file() {
        is_managed_cursor_file(&text)
    } else {
        has_block(&text)
    }
}

/// 当前平台是否支持（三种规则文件读写均跨平台）。
pub fn supported(_agent: AgentTarget) -> bool {
    true
}

/// 目标文件的展示路径（把 home 前缀折叠为 `~`）。
pub fn display_path(agent: AgentTarget) -> String {
    collapse_home(&agent.file())
}

fn collapse_home(p: &Path) -> String {
    let home = paths::home();
    if let Ok(rest) = p.strip_prefix(&home) {
        format!("~/{}", rest.display())
    } else {
        p.display().to_string()
    }
}

// MARK: - 安装 / 卸载

/// 安装：写入推荐提示词（独占文件整写 / 共享文件区块 upsert）。
pub fn install(agent: AgentTarget) -> Result<String> {
    let path = agent.file();
    let body = crate::prompts::cli_reference();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("failed to create directory: {}", dir.display()))?;
    }
    let new_text = if agent.is_owned_file() {
        build_cursor_rule(&body)
    } else {
        let old = std::fs::read_to_string(&path).unwrap_or_default();
        upsert_block(&old, &body)
    };
    atomic_write(&path, new_text.as_bytes())
        .with_context(|| format!("failed to write rule file: {}", path.display()))?;
    Ok(crate::i18n::tr(crate::i18n::Lang::current(), "cmd.ruleInstalled").to_string())
}

/// 卸载：独占文件仅在含头标记时删除；共享文件移除托管区块、保留其它内容。
pub fn uninstall(agent: AgentTarget) -> Result<String> {
    let path = agent.file();
    let lang = crate::i18n::Lang::current();
    if agent.is_owned_file() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if is_managed_cursor_file(&text) {
                std::fs::remove_file(&path)
                    .with_context(|| format!("failed to remove rule file: {}", path.display()))?;
            }
        }
    } else if let Ok(old) = std::fs::read_to_string(&path) {
        if has_block(&old) {
            let new_text = remove_block(&old);
            atomic_write(&path, new_text.as_bytes())
                .with_context(|| format!("failed to write rule file: {}", path.display()))?;
        }
    }
    Ok(crate::i18n::tr(lang, "cmd.ruleRemoved").to_string())
}

/// 在文件管理器中定位规则文件。
pub fn reveal(agent: AgentTarget) {
    let path = agent.file();
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .args(["-R", &path.to_string_lossy()])
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| path.clone());
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.to_string_lossy()))
            .spawn();
    }
}

/// 用系统默认程序打开规则文件。
pub fn open(agent: AgentTarget) {
    let path = agent.file();
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(&path).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(&path)
            .spawn();
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = "RULE BODY LINE 1\nRULE BODY LINE 2";

    #[test]
    fn parse_known_and_unknown() {
        assert_eq!(AgentTarget::parse("cursor"), Some(AgentTarget::Cursor));
        assert_eq!(AgentTarget::parse("claude"), Some(AgentTarget::ClaudeCode));
        assert_eq!(AgentTarget::parse("codex"), Some(AgentTarget::Codex));
        assert_eq!(AgentTarget::parse("other"), None);
    }

    #[test]
    fn upsert_into_empty() {
        let out = upsert_block("", BODY);
        assert!(out.starts_with(BLOCK_BEGIN));
        assert!(out.contains(BODY));
        assert!(out.trim_end().ends_with(BLOCK_END));
        assert!(has_block(&out));
    }

    #[test]
    fn upsert_appends_and_preserves_user_content() {
        let user = "# My CLAUDE.md\n\nsome personal rules\n";
        let out = upsert_block(user, BODY);
        assert!(out.contains("some personal rules"));
        assert!(out.contains(BLOCK_BEGIN));
        // user content stays before the block
        assert!(out.find("some personal rules").unwrap() < out.find(BLOCK_BEGIN).unwrap());
    }

    #[test]
    fn upsert_is_idempotent_and_replaces_inner() {
        let once = upsert_block("keep me\n", BODY);
        let twice = upsert_block(&once, "NEW BODY");
        assert_eq!(twice.matches(BLOCK_BEGIN).count(), 1, "no duplicate block");
        assert_eq!(twice.matches(BLOCK_END).count(), 1);
        assert!(twice.contains("NEW BODY"));
        assert!(!twice.contains("RULE BODY LINE 1"));
        assert!(twice.contains("keep me"));
    }

    #[test]
    fn remove_only_block_keeps_rest() {
        let user = "# Title\n\nkeep this\n";
        let with = upsert_block(user, BODY);
        let without = remove_block(&with);
        assert!(!has_block(&without));
        assert!(without.contains("keep this"));
        assert!(without.contains("# Title"));
    }

    #[test]
    fn remove_from_empty_block_yields_empty() {
        let only = upsert_block("", BODY);
        let out = remove_block(&only);
        assert!(out.is_empty(), "removing the sole block clears the file: {out:?}");
    }

    #[test]
    fn remove_noop_when_absent() {
        let user = "no block here\n";
        assert_eq!(remove_block(user), user);
    }

    #[test]
    fn cursor_file_build_and_recognize() {
        let f = build_cursor_rule(BODY);
        assert!(f.starts_with("---\nalwaysApply: true\n---\n"));
        assert!(is_managed_cursor_file(&f));
        assert!(f.contains(BODY));
        // a user file without the marker must not be recognized as ours
        assert!(!is_managed_cursor_file("---\nalwaysApply: true\n---\nuser rule\n"));
    }
}
