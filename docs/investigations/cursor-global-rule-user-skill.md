# Cursor 全局规则迁移为用户级 always-on Skill

> 状态：未来优化方向，尚未实现
>
> 调查日期：2026-07-12
>
> 调查方式：仅静态分析 Cursor / Cursor CLI / Grok 本地安装包、日志与 transcript；未发起任何 Cursor Agent 请求

## 1. 问题与结论

AskHuman 当前把 Cursor 的托管提示词写到：

```text
~/.cursor/rules/askhuman.mdc
```

这不是一个在所有 Cursor 使用场景下都可靠的用户级全局落点。当前实现依赖 Cursor 从工作区目录向
文件系统根逐级扫描 `.cursor/rules/**/*.mdc` 时途经用户 Home：

- Cursor CLI 始终有 cwd；cwd 位于 Home 下时会途经 `~/.cursor/rules`，所以规则能够生效。
- Cursor IDE 只有在真正打开 Folder / Workspace 后才会为每个 workspace root 创建规则加载器。
- 仅打开单个文件、没有 workspace folder 时，IDE 不创建该加载器，即使文件位于 `~/.codex` 等
  Home 子目录中，也不会读取 `~/.cursor/rules/askhuman.mdc`。

因此现有 UI 的“项目需位于 Home 下”提示不完整，还缺少“IDE 必须打开 Folder / Workspace”这一
必要条件。更根本地说，文件规则仍由 workspace 驱动，不是真正无条件加载的用户规则。

候选修复是把 Cursor 托管产物迁移为：

```text
~/.cursor/skills/askhuman/SKILL.md
```

Cursor IDE 3.7.36 与 Cursor CLI 2026.06.26 的本地代码都会独立扫描用户级 `~/.cursor/skills/`；
带 `alwaysApply: true` 的 Skill 会被转换为全局 Rule，不依赖 workspace root。

## 2. 静态证据

### 2.1 失败会话

2026-07-12 22:52 的 Cursor IDE 会话中：

- transcript 显示模型不知道 AskHuman，把用户所说的 `AskHuman` 误认成了 `AskQuestion`。
- Hook 日志两轮请求都记录 `workspace_roots: []`。
- Cursor workspace Hook 日志反复记录 `No workspace folder found`。
- 规则缓存 `ruleCount` 为 18，恰好等于 `~/.cursor/skills-cursor/` 中 18 个内置 Skill；
  `askhuman.mdc` 未进入缓存。
- Stop Hook 本身工作正常，第一次回答结束后确实通过 AskHuman 收到了用户 continuation；这说明
  Hook 链与规则注入链是独立的，不能用 Hook 正常来推断模型已经收到 AskHuman 指令。

### 2.2 Cursor IDE 3.7.36

`Cursor.app` 内的 `cursor-agent-exec` 扩展：

1. 从 `workspace.workspaceFolders` 生成 `workspacePaths`。
2. 对每个 workspace path 创建一个 `LocalCursorRulesService`。
3. 该服务的 `loadRulesFromDirAndAncestors` 才会从 workspace 逐级向父目录扫描
   `.cursor/rules/**/*.mdc`、`AGENTS.md` 等文件。
4. `workspacePaths` 为空时，没有任何 `LocalCursorRulesService`，因此没有父级扫描入口。

同一个扩展的 `AgentSkillsCursorRulesService` 则始终扫描用户目录中的：

- `~/.cursor/skills/`
- `~/.agents/skills/`
- 开启 third-party extensibility 时的 `~/.claude/skills/`、`~/.codex/skills/`
- Cursor 托管的 `~/.cursor/skills-cursor/`

它解析 Skill 后，若 frontmatter 的 `alwaysApply` 为 `true`，会直接把 Skill 转为 `global` Rule。

### 2.3 Cursor CLI 2026.06.26

本机 Cursor CLI 的打包代码包含同一套 `AgentSkillsCursorRulesService`、用户级 Skill 根目录和
`alwaysApply -> global Rule` 转换，因此迁移后 CLI 与 IDE 可共用同一产物。

### 2.4 官方规则边界

Cursor 官方文档把 `.cursor/rules` 定义为项目规则；真正的 User Rules 在 Cursor Settings → Rules
中创建，是纯文本、全项目始终应用：

<https://cursor.com/docs/context/rules>

本机 3.7.36 代码显示该 User Rules UI 通过 Cursor 登录态调用 `knowledgeBaseAdd / Update / Remove`
云端 API，并没有稳定的外部本地配置文件。AskHuman 不应直接修改 Cursor 的 SQLite、令牌或私有云端
接口，因此无法用受支持方式实现 User Rules 的一键自动安装。

## 3. Grok 兼容性与隔离

### 3.1 Grok 确实默认读取 Cursor Skills

本机 Grok 0.2.93 文档明确列出以下兼容目录：

```text
~/.cursor/skills/    User / lowest priority
./.cursor/skills/    Local or repo / high priority
```

且默认启用；只有 `[compat.cursor] skills = false` 或环境变量
`GROK_CURSOR_SKILLS_ENABLED=false` 才会关闭整类扫描。

因此若直接放一个普通 `~/.cursor/skills/askhuman/SKILL.md`，Grok 也会发现它。Cursor 的规则可能是
CLI 变体，而 Grok 的 AskHuman 集成固定为 MCP 变体；让 Grok 自动调用 Cursor 这份 Skill 会产生冲突。

### 3.2 推荐的单文件隔离

候选 Skill 使用：

```yaml
---
name: askhuman
description: AskHuman mandatory interaction protocol for Cursor.
alwaysApply: true
disable-model-invocation: true
user-invocable: false
metadata:
  surfaces:
    - ide
    - cli
---
```

各方行为：

- **Cursor Rule 管线**：先读取 `alwaysApply: true`，把完整 Skill 转成全局 Rule。
- **Cursor Skill 管线**：`disable-model-invocation: true` 避免模型把同一文件作为普通 Skill 再次自动调用；
  `metadata.surfaces` 只允许 Cursor 的 `ide` / `cli` surface。
- **Grok**：不把 `alwaysApply` 解释为常驻规则；`disable-model-invocation: true` 阻止模型自动调用，
  `user-invocable: false` 隐藏斜杠入口。因此 Grok 继续只使用自己的
  `~/.grok/skills/interaction-protocol/SKILL.md`。

不建议安装 Cursor Skill 时去改 Grok 的 `[compat.cursor]`，因为这会关闭用户所有 Cursor Skills；
也暂不写 Grok `[skills].ignore`，避免一个 Cursor 集成操作跨 Agent 修改 Grok 配置。

## 4. 未来实现规格

### 4.1 新产物

- 路径：`~/.cursor/skills/askhuman/SKILL.md`
- 由 AskHuman 独占管理，文件中保留可识别的托管标记。
- 正文继续按 Cursor 当前模式写入 `cli_reference()` 或 `mcp_reference()`。
- 不同时保留新 Skill 与旧 MDC，避免相同强制协议重复进入上下文、浪费 token。

### 4.2 状态与“需更新”

Cursor 状态判定建议分为：

| 磁盘状态 | installed | needs update | 说明 |
|---|---:|---:|---|
| 新 Skill 存在且正文匹配当前模式 | 是 | 否 | 最新状态 |
| 新 Skill 存在但正文漂移 | 是 | 是 | 正常提示更新 |
| 仅旧托管 MDC 存在 | 是 | 是 | 已安装用户看到“更新”，而不是“未安装” |
| 新 Skill 与旧 MDC 同时存在 | 是 | 是 | 更新时收敛为仅新 Skill |
| 两者均不存在 | 否 | 否 | 未集成 |

`installed_variant` 应优先读取新 Skill；仅存在旧 MDC 时继续识别其 CLI / MCP 变体，以保证
`agent_mode` 不会误判成 None。

### 4.3 更新 / 迁移顺序

更新必须按以下顺序：

1. 创建 `~/.cursor/skills/askhuman/`。
2. 原子写入完整的新 `SKILL.md`。
3. 重新读取并验证托管标记、frontmatter 和正文变体。
4. 新 Skill 成功后，才移除旧 MDC 中的 AskHuman 托管区块。
5. 旧 MDC 若只剩 frontmatter / 空白则删除文件；若含用户其它内容则保留残余内容。

若步骤 1–3 失败，旧 MDC 必须原样保留。这样迁移失败不会让用户突然失去规则。

### 4.4 安装与卸载

- 全新安装只写新 Skill，不再创建 MDC。
- 卸载删除带 AskHuman 托管标记的新 Skill；目录为空时可删除 `askhuman/` 目录。
- 为处理历史状态，卸载也应清理旧 MDC 的 AskHuman 托管区块，但不得删除用户区块外内容。
- `display_path`、打开和定位操作改为指向新 `SKILL.md`。

### 4.5 可能涉及的代码

- `src-tauri/src/paths.rs`：增加 Cursor AskHuman Skill 目录 / 文件路径；保留 legacy MDC 路径助手。
- `src-tauri/src/integrations/agent_rules.rs`：Cursor 分支改为 Skill，增加 legacy 状态、迁移与清理。
- `src/views/SettingsView.vue`、`src/i18n/{zh,en}.ts`：路径与提示文案不再声称依赖 Home 下项目。
- `docs/specs/agent-rules-config.md`、`docs/overview.md`：实现时更新最终契约。

## 5. 验证清单

未来实现时至少覆盖：

1. 新 Skill 构建内容与 frontmatter 精确测试。
2. 新安装、重复安装、正文更新幂等。
3. 仅旧 MDC → 显示已安装且需更新。
4. 迁移先写新 Skill、后清旧 MDC；模拟写失败时旧 MDC 保留。
5. 旧 MDC 含用户额外内容时只移除 AskHuman 区块。
6. 新旧并存时更新后只保留新 Skill。
7. CLI / MCP 变体识别与 `agent_mode` 回归测试。
8. Grok Skill 保持原样，不修改 `~/.grok/config.toml`。
9. 静态复核目标 Cursor / Cursor CLI 版本仍保留用户 Skill 与 `alwaysApply` 逻辑。
10. 不运行付费 Cursor Agent；若将来需要真实 IDE 会话验收，必须先单独取得用户明确许可。

## 6. 风险

- Cursor 官方稳定的真正全局入口仍是云端 User Rules；`alwaysApply` Skill 是当前 IDE / CLI 已实现的
  本地能力，但公开文档没有把它承诺为 User Rules 的替代品。Cursor 升级后需静态复核。
- `metadata.surfaces` 是 Cursor 当前实现字段，Grok 把 `metadata` 当普通元数据；真正阻止 Grok 自动使用
  这份 Skill 的关键是 `disable-model-invocation: true`。
- 上游若改变“alwaysApply Skill 同时进入 Rule 与 Skill 管线”的顺序，需要重新验证隔离组合。
