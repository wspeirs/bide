<script setup lang="ts">
import { ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { UpdateReport } from '../types';
import { Loader2, CheckCircle2, AlertTriangle, X } from '@lucide/vue';

const props = defineProps<{
  open: boolean;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

const isUpdating = ref(false);
const report = ref<UpdateReport | null>(null);

function handleClose() {
  if (isUpdating.value) return;
  emit('close');
}

function handleKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    handleClose();
  }
}

async function runUpdate() {
  isUpdating.value = true;
  report.value = null;
  try {
    const res = await invoke<UpdateReport>('app_update');
    report.value = res;
  } catch (err) {
    report.value = {
      success: false,
      oldVersion: '',
      newVersion: '',
      versionChanged: false,
      output: '',
      error: typeof err === 'string' ? err : (err as Error)?.message || String(err),
      restartsTriggered: [],
      restartsPending: [],
    };
  } finally {
    isUpdating.value = false;
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      runUpdate();
    } else {
      report.value = null;
      isUpdating.value = false;
    }
  },
  { immediate: true }
);
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

    <div class="relative w-full max-w-xl flex flex-col bg-[#1e1f22] border border-[#393b40] rounded shadow-2xl overflow-hidden z-10 text-xs font-sans">
      <div class="flex items-center justify-between px-4 py-2.5 bg-[#252629] border-b border-[#2b2d30] shrink-0">
        <span class="font-semibold text-[#dfe1e5]">Update omp</span>
        <button
          v-if="!isUpdating"
          type="button"
          class="p-1 rounded text-[#6f737a] hover:text-[#dfe1e5] hover:bg-[#35373c] transition-colors"
          @click="handleClose"
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <div class="p-4 space-y-3 max-h-[70vh] overflow-y-auto">
        <div v-if="isUpdating" class="flex flex-col items-center justify-center py-8 gap-3 text-[#dfe1e5]">
          <Loader2 class="w-8 h-8 animate-spin text-[#3574f0]" />
          <span class="font-medium">Updating omp…</span>
          <span class="text-xs text-[#6f737a]">Running omp update in background</span>
        </div>

        <template v-else-if="report">
          <!-- Status result banner -->
          <div
            v-if="report.success"
            class="flex items-start gap-2.5 p-3 rounded bg-[#59a869]/10 border border-[#59a869]/30 text-[#dfe1e5]"
          >
            <CheckCircle2 class="w-4 h-4 text-[#59a869] shrink-0 mt-0.5" />
            <div>
              <div class="font-medium text-[#59a869]">
                <span v-if="report.versionChanged">
                  Updated successfully: {{ report.oldVersion }} → {{ report.newVersion }}
                </span>
                <span v-else>
                  Already up to date ({{ report.newVersion || report.oldVersion || 'current' }})
                </span>
              </div>
              <div v-if="report.restartsTriggered.length > 0" class="text-[13px] text-[#bcbec4] mt-1">
                Restarts triggered: {{ report.restartsTriggered.length }} tab(s)
              </div>
              <div v-if="report.restartsPending.length > 0" class="text-[13px] text-[#e2a53b] mt-0.5">
                Restarts pending: {{ report.restartsPending.length }} active tab(s) will restart after turn settles
              </div>
            </div>
          </div>

          <div
            v-else
            class="flex items-start gap-2.5 p-3 rounded bg-[#e55765]/10 border border-[#e55765]/30 text-[#dfe1e5]"
          >
            <AlertTriangle class="w-4 h-4 text-[#e55765] shrink-0 mt-0.5" />
            <div>
              <div class="font-medium text-[#e55765]">Update Failed</div>
              <div v-if="report.error" class="text-[13px] text-[#bcbec4] mt-1">{{ report.error }}</div>
            </div>
          </div>

          <!-- Output logs -->
          <div v-if="report.output" class="space-y-1">
            <span class="text-[13px] text-[#6f737a] uppercase font-mono">Output</span>
            <pre class="p-2.5 rounded bg-[#252629] border border-[#2b2d30] font-mono text-[13px] text-[#bcbec4] whitespace-pre-wrap max-h-48 overflow-y-auto m-0 leading-relaxed">{{ report.output }}</pre>
          </div>
        </template>
      </div>

      <div class="px-4 py-2.5 bg-[#252629] border-t border-[#2b2d30] flex justify-end shrink-0">
        <button
          type="button"
          :disabled="isUpdating"
          class="px-4 py-1.5 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          @click="handleClose"
        >
          Close
        </button>
      </div>
    </div>
  </div>
</template>
