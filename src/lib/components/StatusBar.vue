<script setup lang="ts">
import { computed } from 'vue';
import { Bug } from '@lucide/vue';
import type { ProjectState } from '../types';

const props = defineProps<{
  project: ProjectState;
}>();

function formatWindow(tokens: number): string {
  if (tokens < 1_000) return String(tokens);
  const [scale, suffix] = tokens < 1_000_000 ? [1_000, 'K']
    : tokens < 1_000_000_000 ? [1_000_000, 'M'] : [1_000_000_000, 'B'];
  const value = tokens / scale;
  return `${value < 10 ? Number(value.toFixed(1)) : Math.round(value)}${suffix}`;
}

const contextLabel = computed(() => {
  const { contextPercent, contextWindow } = props.project;
  const window = contextWindow === undefined ? '?' : formatWindow(contextWindow);
  return contextPercent === null ? window : `${contextPercent.toFixed(1)}%/${window}`;
});
</script>

<template>
  <div class="flex items-center justify-between px-3 py-1.5 bg-[#1e1f22] border-t border-[#2b2d30] text-xs font-mono text-[#bcbec4] select-none shrink-0 min-h-[28px]">
    <!-- Debug toggle, model name and context % -->
    <div class="min-w-0 flex flex-wrap items-center gap-x-1.5">
      <button
        type="button"
        @click="project.debugOpen = !project.debugOpen"
        title="Toggle debug pane (JSON-RPC)"
        :class="[
          'p-0.5 rounded cursor-pointer transition-colors',
          project.debugOpen ? 'text-[#3574f0] bg-[#3574f0]/20' : 'text-[#6f737a] hover:text-[#dfe1e5]'
        ]"
      >
        <Bug class="w-3.5 h-3.5" />
      </button>
      <span class="text-[#dfe1e5]" title="Primary model and thinking level">{{ project.model }}{{ project.thinking ? ` (${project.thinking})` : '' }}</span>
      <template v-if="project.advisorModel">
        <span class="text-[#6f737a]">·</span>
        <span title="Advisor model and thinking level">Advisor: {{ project.advisorModel }}{{ project.advisorThinkingLevel ? ` (${project.advisorThinkingLevel})` : '' }}</span>
      </template>
      <span class="text-[#6f737a]">·</span>
      <span>ctx {{ contextLabel }}</span>
    </div>

    <!-- Status indicator -->
    <div class="flex shrink-0 items-center gap-1.5">
      <template v-if="project.isStreaming">
        <span class="w-2 h-2 rounded-full bg-[#3574f0] animate-pulse"></span>
        <span class="text-[#3574f0] font-sans">streaming</span>
      </template>
      <template v-else-if="project.status === 'ready'">
        <span class="w-2 h-2 rounded-full bg-[#59a869]"></span>
        <span class="text-[#59a869] font-sans">ready</span>
      </template>
      <template v-else-if="project.status === 'error'">
        <span class="w-2 h-2 rounded-full bg-[#e55765]"></span>
        <span class="text-[#e55765] font-sans">error</span>
      </template>
      <template v-else>
        <span class="w-2 h-2 rounded-full border border-[#6f737a]"></span>
        <span class="text-[#6f737a] font-sans">starting</span>
      </template>
    </div>
  </div>
</template>
