# PROGRESS

按具体任务 / 需求记录待办与当前进展。任务 / 需求完成后删除其 section（历史留在 git）。

## 进行中：Todo 队列 + whats-next（本 worktree feat/todo-whats-next）

设计定案见 `docs/specs/todo-whats-next.md`。已完成：存储（todos.json）、CLI todo 子命令、
whats-next CLI/MCP、rules 变更、Popup 折叠待办区、Stop 卡待办派发、GUI 待办窗口 +
托盘/AgentsView 入口（cargo test 693 通过 + vitest 34 通过）。
当前：待经 AskHuman 确认 IM `/todo` 系的钉钉卡内输入方案（需新注册模板，无法纯代码实现）
与测试 bot 配置，然后实现 D8 并统一验收。

## 待办：TCC 弹窗修复真机验证

TCC（文件权限）弹窗修复用户尚未真机验证（Agent 任务确认弹层已验收）。

## 待办：项目 review 的 P2 项（择机）

报告见 `docs/investigations/project-review-2026-07.md`。剩余择机项：
types.ts 改为从 Rust 派生（ts-rs/specta）、TS 7 升级（等 vue-tsc 支持）、
agents.snapshot() typed 化 + pnpm/Node 版本对齐（R5）。

## 待办：Cursor 全局 Rules 迁移为用户级 always-on Skill

调查与候选设计见 `docs/investigations/cursor-global-rule-user-skill.md`。无 workspace folder 的 Cursor IDE
不创建项目 Rules 加载器，因此不会读取 `~/.cursor/rules/askhuman.mdc`。未来改为用户级
`~/.cursor/skills/askhuman/SKILL.md`，旧安装显示“需更新”，迁移时先写新 Skill、再清理旧托管 MDC。
Grok 默认会扫描 Cursor Skills，候选 frontmatter 已设计为对 Cursor 常驻、对 Grok 不可调用。

## 待办：daemon 二进制变化检测 —— 轮询 vs filewatch（后续评估，优先级低）

二进制变化检测目前是 **15s 轮询** `current_exe()` 指纹（稳态≈1 次 `stat`，靠 `binhash.json` 内容哈希缓存避免重哈希）。
是否改 **filewatch** 待权衡——难点：二进制走原子替换（rename 换 inode，需盯父目录 + 按文件名过滤 + 每次替换后重挂，
参考 `config_watch.rs`）、装在任意目录（`~/.local/bin`/brew/npm 前缀/`.app` bundle…）、且 watcher 仍要 stat/hash 才能确认
内容**真**变（指纹是内容哈希而非 mtime）。延迟要求松（~15s 够）+ Hello 路径兜底，故暂保持轮询。
