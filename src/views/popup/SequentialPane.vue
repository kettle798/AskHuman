<script setup lang="ts">
// 旧版（顺序模式）：单题 / 实验开关关时——一次显示一个问题，上一步/下一步左右滑动切换。
import { useI18n } from "vue-i18n";
import { usePopupContext } from "./context";

const { t } = useI18n();
const {
  request,
  showQuestionHeader,
  showDescription,
  questionHeaderLabel,
  qHeaderRef,
  transitionName,
  onQuestionEntered,
  current,
  currentQuestion,
  renderedHtml,
  viewSource,
  onContentClick,
  chosen,
  userInput,
  images,
  replyFiles,
  single,
  selectOnly,
  optionHotkey,
  toggle,
  setInputRef,
  setThumbsRef,
  autoGrow,
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
  <!-- 问题头部：间距 + 分割线 + 问号图标 + 「Question i/n」 -->
  <div
    v-if="showQuestionHeader"
    :ref="(el) => (qHeaderRef = el as HTMLElement | null)"
    class="q-header"
    :class="{ 'with-divider': showDescription }"
  >
    <svg class="q-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="12" cy="12" r="9" />
      <path d="M9.2 9.3a2.8 2.8 0 0 1 5.4 1c0 1.9-2.8 2.5-2.8 2.5" />
      <path d="M12 17.2h.01" />
    </svg>
    <span class="q-label">{{ questionHeaderLabel }}</span>
  </div>

  <!-- 当前问题区（上一个/下一个左右滑动） -->
  <Transition :name="transitionName" mode="out-in" @after-enter="onQuestionEntered">
    <div class="question-pane" :key="current">
      <div
        v-if="request?.isMarkdown && !viewSource && currentQuestion?.message"
        class="markdown-body"
        v-html="renderedHtml"
        @click="onContentClick"
      ></div>
      <pre v-else-if="currentQuestion?.message" class="plain-body">{{ currentQuestion?.message }}</pre>

      <div v-if="currentQuestion && currentQuestion.predefinedOptions.length" class="options">
        <div
          v-for="(opt, i) in currentQuestion.predefinedOptions"
          :key="i"
          class="option"
          :class="{ selected: chosen.includes(opt.text), single }"
          @click="toggle(current, opt.text)"
        >
          <span class="check" :class="{ radio: single }">{{ single ? "" : (chosen.includes(opt.text) ? "✓" : "") }}</span>
          <span class="label"><span v-if="opt.recommended" class="rec-badge"><span class="rec-badge-pill"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M14 9V5a3 3 0 0 0-3-3l-4 9v11h11.28a2 2 0 0 0 2-1.7l1.38-9a2 2 0 0 0-2-2.3z"></path><path d="M7 22H4a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2h3"></path></svg>{{ t("popup.recommended") }}</span></span>{{ opt.text }}</span>
          <kbd v-if="optionHotkey(i)" class="opt-sc">{{ optionHotkey(i) }}</kbd>
        </div>
      </div>

      <!-- 输入框 + 内置「添加图片」小图标（右下角）；严格选择模式隐藏 -->
      <div v-if="!selectOnly" class="input-wrap">
        <textarea
          :ref="(el) => setInputRef(el as HTMLTextAreaElement | null, current)"
          v-model="userInput"
          class="textarea"
          :placeholder="t('popup.inputPlaceholder')"
          @input="autoGrow(current)"
          @keyup="onUserCaretMaybeMoved"
          @mousedown="onTextareaMouseDown"
        ></textarea>
        <button
          v-if="speechSupported"
          class="mic-btn"
          :class="{ loading: listening && !speechReady, recording: speechReady }"
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
          @click="toggleSpeech"
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
          @click="pickFiles(current)"
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="3" width="18" height="18" rx="2" />
            <circle cx="8.5" cy="8.5" r="1.6" />
            <path d="M21 15l-5-5L5 21" />
          </svg>
        </button>
      </div>
      <p v-if="!selectOnly && speechError" class="speech-error">
        {{ speechErrorText(speechError) }}
      </p>
      <p v-else-if="!selectOnly && listening && speechStatus" class="speech-status">
        {{ speechStatusText(speechStatus) }}
      </p>

      <div v-if="!selectOnly && images.length" :ref="(el) => setThumbsRef(el as HTMLElement | null, current)" class="thumbs">
        <div v-for="(img, i) in images" :key="i" class="thumb">
          <img :src="img.data" alt="" />
          <button class="remove" type="button" @click="removeImage(current, i)">
            ×
          </button>
        </div>
      </div>

      <div v-if="!selectOnly && replyFiles.length" class="reply-files">
        <div
          v-for="(f, i) in replyFiles"
          :key="f.path"
          class="reply-file"
          :title="f.path"
        >
          <span class="rf-icon">📄</span>
          <span class="rf-name">{{ f.name }}</span>
          <button class="rf-remove" type="button" @click="removeReplyFile(current, i)">
            ×
          </button>
        </div>
      </div>
    </div>
  </Transition>
</template>
