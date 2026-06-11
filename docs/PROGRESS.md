# PROGRESS

按具体任务 / 需求记录待办与当前进展。任务 / 需求完成后删除其 section（历史留在 git）。

## 版本自更新机制（方案已确认，待实现）

需求/方案已成稿并确认：`docs/specs/self-update.md`、`docs/plans/self-update.md`；
提交规范已写入 `AGENTS.md`。

实现按 plan 任务顺序推进：① `update/` 模块 → ② 状态/命令/i18n → ③ ipc+daemon 广播
→ ④ 前端弹窗/设置 → ⑤ cliff.toml+release.yml → ⑥ 测试+文档+实测。
