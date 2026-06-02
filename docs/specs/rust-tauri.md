# 需求：HumanInLoop 迁移到 Rust + Tauri（跨平台）

## 背景

- 当前 `HumanInLoop` 是 **Swift Native、仅 macOS** 的实现（单一 `AskHuman` 二进制，AppKit 控制生命周期 + SwiftUI 界面），见 `docs/specs/swift-native.md`。
- 目标：用 **Rust + Tauri 2.x** 重写，使其能在 **macOS / Windows / Linux** 运行。
- 隔壁 `../HumanInLoop-Rust` 是当前 Swift 版的「原版」Tauri(Rust + Vue) 工程，**仅作参考，不照搬**：它功能冗余（MCP、音效、字体、快捷键、自动更新、自定义提示词等）、编译慢。
- 本次只把 **当前 Swift 版已有的功能** 迁移到 Tauri，保持工程精简。

## 总体目标

实现 **单一二进制 `AskHuman`**，跨平台，功能与当前 Swift 版**对齐**：

1. CLI 提问能力（沿用当前命令格式与 stdout 输出格式）
2. 本地原生弹窗交互（Tauri WebView 窗口）
3. 设置界面（3 个 Tab）
4. 可扩展的「通信 Channel」抽象：本地弹窗 + Telegram，多开并行抢答
5. Cursor Hook 安装/移除（mac/Linux）
6. 观感尽量贴近 macOS 原生

## 功能范围（与当前 Swift 版对齐）

### 1. CLI 命令

| 调用形式 | 行为 |
| --- | --- |
| `AskHuman <message> [-o <option> ...] [--no-markdown]` | 提问：通过启用的 Channel 发起询问，结果写入 stdout |
| `AskHuman --settings` | 启动设置界面 |
| `AskHuman --help` / `-h` | 显示帮助 |
| `AskHuman --version` / `-v` | 显示版本 |
| `AskHuman`（无参数） | stderr 报错 `错误: 缺少提问内容`，打印帮助，退出码 1 |

- `<message>` 位置参数，必填，仅允许一个
- `-o` / `--option` 可重复，追加预定义选项；缺参报错
- `--no-markdown` 关闭 Markdown，默认开启
- 第一个 token 以 `-` 开头但非已知 flag → 报错
- 输出区块：`[选择的选项]` / `[用户输入]` / `[图片]`（仅非空输出，区块间空行）；取消时 `[状态]`；三块皆空但发送时保底 `[用户输入]\n用户确认继续`
- 退出码：成功/取消 = 0，异常 = 1
- 图片落盘到 `temp_dir/humaninloop/<request_id>/`，不主动清理

### 2. 通信 Channel（核心抽象）

- 每个 Channel 可在设置中独立开关（本地弹窗 / Telegram）
- 一次提问**并行**发起所有已启用 Channel
- **任一端先给出最终回答（发送/取消）即采用，其余自动关闭**
- 架构可扩展，便于未来新增 Channel
- 没有任何 Channel 启用时，兜底强制启用本地弹窗

### 3. 本地弹窗 Channel

- 展示提问内容（Markdown 渲染；`--no-markdown` 时纯文本）
- 预定义选项多选
- 自由文本输入
- 图片附件：粘贴 / 拖拽 / 选择文件；缩略图预览 + 删除
- 「发送」与「取消」；关闭窗口视为取消
- 置顶（来自 General 设置）
- 窗口尺寸：默认取配置；开启「记住窗口尺寸」时，用户拉伸后持久化

### 4. Telegram Channel（与当前 Swift 版一致）

- 发送提问消息（Markdown 时用 MarkdownV2 转义；预定义选项作为 inline 按钮，可点选切换，✅ 反映选中态）
- 发送操作消息（reply 键盘含「↗️发送」按钮）
- 长轮询接收：选项切换、文本回复、点击「发送」
- **不接收图片**
- 支持自定义 API Base URL（代理）
- 设置中可「测试连接」
- Chat ID 仅支持数字，`@username` 不支持

### 5. 设置界面（3 个 Tab）

- **General**：主题（跟随系统 / 浅色 / 深色）、窗口置顶
- **集成（Integration）**：参考提示词（展示 + 复制）、Cursor Hook（状态 / 安装 / 移除 / 打开 hooks.json）
- **Channel**：本地弹窗设置（启用、记住尺寸、默认尺寸）、Telegram 设置（启用、Bot Token、Chat ID、API Base URL、测试连接）、未来扩展占位

### 6. Cursor Hook

- 安装：写入 hook 脚本，在 `~/.cursor/hooks.json` 的 `preToolUse` 注册 `matcher = "Shell"` 的条目
- 作用：检测 Shell 工具调用 `AskHuman` 时，把 timeout 提升到 24 小时（86400000ms），否则返回 `{}`
- 识别依据：条目 `command` 含 `humaninloop-timeout.sh`
- 移除：仅删本应用注入的条目，并删除脚本本身；保留其他 hook
- 状态查询 + 在系统文件管理器中定位 `hooks.json`
- **仅 mac/Linux；Windows 上安装/移除禁用并提示**

### 7. 配置

- 路径：`~/.humaninloop/config.json`（所有平台一致）
- 内容：General（主题、置顶）、各 Channel 启用状态与参数

## 明确不做（相比原 Rust 版）

- 不实现 MCP 服务器二进制（`HumanInLoop`）
- 不实现音效、字体设置、快捷键设置、自定义提示词/快捷回复、继续回复、自动更新、托盘图标
- 不保留「Markdown 原生 / WebView 切换」设置项（Tauri 下统一前端渲染，该设置失去意义）

## 技术约束 / 决策记录（与用户确认）

1. **路线**：在**当前仓库**从零新建精简 Tauri 工程；参考但不照搬 `../HumanInLoop-Rust`；只对齐当前 Swift 功能。
2. **前端栈**：Vue 3 + Vite + TypeScript + **手写 macOS 风 CSS**（不引入 UnoCSS / 组件库 / 桌面模拟器库）。
3. **运行模型**：单进程单二进制 `AskHuman`，按 argv 决定「弹窗 / 设置」模式；Rust 侧并行运行 Telegram，沿用「多 Channel 抢答」；Tauri / WebView 日志走 stderr，保证 stdout 干净。
4. **Markdown**：前端用 markdown-it 渲染；`--no-markdown` 时按纯文本。
5. **Telegram**：用 `reqwest` 手写 Bot API（不引 teloxide），行为对齐当前（不收图片）。
6. **配置**：仍用 `~/.humaninloop/config.json`，schema 与现版基本一致，**去掉 `markdownRenderer` 字段**。
7. **Cursor Hook**：mac/Linux 沿用 bash 脚本注入 hooks.json；**Windows 禁用并提示**。
8. **分发**：`cargo tauri build --no-bundle` 产裸二进制，`install.sh` 装到 `~/.local/bin/AskHuman`（mac/Linux）；Windows 产 `.exe`，手动加入 PATH。
9. **平台验证**：代码三平台通用；本地只验证 macOS 跑通，Win/Linux 靠 GitHub Actions + 文档。
10. **UI 风格**：手写 macOS 风 CSS——系统字体栈、系统强调色 / focus 光晕、macOS 控件度量；macOS 上用 Tauri `windowEffects` 提供原生毛玻璃，Win/Linux 退化为不透明背景。
11. **代码组织**：新 Tauri 工程与现有 Swift 代码**暂时并存**于根目录；待 Tauri 功能对齐并验证后再删除 Swift 代码（`Package.swift` / `Sources/` / `Tests/`）。

## 反馈意见（计划评审阶段补充）

- **编译性能**：先按计划实现，**完成后再专门分析编译性能**，依据实测再决定是否引入更激进的提速措施（如 `ureq` + `std::thread` 替代 `tokio` + `reqwest`、`sccache` / `mold`、精简 features 等）。详见计划 §21。

## 反馈意见（Step 8 样式打磨阶段补充）

以下为开发到样式打磨阶段时，用户当面验收并逐项确认的 UI 决策（已实现）：

- **毛玻璃材质**：macOS 用 `underWindowBackground`（vibrancy）；弹窗与设置窗口均启用；窗口透明、底色交给系统材质（移除原纯色底）。Win/Linux 退化为纯色不透明。
- **标题栏**：macOS 用 `TitleBarStyle::Overlay` + `hidden_title`，使毛玻璃材质铺满整窗（含标题栏区域），并隐藏标题文字、保留红绿灯按钮。
- **拖拽**：Overlay 下 webview 盖住原生标题栏，故用 `data-tauri-drag-region` 自定义拖拽区——弹窗顶部导航栏、底部操作栏空白处均可拖动窗口（按钮/输入正常交互）。
- **毛玻璃色罩**：在材质上叠一层轻色罩平衡通透与可读性，深色 `rgba(0,0,0,0.2)`、浅色 `rgba(255,255,255,0.2)`。
- **Markdown 配色**：前端 `markdown-it` 渲染配色对齐 Swift `HTMLMarkdownRenderer`（深色链接 `#4ea1ff`、代码块底 `rgba(255,255,255,0.12)`/`rgba(0,0,0,0.06)`、表头底色、引用左边框、`hr`、删除线、图片自适应等）。
- **弹窗顶部导航栏（新增，原计划 §8.2 未含）**：左侧绿色状态点 + 标题「Question from the Loop」；右侧三枚图标按钮——置顶切换（开启高亮）、主题循环（系统→浅色→深色，实时生效并写回配置）、打开设置窗口（同进程内创建）。参考原 Rust 版导航栏样式并美化。
  - 配套后端：`popup_init` 增加 `alwaysOnTop`；新增命令 `update_theme`（持久化+应用主题）、`open_settings`（同进程创建设置窗口）；新增权限 `core:window:allow-start-dragging`、`core:window:allow-set-always-on-top`。

## 参考资料

- 当前 Swift 版需求与计划：`docs/specs/swift-native.md`、`docs/plans/swift-native.md`
- 原 Rust/Tauri 工程（仅参考）：`../HumanInLoop-Rust`
- 本次开发计划：`docs/plans/rust-tauri.md`
