<script setup lang="ts">
import { computed } from "vue";

// 视图模式由 Rust 侧通过窗口 URL 的查询参数注入：?view=popup | settings
const view = computed(() => {
  const params = new URLSearchParams(window.location.search);
  return params.get("view") ?? "popup";
});
</script>

<template>
  <main class="app-root">
    <section class="placeholder">
      <h1>HumanInLoop</h1>
      <p>视图模式：{{ view }}</p>
      <p class="hint">脚手架就绪（Step 1）。后续步骤接入弹窗与设置界面。</p>
    </section>
  </main>
</template>

<style scoped>
.app-root {
  height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  box-sizing: border-box;
}
.placeholder {
  text-align: center;
  color: var(--text-primary);
}
.placeholder h1 {
  margin: 0 0 8px;
  font-size: 22px;
  font-weight: 600;
}
.hint {
  color: var(--text-secondary);
  font-size: 13px;
}
</style>
