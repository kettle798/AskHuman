# Agent 激活信号 Demo —— 调研结论与实测记录（Claude / Codex / Cursor）

> 记录 IM 渠道激活方案（`docs/todos/im-channel-activation.md`）相关的**方案/调研结论**与**实测结果**。
>
> 关联设计：`docs/todos/im-channel-activation.md` 的三层信号模型——
> **进程存活＝电平骨干、turn-start↔turn-end＝判忙闲、TTL＝仅兜底**。本 Demo 用来**实测验证**该模型对各家 Agent CLI 是否成立。

---

## 0. 红线：未经许可，不得实际调用任何 Agent 做实测

**实际运行 claude / cursor-agent / codex 会消耗用户的 token。** 因此：

- **任何需要真正启动 / 提示 / 驱动 Agent 的实测，必须先经用户明确许可**（通过 `AskHuman` 征询），并由**用户来操作 Agent**（启动、发提示、停止、关窗口等）。AI 只负责搭 harness、观察日志、分析结果。
- 不得为了「顺便验证」而擅自 `claude -p "..."` / `codex exec ...` / `cursor-agent ...` 之类调用。
- 纯文档查证、**读源码**、跑 harness 自带的**非 Agent 冒烟自测**（直接 `node envprobe.cjs <agent>` 等）不受此限。

进度：**Claude Code 已实测通过**（§5）。**Codex 已实测通过**（§6）。**Cursor 已完成静态核对**（§7，读本机安装包 bundle 得到完整 hook 加载/事件/契约/env 注入，配置已据此对齐；待用户许可后做最小轮次实测）。

---

## 1. 调研结论（三家对照）

### 1.1 核心问题：不用 Hook 能否拿到会话 ID？

**三家 CLI 都能**——调用 CLI 工具（shell 子进程）时都会向子进程注入一个「会话 ID」环境变量：

| Agent | 会话 ID env | 来源 | 旁证/可靠度 |
|---|---|---|---|
| Claude Code | `CLAUDE_CODE_SESSION_ID`（+`CLAUDECODE`/`CLAUDE_CODE_CHILD_SESSION`/`CLAUDE_PROJECT_DIR`） | 文档 + **实测确认** | §5 已验证：与 hook `session_id` 一致 |
| Cursor (cursor-agent CLI) | `CURSOR_CONVERSATION_ID`（+`CURSOR_AGENT=1`/`AGENT_TRANSCRIPTS`） | **实测旁证 + bundle 静态确认**（注入点见 §7.3） | shell 工具子进程注入 `CURSOR_CONVERSATION_ID` |
| Codex CLI | `CODEX_THREAD_ID`（= 线程/会话 ID） | **源码确认**（见 §6.1） | 与早期「文档未见」相反，以源码为准 |

> **env 的两个共同局限**（决定 Hook 仍不可替代）：
> 1. env 只在「被调用那一刻」给值——**给不了 turn 开始/结束事件**。要在「第一次提问之前」就 arm，仍需 turn-start Hook。
> 2. env **不直接给 Agent 进程 PID**——做「进程存活轮询」（电平骨干）需要 PID，得顺进程树向上 walk 找到 Agent 进程。harness 正是要验证这个 walk 是否可靠。

⚠️ 跨平台坑（macOS 不受影响）：Claude 在 Linux 上 `CLAUDE_CODE_ENV_SCRUB` 会把 Bash 子进程放进**隔离 PID namespace**，导致 `ps`/`kill` 看不到宿主进程 → walk / `kill -0` 失效。本机 macOS 无此问题；生产实现需注意 Linux 分支。

### 1.2 三家对照表

| 维度 | Claude Code (2.1.176) | Cursor Agent (cursor-agent CLI) | Codex CLI |
|---|---|---|---|
| 子进程 env 带会话 ID？ | **是** `CLAUDE_CODE_SESSION_ID` | **是** `CURSOR_CONVERSATION_ID`（bundle 确认） | **是** `CODEX_THREAD_ID`（源码） |
| turn-start 事件 | `UserPromptSubmit` | `beforeSubmitPrompt` | `UserPromptSubmit` |
| turn-end 事件 | `Stop` | `stop` | `Stop` |
| 会话结束事件 | `SessionEnd`（可靠，除 `kill -9`） | `sessionEnd`（CLI 确有触发点，但**硬杀必丢**→仍靠轮询，待实测） | **无 SessionEnd** → 只能靠进程存活轮询兜底 |
| Hook 配置位置 | `~/.claude/settings.json` 或项目 `.claude/settings.json` | 多源合并：企业/团队/用户 `~/.cursor/hooks.json`/项目 `.cursor/hooks.json`（version 1）+ **还读** `.claude/settings*.json`（兼容） | `~/.codex/config.toml` 的 `[hooks]` 或 `.codex/hooks.json` |
| Hook 输入会话字段 | `session_id` | `session_id`（+`workspace_roots`/`transcript_path`；shell 工具 env 才是 `CURSOR_CONVERSATION_ID`） | `session_id`（+`turn_id`；**无** `reason`） |
| 进程粒度 | 单 `claude` 进程＝单会话 ✓ | cursor-agent CLI 单进程/会话（IDE 版才粗粒度）⚠️待实测 | 单 `codex` 进程＝单会话 ✓ |
| Hook 是否默认开启 | 是 | **是**（`loadProjectHooks` 默认 true；需在 hooks.json 配置事件） | **是**（`Feature::CodexHooks` stage=Stable, default_enabled=true，源码确认；旧的 `[features] codex_hooks` 已是兼容别名，无需手动开） |
| Hook 信任机制 | 项目级 settings 自动加载 | **无信任哈希**：项目层默认加载（不像 Codex 要逐条信任）；IDE 版另有 Settings>Hooks | **需「信任」**：项目须被信任 + 每条 hook 内容哈希须被信任（见 §6.2） |

### 1.3 综合结论

1. **「不用 Hook 拿会话 ID」三家 CLI 都成立**（Claude/Cursor 实测、Codex 源码）。
2. **三家都需 Hook 才能拿 turn-start 事件**（在第一次提问之前就 arm）；纯 env 给不了 turn 边沿。
3. **「会话是否还在」最终都得靠进程存活轮询兜底**：Codex 干脆**没有** SessionEnd；Claude 的 SessionEnd 在 `kill -9` 下会丢；Cursor 虽有 `sessionEnd` 触发点但硬杀同样必丢。这正是设计 doc 的电平骨干。
4. harness 的**非 Agent 冒烟自测**已通过：`poller` 能 `arm→LIVE→DEAD`，`hooklog`/`envprobe` 读 env、回溯进程树、写日志均正常（三 profile 均冒烟过）。

---

## 2. Demo 组成（共享核心 + 按家族 profile/配置）

```
demo/agent-lifecycle/
  FINDINGS.md                       本文件
  .gitignore                        忽略 logs/（运行时产物）
  harness/                          ── 三家共享的核心，profile 驱动 ──
    common.cjs                      进程树回溯 / 猜 agent pid / env 收集 / pid 文件 / kill -0 探活（全部 profile 驱动）
    hooklog.cjs                     被各 hook 调用：node hooklog.cjs <agent> <Event> → logs/<agent>/events.jsonl
    envprobe.cjs                    「无 Hook 路径」探针：node envprobe.cjs <agent> → logs/<agent>/envprobe-*.json
    poller.cjs                      「电平骨干」：node poller.cjs <agent> [intervalMs] → logs/<agent>/poller.jsonl
    codex-trust.cjs                 复刻 Codex 信任哈希算法：node codex-trust.cjs <hooks.json> → 打印 [hooks.state] 条目
    profiles/
      claude.cjs                    会话 ID env 名 / 要收集的 env / 进程识别 token / hook JSON 字段
      codex.cjs
      cursor.cjs
  agents/                           ── 每家一个「启动目录」，内含其项目级 hook 配置 ──
    claude/.claude/settings.json    9 个生命周期事件 → hooklog.cjs claude <Event>
    codex/.codex/hooks.json         Codex 事件集（无 SessionEnd/Notification）→ hooklog.cjs codex <Event>
    cursor/.cursor/hooks.json       sessionStart/sessionEnd/beforeSubmitPrompt/stop/preToolUse/postToolUse/afterFileEdit → hooklog.cjs cursor <Event>
    cursor/.claude/settings.json    交叉触发实验：intended=claude，被 Cursor 兼容加载 → 实测「重复触发 + 去重判据」（§7.6）
  logs/<agent>/                     每家独立子目录：events.jsonl / poller.jsonl / envprobe-*.json / pid.json
```

### 2.1 抽象方式（为什么不用完全重写）

- **差异收敛到 profile**：各家不同的只有「会话 ID env 名、要收集哪些 env、怎么在进程树里认出 Agent 进程、hook JSON 里会话字段叫什么」。这些进 `profiles/<agent>.cjs`，其余逻辑（进程链回溯、`kill -0` 探活、JSONL 落盘、pid 文件）三家共用。
- **脚本接 `<agent>` 参数**：`hooklog/envprobe/poller` 第一个参数都是 agent 名，据此 `loadProfile` 并把日志写到 `logs/<agent>/`，三家互不干扰、可并行。
- **进程识别坑已内建**：cursor-agent 可执行名是 `agent`（不含 "cursor-agent"），profile 用 `processTokens:["cursor-agent","agent"]`（argv0 basename 精确匹配 `agent`）+ `commandTokens:["cursor-agent"]`（完整命令行特异子串）兜底；`SELF_MARKERS` 始终排除 harness 自身。

### 2.2 关键纪律

- `hooklog` **绝不往 stdout 写**（Claude/Codex 的 `UserPromptSubmit`/`SessionStart` stdout 会被当上下文注入模型）；所有信息进日志文件；始终 `exit 0` fail-open。
  - Cursor 已确认（bundle，§7.4）：**exit 0 + 空 stdout = no-op**，不阻塞、不报错；权限类 hook 才会读 stdout JSON 当裁决，但本 demo 只挂观测类 + 空输出，故安全。`exit 2` 才会阻塞（Claude 风格），其它非零仅在 `failClosed`(默认 false) 时阻塞。
- 配置里命令写**绝对路径**（仓库整体是一个 git repo）。若仓库迁移，需同步改 `agents/*/.../*.json` 里的绝对路径。

---

## 3. 软链问题：三家都**不需要**软链

用户担心「`.claude`/`.codex`/`.cursor` 是否必须放在项目根（git 根）才生效，否则要做软链」。结论：**都不用软链，在各自 `agents/<家>/` 目录启动即可**。

- **Claude**：从 cwd 起读 `.claude/`（并向上合并）。实测确认：在子目录 `agents/claude/` 启动 claude，其 `.claude/settings.json` 即生效（§5 C7）。
- **Codex**（源码确认，`codex-rs/config/src/loader/mod.rs`）：
  - `find_project_root` 默认按 `project_root_markers=[".git"]` 向上找——所以在本仓库内，Codex 的「项目根」会算成**仓库根**。
  - **但** `load_project_layers` 会从 cwd 向上**逐级扫描到项目根**，对**沿途每个**含 `.codex/` 的目录都加载其 `config.toml`/`hooks.json`。因此把 `.codex/` 放在 `agents/codex/`（位于 cwd 与 git 根之间），在该目录启动 codex 就会被发现并加载——**无需软链、无需放到 git 根**。
  - 兜底：万一需要强制「以 cwd 为根」，可 `codex -c 'project_root_markers=[]'`（源码里空数组→根=cwd）。
- **Cursor**（bundle 确认，§7.2）：用户级 `~/.cursor/hooks.json` **恒加载**；项目级 `<workspace>/.cursor/hooks.json` 由 `loadProjectHooks`（默认 true）门控，`<workspace>` = 启动时的工作区根。**无信任哈希**（不像 Codex 要逐条信任）。在 `agents/cursor/` 启动即可，无需软链。另外它**还会读** `<workspace>/.claude/settings.json`（兼容 Claude 配置）。

---

## 4. 运行方式

> 启动 / 操作 Agent 由**用户**来做（见 §0 红线）。AI 负责起 poller、观察日志。`<agent>` ∈ `claude|codex|cursor`。

1. **（AI）起轮询器**（后台），等 `logs/<agent>/pid.json` 出现：
   ```bash
   node demo/agent-lifecycle/harness/poller.cjs <agent> 1000
   ```
2. **（用户）在对应启动目录起 Agent**：
   ```bash
   cd demo/agent-lifecycle/agents/<agent> && <claude|codex|cursor-agent>
   ```
3. **（用户）按该家测试矩阵逐项操作**；每步 AI 读 `logs/<agent>/events.jsonl` / `poller.jsonl` / `envprobe-latest.json` 分析。
4. 看关键事件：
   ```bash
   node -e 'require("fs").readFileSync("demo/agent-lifecycle/logs/<agent>/events.jsonl","utf8").trim().split("\n").forEach(l=>{const r=JSON.parse(l);console.log(r.ts,r.event,"sid="+(r.session_id||"-"),"agent_pid="+r.agent_pid)})'
   ```

清理一次实测：`rm -f demo/agent-lifecycle/logs/<agent>/*`

---

## 5. Claude Code 实测结果（已通过）

实测时间 2026-06-13，claude 2.1.176 / macOS arm64。一个 claude 会话＝一个独立 `claude` 进程。
（实测时 harness 还在旧路径 `demo/claude-activation/`，逻辑与现共享版一致。）

### 5.1 验证清单（全部通过）

- [x] **C1** claude 调 Bash 工具时，子进程 env 含 `CLAUDECODE=1`/`CLAUDE_CODE_SESSION_ID`/`CLAUDE_CODE_CHILD_SESSION=1`/`CLAUDE_CODE_ENTRYPOINT=cli`。注意：Bash 工具子进程**没有** `CLAUDE_PROJECT_DIR`，而 **hook 子进程有**（两类子进程 env 不完全一样）。
- [x] **C2** Bash 子进程 env 的 `CLAUDE_CODE_SESSION_ID` == hook JSON 的 `session_id` == hook env 的 `CLAUDE_CODE_SESSION_ID`，三者一致。
- [x] **C3** 从 CLI 子进程向上 walk 能稳定定位 claude：`node → /bin/zsh(Bash工具包装) → claude → -zsh → login → Terminal`；以 `claude` 名启动时 `comm` 即 `claude`。
- [x] **C4** turn-start(`UserPromptSubmit`)↔turn-end(`Stop`) 成对；中间夹 `PreToolUse`/`PostToolUse`。
- [x] **C5** 见矩阵：**只有 `kill -9` 丢了 `SessionEnd`，进程存活轮询全程不漏**。
- [x] **C6** `/clear` 会 `SessionEnd(reason=clear)`→`SessionStart(source=clear)`，**session_id 轮换**但**进程 pid 不变** → 绑进程比绑 session_id 稳。
- [x] **C7** 项目级 `.claude/settings.json` 的 9 个 hook 全部加载并触发（子目录启动即生效，无需放 git 根）。

### 5.2 关闭矩阵（0 计费轮次：仅启动 + 斜杠命令 + 外部 kill/关窗）

| 场景 | `SessionEnd`? | reason | session_id | 进程 | poller |
|---|---|---|---|---|---|
| `/clear` | **触发** + 紧接 `SessionStart(source=clear)` | `clear` | **轮换** | **不变** | 仍 LIVE |
| 正常 `/exit` | **触发** | `prompt_input_exit` | — | 退出 | **DEAD**（~0.9s 后） |
| **`kill -9`** | **不触发（事件丢失）** | — | — | 被杀 | **DEAD** ✓ |
| 关终端窗口 | **触发**（收 SIGHUP 优雅收尾） | `other` | — | 退出 | **DEAD** |

poller 全程自动在 3 个会话间 re-arm，每次 `arm→LIVE→DEAD` 正确。

### 5.3 实测结论

1. **「电平骨干＝进程存活」是唯一不漏的信号**：`kill -9` 下 `SessionEnd` 完全丢失，只有进程存活轮询抓到死亡。
2. **关窗 ≠ 崩溃**：关窗口时 claude 收 SIGHUP 仍优雅触发 `SessionEnd(reason=other)`；真会丢事件的是 `kill -9`/崩溃。
3. **绑「进程」比绑「session_id」稳**：`/clear` 让 session_id 轮换但进程不变。
4. **不用 Hook 也能拿会话 ID**：读 `CLAUDE_CODE_SESSION_ID` 即可；但仍需 Hook 在「第一次提问前」arm + 拿 turn-start。

---

## 6. Codex CLI（实测进行中：最小模式已通过）

### 6.0 实测结果（2026-06-13，codex npm 包 / macOS arm64）

最小模式（A1–A4 的启动+1 turn+正常退出）**已通过**：

- **信任算法实测正确**：写入 `~/.codex/config.toml [hooks.state]` 后，`/hooks` 里 9 个 hook 全部 **Active/Trusted**、未被要求重新审阅，且事件确实触发 → §6.2 复刻的哈希算法正确。
- **不用 Hook 拿会话 ID 成立**：shell 工具子进程 env `CODEX_THREAD_ID = 019ec093-…`，**等于** hook stdin 的 `session_id`。另有 `CODEX_CI=1` / `CODEX_MANAGED_BY_NPM=1` / `CODEX_MANAGED_PACKAGE_ROOT`。
- **子进程 env 不对称（重要）**：**shell 工具**子进程有 `CODEX_THREAD_ID`；**hook** 子进程**没有**（只有 `CODEX_MANAGED_*`），hook 靠 stdin JSON 拿 `session_id`。与 Claude 一致：不同类型子进程 env 不同。
- **turn 成对**：`UserPromptSubmit`(turn_id) → 多组 `PreToolUse`/`PostToolUse`(tool=`Bash`) → `Stop`(同 turn_id)。
- **进程定位**：walk 命中 codex pid（comm 为原生二进制 `.../codex-darwin-arm64/.../bin/codex`，含 "codex"）；链路 `node(envprobe) → codex(原生) → node(npm 启动器) → -zsh → login → Terminal`（codex 有个 node 启动器父进程，二者同生共死）。
- **无 SessionEnd，结束只靠轮询**：正常退出时 `events.jsonl` **零事件**，仅 poller 抓到 `DEAD`。坐实「Codex 会话结束完全靠进程存活轮询」。
- 其它：`transcript_path` 为 `~/.codex/sessions/<日期>/rollout-…-<session_id>.jsonl`；`permission_mode` 实测为 `bypassPermissions`。

加测批次（B5/B6/B7 + kill-9）结果：

- **B5 无工具 turn**：`UserPromptSubmit`(turn=019ec097-668c) → `Stop`(同 turn)，中间**无** Pre/PostToolUse → `Stop` 不依赖工具，turn 边沿可靠。
- **B6 多工具 turn**：一个 turn(019ec097-a3da) 内多组 `PreToolUse`/`PostToolUse`(Bash)，外层仅一对 `UserPromptSubmit`/`Stop`。
- **turn_id 每轮轮换、session_id 跨轮稳定**：同一进程(28097) 同一 session(019ec097) 下，B5/B6 两轮 turn_id 不同（…668c / …a3da）。
- **kill -9（硬杀）**：poller 约 1s 内抓到 `DEAD`（10:48:56）。重启会话时 poller 自动 re-arm（22956→28097，source=hook:SessionStart，**仅启动即 arm、0 turn**）。
- **B7 `/new`（干净复测：hi → /new → hi）**：`/new` **会再触发一次 `SessionStart`**（`source=startup`，与启动同源、无法据此区分），**session_id 轮换**（`019ec09c-0f3b…` → `019ec09c-4219…`），但**进程 pid 不变**（32342）。poller 因 pid 未变保持 LIVE、无需 re-arm。→ **与 Claude `/clear` 完全一致：会话身份应绑进程 pid，不要绑 session_id**。（首轮误判为「/new 无效」是因当时在启动后 71ms 内就 /new，事件挤在一起。）

综合：Codex 三层信号模型成立——turn-start(`UserPromptSubmit`)↔turn-end(`Stop`) 成对、`CODEX_THREAD_ID` 免 Hook 拿会话 ID、**会话结束唯一可靠信号＝进程存活轮询**（无 SessionEnd，正常退出/硬杀都零事件、全靠 poller）、`/new` 轮换 session_id 但 pid 不变（**身份绑 pid**）。三家在「绑进程 pid」「进程存活兜底」上结论一致。

### 6.1 源码结论（来源：用户提供的 `/Users/wutian/Developer/codex`）

- **子进程注入会话 ID**：`codex-rs/core/src/unified_exec/process_manager.rs::open_session_with_sandbox` 往 shell 工具子进程 env 插入 `CODEX_THREAD_ID = thread_id`（= 会话/线程 ID）；`exec_env.rs` 注释明确「即便 `include_only` 也注入」。→ **不用 Hook 即可读 `CODEX_THREAD_ID` 拿会话 ID**。另有 `CODEX_CI` 等。
- **事件集**（`codex-rs/config/src/hook_config.rs::HookEventsToml`）：`PreToolUse` / `PermissionRequest` / `PostToolUse` / `PreCompact` / `PostCompact` / `SessionStart` / `UserPromptSubmit` / `SubagentStart` / `SubagentStop` / `Stop`。**没有 `SessionEnd`，没有 `Notification`。** → 会话结束只能靠进程存活轮询。
- **hook 输入字段**（schema）：`session_id` / `transcript_path` / `cwd` / `hook_event_name` / `model` / `permission_mode`；`SessionStart` 多 `source`；`UserPromptSubmit` 多 `prompt`/`turn_id`/`agent_id`/`agent_type`。**无 `reason`**（无 SessionEnd）。
- **hooks.json 形状**（`HooksFile`，`deny_unknown_fields`）：`{"hooks": {"<Event>":[{"matcher"?, "hooks":[{"type":"command","command":"...","timeout"?(秒)}]}]}}`。本 demo 用 `.codex/hooks.json`（与 Claude settings 同构，便于核对）。
- **hooks 默认开启**：`Feature::CodexHooks` stage=Stable、default_enabled=true（`codex-rs/features/src/lib.rs`）；旧 `[features] codex_hooks=true` 仅兼容别名，**无需手动开**。
- **项目根/`.codex` 定位**：见 §3——nested `agents/codex/.codex/` 会被加载，**无需软链**。

### 6.2 信任机制（源码确认 + 本轮程序化写入）

- **项目信任**：`.codex` 项目层受信任门控（`trust_context.decision_for_dir`）。查 `~/.codex/config.toml` 已有 `[projects."/Users/wutian/Developer/HumanInLoop"] trust_level="trusted"`；因 Codex 项目根算到仓库根，且 `decision_for_dir` 会回退到 `project_root` 键匹配——**本项目已自动受信任，无需另加条目**。
- **hook 信任**：每条 hook 有内容哈希（`HookStateToml.trusted_hash`）；**未信任的 hook 不会执行**，需启动时的 hooks 审阅确认（TUI `startup_hooks_review`），或 `--dangerously-bypass-hook-trust`。哈希「内容相关、路径无关」——内容改了要重新信任。
- 本轮策略（用户改定）：**程序化写入信任**，正好验证算法。已用 `harness/codex-trust.cjs` 复刻 Codex 源码算法，把 9 条 hook 的 `trusted_hash` 写入**用户级** `~/.codex/config.toml` 的 `[hooks.state."<key>"]`（备份在 `~/.codex/config.toml.bak.*`）。

  **算法（源码出处见脚本头注）**：
  - **状态键** `hook_key`（`hooks/src/lib.rs`）= `"<abs hooks.json 路径>:<event_label>:<group_index>:<handler_index>"`；`event_label` 是 snake_case（`session_start`/`user_prompt_submit`/…）。**注意**：`[hooks.state]` 只从 **User/SessionFlags** 配置层读取（`config_rules.rs`），所以必须写进 `~/.codex/config.toml`，写进项目 `.codex/config.toml` 无效。
  - **哈希** `version_for_toml`（`config/src/fingerprint.rs`）= `"sha256:" + sha256_hex( 紧凑( 键名递归字典序排序( json(identity) ) ) )`。
  - **identity** = `NormalizedHookIdentity { event_name:<label>, <flatten 的 MatcherGroup{matcher?,hooks:[handler]}> }`；handler 归一为 `{type:"command",command,timeout(默认600,min1),async(bool),statusMessage?}`；`commandWindows` 在非 Windows 丢弃、`None` 字段不序列化；matcher 对 `UserPromptSubmit`/`Stop` 强制 None、其余保持原样（`events/common.rs`）。
  - **正确性验证**＝实测：启动 codex 后看 hooks 是否 **Active/Trusted**（未被要求重新审阅）且事件确实写进日志；若显示 Modified/Untrusted 说明哈希算错，需重算。

### 6.3 测试方案（最少轮次优先；可复用于 Cursor）

> 计费=发一个 prompt（turn）。下表把信号尽量压到**免费动作**（启动 / 斜杠命令 / 外部 kill/关窗 / shell 工具内跑 envprobe）。

**A. 最小模式（≈1 个计费 turn，先跑这个）**

1. （AI）后台起 poller：`node demo/agent-lifecycle/harness/poller.cjs codex 1000`。
2. （你，0 turn）`cd demo/agent-lifecycle/agents/codex && codex`。启动后：
   - 用 `/hooks`（或看启动时有无 hooks 审阅弹窗）确认本 demo 9 个 hook 为 **Active/Trusted**（**这步即验证 §6.2 的哈希算法**：若没被要求重新信任，说明算对了）。
   - AI 读 `logs/codex/events.jsonl`：`SessionStart` 是否触发、`session_id`/`source`/`model`/`permission_mode`、hook 子进程里有哪些 `CODEX_*`；poller 是否立即 `arm→LIVE`（walk 到 `codex` 进程）。
3. （你，**1 turn**）发一条 prompt，让 codex 用 shell 跑 envprobe：
   > `请用 shell 运行：node /Users/wutian/Developer/HumanInLoop/demo/agent-lifecycle/harness/envprobe.cjs codex`

   这一个 turn 同时覆盖：`UserPromptSubmit`(turn-start，带 `prompt`/`turn_id`) → `PreToolUse`→`PostToolUse`（shell 工具）→ `Stop`(turn-end)；envprobe 落盘里能看到 **shell 子进程 env 是否含 `CODEX_THREAD_ID`、是否==hook 的 `session_id`**、walk 能否定位 codex 进程。
4. （你，0 turn）关闭矩阵（每种之间 AI 读 poller）：① 正常退出（`/quit` 或 Ctrl-C 两次）；② 重开后 `kill -9 <codex pid>`；③ 重开后直接关终端窗口。每种都确认 **无 `SessionEnd`**（Codex 本就没有），poller 是否都抓到 `DEAD`。

**B. 加测项（想要更完整信息，按需各 +1 turn）**

5. （+1 turn）发一条**不调用工具**的 prompt（如「只回一个 hi，别用工具」）→ 确认 `UserPromptSubmit`↔`Stop` 成对、中间**无** Pre/PostToolUse（即 Stop 不依赖工具）。
6. （+1 turn）发一条**多工具**的 prompt（如「先列目录再读 README 头几行」）→ 一个 turn 内多组 `PreToolUse`/`PostToolUse`、外层仅一对 `UserPromptSubmit`/`Stop`（验证配对稳健）。
7. （0 turn）`/new`（Codex 新开一段对话）→ 是否再次 `SessionStart`、**`thread_id` 是否轮换**、**进程 pid 是否不变**（对照 Claude `/clear` 的「身份绑进程」结论）。
8. （可能计费）`/compact` → 是否 `PreCompact`/`PostCompact`（compact 会让模型总结，可能产生费用，视情况做）。

> `SubagentStart`/`SubagentStop` 需 codex 真正派生子代理才会触发，不易低成本构造，本轮先不强测。

---

## 7. Cursor Agent（已完成 bundle 静态核对，待实测）

> 核对对象：本机安装包 `~/.local/share/cursor-agent/versions/2026.06.12-01-15-52-7244546/`
> （webpack 分包的压缩 JS：hooks 模块在 `2097.index.js`，事件枚举/env 注入在 `index.js`）。
> 纯静态读包，**未运行** cursor-agent（不违反 §0）。

### 7.1 实测旁证：cursor-agent 的 ambient env + 进程 walk（零成本，非主动调用）

当前会话本就跑在 cursor-agent CLI 里，对 harness 直接 `node envprobe.cjs cursor`（读**自身** shell 工具子进程 env，不主动驱动 Agent，不违反 §0）实测到：

```
CURSOR_AGENT = 1
CURSOR_CONVERSATION_ID = 2083ffb0-…-052e009ddcc9     # == 该会话 transcript UUID
AGENT_TRANSCRIPTS = ~/.cursor/projects/<proj>/agent-transcripts
CURSOR_INVOKED_AS = agent
CURSOR_ASKPASS_SOCKET / CURSOR_ASKPASS_SECRET / CURSOR_RIPGREP_PATH
```

进程 walk 也**实测命中**：`node(envprobe) → /bin/zsh → agent(pid 1051) → -zsh → login → Terminal`，
`agent_comm=/Users/wutian/.local/bin/agent`、`command=agent --use-system-ca …/index.js --yolo`、`alive=alive`。
→ 坐实 §7.3 的「shell 工具子进程注入 `CURSOR_AGENT`/`CURSOR_CONVERSATION_ID`/`AGENT_TRANSCRIPTS`」与 profile 的 `processTokens:["agent"]` 能稳定定位 cursor-agent 进程。（这是「免 Hook 拿会话 ID」+「进程 walk」两点的零成本实证；hook 链路仍待许可后实测。）

### 7.2 Hook 加载机制（bundle 确认）

加载器 `load({loadProjectHooks=true})` 按下列**多源合并**（后者覆盖/追加前者），`loadProjectHooks` **默认 true**：

| 源 | 路径 |
|---|---|
| enterprise | macOS `/Library/Application Support/Cursor/hooks.json`；win `C:\ProgramData\Cursor\hooks.json`；linux `/etc/cursor/hooks.json` |
| team | `<workspace>/.cursor/managed/active-team-hooks/hooks.json` |
| user | `~/.cursor/hooks.json`（**恒加载**） |
| project | `<workspace>/.cursor/hooks.json`（`loadProjectHooks` 门控，默认开） |
| claude-user | `~/.claude/settings.json`（**兼容 Claude 配置**） |
| claude-project | `<workspace>/.claude/settings.json` |
| claude-project-local | `<workspace>/.claude/settings.local.json` |

- **无信任哈希机制**（不像 Codex 要逐条 `trusted_hash`）；项目层默认就加载。
- `hooks.json` 支持**块注释**（解析前 `/* … */` 被剥除，即 JSONC）。
- Claude 的 `.claude/settings.json` 会经「事件名/工具名兼容映射」并入（见 §7.3）。

### 7.3 事件名、字段与 env 注入（bundle 确认）

**原生事件枚举（21 个，camelCase）**：
`beforeShellExecution` / `beforeMCPExecution` / `afterShellExecution` / `afterMCPExecution` /
`beforeReadFile` / `afterFileEdit` / `beforeTabFileRead` / `afterTabFileEdit` / `stop` /
`beforeSubmitPrompt` / `afterAgentResponse` / `afterAgentThought` / `sessionStart` / `sessionEnd` /
`preCompact` / `subagentStart` / `subagentStop` / `preToolUse` / `postToolUse` / `postToolUseFailure` / `workspaceOpen`。
（CLI 内对以上**都有触发点** `executeHookForStep(...)`——含 `sessionStart`/`sessionEnd`/`beforeSubmitPrompt`/`stop`。）

**Claude→Cursor 兼容映射**（让 `.claude/settings.json` 能用）：
`PreToolUse→preToolUse`、`PostToolUse→postToolUse`、`UserPromptSubmit→beforeSubmitPrompt`、`Stop→stop`、
`SubagentStop→subagentStop`、`SessionStart→sessionStart`、`SessionEnd→sessionEnd`、`PreCompact→preCompact`；
**`PermissionRequest`/`Notification` → 无对应（忽略）**。工具名映射：`Bash→Shell`、`Edit→Write`、`Glob→无`，`Read/Write/Grep/WebFetch/WebSearch/Task` 直通。

**两类子进程 env（与 Claude/Codex 同样不对称）**：
- **shell 工具子进程**（envprobe 走这条，「免 Hook 拿会话 ID」）：`CURSOR_AGENT="1"`、`CURSOR_CONVERSATION_ID=<safe 会话 ID>`（有 conversationId 时）、`AGENT_TRANSCRIPTS=<projectDir>/…`（有 projectDir 时）、`SUDO_ASKPASS`/`CURSOR_ASKPASS_SOCKET`/`CURSOR_ASKPASS_SECRET`（用 askpass 时）。
- **hook 子进程**（hooklog 走这条）：`CURSOR_PROJECT_DIR`、`CURSOR_VERSION`、`CURSOR_USER_EMAIL`、`CURSOR_TRANSCRIPT_PATH`、`CLAUDE_PROJECT_DIR`（兼容）；**会话 ID 不在 env**，靠 stdin。

**hook stdin payload（base）**：`{ hook_event_name, cursor_version, workspace_roots:[workspace], user_email, session_id, transcript_path, …各事件附加字段 }`（`subagentStop` 另带 `agent_transcript_path`）。
→ **hook 里会话字段是 `session_id`**（不是 `conversation_id`；后者只在 shell 工具 env）。

**payload 传输方式**：默认 `argv_heredoc`——把命令包成 `cmd <<'CURSOR_HOOK_EOF'\n{json}\nCURSOR_HOOK_EOF`（POSIX）或 `@'\n{json}\n'@ | & cmd`（PowerShell）；即 **JSON 走 stdin（heredoc）**。可选 `stdin` 直传模式。两种模式 hook 都从 **stdin 读 JSON** → 现有 `hooklog.cjs` 读 stdin 即兼容。命令经 **shell** 执行，多 token 命令串 OK。

### 7.4 stdout / 退出码契约（bundle 确认）

| hook 返回 | 行为 |
|---|---|
| `exit 0` + **空 stdout** | **no-op**：不阻塞、不报错（本 demo 观测类 hook 即此路径，安全） |
| `exit 0` + JSON stdout | 解析为裁决；支持 Claude 嵌套 `hookSpecificOutput` 兼容 |
| **`exit 2`** | **阻塞**（Claude 风格）；stdout/stderr 作为阻塞消息 |
| 其它非零 | 记为失败；**仅当 `failClosed`(默认 false) 才阻塞**，否则非阻塞 |
| 超时 / spawn 失败 | 同上，仅 `failClosed` 才阻塞 |

各事件返回结构：权限类（`beforeShellExecution`/`beforeMCPExecution`/`beforeReadFile`/`preToolUse`…）→ `{permission:"allow"|"deny"|"ask", user_message?}`；`beforeSubmitPrompt`/`sessionStart` → `{continue:bool, user_message?}`；`stop`/`subagentStop` → `{}`（另有 `loop_limit`/`loop_count` 防 stop-hook 自循环）。

### 7.5 配置现状与最小轮次实测计划

`agents/cursor/.cursor/hooks.json`（version 1）已按上述对齐，挂**观测类**事件：`sessionStart`/`sessionEnd`/`beforeSubmitPrompt`/`stop`/`preToolUse(*)`/`postToolUse(*)`/`afterFileEdit`，全部 `exit 0` 空输出 → 不阻塞。profile `cursor.cjs` 的会话字段已改为 `session_id`、env 列表已含两类子进程的注入项。

**最小轮次实测（沿用 §8，≈1 个计费 turn）**——待用户许可+操作：
1. （AI，0 turn）后台起 poller：`node demo/agent-lifecycle/harness/poller.cjs cursor 1000`。
2. （你，0 turn）`cd demo/agent-lifecycle/agents/cursor && cursor-agent`（或 `agent`）。读 `logs/cursor/events.jsonl` 看 `sessionStart` 是否触发、hook 子进程 env、poller 是否 `arm→LIVE`（walk 命中 `agent`/`cursor-agent` 进程）。
3. （你，**1 turn**）让它用 shell 跑：`node /Users/wutian/Developer/HumanInLoop/demo/agent-lifecycle/harness/envprobe.cjs cursor` → 一个 turn 覆盖 `beforeSubmitPrompt`→`preToolUse`/`postToolUse`→`stop`，并验证 shell 子进程 env 是否含 `CURSOR_CONVERSATION_ID`、walk 能否定位进程。
4. （你，0 turn）关闭矩阵：① 正常退出；② `kill -9`；③ 关窗 → 看 `sessionEnd` 哪些场景触发、poller 是否都抓到 `DEAD`（预期硬杀丢 `sessionEnd`，靠 poller 兜底）。

### 7.6 ⚠️ 重复触发：Cursor 兼容加载 Claude hook → 同一 hook 在 Cursor 下触发两次

**问题**（bundle 确认）：Cursor 的 hook 加载器**恒加载** `~/.claude/settings.json`（claude-user 源**无门控**），项目 `.claude/settings.json`/`.claude/settings.local.json` 也在 `loadProjectHooks`（默认 true）下加载，并经事件名映射并入。**没有任何配置开关能关掉这层 Claude 兼容**（只有一个管「输出格式」的 `enableClaudeNestedHookSpecificOutputCompatibility`，与是否加载无关）。

后果：若生产里我们为了同时支持两家，把生命周期 hook **既写进 Claude 配置又写进 Cursor 配置**，那么——
- 跑 **Claude Code**：只有 Claude 读 `~/.claude/settings.json` → 触发**一次** ✓
- 跑 **cursor-agent**：Cursor 读 `.cursor/hooks.json`（自己的）**外加**兼容读 `.claude/settings.json` → 两边都有我们的 hook → **同一事件触发两次** ✗（凡两家都有的事件：SessionStart/sessionStart、UserPromptSubmit/beforeSubmitPrompt、Stop/stop、Pre/PostToolUse…）

**解决：在 hook 脚本里运行时判定「真实 Agent」，只让归属一致的那次生效。**
判据来自 **hook 子进程 env**（§7.3/§7.4，bundle 确认）：
- Cursor 的 hook 子进程**恒有** `CURSOR_VERSION`/`CURSOR_PROJECT_DIR`（且兼容性地也设 `CLAUDE_PROJECT_DIR`）。
- Claude 的 hook 子进程有 `CLAUDECODE`/`CLAUDE_CODE_SESSION_ID`，但**没有** `CURSOR_*`。
- ⚠️ `CLAUDE_PROJECT_DIR` **不能**用作「是 Claude」的判据——Cursor 也设它。必须**先判 Cursor**（看 `CURSOR_*`），排除后再认 Claude。

规则：每条 hook 记住自己「注册给哪家」（intended）；运行时按 env 算出真实 agent（running）；`running !== intended` 就 **`exit 0` 跳过**。于是：
- Claude 跑 → `.claude` 项 running=claude=intended → 执行（一次）✓
- Cursor 跑 → `.cursor` 项 running=cursor=intended → 执行；`.claude` 项 running=cursor≠claude → **跳过** → 净一次 ✓

harness 已落地该判据：`common.cjs::detectRunningAgent()`（顺序 Cursor→Codex→Claude）；`hooklog.cjs` 记录 `running_agent`/`dedupe_skip`（demo 里**仍照记两次**以便实测看到重复，生产实现应在 `dedupe_skip` 时直接跳过）。
`agents/cursor/.claude/settings.json`（intended=claude）即**交叉触发实验**：在 cursor-agent 下，这些会写进 `logs/claude/events.jsonl` 且 `running_agent=cursor`/`dedupe_skip=true`，从而坐实「Cursor 重复触发 + 判据可去重」。其中 `Notification` 无 Cursor 对应（应**不**出现，作负向对照）。

> 备注：Codex 不读 `.claude`/`.cursor`，无此交叉问题；但 `detectRunningAgent` 对它也给正确结果，规则统一无副作用。

---

## 8. 低轮次（省 token）测试方法论

> 背景：有的 Agent **按轮次（turn）计费**——每发一次 prompt 收一次费（Cursor 尤其明显）。测试要把「信号验证」和「花钱的 turn」**解耦**：能用免费动作触发的信号，绝不发 prompt。

### 8.1 核心原则

1. **区分「免费动作」与「计费动作」**：
   - **免费**：启动会话、斜杠命令（不走模型）、外部 `kill`/关窗、读自身 ambient env、跑常驻 hook/poller。
   - **计费**：发一个 prompt（= 一个 turn）。
2. **把观测前移到免费动作上**：常驻 hook 日志 + 进程存活轮询 + ambient env，让多数信号在「启动/关闭/斜杠命令」时就被记录。
3. **唯一要花钱的 turn 一次覆盖多个信号**：用一个 prompt 同时验证 env 探针 + 工具调用 + turn 成对。

### 8.2 各信号需要几个 prompt（Claude 实测归纳，可平移）

| 要验证的信号 | 触发方式 | 计费 prompt 数 |
|---|---|---|
| 项目级 hooks 是否加载 / `SessionStart` / 首次 arm | 启动 Agent 即触发 | **0** |
| hook 子进程能拿到哪些 env（含会话 ID） | `SessionStart` hook 自动记录 | **0** |
| 会话结束事件 + reason（正常退出） | 退出命令 | **0** |
| 崩溃下事件是否丢 / 进程存活轮询是否兜住 | 外部 `kill -9` | **0** |
| 关窗的收尾行为 | 关终端窗口 | **0** |
| `/clear` 是否轮换会话 ID / 进程是否不变 | 斜杠命令 | **0** |
| **shell 工具子进程**的 env（区别于 hook 子进程） | 让 Agent 跑一次 `envprobe.cjs <agent>` | **1** |
| turn-start↔turn-end 成对 | 发一个会调用工具的 prompt | **1**（可与上一行**合并**） |

→ **整套验证的理论最小成本 = 1 个 prompt**：让 Agent 用 shell 跑 envprobe（同时覆盖「子进程 env」+「turn 成对」+「Pre/PostToolUse」）；其余全 0 prompt。

### 8.3 套到各家

- **Cursor**（最该用）：`sessionStart`/`sessionEnd`/`stop` 靠「启动+关闭/外部 kill」触发（0 轮）；`CURSOR_CONVERSATION_ID` 直接读 ambient env（0 轮）；只有 `beforeSubmitPrompt`↔`stop` 成对需 1 轮。
- **Codex**：`SessionStart`/进程死亡 0 轮；`UserPromptSubmit`↔`Stop` 需 1 轮；注意**无 SessionEnd**，结束完全靠 poller。
- 通用：先把 hook 日志 + poller 挂上，再用**一个**精心设计的 prompt 收集所有「必须对话才有」的信号。

---

## 9. 对设计 doc 的影响（建议回写 `docs/todos/im-channel-activation.md`）

- §6 表：Claude 行标「实测确认」；Codex 行更新为「`CODEX_THREAD_ID` env 带会话 ID、**无 SessionEnd**、hooks 需信任」；Cursor 行标「cursor-agent CLI 有 `CURSOR_CONVERSATION_ID`」。
- §10「PPID-at-ask 兜底」：实测 walk 路径 `子进程 → /bin/zsh(Bash包装) → claude`，确认「向上 walk 找稳定 Agent 进程」可行且必要。
- 新增注意点：会话身份**应以进程 pid 为准**（`session_id`/`thread_id` 可能随 `/clear` 轮换）；**进程存活轮询是三家通用的不可漏底**（Codex 尤其，因为它根本没有 SessionEnd）。
- **跨家族重复触发（重要，§7.6）**：Cursor 恒兼容加载 `~/.claude/settings.json` 且**无开关可关**——若 AskHuman 同时给 Claude 与 Cursor 装生命周期 hook，cursor-agent 下会**触发两次**。生产实现必须在 hook 里**运行时判定真实 Agent**（`CURSOR_VERSION`→cursor、`CODEX_*`→codex、`CLAUDECODE`→claude；注意 `CLAUDE_PROJECT_DIR` 会被 Cursor 兼容设置、不可作判据），`running !== intended` 即跳过；否则一次提问会被上报两次（误判忙闲 / 重复 attach）。
