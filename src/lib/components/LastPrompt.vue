<script setup lang="ts">
import { nextTick, ref, watch } from 'vue';
import { finalResponse } from '../stores/projects';
import type { ProjectState, PromptEntry } from '../types';
import Markdown from './Markdown.vue';

const props = defineProps<{
  project: ProjectState;
}>();

const listRef = ref<HTMLElement | null>(null);

watch(
  () => props.project.history.length,
  async () => {
    await nextTick();
    listRef.value?.scrollTo({ top: listRef.value.scrollHeight });
  },
);

function summary(p: string): string {
  const words = p.trim().split(/\s+/);
  return words.slice(0, 8).join(' ') + (words.length > 8 ? '…' : '');
}

function responseFor(e: PromptEntry, i: number): string {
  return (
    e.response ??
    (i === props.project.history.length - 1 && props.project.isStreaming
      ? finalResponse(props.project.activityLog)
      : '')
  );
}
</script>

<template>
  <div class="flex flex-col h-full w-full bg-[#1e1f22] select-text overflow-hidden">
    <div class="px-3 py-1 bg-[#252629] border-b border-[#2b2d30] text-[13px] font-mono uppercase tracking-wider text-[#6f737a] select-none flex items-center justify-between shrink-0">
      <span>Prompt History</span>
    </div>

    <div ref="listRef" class="flex-1 overflow-y-auto p-2 space-y-1">
      <div v-if="project.history.length === 0" class="p-1 font-mono text-[15px] leading-relaxed">
        <span class="text-[#6f737a] italic select-none">No prompt sent yet</span>
      </div>

      <details
        v-for="(e, i) in project.history"
        :key="e.id"
        class="rounded border border-[#2b2d30] bg-[#252629]"
      >
        <summary class="cursor-pointer truncate font-mono text-[14px] px-2.5 py-1.5 text-[#dfe1e5] select-none">
          {{ summary(e.prompt) }}
        </summary>
        <div class="px-2.5 pb-2.5 pt-1 border-t border-[#2b2d30] space-y-2">
          <div class="flex items-start gap-2">
            <span class="text-[#3574f0] select-none font-bold">&gt;</span>
            <pre class="font-mono text-[14px] text-[#dfe1e5] whitespace-pre-wrap break-words flex-1 m-0">{{ e.prompt }}</pre>
          </div>
          <div class="border-t border-[#2b2d30]" />
          <Markdown v-if="responseFor(e, i)" :source="responseFor(e, i)" />
          <span
            v-else-if="i === project.history.length - 1 && project.isStreaming"
            class="text-xs italic text-[#6f737a]"
          >Running…</span>
          <span v-else class="text-xs italic text-[#6f737a]">No final response</span>
        </div>
      </details>
    </div>
  </div>
</template>
