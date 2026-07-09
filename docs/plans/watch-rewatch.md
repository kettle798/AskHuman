# Watch 卡片「重新关注」按钮 —— 开发计划

> spec: `docs/specs/watch-rewatch.md`

## 概述

仅对 `AutoStopped` 终态的 watch 卡片提供可点击按钮「已切换到 {to} · 重新关注」，点击后发新卡
（旧卡按钮变为 disabled「已重新关注」），四渠道均支持。

## 1. daemon 路由保活 — rewatchable 标记

### 问题

当前 watch 终态化后 `WatchEntry` 从 `subs` 移除 → `ensure_watch_routes` 不再为该 message_id
注册路由 → IM 平台发来的按钮回调无人接收。

### 解法

对 `AutoStopped` 终态，**不从 `subs` 移除 entry，而是标记 `rewatchable: true`**：

- 路由保活：entry 仍在 `subs` → `ensure_watch_routes` 继续注册该 message_id 的路由
- 引擎跳过：`watch_tick` 遍历 `subs` 时跳过 `rewatchable == true` 的 entry（不轮询/编辑）
- 上限不计：每渠道上限 `MAX_WATCHES` 计数时排除 `rewatchable` entry
- 空闲退出不计：闲退守卫只看非 `rewatchable` 的活跃订阅
- 持久化：`PersistedWatch` 加 `#[serde(default)] rewatchable: bool`（跨重启保留，向后兼容旧文件）
- TTL 清理：session 结束时或引擎 tick 发现对应 session 已不在注册表时，移除 rewatchable entry

### WatchEntry 变更

```rust
struct WatchEntry {
    // ... 既有字段不变 ...
    rewatchable: bool, // 新增：终态已定格但保留路由供重新关注
}
```

### finalize_and_drop_watches 变更

`final_kind.is_rewatchable()` 为 true 时：不 `retain` 移除 entry，改为遍历 `subs` 将
匹配的 entry 标记 `rewatchable = true`。持久化 + notify 不变。

## 2. watch.rs — 可重新关注的终态模型

### 2.1 FinalKind 扩展

新增 `Rewatched` 变体（点击后旧卡定格用）和判定方法：

```rust
pub enum FinalKind {
    // ... 既有变体不变 ...
    Rewatched, // 新增：用户已从该卡重新关注
}

impl FinalKind {
    pub fn is_rewatchable(&self) -> bool {
        matches!(self, FinalKind::AutoStopped(_))
    }
}
```

### 2.2 WatchButtons 扩展

新增 `Rewatch` 变体，携带按钮文案和 `session_id`（按钮回调需嵌入）：

```rust
pub enum WatchButtons {
    Active { unwatch: String, refresh: String },
    Final { label: String },
    Rewatch { label: String, session_id: String }, // 新增
}
```

### 2.3 文案与 card_view

- `card_view` 中需额外接收 `session_id` 参数（`Option<&str>`），当 `kind.is_rewatchable()`
  为 true 且 session_id 有值时产出 `WatchButtons::Rewatch`，否则回退 `WatchButtons::Final`。
- 新增 i18n key：
  - `watch.btnRewatch`：ZH `"已切换到 {to} · 重新关注"` / EN `"Switched to {to} · Re-watch"`
  - `watch.btnRewatched`：ZH `"已重新关注"` / EN `"Rewatched"`

## 3. 各渠道渲染 — Rewatch 按钮

### 3.1 飞书 (feishu/card.rs)

`watch_buttons_element` 处理 `WatchButtons::Rewatch`：渲染为**单个可点击按钮**（非 disabled），
`type: "default"`，callback value 携带 `{"watch":"rewatch","sid":"<session_id>"}`。

`WatchAction` 新增 `Rewatch(String)` 变体（`String` = session_id）。`parse_watch_action` 识别
`"rewatch"` 动作并从 value 对象的 `sid` 字段取 session_id。

### 3.2 钉钉 (dingtalk/watch.rs)

模板变量扩展：新增 `rewatchable`（boolean 字符串）和 `session_id`、`rewatch_label` 变量。
模板需新增一个 SingleButton（文案用 `rewatch_label` 变量），条件显隐：`finalized == "true" &&
rewatchable == "true"` 时显示；`finalized == "true" && rewatchable == "false"` 时显示原有禁用标签。
按钮 actionId = `"watch_rewatch"`，params 带 `sid: session_id`。

`build_watch_param_map` 追加 3 个变量。`parse_watch_action` 额外识别 `watch_rewatch` actionId
并从 params 取 `sid`。

**模板发布**：需更新 `docs/assets/dingtalk-watch-card-template.json` 并在开发者后台重新发布。

### 3.3 Telegram (telegram/watch.rs)

当 `Rewatch` 时，仍渲染 HTML 正文（同 Final 带终态标签），但**附带 inline keyboard** 含一个
按钮，`callback_data = "watch:rw:<session_id>"`（前缀短，适配 64 字节限制）。

新增常量 `CB_REWATCH_PREFIX = "watch:rw:"`，回调解析按此前缀截取 session_id。

### 3.4 Slack (slack/watch.rs)

当 `Rewatch` 时，在 context 终态标签之后追加 actions block，含单个 button：
`action_id = "watch_rewatch"`，`value = session_id`。

新增常量 `ACTION_REWATCH = "watch_rewatch"`。

## 4. daemon 回调处理

### 4.1 WatchBtn / WatchAction 扩展

```rust
enum WatchBtn {
    Unwatch,
    Refresh,
    Rewatch(String), // session_id
}
```

飞书 `WatchAction::Rewatch(String)` → `handle_watch_fs_action` 新分支；
TG/Slack/DingTalk 解析后统一映射到 `WatchBtn::Rewatch(session_id)` 进入 `apply_watch_action`。

### 4.2 rewatch 核心逻辑

由 entry 的 `session_id` 驱动（按钮回调数据中的 `sid` 作冗余校验），流程：

1. 从 `subs` 取出 rewatchable entry（拿到 session_id、channel）
2. 从 `AgentRegistry::snapshot()` 按 `session_id` 查找记录，取 `seq`
3. **从 `subs` 移除该 rewatchable entry**（不再需要路由）
4. 构建 frame：`watch::build_frame(seq, rec, waiting)`
5. 更新旧卡为 `Final { label: "已重新关注" }`
6. 发送新卡：`client.send(&frame, mode, now, lang)` — mode 按帧状态决定
7. `register_watch_at(...)` 注册新订阅（含上限校验、换新卡收尾）

**旧卡更新**（按钮变 disabled「已重新关注」）：
- 飞书：ack oneshot 返回 `Final { label: "已重新关注" }` 渲染的卡片
- TG：`edit_message_text` 更新旧消息（移除 keyboard + 终态标签改「已重新关注」）
- Slack：`chat.update` 更新旧消息（移除 actions + context 改「已重新关注」）
- 钉钉：`updateCardDataByKey`（`rewatchable="false"` + `final_label="已重新关注"`）

**回退（找不到 session）**：旧卡变 disabled「已结束」，不发新卡，移除 rewatchable entry。

### 4.3 各渠道回调入口

| 渠道 | 入口 | 解析 | session_id 来源 |
|---|---|---|---|
| 飞书 | `handle_watch_fs_action` | `WatchAction::Rewatch(sid)` | value `sid` 字段 |
| 钉钉 | `handle_watch_dd_action` | actionId `"watch_rewatch"` | params `sid` 字段 |
| TG | `handle_watch_tg_action` | `CB_REWATCH_PREFIX` 前缀 | callback_data 截取 |
| Slack | `handle_watch_slack_action` | `ACTION_REWATCH` | action `value` 字段 |

## 5. i18n 新增

| key | ZH | EN |
|---|---|---|
| `watch.btnRewatch` | `已切换到 {to} · 重新关注` | `Switched to {to} · Re-watch` |
| `watch.btnRewatched` | `已重新关注` | `Rewatched` |

## 6. 实施顺序

### Phase 1：核心模型 + 飞书（先跑通一个渠道验收）

1. `watch.rs`：FinalKind::Rewatched + is_rewatchable + WatchButtons::Rewatch + rewatch 文案函数 +
   card_view 适配 + i18n + 单测
2. `WatchEntry` 加 `rewatchable` 字段；`PersistedWatch` 加 `#[serde(default)] rewatchable`
3. `daemon/mod.rs`：
   - `finalize_and_drop_watches` 对 `is_rewatchable()` 标记而非移除
   - 引擎 `watch_tick` 跳过 rewatchable entry
   - 上限/空闲退出过滤 rewatchable
   - TTL 清理（session 结束或不在注册表→移除）
4. `feishu/card.rs`：`WatchButtons::Rewatch` 渲染 + `WatchAction::Rewatch` + parse 扩展 + 单测
5. `daemon/mod.rs`：`handle_watch_fs_action` Rewatch 分支 + 发新卡 + 旧卡 ack
6. 编译 + `cargo test` + install + **飞书真机验收**

### Phase 2：钉钉

7. `dingtalk/watch.rs`：模板变量扩展 + parse 扩展 + 单测
8. 钉钉模板 JSON 更新 + 开发者后台发布
9. `daemon/mod.rs`：`handle_watch_dd_action` Rewatch 分支
10. 编译 + `cargo test` + **钉钉真机验收**

### Phase 3：Telegram + Slack

11. `telegram/watch.rs`：Rewatch inline keyboard + parse + 单测
12. `daemon/mod.rs`：`handle_watch_tg_action` Rewatch 分支
13. `slack/watch.rs`：Rewatch actions block + parse + 单测
14. `daemon/mod.rs`：`handle_watch_slack_action` Rewatch 分支
15. 编译 + `cargo test` + **TG/Slack 真机验收**
