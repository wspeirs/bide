<script setup lang="ts">
import { ref, watch } from 'vue';
import { X } from '@lucide/vue';
import { useProjectsStore } from '../stores/projects';
import type { ProjectState } from '../types';

const props = defineProps<{
  open: boolean;
  project: ProjectState;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

const store = useProjectsStore();

const text = ref('');
const source = ref<'workspace' | 'global' | 'none'>('none');
const warning = ref<string | null>(null);
const error = ref<string | null>(null);
const loading = ref(false);
const saving = ref(false);
const loadFailed = ref(false);

function handleClose() {
  emit('close');
}

function handleKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    handleClose();
  }
}

async function handleSave() {
  saving.value = true;
  error.value = null;
  try {
    await store.setWorkspaceConfig(props.project.id, text.value);
    handleClose();
  } catch (err) {
    error.value = typeof err === 'string' ? err : (err as Error)?.message || String(err);
  } finally {
    saving.value = false;
  }
}

watch(
  () => [props.open, props.project.id] as const,
  ([open, id], _, onCleanup) => {
    if (open) {
      let cancelled = false;
      onCleanup(() => {
        cancelled = true;
      });
      loading.value = true;
      loadFailed.value = false;
      error.value = null;
      text.value = '';
      source.value = 'none';
      warning.value = null;
      store
        .getWorkspaceConfig(id)
        .then((cfg) => {
          if (cancelled) return;
          text.value = cfg.text;
          source.value = cfg.source;
          warning.value = cfg.warning;
        })
        .catch((err) => {
          if (cancelled) return;
          loadFailed.value = true;
          error.value = typeof err === 'string' ? err : (err as Error)?.message || String(err);
        })
        .finally(() => {
          if (!cancelled) loading.value = false;
        });
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

    <div class="relative w-full max-w-3xl h-[80vh] flex flex-col bg-[#1e1f22] border border-[#393b40] rounded shadow-2xl overflow-hidden z-10 text-xs font-sans">
      <!-- Header -->
      <div class="flex items-start justify-between px-4 py-2.5 bg-[#252629] border-b border-[#2b2d30] shrink-0">
        <div class="flex flex-col gap-0.5">
          <span class="font-semibold text-[#dfe1e5] text-sm">omp config — {{ project.name }}</span>
          <span class="text-[11px] text-[#6f737a]">
            <template v-if="source === 'workspace'">
              Shared by all worktrees of this repo. Written to .omp/config.yml (git-excluded) when a workspace opens.
            </template>
            <template v-else-if="source === 'global'">
              No workspace config yet — starting from your global omp config. Nothing is written until you save.
            </template>
            <template v-else>
              No workspace config yet.
            </template>
          </span>
        </div>
        <button
          type="button"
          class="p-1 rounded text-[#6f737a] hover:text-[#dfe1e5] hover:bg-[#35373c] transition-colors cursor-pointer"
          @click="handleClose"
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <!-- Warning if tracked -->
      <div
        v-if="warning"
        class="px-4 py-2 bg-[#e55765]/10 border-b border-[#e55765]/30 text-[#e55765] text-xs shrink-0"
      >
        {{ warning }}
      </div>

      <!-- Body / Editor -->
      <div class="flex-1 p-3 flex flex-col overflow-hidden bg-[#1e1f22]">
        <div v-if="loading" class="flex-1 flex items-center justify-center text-[#6f737a]">
          Loading config...
        </div>
        <textarea
          v-else
          v-model="text"
          :disabled="Boolean(warning) || saving || loadFailed"
          spellcheck="false"
          class="flex-1 w-full h-full p-2.5 bg-[#141416] border border-[#393b40] rounded text-[#bcbec4] font-mono text-xs resize-none focus:outline-none focus:border-[#3574f0] select-text disabled:opacity-50 disabled:cursor-not-allowed leading-relaxed"
          placeholder="# Per-workspace omp config (YAML format)"
        ></textarea>
      </div>

      <!-- Error message if save failed -->
      <div
        v-if="error"
        class="px-4 py-1.5 bg-[#e55765]/10 border-t border-[#e55765]/30 text-[#e55765] text-xs font-mono shrink-0"
      >
        {{ error }}
      </div>

      <!-- Footer -->
      <div class="px-4 py-2.5 bg-[#252629] border-t border-[#2b2d30] flex items-center justify-between shrink-0">
        <span class="text-[11px] text-[#6f737a]">
          Saving restarts this repo's omp sessions (after any running turn).
        </span>
        <div class="flex items-center gap-2">
          <button
            type="button"
            class="px-3 py-1.5 rounded border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors cursor-pointer"
            @click="handleClose"
          >
            Cancel
          </button>
          <button
            type="button"
            :disabled="Boolean(warning) || saving || loading || loadFailed"
            class="px-3 py-1.5 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer"
            @click="handleSave"
          >
            {{ saving ? 'Saving...' : 'Save' }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
