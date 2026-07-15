<script setup lang="ts">
// 折叠待办区（spec todo-whats-next D7）：页脚上方常驻，显示该提问项目的待办队列。
// 单题弹窗（非 whats-next / 非严格选择）里每条待办是可选 chip：选中＝提交时其文本并入回答、
// 后端按 id 出队；多题 / whats-next 弹窗只保留增删查看。底部快速新增框随手记想法。
import { useI18n } from "vue-i18n";
import { usePopupContext } from "./context";

const { t } = useI18n();
const {
  todos,
  todosOpen,
  todoChosenIds,
  todoNewText,
  todoChipsEnabled,
  todoSectionVisible,
  toggleTodo,
  addTodo,
  removeTodo,
  submitting,
} = usePopupContext();

// 回车快速新增；IME 组合确认（如中文选字）不触发。
function onNewTodoKeydown(e: KeyboardEvent) {
  if (e.isComposing) return;
  e.preventDefault();
  addTodo();
}
</script>

<template>
  <div v-if="todoSectionVisible" class="todo-section">
    <button
      class="todo-header"
      type="button"
      :aria-expanded="todosOpen"
      @click="todosOpen = !todosOpen"
    >
      <svg
        class="todo-caret"
        :class="{ open: todosOpen }"
        viewBox="0 0 12 12"
        width="10"
        height="10"
        aria-hidden="true"
      >
        <path
          d="M4 2.5 8 6l-4 3.5"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
      <span class="todo-title">{{ t("popup.todos.title") }}</span>
      <span v-if="todos.length" class="todo-count">{{ todos.length }}</span>
      <span v-if="!todosOpen && todoChosenIds.length" class="todo-picked">
        {{ t("popup.todos.picked", { n: todoChosenIds.length }) }}
      </span>
    </button>

    <div v-if="todosOpen" class="todo-body">
      <p v-if="!todos.length" class="todo-empty">{{ t("popup.todos.empty") }}</p>
      <div v-else class="todo-list">
        <div v-for="td in todos" :key="td.id" class="todo-row">
          <button
            class="todo-chip"
            :class="{
              selected: todoChosenIds.includes(td.id),
              selectable: todoChipsEnabled,
            }"
            type="button"
            :disabled="!todoChipsEnabled || submitting"
            :title="todoChipsEnabled ? t('popup.todos.chipHint') : td.text"
            @click="toggleTodo(td.id)"
          >
            <span class="todo-chip-text">{{ td.text }}</span>
          </button>
          <button
            class="todo-del"
            type="button"
            :disabled="submitting"
            :title="t('popup.todos.delete')"
            @click="removeTodo(td.id)"
          >
            <svg viewBox="0 0 12 12" width="9" height="9" aria-hidden="true">
              <line x1="2" y1="2" x2="10" y2="10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
              <line x1="10" y1="2" x2="2" y2="10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
            </svg>
          </button>
        </div>
      </div>
      <div class="todo-add">
        <input
          v-model="todoNewText"
          class="todo-input"
          type="text"
          :placeholder="t('popup.todos.addPlaceholder')"
          :disabled="submitting"
          @keydown.enter.exact="onNewTodoKeydown"
        />
        <button
          class="todo-add-btn"
          type="button"
          :disabled="!todoNewText.trim() || submitting"
          @click="addTodo"
        >
          {{ t("popup.todos.add") }}
        </button>
      </div>
    </div>
  </div>
</template>
