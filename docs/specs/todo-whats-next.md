# Todo 队列 + whats-next：任务间隙的下一步派发

> 状态：设计定案（2026-07-15，经六轮 AskHuman 评审），待实现。
> 实现分支：`feat/todo-whats-next`（worktree `../HumanInLoop-todo`，Dev Instance popup-only）。

## 1. 需求

agent 正在执行一个耗时任务时，用户脑中已有下一个想法。用户**不想打断当前任务**（插话
Interject 是「打断进行中」的纠偏语义，不适合），希望：

1. 随手把想法排进一个 **todo 列表**；
2. 列表**持久化**：agent 报错退出 / daemon 重启后，下次在这个项目仍能访问；
3. agent **完成当前任务后**主动拿到待办；或在它提问时，用户能把待办快速发给它；
4. 某条待办**开始执行后自动从列表清除**。

与插话（`docs/specs/agent-interject.md`）的分工：插话在**任务进行中**注入（PreToolUse
拦截、打断当前工具调用）；todo 在**任务完成后的间隙**派发（不打断任何进行中的工作）。
两者互不替代、共存。

## 2. 设计总览

```
输入入口（随时增删查）                 送达出口（何时进入 agent）
┌──────────────────────────┐          ┌────────────────────────────────┐
│ CLI  AskHuman todo …     │          │ whats-next 提问（主路径）：      │
│ Popup 折叠待办区（增删）  │──┐    ┌─▶│  agent 完成任务后必调，          │
│ IM   /todo、/todo-rm     │  │    │  │  待办渲染为选项 chip，           │
│ GUI  独立待办窗口 + 托盘  │  ▼    │  │  选中提交＝开始执行＋原子出队    │
└──────────────────────────┘ 项目级 │  ├────────────────────────────────┤
                             todo 队列─▶│ Popup 折叠待办区 chip（辅路径）：│
                             （持久化）  │  普通提问也可点选待办作答＋出队  │
                                        └────────────────────────────────┘
```

- 用户否决了 Stop hook 注入方案：Stop 时用户可能已明确要结束，此时 agent 又跑新任务
  「太奇怪」。取待办必须发生在**结束确认之前**。
- 定案核心：把现有 rules 的「结束前必须问、确认后才能结束」**标准化**为一次
  `whats-next` 提问——待办即选项，「结束本轮」也是选项；选待办＝继续干，选结束＝
  同意结束。一问两用，不重复啰嗦。

## 3. 设计定案

### D1 数据模型与持久化：项目级 FIFO 队列

- 归属：**项目级**（`project.rs` 的 git 根路径 key，回退 cwd）。同项目的新会话 / 新
  agent / 多 agent 共享一份队列；不随 session 结束清理，长期保留直到出队或删除。
- 条目：`{ id: uuid, text: String, created_at_ms }`。纯文本，首期不支持附件。
- 顺序：FIFO 追加，首期不做手动排序/置顶（出队按 id 任选，顺序只影响展示）。
- 存储（第 9 轮定案，实现简化）：`~/.askhuman/state/todos.json` 即**唯一数据源**，
  所有进程直读直写；写操作（读-改-写）持 flock 串行化（与 `history.jsonl` 跨进程写锁
  同模式；Windows 无锁 best-effort，与 history 现状一致）。**不做** daemon 内存运行态
  ——todo 无热路径（whats-next 每轮一次、增删查是人操作频率），双层结构无收益。
  文件形态：`{ "projects": { "<project_key>": [ {id,text,createdAtMs}, … ] } }`，
  空项目键剪除；原子写（tmp + rename）。
- 附带收益：CLI `todo` / `--whats-next` 的待办读写**不依赖 daemon 存活**；
  Unix / Windows 同一套代码，无平台特例分支。

### D2 whats-next：取代「结束前确认」的标准化提问

**CLI**：`AskHuman --whats-next [<Message>] [--stdin] [-f <file> …]`

- Message＝agent 的**完成报告**（可选，推荐 `--stdin` heredoc），`-f` 可附报告文件；
  **不接受** `-q` / `-o`（问题与选项由系统固定生成）。
- 语义：**必然发出一个提问**，走完全现成的普通 Ask 链路（popup + 四 IM、抢答协调、
  活跃槽 ∪ watch 投放、24h 等待、排空）。项目 key 取 CLI 调用 cwd 的 git 根。
- 问题固定为「接下来做什么？」（按界面语言本地化）；选项＝该项目当前各待办条目
  （每条一个 chip/按钮，携带条目 id）+ **恒有**一个「结束本轮」选项（列表末位）。
  无待办时只有「结束本轮」+ 自由输入框。
- 回答**写入回复历史**（它承载完成报告，是 agent 的正式提问；区别于 Stop 确认卡的
  hook 兜底卡不写历史）。

**MCP**：server 新增第二个工具 `whats_next`（与 `ask` 并列），入参
`{ message?, files? }`；薄壳实现与 `ask` 一致——spawn `AskHuman --whats-next --output
json …` 子进程，结果映射进 structuredContent。

**提交结果 → 语义映射**（纯函数，完整单测）：

| 用户提交 | 语义 | 出队 |
|---|---|---|
| 选中某待办 chip（可附自由文本补充） | 执行该条（补充一并送达） | ✅ 按 id 原子出队 |
| 只填自由文本 | 执行该文本（全新指令） | 不出队 |
| 选「结束本轮」且无文本 | 同意结束：agent 输出结束 marker 后自然停止 | 不出队 |
| 选「结束本轮」但填了文本 | **视为继续**：文本是新指令（有话说＝还没完，与 Stop 卡「纯文字=继续」一致） | 不出队 |
| 取消 / 关窗 / 超时 | 沿用普通 Ask 取消语义（`[status]` 指示继续询问） | 不出队 |

- **出队时机＝「开始执行」**：Coordinator 首个终态回答的唯一汇聚点上，把选中的待办
  id 出队并 persist——即用户要求的「agent 开始执行后自动清掉条目」。
- 单选逐条循环：一次只派一条；agent 做完又回到 whats-next，天然形成逐条循环，
  每条之间都有人工确认点。

### D3 whats-next 结果输出：一段纯文本（评审定案：不要特殊结构）

whats-next 是固定题目，结果不需要区块结构，stdout 就是一段文本：

- 派活（选待办 / 只填文本 / 结束+文本）→ 输出**任务文本本身**：选中待办的原文；
  有补充文本时按空行拼在其后；只填文本时即该文本。
- 准许结束 → 输出固定一句（英文，agent 契约）：
  `The user approved ending this turn — no more tasks.`
  agent 据 rules 既有 marker 行（保持原文，见 D4）输出 `[user_confirmed_end_turn]`。
- 取消 / 超时 → 沿用现有 `[status]` 区块语义（指示继续询问）。
- MCP `whats_next` 工具返回同一段文本。
- 普通 Ask 的既有区块（`[selected_options]` / `[user_input]` / `[files]` / `[status]`）
  完全不变；`--agent-help` 增补 whats-next 用法说明（含 `--stdin` 报告写法，rules 里
  不重复）。

### D4 rules 文案变更草稿（`prompts.rs` 单一来源，始终英文）

评审定案：**在原措辞上做最小调整**——把「结束前必须提问请求反馈」改为「必须调
whats-next」，不新增用法细节（那是 `--agent-help` 的职责），marker 行原样保留。

CLI 版 `cli_reference()` 原三行中，前两行调整为：

```text
- Before completing the turn/request, you MUST run `{program} --whats-next`
  (optionally with a completion report as the Message) to ask me what to do next.
- If it returns a task, start working on it immediately and repeat this protocol
  when done. Do NOT end the turn/conversation or mark the request as complete
  unless `{program} --whats-next` returned that I approved ending the turn and
  there are no more tasks.
```

第三行（「After the user explicitly approves ending the turn, you MUST append the
`{end_marker}` marker …」）**原样不动**。

MCP 版 `mcp_reference()` 对应前两行调整为：

```text
- Before completing the turn/request, you MUST call the AskHuman `whats_next`
  tool (optionally with a completion report as its message) to ask me what to do
  next.
- If it returns a task, start working on it immediately and repeat this protocol
  when done. Do NOT end the turn/conversation or mark the request as complete
  unless the `whats_next` result says I approved ending the turn and there are
  no more tasks.
```

- 其余纪律（必须经 AskHuman 提问、附件经 `-f`、relentless interview、不擅自改方案、
  subagent 例外）**全部保留**；「anything I need to review must go through …」继续
  覆盖完成报告之外的中间产物。
- Grok skill 正文复用 `mcp_reference()`，自动跟随。
- rules 是托管产物：升级二进制后按现有 `agents update` / 过期徽标机制更新四家安装文案。

### D5 与 Stop 结束确认、插话的关系；Stop 卡待办派发（兜底送达点）

- whats-next 是**提示词层**协议；Stop 确认（`agent-stop-confirmation.md`）是 **hook
  层**兜底——agent 不守规矩直接停时仍有接管卡。两者独立开关、语义兼容：
  经 whats-next 选「结束」→ agent 按 rules 输出 `[user_confirmed_end_turn]` marker →
  Stop hook 现有的 marker 检测静默放行，**不会重复弹卡**。
- **Stop 卡加入待办派发**（第 8 轮定案）：Stop 确认卡选项变为「各待办 chip +
  继续对话 + 结束对话」——agent 跑偏直接停时，停下的那一刻仍能一键派下一条。
  与第 1 轮否决的「Stop hook **自动**注入」不同：这是确认卡上的**手动选择**，
  不点待办就不会执行。
  - 项目 key：Stop hook stdin 自带 `cwd` / `workspace_roots`，映射 git 根。
  - 提交映射：选待办（±文字）＝以该条（带补充）为 continuation 并按 id 出队；
    只填文字＝继续（文字为指令）不出队；选「继续对话」＝原有语义；
    选「结束」＝结束并**丢弃文字**（维持 Stop 卡现有规格，不与 whats-next 强行统一）。
  - continuation prompt 沿用现有各家原生语义分流（Claude 结构化包裹 / Codex、Cursor
    裸传）；Stop 确认按家开关、默认关、Grok 不支持、不写历史等既有约束全部不变。
- 插话不变：任务进行中仍用 Interject / `/msg`；todo 不提供「打断进行中任务」的能力。

### D6 入口一：CLI `todo` 子命令（跨平台）

```bash
AskHuman todo add <text>     # 追加一条（按 cwd 的 git 根归项目）
AskHuman todo list           # 列出本项目待办（带编号）
AskHuman todo rm <编号>      # 删除一条
AskHuman todo clear          # 清空本项目（需交互确认，或 --yes 跳过）
```

- Unix 经 daemon（连接或拉起，daemon 内存为准）；非 Unix 直接文件 + 锁。
- 输出人类可读；后续需要时再加 `--output json`（首期不做）。

### D7 入口二：Popup 折叠待办区（普通提问也显示）

- 弹窗（PopupView）新增**可折叠**「待办」区，显示**该提问项目**的待办列表
  （项目 key 随 AskRequest 已有传递）：
  - 每条待办是**可选 chip**：选中＝提交时该条文本并入本次回答送达（进 `[user_input]`，
    与手输文本按空行拼接），并按 id **best-effort 出队**；
  - 每条行内**删除**按钮；底部**快速新增**输入框——不打断作答流程顺手记想法；
  - whats-next 弹窗中待办即问题选项本体（D2），折叠区只保留管理功能（增删），
    不重复渲染 chip。
- **chip 点选作答仅在单题弹窗启用**（多题时「待办算哪题的回答」有歧义）；多题弹窗
  折叠区只保留增删查看。
- IM 普通提问卡**不加**待办区（评审定案：任务中途的提问用待办作答易答非所问，
  且多题卡歧义；IM 侧待办送达统一走 whats-next 卡）。

### D8 入口三：IM `/todo`、`/todo-rm`（Unix，daemon 入站命令层）

- `/todo`（无参）→ 复用现有跨渠道**单选卡**选一个存活 agent（agent 定位其项目）；
  `/todo <n>`（n＝`/status` 稳定编号）→ 直达。
- 确定 agent 后发**管理卡**：列出该项目全部待办 + 新增入口：
  - 飞书 / 钉钉卡片支持表单输入 → 卡上直接带输入框提交新增；
  - Telegram / Slack 无可靠卡内输入 → 卡上提示 `/todo <n> <text>` 直接追加
    （该形式四渠道通用）。
- `/todo-rm` → 同样先选 agent，再复用现有单选卡逐条选择删除（复用既有卡片模型，
  无新协议）。
- 无存活 agent 会话时回提示（首期不做「最近 workspace 索引选项目」；GUI/CLI 入口
  不受此限制）。与 `/status` 同门控（daemon 存活即可用）。

### D9 入口四：GUI 独立待办窗口 + 托盘（Unix，GUI Host 承载）

- 新窗口类型（`WindowKind::Todo`，全局唯一），入口：托盘菜单「待办…」+ Agent 状态
  窗口各 agent 卡片（预选该 agent 的项目）。
- 带**项目选择器**：候选＝有待办的项目 ∪ 活跃 agent 的项目 ∪ 最近 workspace 索引。
- 功能：列表、新增、删除、清空（确认）；实时同步＝宿主进程监听 `todos.json` 文件变化
  （复用 `config_watch.rs` 的 notify 模式，第 9 轮定案）——daemon 未运行时窗口照样可用。

### D10 平台矩阵

| 能力 | macOS / Linux | Windows |
|---|---|---|
| `--whats-next` / MCP `whats_next` | ✅（提问经 daemon；待办直读文件） | ✅ 单进程回退 |
| CLI `todo` 子命令 | ✅ 直接文件 + flock | ✅ 直接文件（无锁 best-effort） |
| Popup 折叠待办区 | ✅ 直读文件 | ✅ 直读文件 |
| IM `/todo`、`/todo-rm` | ✅ | —（无 daemon） |
| GUI 待办窗口 / 托盘入口 | ✅ | —（无 GUI Host / 托盘） |

### D11 竞态与边界

- **提交以卡上文本为准，出队 best-effort**：同项目两个 agent 同时 whats-next 时同一
  待办出现在两张卡，先提交者出队；后提交者选同一条 → 文本照常送达（你点的就是你要
  的），出队发现条目已不在则跳过，不报错、不要求重选（IM 卡片无法强制刷新到眼前）。
  GUI/CLI 删除赶在点卡之前同理。
- 同一请求内**首答胜出**等既有约定全部沿用（whats-next 就是普通 Ask）。
- 待办文本超长时 IM 按钮/选项按各渠道既有截断规则展示，送达内容始终为原文全文。
- daemon 重启 / agent 崩溃：队列由 `todos.json` 恢复，无会话绑定故无需清理逻辑。
- Dev Instance：`ASKHUMAN_HOME` 隔离下各实例有各自的 `todos.json`（自然成立）。

## 4. 单元测试要求（骨架，实现计划再展开）

- 队列存储：add/rm/clear/出队幂等、persist 往返、空项目剪除、文件锁（非 Unix 路径）。
- whats-next 参数解析：与 `-q`/`-o` 互斥、Message/`--stdin`/`-f` 组合。
- 提交映射五分支（D2 表）纯函数全覆盖；出队 best-effort（条目已删）分支。
- 输出契约：任务文本（含补充拼接）/ 固定结束句 / 取消 `[status]` 三种渲染；
  MCP 返回同一文本。
- rules：新文案包含 whats-next 指令、不再含旧「请求反馈」两行、marker 行原样保留；
  CLI/MCP/Grok skill 三处一致性。
- Popup：单题启用 chip、多题只管理；chip 文本并入 user_input 的拼接规则。
- IM：`/todo` 选 agent / 直达 / 无 agent 提示；管理卡两种新增形态；`/todo-rm` 流程。
- Stop 卡：待办 chip 前置、四分支提交映射（含「结束丢弃文字」维持现状）、
  选待办出队、marker 抑制下不弹卡不出队；既有 Stop 确认测试不回归。
- 竞态：双卡同条目先后提交、提交前删除。

## 5. 反馈意见记录

- （2026-07-15 第 1 轮）项目级归属定案；Stop hook 注入被否——「用户已确认结束后 agent
  又跑任务太奇怪」，取待办须在结束确认之前；需要全渠道可用的工作流，不只弹窗。
- （第 2 轮）用户提出 `--whats-next` 方案：agent 结束前必调、必发一问，有待办列待办、
  无待办给输入框，提交待办即删除。IM 侧定 `/todo <n>` 选 agent 后发管理卡（列待办 +
  输入新增）。Popup 点选填入的「编辑后不知道提交哪条」问题被用户指出。
- （第 3 轮）whats-next **取代**结束前确认（选结束＝同意结束、输出 marker）；单选逐条
  循环；「待办渲染为 chip 按 id 出队 + 自由文本补充」解法获认可；`/todo-rm` 选 agent
  后用现有单选卡选条目；Popup 折叠区含管理功能；IM 普通卡交互待设计（后被收回）。
- （第 4 轮）IM 普通提问卡**不做**待办区；Popup 折叠区 chip 模型认可；MCP 首期一起做；
  「结束+文字＝继续」定案；FIFO 不做排序。
- （第 5 轮）whats-next 卡片形态（固定问题 + 恒有结束选项 + 报告作 Message + 写历史）
  认可；IM 仅存活 agent 时可用；竞态「照常送达 + best-effort 出队」认可；平台矩阵认可；
  CLI 子命令加 `clear`（需确认）。
- （第 6 轮）worktree `../HumanInLoop-todo` + 分支 `feat/todo-whats-next`，popup-only
  Dev Instance；一次做完再统一验收；先审 spec 再动代码。
- （第 7 轮 spec 评审）rules 文案要**最小改动、不啰嗦**：在原措辞上把「必须提问请求
  反馈」换成「必须调 whats-next」即可；`--stdin` 等用法细节归 `--agent-help`，rules 不写；
  whats-next 结果**不要特殊结构**——就是一段文本（任务内容，或「用户同意结束」的固定句）。
- （第 8 轮）用户提出并定案：**Stop 确认卡也加入待办派发**（选项＝待办 chip + 继续 +
  结束），作为 rules 不被遵守时的兜底送达点；提交映射与 whats-next 对齐，唯
  「结束+文字＝结束丢弃文字」维持 Stop 卡现状。
- （第 9 轮实现前确认）存储简化定案：`todos.json` 即唯一数据源、全进程直读直写 +
  flock 串行化，不做 daemon 内存态与 CRUD IPC；GUI 待办窗口实时同步改为宿主监听文件
  变化。CLI 待办操作因此不依赖 daemon 存活。
