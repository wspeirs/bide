<script setup lang="ts">
import type { ProjectState } from '../types';

defineProps<{
  project: ProjectState;
}>();
</script>

<template>
  <div class="flex flex-col h-full w-full bg-[#1e1f22] select-text overflow-hidden">
    <div class="px-3 py-1 bg-[#252629] border-b border-[#2b2d30] text-[13px] font-mono uppercase tracking-wider text-[#6f737a] select-none flex items-center justify-between shrink-0">
      <span>Last Prompt</span>
    </div>

    <div class="flex-1 p-3 font-mono text-[15px] overflow-y-auto leading-relaxed">
      <div v-if="project.lastPrompt" class="flex items-start gap-2">
        <span class="text-[#3574f0] select-none font-bold">&gt;</span>
        <pre class="font-mono text-[#dfe1e5] whitespace-pre-wrap break-words flex-1 m-0">{{ project.lastPrompt }}</pre>
      </div>
      <span v-else class="text-[#6f737a] italic select-none">No prompt sent yet</span>
    </div>
  </div>
</template>
