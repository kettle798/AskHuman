# 实现计划：Watch 仅限工作中 Agent

## 背景

当前 `/watch` 相关交互会列出空闲 agent，但关注一个空闲 agent 没有实际意义。本次改动让 `/watch` 聚焦于工作中的 agent。

## 三项变更

### 1. Watch 单选卡只列工作中 Agent

**触点**：`select.rs` + `daemon/mod.rs` 调用处。

- 在 `select.rs` 新增 `watch_options()` 函数：仅返回 `state == "working"` 的 agent（**含 grok**，区别于 `msg_options` 排除 grok）。其余逻辑与 `agent_options()` 一致（圆点、编号、运行时长、关注徽标）。
- `daemon/mod.rs` 中 `/watch` 无参发单选卡处（约 line 5282），将 `agent_options()` 替换为 `watch_options()`。
- `/status` 单选卡不变，仍用 `agent_options()`（含 working + idle）。
- 新增单测验证 `watch_options()` 只返回 working 记录。

### 2. `/watch <n>` 对空闲 Agent 发一次性回顾卡

**触点**：`daemon/mod.rs` 的 `handle_watch_cmd`。

- 在找到 agent 记录后，除了已有的 `ended` 判定，新增 `idle` 判定（`state == "idle"` 且 `!waiting`——有在途 AskHuman 提问时 phase 为 Waiting 不算 idle）。
- idle 的处理与 ended 一致：构建帧，以 `CardMode::Final(FinalKind::Idle)` 发送一张一次性卡片（回顾当前状态），**不**创建订阅。
- 已结束（ended）保持现有行为不变。

### 3. 正在关注的 Agent 变为空闲 → 自动结束关注

**触点**：`watch.rs` + `daemon/mod.rs` 的 `watch_tick`。

- `watch.rs` 新增 `FinalKind::Idle`（`Clone`，无关联数据）。
- 新增 i18n 键 `watch.btnIdle`：`"Idle · auto-unwatched"` / `"已空闲 · 已自动取消关注"`。
- `watch.rs` 的 `final_label_text` 增加 `FinalKind::Idle` 分支。
- `daemon/mod.rs` 的 `watch_tick` 帧循环中，现有 `let ended = ...` 之后增加：
  ```
  let idle = frame.phase == WatchPhase::Idle;
  ```
  将 `ended` 出现的所有终态逻辑（定格卡片 + 退订）扩展为 `ended || idle`，其中：
  - ended 使用 `FinalKind::Ended`
  - idle 使用 `FinalKind::Idle`
- `WatchPhase::Waiting`（有在途 AskHuman 提问时覆盖 idle/working）**不**触发自动结束。

### 4. `/watch` 无参文本回退

当 `send_agent_picker` 返回 false（无可选项或边缘场景）时，现有代码回退到 `handle_watch_cmd(state, channel_id, None, ...)` 显示文本列表。

- `handle_watch_cmd` None 分支中的 `has_agents` 判定改为仅检查 `working`（不含 idle）：若当前无工作中 agent，提示文案引导等 agent 开始工作后再关注。
- 列表仍复用 `status_text()` 展示全部 working + idle（便于用户了解全貌），但提示词改为「关注一个工作中的 Agent」。
- 新增 i18n 键 `watch.pickHintWorkingOnly`：`"Send {p}watch <n> to follow a working agent with a live status card."` / `"发送 {p}watch <编号> 可关注一个工作中的 Agent，获取实时状态卡片。"`。原 `watch.pickHint` 保留（其余使用处不受影响则直接替换）。

## 不影响的模块

- `/status`（单选卡 + 文本 + 详情）：不变。
- `/unwatch`：不变（已关注列表按订阅而非注册表状态列举）。
- `/msg`：不变。
- 弹窗 / CLI / 设置页 / 历史：不变。
