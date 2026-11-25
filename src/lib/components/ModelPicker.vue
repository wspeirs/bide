<script setup lang="ts">
import { computed } from 'vue';
import { ChevronRight, X } from '@lucide/vue';
import { DialogContent, DialogDescription, DialogOverlay, DialogPortal, DialogRoot, DialogTitle, DialogTrigger } from 'reka-ui';
import type { ModelInfo } from '../types';

const props = defineProps<{
  open: boolean;
  title: string;
  models: ModelInfo[];
  model: string | null;
  emptyLabel?: string;
  loading?: boolean;
  disabled?: boolean;
  saving?: boolean;
  error?: string;
  cannotApply?: boolean;
}>();
const emit = defineEmits<{
  'update:open': [open: boolean];
  select: [model: string | null];
  apply: [];
  'close-auto-focus': [event: Event];
}>();

const groups = computed(() => {
  const groups = new Map<string, ModelInfo[]>();
  for (const model of props.models) {
    if (!groups.has(model.provider)) groups.set(model.provider, []);
    groups.get(model.provider)!.push(model);
  }
  return groups;
});
const label = computed(() => {
  const model = props.models.find(({ provider, id }) => `${provider}/${id}` === props.model);
  return model ? `${model.name || model.id} · ${model.provider}` : props.model ?? props.emptyLabel ?? 'No model selected';
});
</script>

<template>
  <DialogRoot :open="open" @update:open="emit('update:open', $event)">
    <DialogTrigger v-if="$slots.trigger" as-child><slot name="trigger" /></DialogTrigger>
    <DialogPortal>
      <DialogOverlay class="fixed inset-0 z-50 bg-black/60 backdrop-blur-xs" />
      <DialogContent
        class="fixed top-1/2 left-1/2 z-50 -translate-x-1/2 -translate-y-1/2 w-[calc(100%-2rem)] max-w-xl max-h-[85vh] flex flex-col overflow-hidden bg-[#1e1f22] text-[#dfe1e5] border border-[#393b40] rounded shadow-2xl text-xs font-sans"
        @close-auto-focus="emit('close-auto-focus', $event)"
      >
        <header class="flex items-start justify-between gap-3 px-4 py-2.5 bg-[#252629] border-b border-[#2b2d30] shrink-0">
          <div class="min-w-0 space-y-0.5">
            <DialogTitle class="font-semibold text-sm">{{ title }}</DialogTitle>
            <DialogDescription class="text-[#bcbec4]">
              {{ $slots.thinking ? 'Choose a model and thinking level, then apply.' : 'Choose a model, then apply.' }}
            </DialogDescription>
            <p class="truncate" :title="model ?? ''">{{ label }}</p>
          </div>
          <button
            type="button" aria-label="Close model picker" :disabled="saving"
            class="p-1 rounded text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#35373c] cursor-pointer disabled:opacity-50"
            @click="emit('update:open', false)"
          >
            <X class="w-4 h-4" />
          </button>
        </header>
        <p v-if="error" role="alert" class="px-4 py-2 text-[#e55765] break-words">{{ error }}</p>
        <div class="min-h-0 overflow-y-auto p-2">
          <button
            v-if="emptyLabel" type="button" :disabled="disabled" :aria-pressed="model === null"
            class="block w-full text-left rounded px-2 py-1.5 hover:bg-[#2b2d30] focus-visible:outline-[#3574f0] aria-pressed:text-[#81a8fb] aria-pressed:font-semibold cursor-pointer disabled:opacity-50"
            @click="emit('select', null)"
          >{{ emptyLabel }}</button>
          <p v-if="loading" role="status" class="p-2 text-[#bcbec4]">Loading models…</p>
          <p v-else-if="!groups.size" class="p-2 text-[#bcbec4]">No models available.</p>
          <details v-for="[provider, list] in groups" :key="provider" class="group">
            <summary class="flex list-none [&::-webkit-details-marker]:hidden items-center gap-2 rounded px-2 py-2 font-semibold cursor-pointer hover:bg-[#2b2d30] focus-visible:outline-[#3574f0]">
              <ChevronRight aria-hidden="true" class="w-3.5 h-3.5 group-open:rotate-90" />
              {{ provider }}
            </summary>
            <div class="ml-3.5 pl-2 border-l border-[#393b40]">
              <button
                v-for="item in list" :key="item.id" type="button" :disabled="disabled"
                :aria-pressed="`${provider}/${item.id}` === model"
                class="block w-full text-left wrap-break-word rounded px-2 py-1.5 hover:bg-[#2b2d30] focus-visible:outline-[#3574f0] aria-pressed:text-[#81a8fb] aria-pressed:font-semibold cursor-pointer disabled:opacity-50"
                @click="emit('select', `${provider}/${item.id}`)"
              >{{ item.name || item.id }}</button>
            </div>
          </details>
        </div>
        <footer class="flex items-center justify-between gap-3 px-4 py-2.5 bg-[#252629] border-t border-[#2b2d30] shrink-0">
          <slot name="thinking" />
          <div class="ml-auto flex items-center gap-2">
            <button
              type="button" :disabled="saving" @click="emit('update:open', false)"
              class="px-2 py-1 rounded border border-[#393b40] hover:bg-[#35373c] focus-visible:outline-[#3574f0] cursor-pointer disabled:opacity-50"
            >Cancel</button>
            <button
              type="button" :disabled="cannotApply" @click="emit('apply')"
              class="px-2 py-1 rounded bg-[#3574f0] text-white focus-visible:outline-white cursor-pointer disabled:opacity-50"
            >Apply</button>
          </div>
        </footer>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
