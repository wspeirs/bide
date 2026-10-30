<script setup lang="ts">
import { ref, watch, nextTick } from 'vue';
import { ArrowRight, ArrowLeft, Trash2 } from '@lucide/vue';
import type { ProjectState } from '../types';

interface Props {
  project: ProjectState;
}

const props = defineProps<Props>();

const expanded = ref<Record<number, boolean>>({});
const scroller = ref<HTMLDivElement | null>(null);
let stick = true;

function onScroll() {
  if (!scroller.value) return;
  stick = scroller.value.scrollHeight - scroller.value.scrollTop - scroller.value.clientHeight < 24;
}

watch(
  () => props.project.debugLog.length,
  async () => {
    if (stick) {
      await nextTick();
      if (scroller.value) {
        scroller.value.scrollTop = scroller.value.scrollHeight;
      }
    }
  },
  { flush: 'post', immediate: true }
);

function pretty(json: string): string {
  try {
    return JSON.stringify(JSON.parse(json), null, 2);
  } catch {
    return json;
  }
}

function clear() {
  props.project.debugLog.length = 0;
  expanded.value = {};
}
</script>

<template>
  <div class="flex flex-col h-full w-full bg-[#1e1f22] select-text overflow-hidden">
    <div class="px-3 py-1 bg-[#252629] border-b border-[#2b2d30] text-[12px] font-mono uppercase tracking-wider text-[#6f737a] select-none flex items-center justify-between shrink-0">
      <span>Debug / JSON-RPC ({{ project.debugLog.length }})</span>
      <button type="button" @click="clear" title="Clear" class="hover:text-[#dfe1e5] cursor-pointer">
        <Trash2 class="w-3.5 h-3.5" />
      </button>
    </div>

    <div ref="scroller" @scroll="onScroll" class="flex-1 overflow-y-auto font-mono text-[12px] leading-relaxed">
      <template v-for="e in project.debugLog" :key="e.id">
        <button
          type="button"
          @click="expanded[e.id] = !expanded[e.id]"
          class="w-full flex items-center gap-2 px-3 py-0.5 text-left hover:bg-[#2b2d30] cursor-pointer"
        >
          <span class="text-[#6f737a] shrink-0">{{ e.time }}</span>
          <ArrowRight v-if="e.dir === 'out'" class="w-3 h-3 text-[#e2a53b] shrink-0" />
          <ArrowLeft v-else class="w-3 h-3 text-[#59a869] shrink-0" />
          <span class="text-[#3574f0] shrink-0">{{ e.kind }}</span>
          <span class="truncate text-[#6f737a]">{{ e.json.slice(0, 200) }}</span>
        </button>
        <pre v-if="expanded[e.id]" class="m-0 px-6 py-1 bg-[#252629] text-[#bcbec4] whitespace-pre-wrap break-all">{{ pretty(e.json) }}</pre>
      </template>
    </div>
  </div>
</template>
