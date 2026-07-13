<script setup lang="ts">
// 纵向模式（实验开关 + 多题）：所有问题纵向平铺成卡片，scroll-spy 定位当前题。
import { useI18n } from "vue-i18n";
import { usePopupContext } from "./context";

const { t } = useI18n();
const {
  request,
  questions,
  total,
  viewSource,
  questionHtml,
  onContentClick,
  chosenByQ,
  inputByQ,
  imagesByQ,
  replyFilesByQ,
  single,
  selectOnly,
  current,
  expandedQ,
  cardOptionHotkey,
  toggle,
  setActive,
  setCardRef,
  setInputRef,
  setSentinelRef,
  setThumbsRef,
  autoGrow,
  onTextareaFocus,
  onTextareaBlur,
  onUserCaretMaybeMoved,
  onTextareaMouseDown,
  speechSupported,
  listening,
  speechReady,
  speechError,
  speechStatus,
  speechHotkeyLabel,
  speechErrorText,
  speechStatusText,
  toggleSpeech,
  pickFiles,
  removeImage,
  removeReplyFile,
} = usePopupContext();
</script>

<template>
  <div
    v-for="(q, qi) in questions"
    :key="qi"
    :ref="(el) => setCardRef(el as HTMLElement | null, qi)"
    class="q-card"
    :data-q-index="qi"
    @mousedown="setActive(qi, false)"
  >
    <!-- 问题头部：问号图标 + 「Question i/n」。每题上方加分割线（与 Message/上一题区隔）。 -->
    <div
      class="q-header with-divider"
    >
      <svg class="q-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="9" />
        <path d="M9.2 9.3a2.8 2.8 0 0 1 5.4 1c0 1.9-2.8 2.5-2.8 2.5" />
        <path d="M12 17.2h.01" />
      </svg>
      <span class="q-label">{{
        t("popup.question.indexed", { i: qi + 1, n: total })
      }}</span>
    </div>

    <div
      v-if="request?.isMarkdown && !viewSource && q.message"
      class="markdown-body"
      v-html="questionHtml(q)"
      @click="onContentClick"
    ></div>
    <pre v-else-if="q.message" class="plain-body">{{ q.message }}</pre>

    <div v-if="q.predefinedOptions.length" class="options">
      <div
        v-for="(opt, i) in q.predefinedOptions"
        :key="i"
        class="option"
        :class="{ selected: (chosenByQ[qi] ?? []).includes(opt.text), single }"
        @click="toggle(qi, opt.text)"
      >
        <span class="check" :class="{ radio: single }">{{ single ? "" : ((chosenByQ[qi] ?? []).includes(opt.text) ? "✓" : "") }}</span>
        <span class="label"><span v-if="opt.recommended" class="rec-badge"><span class="rec-badge-pill"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M14 9V5a3 3 0 0 0-3-3l-4 9v11h11.28a2 2 0 0 0 2-1.7l1.38-9a2 2 0 0 0-2-2.3z"></path><path d="M7 22H4a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2h3"></path></svg>{{ t("popup.recommended") }}</span></span>{{ opt.text }}</span>
        <kbd v-if="cardOptionHotkey(qi, i)" class="opt-sc">{{ cardOptionHotkey(qi, i) }}</kbd>
      </div>
    </div>

    <!-- 输入框 + 内置「添加图片」小图标（右下角）；严格选择模式隐藏 -->
    <div v-if="!selectOnly" class="input-wrap">
      <textarea
        :ref="(el) => setInputRef(el as HTMLTextAreaElement | null, qi)"
        v-model="inputByQ[qi]"
        class="textarea"
        :class="{ collapsed: !expandedQ(qi) }"
        rows="1"
        :placeholder="t('popup.inputPlaceholder')"
        @input="autoGrow(qi)"
        @focus="onTextareaFocus(qi)"
        @blur="onTextareaBlur(qi)"
        @keyup="onUserCaretMaybeMoved"
        @mousedown="onTextareaMouseDown"
      ></textarea>
      <template v-if="expandedQ(qi)">
        <button
          v-if="speechSupported"
          class="mic-btn"
          :class="{ loading: listening && current === qi && !speechReady, recording: listening && current === qi && speechReady }"
          type="button"
          :title="
            speechReady
              ? t('popup.speech.stop') +
                (speechHotkeyLabel ? ' ' + speechHotkeyLabel : '')
              : listening
              ? t('popup.speech.preparing')
              : t('popup.speech.start') +
                (speechHotkeyLabel ? ' ' + speechHotkeyLabel : '')
          "
          :aria-label="
            listening ? t('popup.speech.stop') : t('popup.speech.start')
          "
          @mousedown.prevent
          @click="(setActive(qi, false), toggleSpeech())"
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
            <rect x="9" y="2" width="6" height="12" rx="3" />
            <path d="M5 11a7 7 0 0 0 14 0" />
            <path d="M12 18v3" />
          </svg>
        </button>
        <button
          class="img-btn"
          type="button"
          :title="t('popup.addImage')"
          :aria-label="t('popup.addImage')"
          @click="pickFiles(qi)"
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="3" width="18" height="18" rx="2" />
            <circle cx="8.5" cy="8.5" r="1.6" />
            <path d="M21 15l-5-5L5 21" />
          </svg>
        </button>
      </template>
    </div>
    <p v-if="!selectOnly && current === qi && speechError" class="speech-error">
      {{ speechErrorText(speechError) }}
    </p>
    <p v-else-if="!selectOnly && current === qi && listening && speechStatus" class="speech-status">
      {{ speechStatusText(speechStatus) }}
    </p>

    <div
      v-if="!selectOnly && (imagesByQ[qi] ?? []).length"
      :ref="(el) => setThumbsRef(el as HTMLElement | null, qi)"
      class="thumbs"
    >
      <div v-for="(img, i) in imagesByQ[qi]" :key="i" class="thumb">
        <img :src="img.data" alt="" />
        <button class="remove" type="button" @click="removeImage(qi, i)">
          ×
        </button>
      </div>
    </div>

    <div v-if="!selectOnly && (replyFilesByQ[qi] ?? []).length" class="reply-files">
      <div
        v-for="(f, i) in replyFilesByQ[qi]"
        :key="f.path"
        class="reply-file"
        :title="f.path"
      >
        <span class="rf-icon">📄</span>
        <span class="rf-name">{{ f.name }}</span>
        <button class="rf-remove" type="button" @click="removeReplyFile(qi, i)">
          ×
        </button>
      </div>
    </div>

    <!-- 底部哨兵：进视口即「已看到」该题（兼容超长题） -->
    <div
      :ref="(el) => setSentinelRef(el as HTMLElement | null, qi)"
      class="q-sentinel"
      :data-q-sentinel="qi"
      aria-hidden="true"
    ></div>
  </div>
</template>
