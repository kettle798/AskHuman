# macOS 状态栏菜单首次 hover 子菜单自动关闭

> 调查日期：2026-07-09

## 现象

每次 install（或首次启动）后，点击状态栏图标打开菜单，鼠标 hover 到子菜单（如 "Agent 状态"）时，
**菜单在子菜单展开的瞬间自动关闭**。关闭后再次操作则正常，只影响每次启动后的第一次。

## 根因

### 直接原因

`tao` 的 `AppState::launched()` 在 Tauri setup hook **之前**执行以下序列：

```
setActivationPolicy(.regular)          ← tao 默认策略
activateIgnoringOtherApps(true)        ← 激活应用
```

对于无窗口的状态栏应用（如 AskHuman gui-host），这创建了一个异常的激活状态：
应用以 `.regular`（有 Dock 图标级别的常规 app）身份被激活，但没有任何窗口。

macOS 在首次展开子菜单时（需要创建子菜单窗口），检测到这个异常状态，**隐式关闭整个菜单**。
不调用 `cancelTracking` 或 `cancelTrackingWithoutAnimation`，是 macOS 内部行为。

Tauri setup hook 随后把策略切到 `.accessory`，但 `activateIgnoringOtherApps(true)` 的副作用
已经留下了。

### 时序细节

```
tao EventLoop::run()
  └─ [NSApp run]
       └─ applicationDidFinishLaunching
            └─ AppState::launched()
                 ├─ apply_activation_policy → .regular（默认）
                 ├─ activateIgnoringOtherApps(true)     ← 这里造成问题
                 ├─ waker.start()
                 └─ dispatch Event::NewEvents(Init)
                      └─ Tauri setup hook
                           └─ set_activation_policy(.accessory)  ← 太晚了
```

代码位置：
- `tao-0.35.3/src/platform_impl/macos/app_state.rs:284` `AppState::launched()`
- `tao-0.35.3/src/platform_impl/macos/app_state.rs:293` `activateIgnoringOtherApps(ignore)`

### 为什么只影响首次

首次菜单关闭后，应用的激活状态被 macOS 重置（变为非活跃）。
后续操作时应用不再处于异常激活状态，菜单正常工作。

## 验证矩阵（Swift demo 逐项测试）

| 策略 | activate | 后续操作 | 结果 |
|------|----------|----------|------|
| `.regular` | `activate(true)` | - | ❌ 关闭 |
| `.regular` | 不 activate | - | ✅ 正常 |
| `.accessory` | `activate(true)` | - | ✅ 正常 |
| `.accessory` | 不 activate | - | ✅ 正常 |
| `.regular` → `.accessory` | `activate(true)` | 无 | ❌ 关闭（tao 实际行为） |
| `.regular` → `.accessory` | `activate(true)` | `deactivate()` | ❌ 仍然关闭 |
| `.regular` → `.accessory` | `activate(true)` | `deactivate()` + `activate(false)` | ✅ 正常 |

关键发现：单独 `deactivate()` 不够，必须跟一次 `activateIgnoringOtherApps(false)` 重新在
`.accessory` 策略下建立干净的激活状态。

## 排除的假设

调查过程中排除了以下假设：

1. **TrayTarget overlay view / performClick 干扰** — 去掉 overlay 仍然复现
2. **0.1µs CFRunLoopTimer 干扰菜单追踪** — 去掉 timer 仍然复现
3. **CFRunLoopObserver 干扰** — 去掉 observers 仍然复现
4. **CFRunLoopSource (event proxy) 干扰** — 去掉 proxy 仍然复现
5. **cancelTrackingWithoutAnimation 被调用** — method swizzle 证实从未被调用
6. **muda NsMenuRef drop 调用 cancelTracking** — 仅影响子菜单的 NSMenu，不影响根菜单
7. **tao event loop 处理事件导致菜单关闭** — observers 中的代码全是 no-op 也复现

## 修复方案

在 `gui_host.rs` 的 setup 中，`set_activation_policy(.accessory)` 之后，
加一次 deactivate + reactivate 重置激活状态：

```rust
app.set_activation_policy(tauri::ActivationPolicy::Accessory);
// tao launched() 在 setup hook 之前以 .regular + activateIgnoringOtherApps(true)
// 激活了应用。对无窗口的状态栏 app，这导致 macOS 在首次展开子菜单时隐式关闭菜单。
// 重置方式：在 .accessory 策略下 deactivate → activate(false)，清除残留的异常激活状态。
unsafe {
    let mtm = objc2_foundation::MainThreadMarker::new().unwrap();
    let ns_app = objc2_app_kit::NSApp(mtm);
    ns_app.deactivate();
    #[allow(deprecated)]
    ns_app.activateIgnoringOtherApps(false);
}
```

## Demo 代码

源码在仓库 `demo/tray-menu-close/` 下。

### Tauri 最小复现（`demo/tray-menu-close/tauri/`）

最小 Tauri app，`cargo run` 即可复现。

### Swift 原生 demo（`demo/tray-menu-close/swift/`）

多个 Swift 文件用于逐项隔离测试：

- `main.swift` — 完整复现 tao + tray-icon 行为，可用 `--no-overlay / --no-timer / --no-observers / --no-proxy` 逐项关闭组件
- `minimal.swift` — 极简 NSStatusItem（基线，无 bug）
- `test_policy.swift` — 测试 activation policy + activate 各组合
- `test_timing.swift` — 测试 late-switch vs early-switch 时序
- `test_fix.swift` — 验证 deactivate + reactivate 修复方案

编译：`swiftc -O -o <name> <name>.swift -framework AppKit`
