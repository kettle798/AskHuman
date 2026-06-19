# PROGRESS

按具体任务 / 需求记录待办与当前进展。任务 / 需求完成后删除其 section（历史留在 git）。

## 进行中：弹窗启动延迟性能优化（埋点 + harness + 基线 + 首轮优化已落地 → 下一步方案6/5）

文档：`docs/specs/popup-launch-performance.md`（调用链、等待点、优化方案、度量方法论 §7）。
harness 计划：`docs/plans/perf-harness-deterministic-mock-im.md`。优化计划：`docs/plans/popup-launch-low-risk-optimization.md`。

已完成：
- **埋点**（`ASKHUMAN_PERF` 门控，默认关、零开销）：`src-tauri/src/perf.rs` + CLI/daemon/helper/前端 ~18 里程碑，
  统一写 `~/.askhuman/perf.log`（`<epoch_ms>\t<perf_id>\t<stage>\t<pid>`），按 `perf_id` 串联整条时间线。
- **确定性 harness**（无脑单命令 `node scripts/perf-popup.mjs`，固定 canonical 场景 + 固定基线 `docs/perf/baseline.json`，
  有则比/无则建/劣化退非零，仅留 `--update-baseline`）：
  - 隔离 daemon（临时 HOME，绝不碰真实 daemon / 在途）+ `ASKHUMAN_NO_KEYCHAIN=1`（零钥匙串副作用）。
  - **本地 mock IM 全 4 渠道**（`scripts/perf-mock-im.mjs`）：建连+发送各注入 ~150ms 当「IM 阻塞弹窗」探针；
    钉钉/Slack 硬编码端点经新 env `ASKHUMAN_{DINGTALK,SLACK}_API_BASE` 指向 mock（仅测试，未设不变）。
  - **冷+热同跑**两组、各出表，基线含 `cold`/`warm`。
  - **屏幕守卫**：锁屏（`ioreg` 读 `CGSSessionScreenIsLocked`）报错不跑、`caffeinate -d` 防息屏、弹窗未上屏即中止。

**基线**（`docs/perf/baseline.json`，屏幕解锁+唤醒+勿遮挡下采，compare 复跑验证回归闸有效）：
- COLD 端到端 p90 ≈ 1092ms，其中 `daemon recv→spawned` ≈ 463ms 几乎全是 IM 建连（3 个 WS 串行各 150ms）——「IM 阻塞弹窗」探针。
- WARM 端到端 p90 ≈ 528ms（router 复用，`im_attach`≈0），大头在 `GUI total show→painted`（window visible ~233ms + page boot ~410ms）。

**首轮低风险优化已落地**（方案7 代码分割 + 方案2 popupInit 提前 + 方案1 main.ts 不阻塞 + 支撑 S：popup_init 作弹窗唯一非钥匙串配置源，见 `docs/plans/popup-launch-low-risk-optimization.md`）。
对比基线（同机 compare，两闸均 OK 无回归）：
- WARM：端到端 p90 **-5.9%**（497 vs 528）；`frontend boot→painted` **-37%**、`popup_init` **-51%**、`page boot` -4%、`tauri build()` -11%。
- COLD：端到端 -0.3%（被 ~463ms IM 建连主导，属方案3/6）；`frontend boot→painted` -16%、`popup_init` -85%。
- 附带：`HistoryView` 改由 `history_init.lang` 应用语言（与 `agents_init` 同模式），`main.ts` 自此零 IPC。

**下一步**：方案6 预热复用（大头、架构级）、方案5 detect 移 daemon、方案3 spawn 提到 attach 前（消掉 COLD 的 IM 建连阻塞）、方案4 attach 省钥匙串、方案8 延后 show/骨架屏（见 spec §4-6）。
**待定**：是否把基线刷新为优化后新数（`--update-baseline`）。

## 待办：daemon 二进制变化检测 —— 轮询 vs filewatch（后续评估，优先级低）

二进制变化检测目前是 **15s 轮询** `current_exe()` 指纹（稳态≈1 次 `stat`，靠 `binhash.json` 内容哈希缓存避免重哈希）。
是否改 **filewatch** 待权衡——难点：二进制走原子替换（rename 换 inode，需盯父目录 + 按文件名过滤 + 每次替换后重挂，
参考 `config_watch.rs`）、装在任意目录（`~/.local/bin`/brew/npm 前缀/`.app` bundle…）、且 watcher 仍要 stat/hash 才能确认
内容**真**变（指纹是内容哈希而非 mtime）。延迟要求松（~15s 够）+ Hello 路径兜底，故暂保持轮询。
