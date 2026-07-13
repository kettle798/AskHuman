<script setup lang="ts">
// 「实验」tab：从 IM 创建 Agent 任务（仅 macOS 有内容，其余平台空态）。
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsContext } from "./context";

const { t } = useI18n();
const ctx = useSettingsContext();
const {
  isMac,
  persist,
  toggleAgentTasks,
  testAgentTaskTerminal,
  taskSettingsBusy,
  taskSettingsMessage,
  refreshAgentTaskSettings,
  taskReadiness,
  taskWorkspaces,
  openReadinessIssue,
  openWorkspacePanel,
} = ctx;
// 父组件仅在 config 加载后渲染本 tab，这里可安全断言非空。
const config = computed(() => ctx.config.value!);
</script>

<template>
  <div v-if="isMac" class="card">
    <div class="row">
      <div class="col">
        <p class="card-title">{{ t("settings.agentTasks.title") }}</p>
        <p class="card-desc">{{ t("settings.agentTasks.description") }}</p>
      </div>
      <span class="spacer"></span>
      <label class="switch">
        <input
          type="checkbox"
          v-model="config.agentTasks.enabled"
          @change="toggleAgentTasks"
        />
        <span class="track"></span>
      </label>
    </div>
    <template v-if="config.agentTasks.enabled">
      <hr class="divider" />
      <div class="row">
        <span class="label">{{ t("settings.agentTasks.permission") }}</span>
        <span class="spacer"></span>
        <select class="select" v-model="config.agentTasks.permissionPrompt" @change="persist">
          <option value="ask">{{ t("settings.agentTasks.permissionAsk") }}</option>
          <option value="agent-default">{{ t("settings.agentTasks.permissionDefault") }}</option>
          <option value="yolo">{{ t("settings.agentTasks.permissionYolo") }}</option>
        </select>
      </div>
      <p v-if="config.agentTasks.permissionPrompt === 'yolo'" class="result err">
        {{ t("settings.agentTasks.yoloWarning") }}
      </p>
      <hr class="divider" />
      <div class="row">
        <span class="label">Terminal.app</span>
        <span class="spacer"></span>
        <button class="btn" type="button" @click="testAgentTaskTerminal">
          {{ t("settings.agentTasks.testTerminal") }}
        </button>
        <button class="btn" type="button" :disabled="taskSettingsBusy" @click="refreshAgentTaskSettings(true)">
          {{ t("settings.agentTasks.refresh") }}
        </button>
      </div>
      <hr class="divider" />
      <p class="label">{{ t("settings.agentTasks.readiness") }}</p>
      <div v-for="item in taskReadiness" :key="item.kind" class="row agent-row">
        <span class="label">{{ item.label }}</span>
        <span class="badge"><span class="dot" :class="item.ready ? 'on' : 'off'"></span>{{ item.ready ? t("settings.agentTasks.ready") : t("settings.agentTasks.notReady") }}</span>
        <span class="spacer"></span>
        <span class="card-desc readiness-conditions">
          <span v-if="item.binaryReady">CLI ✓</span>
          <button
            v-else
            type="button"
            :title="t('settings.agentTasks.openInstallDocs')"
            @click="openReadinessIssue(item.kind, 'binary')"
          >CLI ×</button>
          <span class="readiness-separator">·</span>
          <span v-if="item.lifecycleReady">Lifecycle ✓</span>
          <button
            v-else
            type="button"
            :title="t('settings.agentTasks.goToSetting')"
            @click="openReadinessIssue(item.kind, 'lifecycle')"
          >Lifecycle ×</button>
          <span class="readiness-separator">·</span>
          <span v-if="item.integrationReady">Integration ✓</span>
          <button
            v-else
            type="button"
            :title="t('settings.agentTasks.goToSetting')"
            @click="openReadinessIssue(item.kind, 'integration')"
          >Integration ×</button>
        </span>
      </div>
      <hr class="divider" />
      <div class="row">
        <div class="col">
          <span class="label">{{ t("settings.agentTasks.workspaces") }}</span>
          <span class="card-desc">{{ t("settings.agentTasks.workspaceCount", { n: taskWorkspaces.length }) }}</span>
        </div>
        <span class="spacer"></span>
        <button class="btn" type="button" @click="openWorkspacePanel">
          {{ t("settings.agentTasks.manageWorkspaces") }}
        </button>
      </div>
      <p v-if="taskSettingsMessage" class="result">{{ taskSettingsMessage }}</p>
    </template>
  </div>
  <div v-else class="empty-state">
    <p class="empty-title">{{ t("settings.experimental.emptyTitle") }}</p>
    <p class="empty-desc">{{ t("settings.experimental.emptyDesc") }}</p>
  </div>
</template>
