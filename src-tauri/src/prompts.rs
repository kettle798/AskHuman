//! CLI 调用参考提示词（供设置界面展示与复制）。
//!
//! 该提示词始终为英文（面向 AI 的契约），且**不内嵌** help 文本，
//! 而是指引 AI 执行 `<prog> --agent-help` 获取实时、随界面语言本地化的用法。

/// 组装参考提示词：行为约束规则（英文固定）+ 一行执行指引。
pub fn cli_reference() -> String {
    let prog = crate::cli::help::program_name();
    format!(
        "- You MUST ask me via the `{prog}` command (through the Shell tool); never ask me directly or finish a task without asking.\n\
- Run `{prog} --agent-help` to learn its usage.\n\
- When requirements are unclear, use `{prog}` to ask for clarification and provide predefined options.\n\
- When there are multiple possible approaches, use `{prog}` to ask instead of deciding on your own.\n\
- When a plan/strategy needs to be updated, use `{prog}` to ask instead of deciding on your own.\n\
- Before you are about to complete the request, you MUST call `{prog}` to ask for feedback.\n\
- Until I have explicitly confirmed via `{prog}` that the task may be completed/ended, do NOT end the conversation/request on your own."
    )
}
