<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { renderSVG } from 'uqr';
import { X, Copy, Check, RefreshCw } from '@lucide/vue';
import { useProjectsStore } from '../stores/projects';

const props = defineProps<{
  open: boolean;
  projectId: string;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

const store = useProjectsStore();
const url = ref<string | null>(null);
const error = ref<string | null>(null);
const loading = ref(false);
const copied = ref(false);
let copyTimer: number | undefined = undefined;

const qrSvg = computed(() => (url.value ? renderSVG(url.value, { border: 2 }) : ''));

async function load() {
  loading.value = true;
  error.value = null;
  url.value = null;
  try {
    url.value = await store.collabLink(props.projectId);
  } catch (err) {
    error.value = typeof err === 'string' ? err : (err as Error)?.message || String(err);
  } finally {
    loading.value = false;
  }
}

// Fetch on every open: the room and its link rotate when the session changes.
watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) load();
  },
  { immediate: true },
);

async function copyLink() {
  if (!url.value) return;
  try {
    await navigator.clipboard.writeText(url.value);
    copied.value = true;
    clearTimeout(copyTimer);
    copyTimer = window.setTimeout(() => {
      copied.value = false;
    }, 1500);
  } catch (err) {
    console.error('Failed to copy collab link:', err);
  }
}

function handleKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape') emit('close');
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
      @click="emit('close')"
    ></button>

    <div class="relative w-full max-w-md flex flex-col bg-[#1e1f22] border border-[#393b40] rounded shadow-2xl overflow-hidden z-10 text-xs font-sans">
      <div class="flex items-center justify-between px-4 py-2.5 bg-[#252629] border-b border-[#2b2d30] shrink-0">
        <span class="font-semibold text-[#dfe1e5]">Control from phone</span>
        <button
          type="button"
          class="p-1 rounded text-[#6f737a] hover:text-[#dfe1e5] hover:bg-[#35373c] transition-colors"
          @click="emit('close')"
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <div class="flex flex-col items-center gap-3 p-4 text-[#bcbec4]">
        <p v-if="loading" class="text-[#6f737a]">Fetching collab link…</p>
        <template v-else-if="error">
          <p class="text-[#e55765] select-text break-all font-mono">{{ error }}</p>
          <p class="text-[#6f737a]">
            The room may still be connecting. If this persists after an omp upgrade, run
            <code>npm run build:collab</code> and rebuild bide.
          </p>
          <button
            type="button"
            class="flex items-center gap-1 px-2 py-1 rounded bg-[#1e1f22] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] transition-colors"
            @click="load"
          >
            <RefreshCw class="w-3.5 h-3.5" />
            <span>Retry</span>
          </button>
        </template>
        <template v-else-if="url">
          <div class="bg-white p-2 rounded [&>svg]:w-64 [&>svg]:h-64" v-html="qrSvg"></div>
          <p class="select-text break-all font-mono text-[#dfe1e5]">{{ url }}</p>
          <p class="text-[#6f737a]">
            Anyone with this link can prompt and interrupt this session. Starting a New session or
            restarting the tab changes the link.
          </p>
        </template>
      </div>

      <div class="px-4 py-2 bg-[#252629] border-t border-[#2b2d30] flex justify-end gap-2 shrink-0">
        <button
          v-if="url"
          type="button"
          class="flex items-center gap-1 px-2 py-1 rounded bg-[#1e1f22] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] transition-colors"
          @click="copyLink"
        >
          <template v-if="copied">
            <Check class="w-3.5 h-3.5 text-[#59a869]" />
            <span class="text-[#59a869]">Copied</span>
          </template>
          <template v-else>
            <Copy class="w-3.5 h-3.5" />
            <span>Copy link</span>
          </template>
        </button>
        <button
          type="button"
          class="px-3 py-1.5 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium transition-colors"
          @click="emit('close')"
        >
          Close
        </button>
      </div>
    </div>
  </div>
</template>
