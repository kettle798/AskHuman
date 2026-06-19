# PROGRESS

按具体任务 / 需求记录待办与当前进展。任务 / 需求完成后删除其 section（历史留在 git）。

## 进行中：弹窗启动延迟性能优化（埋点 + harness 已落地，优化方案待做）

文档：`docs/specs/popup-launch-performance.md`（完整调用链、等待点清单、优化方案、度量方法论 §7）。

已完成：
- **埋点**（`ASKHUMAN_PERF` 门控，默认关、零开销）：`src-tauri/src/perf.rs` + CLI/daemon/helper/前端 16 个里程碑，
  统一写 `~/.askhuman/perf.log`（`<epoch_ms>\t<perf_id>\t<stage>\t<pid>`），按 `perf_id` 串联整条时间线。
- **harness**：`scripts/perf-popup.mjs`——零交互（弹窗画完首帧自动取消）跑 N 次、聚合中位/p90、
  存/比基线、端到端 p90 超阈（默认 20%）退出码 1。
- 已 `install.sh` 装好并实测：端到端热路径 ~0.55s，GUI/页面加载占 ~90%（基线样例见文档「附」）。

harness 改进：改用**隔离 daemon**（临时 HOME，绝不碰真实 daemon / 在途提问），支持 `--cold`；
新增 spawn 起点（含进程创建）+ 终点双 rAF。隔离基线已刷新到 `docs/perf/baseline.json`
（端到端含 spawn p90 ≈474ms，page boot ~357ms 是大头）。

计划：`docs/plans/popup-launch-low-risk-optimization.md`（首轮低风险组合 = 方案7 代码分割 + 方案2
popupInit 提前 + 方案1 main.ts 不阻塞 + 支撑改动「popup_init 作为弹窗唯一非钥匙串配置源」）。
**待用户确认计划后再改优化代码。** 后续：方案6 预热（大头）、方案5 detect 移 daemon 等见 spec §4-6。

## 待办：daemon 二进制变化检测 —— 轮询 vs filewatch（后续评估，优先级低）

二进制变化检测目前是 **15s 轮询** `current_exe()` 指纹（稳态≈1 次 `stat`，靠 `binhash.json` 内容哈希缓存避免重哈希）。
是否改 **filewatch** 待权衡——难点：二进制走原子替换（rename 换 inode，需盯父目录 + 按文件名过滤 + 每次替换后重挂，
参考 `config_watch.rs`）、装在任意目录（`~/.local/bin`/brew/npm 前缀/`.app` bundle…）、且 watcher 仍要 stat/hash 才能确认
内容**真**变（指纹是内容哈希而非 mtime）。延迟要求松（~15s 够）+ Hello 路径兜底，故暂保持轮询。
