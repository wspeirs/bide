<script setup lang="ts">
import { ref, watchPostEffect, nextTick } from 'vue';
import { useProjectsStore } from '../stores/projects';
import type { ProjectState } from '../types';
import { SendHorizontal } from '@lucide/vue';

const props = defineProps<{
  project: ProjectState;
}>();

const store = useProjectsStore();

const text = ref('');
const textareaRef = ref<HTMLTextAreaElement | null>(null);

watchPostEffect(() => {
  if (props.project.id && textareaRef.value) textareaRef.value.focus();
});

function handleKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    e.preventDefault();
    store.abortPrompt(props.project.id);
    return;
  }
}

async function send(behavior?: string) {
  const trimmed = text.value.trim();
  if (!trimmed) return;
  const promptToSend = text.value;
  text.value = '';

  try {
    await store.sendPrompt(props.project.id, promptToSend, behavior);
  } catch {
    // Restore prompt text on failure
    text.value = promptToSend;
  }
}

function focus() {
  textareaRef.value?.focus();
}

/** Insert text at the textarea caret (replacing any selection), then refocus. */
async function insertAtCursor(s: string) {
  const el = textareaRef.value;
  const start = el?.selectionStart ?? text.value.length;
  const end = el?.selectionEnd ?? text.value.length;
  const prev = text.value[start - 1];
  const ins = (prev && !/\s/.test(prev) ? ' ' : '') + s + ' ';
  text.value = text.value.slice(0, start) + ins + text.value.slice(end);
  await nextTick();
  el?.focus();
  el?.setSelectionRange(start + ins.length, start + ins.length);
}

defineExpose({
  focus,
  insertAtCursor,
});
</script>

<template>
  <div class="flex flex-col h-full w-full bg-[#1e1f22] p-2.5 relative">
    <div class="flex-1 relative flex">
      <textarea
        ref="textareaRef"
        v-model="text"
        @keydown="handleKeyDown"
        :disabled="project.status === 'starting'"
        :placeholder="
          project.isStreaming
            ? 'Agent is working… (send icon for follow-up, Esc to stop)'
            : 'Send a prompt… (use the send icon)'
        "
        class="w-full h-full bg-[#1e1f22] border border-[#2b2d30] focus:border-[#3574f0] focus:ring-1 focus:ring-[#3574f0] rounded p-2.5 font-mono text-[15px] text-[#dfe1e5] placeholder-[#6f737a] resize-none outline-none leading-relaxed transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
      ></textarea>

      <div class="absolute right-2 bottom-2 flex items-center gap-2">
        <button
          type="button"
          @click="send(project.isStreaming ? 'followUp' : undefined)"
          :disabled="!text.trim() || project.status === 'starting'"
          :title="project.isStreaming ? 'Send as follow-up' : 'Send prompt'"
          class="flex items-center justify-center p-1.5 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
        >
          <SendHorizontal class="w-4 h-4" />
        </button>
      </div>
    </div>
  </div>
</template>
