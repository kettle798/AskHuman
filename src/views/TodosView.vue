<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { applyTheme } from "../lib/theme";
import { applyLanguage } from "../i18n";
import {
  todosAdd,
  todosClear,
  todosInit,
  todosList,
  todosProjects,
  todosRemove,
} from "../lib/ipc";
import type { TodoEntry, TodoProjectInfo } from "../lib/types";

const { t } = useI18n();

const projects = ref<TodoProjectInfo[]>([]);
const selected = ref<string>("");
const entries = ref<TodoEntry[]>([]);
// 首次加载完成前显示 Loading（避免空态闪现误导）。
const loaded = ref(false);
const newText = ref("");
const confirmClear = ref(false);
// 防连点重复新增（Enter 连击）。
const adding = ref(false);

function basename(key: string): string {
  const parts = key.replace(/\/+$/, "").split("/");
  return parts[parts.length - 1] || key;
}

async function reloadProjects(): Promise<void> {
  const list = await todosProjects();
  // 预选项目不在候选中（如 daemon 刚退、agent 项目未入 workspace 索引）→ 兜底追加，保持选中稳定。
  if (selected.value && !list.some((p) => p.key === selected.value)) {
    list.push({ key: selected.value, name: basename(selected.value), count: 0 });
  }
  projects.value = list;
  if (!selected.value) {
    selected.value = list[0]?.key ?? "";
  }
}

async function reloadEntries(): Promise<void> {
  entries.value = selected.value ? await todosList(selected.value) : [];
}

async function reloadAll(): Promise<void> {
  try {
    await reloadProjects();
    await reloadEntries();
  } catch (err) {
    console.warn("todos reload failed", err);
  } finally {
    loaded.value = true;
  }
}

async function onSelect(): Promise<void> {
  confirmClear.value = false;
  await reloadEntries();
}

async function addEntry(): Promise<void> {
  const text = newText.value.trim();
  if (!text || !selected.value || adding.value) return;
  adding.value = true;
  try {
    await todosAdd(selected.value, text);
    newText.value = "";
    await reloadAll();
  } catch (err) {
    console.warn("todo add failed", err);
  } finally {
    adding.value = false;
  }
}

function onNewKeydown(e: KeyboardEvent): void {
  // isComposing：IME 组词中的 Enter 不当提交。
  if (e.key === "Enter" && !e.isComposing) {
    e.preventDefault();
    void addEntry();
  }
}

async function removeEntry(id: string): Promise<void> {
  if (!selected.value) return;
  try {
    await todosRemove(selected.value, id);
  } catch (err) {
    console.warn("todo remove failed", err);
  }
  await reloadAll();
}

async function clearAll(): Promise<void> {
  confirmClear.value = false;
  if (!selected.value) return;
  try {
    await todosClear(selected.value);
  } catch (err) {
    console.warn("todo clear failed", err);
  }
  await reloadAll();
}

function absoluteTime(ms: number): string {
  return ms ? new Date(ms).toLocaleString() : "";
}

let unlistenUpdated: UnlistenFn | null = null;
let unlistenGoto: UnlistenFn | null = null;

onMounted(async () => {
  try {
    const init = await todosInit();
    applyTheme(init.theme);
    applyLanguage(init.lang);
  } catch {
    /* 读取失败：保持兜底外观 */
  }
  const preselect = new URLSearchParams(window.location.search).get("project");
  if (preselect) selected.value = preselect;
  // todos.json 被任意进程改写（CLI/弹窗/出队）→ 宿主文件监听推事件 → 重载。
  unlistenUpdated = await listen("todos-updated", () => {
    void reloadAll();
  });
  // 窗口已开时再次带预选项目打开（Agent 卡片入口）→ 切换选中项目。
  unlistenGoto = await listen<string>("todos-goto-project", (e) => {
    if (typeof e.payload === "string" && e.payload) {
      selected.value = e.payload;
      confirmClear.value = false;
      void reloadAll();
    }
  });
  await reloadAll();
});

onBeforeUnmount(() => {
  unlistenUpdated?.();
  unlistenGoto?.();
});
</script>

<template>
  <div class="todos-win">
    <header class="td-header" data-tauri-drag-region>
      <span class="td-title" data-tauri-drag-region>{{ t("todosWin.title") }}</span>
      <select
        v-if="projects.length"
        v-model="selected"
        class="td-select"
        :aria-label="t('todosWin.projectLabel')"
        @change="onSelect"
      >
        <option v-for="p in projects" :key="p.key" :value="p.key" :title="p.key">
          {{ p.name }}{{ p.count ? ` (${p.count})` : "" }}
        </option>
      </select>
    </header>

    <div class="td-body">
      <div v-if="!loaded" class="empty">
        <span class="spinner" />
      </div>

      <div v-else-if="!projects.length" class="empty">
        <p class="empty-title">{{ t("todosWin.noProjects") }}</p>
        <p class="empty-hint">{{ t("todosWin.noProjectsHint") }}</p>
      </div>

      <div v-else-if="!entries.length" class="empty">
        <p class="empty-title">{{ t("todosWin.empty") }}</p>
        <p class="empty-hint">{{ t("todosWin.emptyHint") }}</p>
      </div>

      <ul v-else class="td-list">
        <li v-for="e in entries" :key="e.id" class="td-row">
          <span class="td-text" :title="absoluteTime(e.createdAtMs)">{{ e.text }}</span>
          <button
            type="button"
            class="td-del"
            :title="t('todosWin.delete')"
            :aria-label="t('todosWin.delete')"
            @click="removeEntry(e.id)"
          >
            <svg viewBox="0 0 12 12" aria-hidden="true">
              <path d="M3 3 L9 9 M9 3 L3 9" stroke="currentColor" stroke-width="1.4"
                stroke-linecap="round" />
            </svg>
          </button>
        </li>
      </ul>
    </div>

    <footer v-if="projects.length" class="td-footer">
      <div class="td-add">
        <input
          v-model="newText"
          class="td-input"
          type="text"
          :placeholder="t('todosWin.addPlaceholder')"
          @keydown="onNewKeydown"
        />
        <button
          type="button"
          class="td-btn td-btn-add"
          :disabled="!newText.trim() || adding"
          @click="addEntry"
        >
          {{ t("todosWin.add") }}
        </button>
      </div>
      <div v-if="entries.length" class="td-clear-row">
        <template v-if="confirmClear">
          <span class="td-clear-confirm">{{ t("todosWin.clearConfirm") }}</span>
          <button type="button" class="td-btn" @click="confirmClear = false">
            {{ t("todosWin.confirmCancel") }}
          </button>
          <button type="button" class="td-btn td-btn-danger" @click="clearAll">
            {{ t("todosWin.clearOk") }}
          </button>
        </template>
        <button v-else type="button" class="td-clear" @click="confirmClear = true">
          {{ t("todosWin.clear") }}
        </button>
      </div>
    </footer>
  </div>
</template>

<style scoped>
.todos-win {
  display: flex;
  flex-direction: column;
  height: 100%;
  color: var(--text-primary);
}
.td-header {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--border);
}
.macos .td-header {
  padding-top: 30px;
}
.td-title {
  font-size: 14px;
  font-weight: 600;
  white-space: nowrap;
}
.td-select {
  flex: 0 1 auto;
  min-width: 0;
  max-width: 60%;
  appearance: auto;
  border: 1px solid var(--border);
  border-radius: 7px;
  background: var(--bg-elevated);
  color: var(--text-primary);
  font-size: 12px;
  padding: 3px 8px;
}
.td-body {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  padding: 12px 14px;
}
.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  height: 100%;
  text-align: center;
}
.empty-title {
  font-size: 14px;
  font-weight: 600;
  margin: 0;
}
.empty-hint {
  font-size: 12px;
  color: var(--text-secondary);
  margin: 0;
  max-width: 320px;
}
.spinner {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  border: 2px solid color-mix(in srgb, var(--text-primary) 18%, transparent);
  border-top-color: var(--text-secondary);
  animation: td-spin 0.7s linear infinite;
}
@keyframes td-spin {
  to {
    transform: rotate(360deg);
  }
}
.td-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.td-row {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 9px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm, 8px);
  background: var(--bg-elevated);
}
.td-text {
  flex: 1 1 auto;
  min-width: 0;
  font-size: 13px;
  line-height: 1.45;
  white-space: pre-wrap;
  word-break: break-word;
}
.td-del {
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  margin-top: 1px;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition: background 0.12s ease, color 0.12s ease;
}
.td-del:hover {
  background: color-mix(in srgb, #ff453a 14%, transparent);
  color: #ff453a;
}
.td-del svg {
  width: 12px;
  height: 12px;
}
.td-footer {
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 14px 12px;
  border-top: 1px solid var(--border);
}
.td-add {
  display: flex;
  gap: 8px;
}
.td-input {
  flex: 1 1 auto;
  min-width: 0;
  border: 1px solid var(--border);
  border-radius: 7px;
  background: var(--bg-elevated);
  color: var(--text-primary);
  font-size: 12px;
  padding: 6px 9px;
}
.td-input:focus {
  outline: none;
  border-color: color-mix(in srgb, #0a84ff 55%, transparent);
}
.td-btn {
  appearance: none;
  flex: 0 0 auto;
  border: 1px solid var(--border);
  background: var(--bg-elevated);
  color: var(--text-primary);
  font-size: 12px;
  font-weight: 600;
  padding: 5px 12px;
  border-radius: 7px;
  cursor: pointer;
  transition: background 0.12s ease, color 0.12s ease;
}
.td-btn:hover:not(:disabled) {
  background: color-mix(in srgb, var(--text-primary) 8%, transparent);
}
.td-btn:disabled {
  opacity: 0.45;
  cursor: default;
}
.td-btn-add {
  border-color: transparent;
  background: #0a84ff;
  color: #fff;
}
.td-btn-add:hover:not(:disabled) {
  background: #0071e3;
}
.td-btn-danger {
  border-color: transparent;
  background: #ff453a;
  color: #fff;
}
.td-btn-danger:hover {
  background: #e0352b;
}
.td-clear-row {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
}
.td-clear-confirm {
  flex: 1 1 auto;
  min-width: 0;
  font-size: 11px;
  color: var(--text-primary);
}
.td-clear {
  appearance: none;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: 11px;
  font-weight: 600;
  padding: 2px 6px;
  border-radius: 5px;
  cursor: pointer;
  transition: background 0.12s ease, color 0.12s ease;
}
.td-clear:hover {
  background: color-mix(in srgb, #ff453a 12%, transparent);
  color: #ff453a;
}
</style>
