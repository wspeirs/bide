<script setup lang="ts">
import { ref, watch, onBeforeUnmount, nextTick } from 'vue';
import type { ProjectState } from '../types';
import TimingSpan from './TimingSpan.vue';
import { ChevronRight, ChevronDown, Wrench, CheckCircle2, AlertCircle, Loader2, Ban } from '@lucide/vue';

const props = defineProps<{
  project: ProjectState;
}>();

const containerRef = ref<HTMLDivElement | null>(null);
const userScrolledUp = ref(false);
const expandedTools = ref<Record<string, boolean>>({});

// Live clock so elapsed times of in-flight items tick while the agent works.
const now = ref(Date.now());
let timer: ReturnType<typeof setInterval> | null = null;

watch(
  () => props.project.isStreaming,
  (isStreaming) => {
    if (timer) {
      clearInterval(timer);
      timer = null;
    }
    if (isStreaming) {
      timer = setInterval(() => {
        now.value = Date.now();
      }, 500);
    }
  },
  { immediate: true }
);

onBeforeUnmount(() => {
  if (timer) {
    clearInterval(timer);
    timer = null;
  }
});

function toggleTool(id: string) {
  expandedTools.value[id] = !expandedTools.value[id];
}

function onScroll() {
  if (!containerRef.value) return;
  const { scrollTop, scrollHeight, clientHeight } = containerRef.value;
  userScrolledUp.value = scrollHeight - scrollTop - clientHeight > 40;
}

watch(
  () => {
    const log = props.project.activityLog;
    const last = log[log.length - 1];
    return [
      log.length,
      last?.text?.length,
      last?.argsChars,
      props.project.isStreaming,
    ];
  },
  async () => {
    await nextTick();
    if (!userScrolledUp.value && containerRef.value) {
      containerRef.value.scrollTop = containerRef.value.scrollHeight;
    }
  },
  { flush: 'post', immediate: true }
);

function formatToolArgs(args: unknown): string {
  if (!args) return '';
  if (typeof args === 'string') return args;
  try {
    return JSON.stringify(args, null, 2);
  } catch {
    return String(args);
  }
}

function fmtChars(n: number): string {
  return n < 1000 ? `${n}` : `${(n / 1000).toFixed(1)}k`;
}
</script>

<template>
  <div class="flex flex-col h-full w-full bg-[#1e1f22] select-text overflow-hidden">
    <div class="px-3 py-1 bg-[#252629] border-b border-[#2b2d30] text-[13px] font-mono uppercase tracking-wider text-[#6f737a] select-none flex items-center justify-between shrink-0">
      <span>Activity / Agent Console</span>
      <button
        v-if="userScrolledUp"
        type="button"
        @click="() => {
          userScrolledUp = false;
          if (containerRef) containerRef.scrollTop = containerRef.scrollHeight;
        }"
        class="text-[12px] text-[#3574f0] hover:underline cursor-pointer"
      >
        Scroll to bottom ↓
      </button>
    </div>

    <div
      ref="containerRef"
      @scroll="onScroll"
      class="flex-1 p-3 font-mono text-[15px] overflow-y-auto leading-relaxed space-y-3"
    >
      <template v-for="item in props.project.activityLog" :key="item.id">
        <!-- thinking -->
        <div
          v-if="item.type === 'thinking' && (item.text || !item.endedAt)"
          class="bg-[#252629]/50 border border-[#2b2d30] rounded p-2 text-neutral-400 italic font-mono text-[15px] whitespace-pre-wrap leading-relaxed"
        >
          <div class="text-[12px] uppercase font-sans tracking-wider text-[#6f737a] not-italic mb-1 font-semibold flex items-center gap-1.5">
            <span :class="['w-1.5 h-1.5 rounded-full bg-[#3574f0]/60', item.endedAt ? '' : 'animate-pulse']"></span>
            Thinking
            <span class="ml-auto flex items-center gap-2 normal-case tracking-normal font-normal">
              <TimingSpan :started-at="item.startedAt" :ended-at="item.endedAt" :now="now" />
            </span>
          </div>
          {{ item.text }}
        </div>

        <!-- text -->
        <div
          v-else-if="item.type === 'text'"
          class="font-mono text-[15px] text-[#dfe1e5] leading-relaxed whitespace-pre-wrap break-words"
        >
          {{ item.text }}
        </div>

        <!-- tool_call -->
        <div
          v-else-if="item.type === 'tool_call'"
          class="border border-[#2b2d30] bg-[#252629] rounded text-[15px] overflow-hidden"
        >
          <button
            type="button"
            @click="toggleTool(item.id)"
            class="w-full flex items-center gap-2 px-2.5 py-1.5 text-left hover:bg-[#2b2d30] transition-colors select-none text-[#bcbec4]"
          >
            <ChevronDown v-if="expandedTools[item.id]" class="w-3.5 h-3.5 text-[#6f737a] shrink-0" />
            <ChevronRight v-else class="w-3.5 h-3.5 text-[#6f737a] shrink-0" />
            <Wrench class="w-3.5 h-3.5 text-[#e2a53b] shrink-0" />
            <span class="font-semibold text-[#dfe1e5]">{{ item.tool }}</span>
            <span v-if="item.intent" class="text-[#bcbec4] truncate max-w-[300px]">({{ item.intent }})</span>
            <span class="ml-auto flex items-center gap-2 text-[12px] text-[#6f737a] shrink-0">
              <TimingSpan :started-at="item.startedAt" :ended-at="item.endedAt" :now="now" />
              <span v-if="item.endedAt && item.phase !== 'done'" class="text-[#e2a53b] flex items-center gap-1">
                <Ban class="w-3 h-3" /> stopped
              </span>
              <span v-else-if="item.phase === 'generating'" class="text-[#3574f0] flex items-center gap-1">
                <Loader2 class="w-3 h-3 animate-spin" /> generating · {{ fmtChars(item.argsChars ?? 0) }}
              </span>
              <span v-else-if="item.phase === 'running'" class="text-[#3574f0] flex items-center gap-1">
                <Loader2 class="w-3 h-3 animate-spin" /> running
              </span>
              <span v-else-if="item.isError" class="text-[#e55765] flex items-center gap-1">
                <AlertCircle class="w-3 h-3" /> error
              </span>
              <span v-else-if="item.isError === false" class="text-[#59a869] flex items-center gap-1">
                <CheckCircle2 class="w-3 h-3" /> done
              </span>
            </span>
          </button>
          <div
            v-if="expandedTools[item.id] && item.args"
            class="p-2 border-t border-[#2b2d30] bg-[#1e1f22] text-[#bcbec4] overflow-x-auto"
          >
            <pre class="m-0 font-mono text-[13px] whitespace-pre-wrap">{{ formatToolArgs(item.args) }}</pre>
          </div>
        </div>

        <!-- command -->
        <div
          v-else-if="item.type === 'command'"
          class="font-mono text-xs text-[#bcbec4] bg-[#252629] border border-[#2b2d30] rounded p-2 whitespace-pre-wrap"
        >
          {{ item.text }}
        </div>

        <!-- error -->
        <div
          v-else-if="item.type === 'error'"
          class="font-mono text-xs text-[#e55765] bg-[#e55765]/10 border border-[#e55765]/30 rounded p-2 whitespace-pre-wrap"
        >
          {{ item.text }}
        </div>
      </template>

      <div v-if="props.project.activityLog.length === 0" class="text-[#6f737a] text-xs italic select-none py-4 text-center">
        No activity yet. Send a prompt below to start.
      </div>
    </div>
  </div>
</template>
