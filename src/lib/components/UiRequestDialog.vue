<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { useProjectsStore } from '../stores/projects';
import type { ProjectState, AskQuestion } from '../types';
import { X, HelpCircle } from '@lucide/vue';

const props = defineProps<{
  project: ProjectState;
}>();

const store = useProjectsStore();

const inputValue = ref('');
const picks = ref<Record<string, string[]>>({});
const customs = ref<Record<string, string>>({});

watch(
  () => props.project.pendingUiRequest,
  (req) => {
    inputValue.value = req?.defaultValue || '';
    picks.value = {};
    customs.value = {};
  },
  { immediate: true }
);

// single-select questions need an answer; multi-select may be empty
const canSubmitAsk = computed(() => {
  const req = props.project.pendingUiRequest;
  return !!req?.questions?.every(
    (q) => q.multi || picks.value[q.id]?.length || customs.value[q.id]?.trim()
  );
});

function cancel() {
  const req = props.project.pendingUiRequest;
  if (!req) return;
  store.respondUi(props.project.id, { cancelled: true });
}

function handleSelect(option: string) {
  const req = props.project.pendingUiRequest;
  if (!req) return;
  store.respondUi(props.project.id, { value: option });
}

function handleConfirm(val: boolean) {
  const req = props.project.pendingUiRequest;
  if (!req) return;
  store.respondUi(props.project.id, { confirmed: val });
}

function handleSubmitInput() {
  const req = props.project.pendingUiRequest;
  if (!req) return;
  store.respondUi(props.project.id, { value: inputValue.value });
}

function togglePick(q: AskQuestion, label: string) {
  const cur = picks.value[q.id] ?? [];
  if (q.multi) {
    picks.value[q.id] = cur.includes(label) ? cur.filter((l) => l !== label) : [...cur, label];
  } else {
    picks.value[q.id] = cur.includes(label) ? [] : [label];
    customs.value[q.id] = '';
  }
}

function setCustom(q: AskQuestion, v: string) {
  customs.value[q.id] = v;
  if (!q.multi && v.trim()) {
    picks.value[q.id] = [];
  }
}

function handleSubmitAsk() {
  const req = props.project.pendingUiRequest;
  if (!req?.questions || !canSubmitAsk.value) return;
  const answers = req.questions.map((q) => {
    const customInput = customs.value[q.id]?.trim();
    return {
      id: q.id,
      selectedOptions: picks.value[q.id] ?? [],
      ...(customInput ? { customInput } : {}),
    };
  });
  store.respondUi(props.project.id, { answers });
}
</script>

<template>
  <div
    v-if="props.project.pendingUiRequest"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs select-none"
  >
    <button
      type="button"
      class="fixed inset-0 cursor-default bg-transparent border-none p-0 outline-none"
      @click="cancel"
      aria-label="Cancel dialog"
    ></button>
    <div class="relative w-full max-w-lg flex flex-col bg-[#1e1f22] border border-[#393b40] rounded shadow-2xl overflow-hidden z-10 text-xs font-sans">
      <!-- Header -->
      <div class="flex items-center justify-between px-4 py-2.5 bg-[#252629] border-b border-[#2b2d30] shrink-0">
        <div class="flex items-center gap-2">
          <HelpCircle class="w-4 h-4 text-[#3574f0]" />
          <span class="font-semibold text-[#dfe1e5]">{{ props.project.pendingUiRequest.title || 'Input Required' }}</span>
        </div>
        <button
          type="button"
          @click="cancel"
          class="p-1 rounded text-[#6f737a] hover:text-[#dfe1e5] hover:bg-[#35373c] transition-colors"
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <!-- Body -->
      <div class="p-4 space-y-3 max-h-[70vh] overflow-y-auto">
        <div v-if="props.project.pendingUiRequest.message" class="text-xs text-[#bcbec4] leading-relaxed whitespace-pre-wrap">
          {{ props.project.pendingUiRequest.message }}
        </div>

        <!-- select -->
        <div v-if="props.project.pendingUiRequest.method === 'select' && props.project.pendingUiRequest.options" class="flex flex-col gap-1.5 pt-1">
          <button
            v-for="opt in props.project.pendingUiRequest.options"
            :key="opt"
            type="button"
            @click="handleSelect(opt)"
            class="w-full text-left px-3 py-2 rounded bg-[#252629] border border-[#393b40] hover:bg-[#2b2d30] hover:border-[#3574f0] text-[#dfe1e5] transition-colors text-xs font-medium cursor-pointer"
          >
            {{ opt }}
          </button>
        </div>

        <!-- confirm -->
        <div v-else-if="props.project.pendingUiRequest.method === 'confirm'" class="flex items-center justify-end gap-2 pt-2">
          <button
            type="button"
            @click="handleConfirm(false)"
            class="px-4 py-1.5 rounded bg-[#252629] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors"
          >
            No
          </button>
          <button
            type="button"
            @click="handleConfirm(true)"
            class="px-4 py-1.5 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium transition-colors"
          >
            Yes
          </button>
        </div>

        <!-- input -->
        <form
          v-else-if="props.project.pendingUiRequest.method === 'input'"
          @submit.prevent="handleSubmitInput"
          class="space-y-3"
        >
          <input
            type="text"
            v-model="inputValue"
            class="w-full px-3 py-2 rounded bg-[#1e1f22] border border-[#393b40] focus:border-[#3574f0] text-[#dfe1e5] text-xs font-mono outline-none"
            placeholder="Enter value…"
          />
          <div class="flex items-center justify-end gap-2">
            <button
              type="button"
              @click="cancel"
              class="px-3 py-1.5 rounded bg-[#252629] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              class="px-4 py-1.5 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium transition-colors"
            >
              OK
            </button>
          </div>
        </form>

        <!-- ask -->
        <form
          v-else-if="props.project.pendingUiRequest.method === 'ask' && props.project.pendingUiRequest.questions"
          @submit.prevent="handleSubmitAsk"
          class="space-y-4"
        >
          <div
            v-for="q in props.project.pendingUiRequest.questions"
            :key="q.id"
            class="space-y-1.5"
          >
            <div v-if="q.header" class="text-[11px] uppercase tracking-wider text-[#6f737a]">{{ q.header }}</div>
            <div class="text-[#dfe1e5] font-medium">{{ q.question }}</div>
            <button
              v-for="opt in q.options"
              :key="opt.label"
              type="button"
              @click="togglePick(q, opt.label)"
              :class="[
                'w-full text-left px-3 py-2 rounded border text-[#dfe1e5] transition-colors cursor-pointer',
                picks[q.id]?.includes(opt.label)
                  ? 'bg-[#2b3a5c] border-[#3574f0]'
                  : 'bg-[#252629] border-[#393b40] hover:bg-[#2b2d30] hover:border-[#3574f0]'
              ]"
            >
              <span class="font-medium">
                {{ q.multi ? (picks[q.id]?.includes(opt.label) ? '☑' : '☐') : (picks[q.id]?.includes(opt.label) ? '●' : '○') }} {{ opt.label }}
              </span>
              <span v-if="opt.description" class="block text-[#6f737a] pl-4">{{ opt.description }}</span>
            </button>
            <input
              type="text"
              :value="customs[q.id] ?? ''"
              @input="(e) => setCustom(q, (e.target as HTMLInputElement).value)"
              class="w-full px-3 py-2 rounded bg-[#1e1f22] border border-[#393b40] focus:border-[#3574f0] text-[#dfe1e5] text-xs font-mono outline-none"
              placeholder="Other (type your own)…"
            />
          </div>
          <div class="flex items-center justify-end gap-2">
            <button
              type="button"
              @click="cancel"
              class="px-3 py-1.5 rounded bg-[#252629] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              :disabled="!canSubmitAsk"
              class="px-4 py-1.5 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              Submit
            </button>
          </div>
        </form>

        <!-- editor -->
        <div v-else-if="props.project.pendingUiRequest.method === 'editor'" class="space-y-3">
          <textarea
            v-model="inputValue"
            rows="8"
            class="w-full p-2.5 rounded bg-[#1e1f22] border border-[#393b40] focus:border-[#3574f0] text-[#dfe1e5] text-xs font-mono outline-none resize-y leading-relaxed"
          ></textarea>
          <div class="flex items-center justify-end gap-2">
            <button
              type="button"
              @click="cancel"
              class="px-3 py-1.5 rounded bg-[#252629] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors"
            >
              Cancel
            </button>
            <button
              type="button"
              @click="handleSubmitInput"
              class="px-4 py-1.5 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium transition-colors"
            >
              OK
            </button>
          </div>
        </div>
      </div>

      <!-- Footer for select method cancel -->
      <div v-if="props.project.pendingUiRequest.method === 'select'" class="px-4 py-2 bg-[#252629] border-t border-[#2b2d30] flex justify-end shrink-0">
        <button
          type="button"
          @click="cancel"
          class="px-3 py-1.5 rounded bg-[#1e1f22] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors text-xs"
        >
          Cancel
        </button>
      </div>
    </div>
  </div>
</template>
