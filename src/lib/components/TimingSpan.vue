<script setup lang="ts">
const props = defineProps<{
  startedAt?: number;
  endedAt?: number;
  now: number;
}>();

function fmtDur(ms: number): string {
  if (ms < 1000) return `${Math.max(0, Math.round(ms))}ms`;
  if (ms < 10000) return `${(ms / 1000).toFixed(1)}s`;
  if (ms < 60000) return `${Math.round(ms / 1000)}s`;
  return `${Math.floor(ms / 60000)}m ${Math.round((ms % 60000) / 1000)}s`;
}
</script>

<template>
  <template v-if="props.startedAt">
    <span>{{ new Date(props.startedAt).toLocaleTimeString('en-GB') }}</span>
    <span class="text-[#bcbec4]">{{ fmtDur((props.endedAt ?? props.now) - props.startedAt) }}</span>
  </template>
</template>
