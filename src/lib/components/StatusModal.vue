<script setup lang="ts">
import { ref } from 'vue';
import { X, Copy, Check } from '@lucide/vue';

const props = defineProps<{
  open: boolean;
  text: string;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

const copied = ref(false);
let copyTimer: number | undefined = undefined;

function handleClose() {
  emit('close');
}

async function copyText() {
  try {
    await navigator.clipboard.writeText(props.text);
    copied.value = true;
    clearTimeout(copyTimer);
    copyTimer = window.setTimeout(() => {
      copied.value = false;
    }, 1500);
  } catch (err) {
    console.error('Failed to copy git status text:', err);
  }
}

function handleKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    handleClose();
  }
}
</script>

<template>
  <div
    v-if="open"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs select-none"
    @keydown="handleKeyDown"
  >
    <button
      type="button"
      class="fixed inset-0 cursor-default bg-transparent border-none p-0 outline-none"
      aria-label="Close dialog"
      @click="handleClose"
    ></button>

    <div class="relative w-full max-w-2xl max-h-[80vh] flex flex-col bg-[#1e1f22] border border-[#393b40] rounded shadow-2xl overflow-hidden z-10 text-xs font-sans">
      <div class="flex items-center justify-between px-4 py-2.5 bg-[#252629] border-b border-[#2b2d30] shrink-0">
        <span class="font-semibold text-[#dfe1e5]">Git Status</span>
        <div class="flex items-center gap-2">
          <button
            type="button"
            class="flex items-center gap-1 px-2 py-1 rounded bg-[#1e1f22] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] transition-colors"
            @click="copyText"
          >
            <template v-if="copied">
              <Check class="w-3.5 h-3.5 text-[#59a869]" />
              <span class="text-[#59a869]">Copied</span>
            </template>
            <template v-else>
              <Copy class="w-3.5 h-3.5" />
              <span>Copy</span>
            </template>
          </button>
          <button
            type="button"
            class="p-1 rounded text-[#6f737a] hover:text-[#dfe1e5] hover:bg-[#35373c] transition-colors"
            @click="handleClose"
          >
            <X class="w-4 h-4" />
          </button>
        </div>
      </div>

      <div class="flex-1 p-4 overflow-y-auto select-text font-mono text-xs bg-[#1e1f22] text-[#bcbec4]">
        <pre class="m-0 whitespace-pre-wrap font-mono leading-relaxed">{{ text || 'No output.' }}</pre>
      </div>

      <div class="px-4 py-2 bg-[#252629] border-t border-[#2b2d30] flex justify-end shrink-0">
        <button
          type="button"
          class="px-3 py-1.5 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium transition-colors"
          @click="handleClose"
        >
          Close
        </button>
      </div>
    </div>
  </div>
</template>
