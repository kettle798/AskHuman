'use strict';

// Cursor Agent (cursor-agent CLI) profile.
//
// 来源：本机安装包静态核对 ~/.local/share/cursor-agent/versions/<ver>/（hooks 模块在 2097.index.js，
// 事件枚举/注入逻辑在 index.js）。见 FINDINGS §7.2/§7.3。
//
// 会话 ID 的两条路径（与 Claude/Codex 一致的「子进程类型不同 env 不同」）：
//   - shell 工具子进程（envprobe 走这条）：注入 CURSOR_AGENT=1 + CURSOR_CONVERSATION_ID=<会话 ID>
//     + AGENT_TRANSCRIPTS（若有 projectDir）；故「不用 Hook 拿会话 ID」成立。
//   - hook 子进程（hooklog 走这条）：env 是 CURSOR_PROJECT_DIR / CURSOR_VERSION / CURSOR_USER_EMAIL
//     / CURSOR_TRANSCRIPT_PATH / CLAUDE_PROJECT_DIR；会话 ID 不在 env，靠 stdin JSON 的 `session_id`。
// 进程识别坑：cursor-agent 的可执行名是 `agent`（~/.local/bin/agent → bundle 的 node+index.js），
//   不含 "cursor-agent" 字样——故 processTokens 用 argv0 basename "agent"，再用特异的
//   commandTokens "cursor-agent" 对完整命令行兜底（既不漏识别，又不至于把任意 *agent 进程误判）。
module.exports = {
  name: 'cursor',
  // 「无 Hook 路径」(shell 工具子进程) 用来认会话 ID 的 env 名。
  sessionIdEnvVar: 'CURSOR_CONVERSATION_ID',
  envKeys: [
    // shell 工具子进程注入
    'CURSOR_AGENT',
    'CURSOR_CONVERSATION_ID',
    'AGENT_TRANSCRIPTS',
    'CURSOR_INVOKED_AS', // 由外层 wrapper 设置后被继承
    // hook 子进程注入（buildHookEnvironment）
    'CURSOR_PROJECT_DIR',
    'CURSOR_VERSION',
    'CURSOR_USER_EMAIL',
    'CURSOR_TRANSCRIPT_PATH',
    'CLAUDE_PROJECT_DIR',
  ],
  processTokens: ['cursor-agent', 'agent'],
  commandTokens: ['cursor-agent'],
  // hook stdin JSON 的会话字段名（静态核对：base payload 用 `session_id`，非 conversation_id）。
  sessionIdJsonFields: ['session_id'],
};
