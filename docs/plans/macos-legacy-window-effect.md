# 修复计划：macOS 窗口材质分层与旧系统模糊修复

> 状态：已实施；macOS 10.15.7 Intel 旧系统路径已实机验收通过，macOS 15 精确版本仍建议发布前回归。
> 调查与方案决策于 2026-07-14 确认。
> 关联实现：`src-tauri/src/app/mod.rs` 的 `apply_surface` / `apply_liquid_glass` /
> `set_runtime_window_effect` / `finalize_popup_show`，以及 `src/views/popup/usePopupCore.ts` 的预热领用流程。

## 0. 结论与已确认决策

macOS 15 支持 `NSVisualEffectView`，因此问题不是系统缺少模糊能力，而是当前实现在
macOS 26 以下同时使用了两套 `NSVisualEffectView` 管理方式：

- Tauri `UnderWindowBackground` 窗口效果；
- `tauri-plugin-liquid-glass` 在旧系统的 `NSVisualEffectView` fallback（带自己的按窗口
  label registry）。

修复采用以下已确认方向：

1. **macOS 26 以下统一走 Tauri 原生 blur**，不再进入 Liquid Glass 插件的旧系统
   fallback/registry。
2. **窗口效果应用必须幂等**：初建、预热上屏、配置广播和运行时重复调用都不能
   移除当前有效材质后留下透明窗口。
3. **消除预热弹窗重复 finalize**，同时保留后端幂等防御，避免重复 `show()`、
   提示音、Dock 角标和效果切换。
4. **不再静默吞掉效果应用错误**：Liquid Glass 失败时记录窗口与效果上下文，
   并按 Glass → Blur → Solid 自动降级，保证窗口不会以“透明 + 无模糊”上屏。
5. **新增完全不透明的“纯色 / Solid”材质**，持久化值为 `solid`，不创建任何
   `NSVisualEffectView` / `NSGlassEffectView`，并对当前全部 WebView 窗口立即生效。
6. **材质选项按系统能力展示**：macOS 11–25 显示 Solid + Blur，macOS 26+ 显示
   Solid + Blur + Glass；两类系统的默认可见效果分别为 Blur 与 Glass。
7. **设置项从“窗口效果 / Window effect”更名为“窗口材质 / Window material”**，
   按钮顺序固定为 Solid → Blur → Glass（旧系统隐藏 Glass 后为 Solid → Blur）。

## 1. 现象与根因

### 1.1 用户可见现象

窗口本身仍为透明，前端仍叠加 `--vibrancy-tint` 的 40% 白/黑色罩层，但后方文字与
图形没有被模糊，直接与弹窗内容叠在一起。这说明 WebView/CSS 透明链路仍在，丢失的是
后方的原生材质视图。

该现象与 macOS“降低透明度”的预期不同：后者会把部分透明背景改为实色，
不会保留“后方内容清晰透出”的效果。

### 1.2 旧 macOS 上的非幂等材质切换

当请求效果为默认的 `WindowEffect::Glass` 时，当前运行时路径是：

1. `remove_vibrancy_views()` 从 content view 下删除所有 `NSVisualEffectView`；
2. `apply_liquid_glass()` 调用插件 `set_effect()`。

在 macOS 26+ 上，插件管理的是 `NSGlassEffectView`，第 1 步不会删除它。在 macOS 26
以下，插件 fallback 本身就是 `NSVisualEffectView`，因此第 1 步会把插件的当前视图
从窗口层级移除。

移除操作没有同步清理插件 registry。紧接着的 `set_effect()` 看到同 label 条目仍在，
只更新已经脱离窗口层级的旧视图，不再创建/挂回新视图，最终留下无材质的
透明窗口。

### 1.3 重复应用的正常触发源

同一效果被重复应用不是非法调用，现有流程本身会产生：

- 弹窗 resize 会记忆尺寸并写入 `config.json`；daemon 去抖后向活动 helper 广播完整
  `general`，其中包含未改变的 `windowEffect` 值。
- 弹窗开着时保存任何其它设置，也会触发同样的全量 `general` 广播。
- 预热窗口领用时，`popup-show` 事件触发的 `adopt()` 与首次 `popup_init()` 存在同时
  看到已领用请求的时序窗口，两路都可能进入 `renderInit()`，随后各调一次
  `popup_show_window()`。

因此实现必须把“同效果重复调用”当作正常输入，不能依赖调用方永远只调一次。

### 1.4 错误不可见

`apply_liquid_glass()`、运行时 `set_effect()`/`set_effects()` 以及清理路径的多处结果被
`let _ = ...` 忽略。即使初次创建原生视图就失败，弹窗也会继续显示且没有可用的
错误线索，外观与 registry 问题相同。

## 2. 目标与非目标

### 2.1 目标

- macOS 11–25 上，无论配置中请求 `glass` 还是 `blur`，实际都使用 Tauri
  `UnderWindowBackground` blur，不调用 Liquid Glass fallback。
- `solid` 在所有支持的 macOS 版本上使用完全不透明的主题底色，不创建任何
  Visual Effects 材质视图。
- macOS 11–25 的设置页显示 Solid + Blur；macOS 26+ 显示 Solid + Blur + Glass。
- 旧配置/默认值 `glass` 在 macOS 11–25 上以 Blur 作为有效值和 UI 高亮，不强制
  迁移配置；用户主动选择 Blur/Solid 后持久化用户所选值。
- macOS 26+ 上保持 Liquid Glass / Blur，并新增 Solid。
- 材质切换对 popup、settings、history、agents、interject 等全部 WebView 窗口立即生效，
  并在重启后保持。
- 同一窗口反复收到相同效果、主题或配置广播时，结果保持不变。
- 预热弹窗一个请求最多执行一次 finalize/show 副作用。
- 原生效果设置失败时有日志与可读上下文；Glass 失败回退 Blur，Blur 也失败则
  回退 Solid，不直接显示“透明 + 无模糊”窗口。

### 2.2 非目标

- 不改变 `glass` / `blur` 旧序列化值的语义或迁移旧配置；只增加向后兼容的
  `solid` 值。
- 不修改 Windows/Linux 的不透明背景策略。
- 不改变预热模型、daemon 协议、弹窗性能目标或作答语义。
- 不尝试覆盖 macOS“降低透明度”等用户的辅助功能选择。

## 3. 设计

### 3.1 三态配置与选项矩阵

Rust 配置与 TypeScript 类型同步扩展为三态：

```text
WindowEffect = Glass | Blur | Solid
JSON / TypeScript = "glass" | "blur" | "solid"
```

`WindowEffect::Glass` 仍为平台无关配置的 serde default：它在 macOS 26+ 的有效值为
Glass，在 macOS 11–25 的有效值为 Blur。这使同一份默认配置自然得到已确认的
“新系统默认 Glass，旧系统默认 Blur”，无需平台特化默认值或启动时改写配置。

设置页材质选项矩阵：

| 能力 | 显示选项 | 配置为 `glass` 时的高亮 |
|---|---|---|
| macOS 11–25 / Glass unsupported | Solid、Blur | Blur |
| macOS 26+ / Glass supported | Solid、Blur、Glass | Glass |
| Windows/Linux | 不显示 | 不适用 |

将“持久化 requested effect”与“UI 展示 effective effect”分开。旧 macOS 读到 `glass` 时
高亮 Blur，但不在仅打开/保存其它设置时暗中将配置改成 `blur`；只有用户主动
点击 Blur/Solid 才写入对应值。

`GeneralTab.vue` 不再用 `glassSupported` 决定整个材质设置是否存在；它只决定是否
渲染排在最后的 Glass 按钮。设置项与搜索文案改为“窗口材质 / Window material”，
在所有 macOS 上都可搜索；i18n 新增 `effectSolid: "纯色" / "Solid"`。

### 3.2 区分“请求效果”与“有效效果”

保留并扩展配置模型 `WindowEffect::{Glass, Blur, Solid}`，新增单一解析入口：

```text
resolve_window_effect(requested, glass_supported) -> effective

requested=Glass, glass_supported=true  => Glass
requested=Glass, glass_supported=false => Blur
requested=Blur,  glass_supported=*     => Blur
requested=Solid, glass_supported=*     => Solid
```

运行时能力判定与插件保持同一语义：只有 `NSGlassEffectView` 存在时才视为支持
Liquid Glass。将纯函数的布尔输入与 AppKit class lookup 分开，便于无 macOS 15 runner 时
仍能单测完整决策表。

初建与运行时两条路径必须共用该解析结果，禁止一条路径认为是 Glass、另一条
路径认为是 Blur。

### 3.3 原生材质与前端底色的责任分层

Solid 必须是“完全不透明”，不能只删除 Visual Effects：现有 macOS 窗口与 WKWebView
在构建时都开启了透明，而 `html.vibrancy body` 只有 40% 主题色罩层。如果仅移除
`NSVisualEffectView`，就会再次产生本次报告的清晰透出。

将当前 `.vibrancy` 同时承担的两种语义拆开：

- **macOS 窗口布局类**：只负责 overlay titlebar 下的顶部 padding/红绿灯避让，不随材质
  切换而消失。
- **translucent 材质类**：Glass/Blur 使用透明 HTML 背景 + `--vibrancy-tint`。
- **solid 材质类**：`html/body/#app` 使用完整的 `var(--bg)` 主题底色，不叠加
  `--vibrancy-tint`，不使用 CSS `backdrop-filter`。

初始页面在第一帧前必须知道 effective effect，避免 Solid 窗口先以半透明状态闪现。
实现可将 effective effect 放入各窗口初始 URL 的内部 query（例如 `effect=solid`），由
`index.html` 内联脚本在外部 CSS/Vue 加载前设置正确根类。该 query 为内部建窗参数，
不是用户或 CLI 契约。

运行时切换时，Rust 在完成原生层变更后向每个窗口发送统一的
`window-effect-changed` 事件（payload 为 effective effect），由 `main.ts`/共用 theme helper 统一切换
translucent/solid 根类。不把类切换散落到五个 view 各自的 `settings-updated` 监听中。

原生窗口层同时将 `NSWindow.isOpaque` 设为 true，并使用 Tauri/AppKit 窗口背景色设置
当前深/浅主题实色安全底。但 macOS WKWebView 运行时背景实现不足以单独保证
内容区完全不透明，因此它不取代前端 solid 实色底。切回 Glass/Blur 时，先将
`NSWindow.isOpaque` 恢复 false、原生底色恢复 clear，前端同步恢复 translucent 类。

### 3.4 初始建窗

`apply_surface` 改为只接收已解析的 effective effect：

- effective Blur：构建期直接挂 Tauri `EffectsBuilder` + `UnderWindowBackground`；
- effective Glass：构建透明窗口，build 后由插件挂 `NSGlassEffectView`。
- effective Solid：不挂任何 Effects，设置当前主题的原生窗口底色，并让页面在首帧
  使用 solid 实色根类。

所有建窗入口统一先计算 effective effect，再交给 `apply_surface`，且只在 effective Glass
时调用插件：

- popup cold helper；
- popup warm helper 领用上屏；
- settings / history / agents / interject 等托管窗口。

这保证 macOS 26 以下的默认/旧 `glass` 配置从窗口诞生起就只有 Tauri blur，
不会先建插件 fallback 后再切换；Solid 则从首帧起就是完全实色。

### 3.5 运行时切换与幂等性

将 AppKit 视图操作收口到三个内部动作：

- `apply_native_blur(window)`：
  1. 关闭并清理插件效果/registry（已无条目时为 no-op）；
  2. 恢复 `NSWindow.isOpaque=false` 与 clear 原生底色；
  3. 移除旧的 direct-child `NSVisualEffectView`，避免多层叠加；
  4. 通过 Tauri `set_effects(UnderWindowBackground)` 挂一个新模糊层。
- `apply_native_glass(window)`：
  1. 只在能力判定支持 Glass 时进入；
  2. 恢复 `NSWindow.isOpaque=false` 与 clear 原生底色；
  3. 移除可能残留的 Tauri `NSVisualEffectView`；
  4. 创建或更新插件 `NSGlassEffectView`。
- `apply_solid(window, theme)`：
  1. 关闭并清理插件效果/registry；
  2. 移除 direct-child `NSVisualEffectView`；
  3. 将 `NSWindow.isOpaque=true`，并把原生窗口底色设为当前主题实色；
  4. 向该窗口发送 effective `solid` 事件，让 WebView 用完整实色覆盖内容区。

`set_runtime_window_effect(window, requested)` 先解析 effective effect，再调上述三个动作。
因为旧 macOS 的 Glass 请求会先解析为 Blur，所以不再存在“手工移除插件 fallback，
但保留插件 registry”的分支。

从 Solid 切回 Glass/Blur 时，在挂原生材质前恢复 clear 窗口底色，然后把 effective
effect 发给前端恢复 translucent 类。这两步不能缺任一，否则会出现“材质已挂但被
实色 WebView 遮住”的反向故障。

配置广播可继续携带完整 `general`，不需要为视觉 bug 改动 daemon 协议；运行时应用
本身必须承受重复调用。

主题切换时，如果当前 effective effect 为 Solid，`apply_theme_to_windows` 路径除更新
`NSAppearance` 外还要刷新 Solid 的原生实色底；前端 `applyTheme` 同步让 `var(--bg)` 跟随。
Glass/Blur 下不设实色底，仅让材质跟随新 appearance。

### 3.6 预热弹窗 finalize 一次性

两层同时处理：

1. **前端单次 render guard**：统一首次 `popup_init()` 与 `adopt()` 的领用判定，一旦任一
   路径已将带 interaction 的 init 交给 `renderInit()`，另一路径必须 no-op。
2. **后端 finalize guard**：在预热进程状态中记录是否已上屏，`popup_show_window()` 只允许
   首次调用执行 size/effect/show/sound/focus/Dock 副作用，后续调用直接返回。

后端 guard 是正确性边界，前端 guard 用于从源头消除重复渲染和不必要 IPC。不以材质
幂等性代替 finalize 的单次性。

### 3.7 错误处理与回退

- `apply_liquid_glass` 改为返回真实 `Result`，调用方不再用 `let _` 静默吞掉。
- 日志至少包含：window label、requested/effective effect、操作阶段（create/runtime/finalize/
  cleanup）和底层错误。使用项目现有 stderr/log 路径，不污染 CLI stdout 契约。
- effective Glass 应用失败时，立即调用 `apply_native_blur()`；回退成功后弹窗继续显示，
  不把视觉增强故障升级为整个提问失败。
- Blur 应用也失败时记录第二条错误，不误报 Glass/Blur 已成功，并立即调用
  `apply_solid()` 作为终极可读降级。
- Solid 不依赖 Visual Effects；其原生底色设置失败时仍必须把前端切到完整实色底，
  并记录错误，不回到透明类。
- 正常的“旧 macOS 将 Glass 解析为 Blur”不记为错误。

## 4. 影响文件

### 4.1 Rust

- `src-tauri/src/app/mod.rs`
  - 新增效果能力/解析 helper；
  - 统一初建、运行时和预热上屏的 effective effect；
  - 拆分并硬化 `apply_native_blur` / `apply_native_glass` / `apply_solid`；
  - 处理 Liquid Glass 错误与 Glass → Blur → Solid 回退；
  - 为各窗口初始 URL 携带 effective effect，切换时向每个窗口发事件；
  - 在 Solid/透明材质间切换原生窗口底色；
  - 为 `finalize_popup_show` 加单次 guard；
  - 调整所有建窗调用点。
- `src-tauri/src/config.rs`
  - `WindowEffect` 增加 `Solid` / serde `"solid"`；
  - 保持 `Glass` 为 serde default，由能力解析决定平台默认可见效果；
  - 增加旧配置与三态往返单测。
- `src-tauri/src/commands.rs`
  - 主题切换时刷新 Solid 的原生深/浅底色；
  - 保持 `apply_window_effect` 的对外命令名与参数兼容，内部支持新 `solid` 值。
- `src-tauri/src/macos_window_anim.rs`
  - 若需要，将 `remove_vibrancy_views` 的语义收窄为“清理 Tauri blur 前景层”；
  - 保持所有 AppKit 视图变更只在主线程执行。
- 可能的小范围状态变更：`WarmPopup` 增加原子/互斥的 finalized 标志。

### 4.2 前端

- `src/lib/types.ts`
  - `WindowEffect` 增加 `"solid"`；
  - popup/history/agents/interject 等 init payload 如采用事件 + URL 以外的显式初值，
    保持类型一致。
- `src/index.html`、`src/styles/base.css`、`src/styles/controls.css` 与各 view 的头部样式：
  - 在首帧前根据平台 + effective effect 设置布局/材质根类；
  - 把 `.vibrancy` 的 macOS titlebar 布局语义与 translucent 底色语义拆开；
  - Solid 使用完整 `var(--bg)`，Glass/Blur 继续使用 `--vibrancy-tint`。
- `src/lib/theme.ts`、`src/main.ts`
  - 增加统一的 effective effect 根类应用 helper；
  - 在 Vue 视图挂载前完成初值，并统一监听 `window-effect-changed`。
- `src/views/settings/GeneralTab.vue`、`useGeneralSettings.ts`、`useSearch.ts`、`src/i18n/*`
  - 实现 macOS 11–25 的 Solid → Blur / macOS 26+ 的 Solid → Blur → Glass 选项矩阵；
  - 旧 macOS 的 requested `glass` 以 Blur 高亮；
  - 设置项更名为“窗口材质 / Window material”，新增“纯色 / Solid”文案，并在
    所有 macOS 上可搜索。
- `src/views/popup/usePopupCore.ts`
  - 将预热请求渲染收口到单一 one-shot guard；
  - 覆盖 `popup-show` 事件与首次 `popup_init` 并发返回 interaction 的竞态。

### 4.3 测试与文档

- 在 `app/mod.rs` 现有 test module 或就近纯函数单测中覆盖三态 effect resolution matrix。
- 为前端根类 helper 增加纯函数/DOM 单测，确认 Solid 与 translucent 互斥且不移除
  macOS titlebar 布局类。
- 视前端可测性，为 one-shot init guard 增加小型单测；若 Vue/Tauri IPC 边界使测试
  必须过度 mock，则保留后端 guard 为必测/必验边界，不为测试强行重构业务组件。
- 实施后更新 `docs/overview.md` 的 UI/主题不变式：macOS 11–25 可选 Solid/Blur，macOS 26+
  可选 Solid/Blur/Glass，Solid 不使用 Visual Effects。
- 更新 `docs/wiki/settings.md` / `settings.en.md` 中已失真的“窗口效果仅 macOS 26+”说明。
- 如果代码落地后未改变仓库级地图/不变式，除上述已失真句子外不扩写主 overview。

## 5. 实施顺序

1. 扩展 Rust/TypeScript `WindowEffect` 为 Glass/Blur/Solid，补序列化与旧配置单测。
2. 抽出可单测的 requested + capability → effective effect 解析，补齐四组决策表单测。
3. 拆分 macOS titlebar 布局类与 translucent/solid 材质类，打通初始 URL 与运行时
   `window-effect-changed` 事件，保证 Solid 首帧即实色。
4. 让所有初始建窗路径使用 effective effect；确保 macOS 26 以下不调用插件
   `set_effect(enabled=true)`。
5. 重构运行时效果应用为 `apply_native_blur` / `apply_native_glass` / `apply_solid`，使同值重复
   调用安全，且 Solid ↔ translucent 双向切换不残留遮挡层。
6. 改造错误处理：保留底层 `Result`、增加结构化上下文日志、落实 Glass → Blur →
   Solid 降级链。
7. 在 `usePopupCore.ts` 加 interaction render one-shot guard，同时在 `finalize_popup_show` 加后端
   one-shot guard。
8. 实现设置页两/三选项矩阵、effective 高亮、Solid 文案与搜索。
9. 执行自动测试与静态检查，再按 macOS 真机矩阵验证。
10. 按实现结果更新失真的 overview/wiki 句子；本次经用户明确确认不写 `docs/PROGRESS.md`。
11. 功能逻辑改动后必须运行 `./scripts/install.sh`，然后使用新安装的 `AskHuman` 完成
   最终人工验证。

## 6. 验证计划

### 6.1 自动化

- Rust 单测：
  - Glass + supported → Glass；
  - Glass + unsupported → Blur；
  - Blur + supported/unsupported → Blur；
  - Solid + supported/unsupported → Solid；
  - `"solid"` 序列化往返，且无字段旧配置仍以 requested Glass 为默认；
  - 若 finalize guard 抽成可测状态，验证只有首次 transition 成功。
- 前端单测（如增加）：模拟“首次 init 与 popup-show adopt 都返回 interaction”，
  断言 `popupShowWindow` 只调用一次。
- 前端材质单测：
  - macOS + Solid → 保留 titlebar 布局类，只启用 solid 底色类；
  - macOS + Glass/Blur → 启用 translucent 类、移除 solid 类；
  - Solid ↔ Blur/Glass 反复切换时两个材质类互斥；
  - unsupported 下 requested Glass 的设置按钮高亮 Blur。
- 项目回归：
  - `pnpm test`；
  - `pnpm build`；
  - `cargo test --manifest-path src-tauri/Cargo.toml`；
  - `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`（与当时仓库的 MSRV/工具链一致）。

### 6.2 macOS 15（核心验收）

使用新配置或删除 `general.windowEffect` 后让其回到默认 Glass 请求：

1. 预热开：等热 helper 就绪后连续弹出至少 5 次，每次背景均被模糊，无清晰叠字。
2. 预热关：验证 cold helper 也使用同样 blur。
3. 弹窗开着时调整窗口尺寸，等待超过 config watcher 的 300ms 去抖，模糊不消失。
4. 弹窗开着时保存一项与窗口效果无关的设置，模糊不消失。
5. 快速重复触发预热领用边界，每个弹窗只播放一次提示音、只执行一次上屏。
6. 开启 macOS“降低透明度”时，接受系统将材质改为实色；关闭后 blur 恢复。
7. 设置页以 Solid → Blur 顺序显示且只显示这两项；旧/默认 requested Glass 高亮 Blur。
8. 切到 Solid：当前所有已打开窗口立即变为完全实色，后方高对比文字/图案不可见；
   使用 view hierarchy 检查或临时调试断言 direct child 中无 `NSVisualEffectView` /
   `NSGlassEffectView`。
9. Solid 下切换 light/dark/system 主题，底色立即更新且无透明闪现；重启后仍为 Solid。
10. Solid ↔ Blur 反复切换，无实色遮住 blur、无清晰透出，titlebar 顶部间距与红绿灯避让
    不跳变。
11. 在 Apple Silicon 真机必验；若可获得 Intel macOS 15 环境，再做同样的 cold/hot +
   resize 冒烟，但不阻塞 Apple Silicon 上的修复交付。

### 6.3 macOS 26+

1. 设置页按 Solid → Blur → Glass 显示三个选项，默认 Glass 仍为 `NSGlassEffectView`，视觉与
   修复前一致。
2. 在设置中反复 Glass ↔ Blur ↔ Solid 切换，无崩溃、透明空窗、实色遮住材质、材质叠层
   或深浅主题错位。
3. Glass、Blur、Solid 三种状态下分别调整尺寸、保存其它设置，效果保持。
4. 设置/历史/Agent/Interject 窗口关闭重开后材质仍在，插件 registry 清理不回归。
5. 使用可控的测试注入或临时本地仪器化模拟 Glass `set_effect()` 失败，确认日志包含
   上下文且窗口自动回退 Blur；不为测试保留用户可见的故障开关。
6. 模拟 Blur 应用失败，确认窗口自动进入 Solid，前端实色底仍生效。

### 6.4 其它平台与契约回归

- Windows/Linux 窗口仍使用当前不透明背景，不引入 Glass/Blur 分支行为。
- CLI stdout 只有结果区块；新增材质错误只进 stderr/日志。
- 提交、取消、抢答、预热回退、窗口聚焦和提示音语义不变。

## 7. 验收标准

1. macOS 15 上默认配置的 cold/hot 弹窗都有背景模糊，后方文字不会清晰叠入。
2. resize 和任意 `general` 配置广播后，当前窗口的模糊不消失。
3. macOS 11–25 按 Solid/Blur 显示，macOS 26+ 按 Solid/Blur/Glass 显示；默认高亮和有效材质正确。
4. Solid 对全部已打开和后续新建窗口立即生效、重启保持、完全不透明，且不包含
   direct-child `NSVisualEffectView` / `NSGlassEffectView`。
5. macOS 26+ Solid/Blur/Glass 三模式不回归，切换、主题和窗口重开均正常。
6. 一个预热请求只 finalize/show 一次。
7. 旧 macOS 正常路径不创建插件 `NSVisualEffectView` fallback；应用失败按 Glass → Blur →
   Solid 降级并记录日志。
8. 自动测试、前端 build、Rust test/clippy、`./scripts/install.sh` 和新安装 `AskHuman` 的
   真机验证全部通过。

## 8. 风险与控制

- **能力判定不一致**：本地判定若与插件 `is_supported()` 语义漂移，可能走错材质。
  控制：使用同一 `NSGlassEffectView` class availability 标准，集中为单一 helper。
- **短暂闪烁**：运行时 Blur 重画需要先清旧层再挂新层。控制：全部操作在主线程的单次
  闭包内完成，真机快速重复广播观察无闪烁。
- **Solid 不够“实”**：只设原生窗口底色或只设 CSS 都可能在首帧/切换边界泄出
  后方内容。控制：初始 URL 首帧类 + 前端完整 `var(--bg)` + 原生窗口底色三层同时落地，
  用高对比背景做真机验收。
- **切回透明材质被遮挡**：Solid 的 WebView 实色类若未清理，原生 Glass/Blur 会已挂载但
  不可见。控制：原生层和前端根类由单一 effective effect 事件驱动，把 Solid ↔ Blur/Glass
  双向反复切换纳入必验矩阵。
- **titlebar 布局回归**：现有 `.vibrancy` 同时控制顶部间距，直接移除会使 Solid 下内容
  与红绿灯重叠。控制：先拆分平台布局类和材质类，验证三态下标题栏几何不变。
- **清理范围过宽**：`remove_vibrancy_views()` 按 class 删除 direct child。控制：旧 macOS 不再让
  插件创建同 class fallback，并保持 direct-child 边界，不递归删除 WebKit 内部视图。
- **错误回退递归**：Glass 失败转 Blur、Blur 失败转 Solid 时不得再进入顶层解析。控制：
  降级链直接调下一层具体 action，不递归调 `set_runtime_window_effect`。
- **预热 guard 误拦截**：每个预热 helper 是一次性进程，因此 finalized 不需要为下一请求
  重置。单进程回退路径不走 `popup_show_window`，不受该 guard 影响。

## 9. 提交与交付

建议实现作为一个用户可见修复提交，Conventional Commit 主题示例：

```text
fix(popup): add solid material and restore legacy macOS blur
```

发布说明应分两个用户价值表达：“新增完全不透明的纯色窗口材质”与“修复
macOS 26 以下弹窗背景可能变成清晰透明”，而不对用户暴露插件 registry 等实现细节。
