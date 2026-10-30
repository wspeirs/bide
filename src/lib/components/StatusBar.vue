<script setup lang="ts">
import { Bug } from '@lucide/vue';
import type { ProjectState } from '../types';

defineProps<{
  project: ProjectState;
}>();
</script>

<template>
  <div class="flex items-center justify-between px-3 py-1.5 bg-[#1e1f22] border-t border-[#2b2d30] text-xs font-mono text-[#bcbec4] select-none shrink-0 min-h-[28px]">
    <!-- Debug toggle, model name and context % -->
    <div class="flex items-center gap-1.5">
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
      <span class="text-[#dfe1e5]">{{ project.model }}</span>
      <span class="text-[#6f737a]">·</span>
      <span>ctx {{ project.contextPercent }}%</span>
    </div>

    <!-- Status indicator -->
    <div class="flex items-center gap-1.5">
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
