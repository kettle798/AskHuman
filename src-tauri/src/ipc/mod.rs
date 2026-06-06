//! 进程间通信（IPC）：CLI / GUI Helper ↔ 常驻 Daemon。
//!
//! 传输：NDJSON（一行一个 JSON 消息）over Unix domain socket（mac/Linux）/ Windows named pipe。
//! 本文件定义协议消息类型；编解码见 `codec`，传输（socket 路径/连接/监听）见 `transport`。
//!
//! Phase 0 仅含握手与 daemon 控制（status/stop）；任务提交（submit/show/...）在后续 Phase 引入。

pub mod codec;
pub mod transport;

pub use codec::{read_msg, write_msg};

use crate::daemon::lifecycle::Fingerprint;
use crate::models::{AskRequest, ChannelAction, MessagePrompt, Question, QuestionAnswer};
use serde::{Deserialize, Serialize};

/// IPC 协议版本：不兼容变更时 +1，握手不一致即触发换新。
pub const PROTOCOL_VERSION: u32 = 1;

/// CLI/GUI 连接时的握手信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientHello {
    pub protocol_version: u32,
    pub client_version: String,
    pub binary_path: String,
    pub fingerprint: Fingerprint,
    pub pid: u32,
}

/// 握手结果状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HelloStatus {
    /// 正常，可继续。
    Ok,
    /// Daemon 已过时（二进制指纹/协议变化），将自行退出；客户端应等其下线后用新二进制拉起。
    Restarting,
}

/// 对 `ClientHello` 的回应。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloAck {
    pub protocol_version: u32,
    pub daemon_version: String,
    pub status: HelloStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// `daemon status` 返回的运行信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusInfo {
    pub pid: u32,
    pub version: String,
    pub protocol_version: u32,
    pub uptime_secs: u64,
    pub socket: String,
    pub active_requests: usize,
}

/// CLI 提交的一次提问任务（A11：`-f` 已在 CLI 解析为绝对路径；硬性上送 source name 与解析好的 lang；
/// `request_id` 由 Daemon 分配，故此处不含 id）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRequest {
    /// 共享 Message：描述文本与展示附件（绝对路径）。
    pub message: MessagePrompt,
    /// 问题列表（CLI 已归一化，恒 ≥1）。
    pub questions: Vec<Question>,
    /// 是否按 Markdown 渲染（全局）。
    pub is_markdown: bool,
    /// 调用方来源名（来自 `ASKHUMAN_ENV_SOURCE_NAME`，CLI 读取后上送）。
    pub source: String,
    /// CLI 解析好的界面语言（"en" / "zh"），使 `auto` 跟随调用方而非 Daemon。
    pub lang: String,
}

/// Daemon → GUI Helper 的题目下发（show 是 submit 的子集 + Daemon 分配的 request_id + 上下文）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowPayload {
    pub request_id: String,
    /// 完整请求（含 Daemon 分配的 id），供弹窗渲染。
    pub request: AskRequest,
    /// 调用方来源名（弹窗标题「Question from {source}」）。
    pub source: String,
    /// 界面语言（"en" / "zh"）。
    pub lang: String,
}

/// 客户端（CLI / GUI Helper）→ Daemon 的消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClientMsg {
    /// CLI / 控制连接握手。
    Hello(ClientHello),
    /// `daemon status`。
    Status,
    /// `daemon stop`。
    Stop,
    /// CLI 提交一次提问任务（握手后发送）。
    Submit(TaskRequest),
    /// GUI Helper 握手：出示 Daemon 下发的一次性 token。
    GuiHello { token: String },
    /// GUI Helper 回传用户作答（`action` 区分发送/取消）。
    Answer {
        request_id: String,
        action: ChannelAction,
        #[serde(default)]
        answers: Vec<QuestionAnswer>,
    },
}

/// Daemon → 客户端（CLI / GUI Helper）的消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServerMsg {
    HelloAck(HelloAck),
    Status(StatusInfo),
    Stopping,
    Error { message: String },
    /// 任务已受理，回带 Daemon 分配的 request_id（D→CLI）。
    Accepted { request_id: String },
    /// 流式警告 / 诊断 → CLI 的 stderr（D→CLI）。
    Warn { text: String },
    /// 终态：渲染好的结果文本 + 退出码（D→CLI）。CLI 原样打印 stdout 后按码退出。
    Final { stdout: String, exit_code: i32 },
    /// 下发题目（D→GUI）。
    Show(ShowPayload),
    /// 被其它渠道抢答，通知 GUI 收尾关窗（D→GUI）。
    Cancel { request_id: String, winner: String },
}
