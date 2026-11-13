<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue';
import { useProjectsStore } from '../stores/projects';
import type { ProjectState, RoleInfo, ModelInfo } from '../types';
import { Users, Shield, RotateCcw, Square, Check, ChevronDown, Sparkles, QrCode, Brain, Terminal } from '@lucide/vue';
import CollabModal from './CollabModal.vue';

const CAVEMAN_LEVELS = ['off', 'lite', 'full', 'ultra', 'wenyan'] as const;
const PONYTAIL_LEVELS = ['off', 'lite', 'full', 'ultra'] as const;

const props = defineProps<{
  project: ProjectState;
}>();

const store = useProjectsStore();

const roles = ref<RoleInfo[]>([]);
const isRoleMenuOpen = ref(false);
const menuPos = ref({ left: 0, bottom: 0 });
const collabOpen = ref(false);
const models = ref<ModelInfo[]>([]);
const thinkingLevels = ref<string[]>([]);
const isCustom = computed(() => props.project.role === 'custom');

const modelGroups = computed(() => {
  const groups = new Map<string, ModelInfo[]>();
  for (const m of models.value) {
    if (!groups.has(m.provider)) groups.set(m.provider, []);
    groups.get(m.provider)!.push(m);
  }
  return groups;
});
// Keep the live values selectable while lists load or when discovery omits them.
const modelListed = computed(() => models.value.some((m) => `${m.provider}/${m.id}` === props.project.modelKey));
const thinkingOptions = computed(() => {
  const t = props.project.thinking;
  return !t || thinkingLevels.value.includes(t) ? thinkingLevels.value : [t, ...thinkingLevels.value];
});

// Supported levels depend on the live model.
watch(
  () => props.project.modelKey,
  async (key) => {
    if (key) thinkingLevels.value = await store.listThinkingLevels(props.project.id);
  },
  { immediate: true },
);
watch(
  isCustom,
  async (custom) => {
    if (custom && models.value.length === 0) models.value = await store.listModels(props.project.id);
  },
  { immediate: true },
);

// Fixed position so the pane's overflow-hidden can't clip the menu.
function toggleRoleMenu(e: MouseEvent) {
  isRoleMenuOpen.value = !isRoleMenuOpen.value;
  if (!isRoleMenuOpen.value) return;
  const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
  menuPos.value = { left: r.left, bottom: window.innerHeight - r.top + 4 };
  loadRoles();
}

async function loadRoles() {
  roles.value = await store.listRoles(props.project.id);
}

onMounted(() => {
  loadRoles();
});

function toggleOrchestrate() {
  store.setOrchestrate(props.project.id, !props.project.orchestrate);
}

function toggleAdvisor() {
  store.toggleAdvisor(props.project.id);
}

function handleReview() {
  store.runReview(props.project.id);
}

function handleNewSession() {
  if (props.project.isStreaming) return;
  store.newSession(props.project.id);
}

function handleStop() {
  store.abortPrompt(props.project.id);
}

async function selectRole(roleName: string) {
  isRoleMenuOpen.value = false;
  await store.setRole(props.project.id, roleName);
}

// Pin the live model so the model picker starts from what is running now.
async function selectCustom() {
  isRoleMenuOpen.value = false;
  if (!isCustom.value && props.project.modelKey) await store.setModel(props.project.id, props.project.modelKey);
}

function onModelChange(e: Event) {
  store.setModel(props.project.id, (e.target as HTMLSelectElement).value);
}

function onThinkingChange(e: Event) {
  store.setThinking(props.project.id, (e.target as HTMLSelectElement).value);
}
</script>

<template>
  <div class="relative flex items-center justify-between px-3 py-1.5 bg-[#252629] border-t border-b border-[#2b2d30] text-xs font-sans select-none shrink-0">
    <div class="flex items-center gap-1.5 flex-wrap">
      <!-- [Orchestrate] toggle button -->
      <button
        type="button"
        @click="toggleOrchestrate"
        title="Toggle orchestrate prefix for multi-agent workflows"
        :class="[
          'flex items-center gap-1 px-2 py-1 rounded border transition-colors text-xs font-medium cursor-pointer',
          project.orchestrate
            ? 'bg-[#3574f0]/20 text-[#3574f0] border-[#3574f0]'
            : 'bg-[#1e1f22] text-[#bcbec4] border-[#393b40] hover:bg-[#2b2d30] hover:text-[#dfe1e5]'
        ]"
      >
        <Users class="w-3.5 h-3.5" />
        <span>Orchestrate</span>
      </button>

      <!-- [Model: <role> ▾] dropdown: roles + Custom -->
      <div class="relative">
        <button
          type="button"
          @click="toggleRoleMenu"
          title="Select model role (Ctrl+P in the prompt cycles roles)"
          class="flex items-center gap-1 px-2 py-1 rounded border border-[#393b40] bg-[#1e1f22] text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors text-xs font-medium cursor-pointer"
        >
          <span>Model: <span class="text-[#3574f0]">{{ project.role }}</span></span>
          <ChevronDown class="w-3 h-3 text-[#6f737a]" />
        </button>

        <template v-if="isRoleMenuOpen">
          <button
            type="button"
            class="fixed inset-0 z-40 cursor-default bg-transparent border-none p-0 outline-none"
            @click="isRoleMenuOpen = false"
            aria-label="Close menu"
          ></button>
          <div
            :style="{ left: menuPos.left + 'px', bottom: menuPos.bottom + 'px' }"
            class="fixed z-50 w-max min-w-52 max-w-[min(36rem,calc(100vw-1rem))] bg-[#1e1f22] border border-[#393b40] rounded shadow-xl py-1 max-h-60 overflow-y-auto font-sans text-xs"
          >
            <div v-if="roles.length === 0" class="px-3 py-1.5 text-[#6f737a] italic">
              Loading roles…
            </div>
            <template v-else>
              <button
                v-for="r in roles"
                :key="r.name"
                type="button"
                @click="selectRole(r.name)"
                class="w-full flex items-center justify-between px-3 py-1.5 text-left hover:bg-[#2b2d30] cursor-pointer text-[#dfe1e5] transition-colors"
              >
                <div class="flex flex-col truncate">
                  <span :class="['font-medium', r.name === project.role ? 'text-[#3574f0]' : '']">{{ r.name }}</span>
                  <span v-if="r.selector" class="text-[12px] text-[#6f737a] truncate">{{ r.selector }}</span>
                </div>
                <Check v-if="r.name === project.role" class="w-3.5 h-3.5 text-[#3574f0] shrink-0 ml-1" />
              </button>
            </template>
            <div class="border-t border-[#393b40] my-1"></div>
            <button
              type="button"
              @click="selectCustom"
              class="w-full flex items-center justify-between px-3 py-1.5 text-left hover:bg-[#2b2d30] cursor-pointer text-[#dfe1e5] transition-colors"
            >
              <div class="flex flex-col truncate">
                <span :class="['font-medium', isCustom ? 'text-[#3574f0]' : '']">custom</span>
                <span class="text-[12px] text-[#6f737a] truncate">Pick any model</span>
              </div>
              <Check v-if="isCustom" class="w-3.5 h-3.5 text-[#3574f0] shrink-0 ml-1" />
            </button>
          </div>
        </template>
      </div>

      <!-- Custom model picker -->
      <select
        v-if="isCustom"
        :value="project.modelKey"
        @change="onModelChange"
        title="Model"
        class="max-w-64 px-2 py-1 rounded border border-[#393b40] bg-[#1e1f22] text-[#dfe1e5] hover:bg-[#2b2d30] text-xs font-medium cursor-pointer outline-none"
      >
        <option v-if="!modelListed" :value="project.modelKey">{{ project.modelKey || 'loading…' }}</option>
        <optgroup v-for="[provider, list] in modelGroups" :key="provider" :label="provider">
          <option v-for="m in list" :key="m.id" :value="`${m.provider}/${m.id}`">{{ m.name || m.id }}</option>
        </optgroup>
      </select>

      <!-- Thinking level (any role) -->
      <label
        title="Thinking level (Shift+Tab in the prompt cycles levels)"
        class="flex items-center gap-1 px-2 py-1 rounded border border-[#393b40] bg-[#1e1f22] text-[#dfe1e5] hover:bg-[#2b2d30] text-xs font-medium cursor-pointer"
      >
        <Brain class="w-3.5 h-3.5 text-[#6f737a]" />
        <select
          :value="project.thinking"
          @change="onThinkingChange"
          class="bg-[#1e1f22] text-[#3574f0] outline-none cursor-pointer"
        >
          <option v-for="l in thinkingOptions" :key="l" :value="l">{{ l }}</option>
        </select>
      </label>

      <!-- [🛡 Advisor] toggle button -->
      <button
        type="button"
        @click="toggleAdvisor"
        title="Toggle advisor assistant (/advisor on/off)"
        :class="[
          'flex items-center gap-1 px-2 py-1 rounded border transition-colors text-xs font-medium cursor-pointer',
          project.advisor
            ? 'bg-[#59a869]/20 text-[#59a869] border-[#59a869]'
            : 'bg-[#1e1f22] text-[#bcbec4] border-[#393b40] hover:bg-[#2b2d30] hover:text-[#dfe1e5]'
        ]"
      >
        <Shield class="w-3.5 h-3.5" />
        <span>Advisor</span>
      </button>

      <!-- [Review] button -->
      <button
        type="button"
        @click="handleReview"
        title="Request code review (/review)"
        class="flex items-center gap-1 px-2 py-1 rounded border border-[#393b40] bg-[#1e1f22] text-[#bcbec4] hover:bg-[#2b2d30] hover:text-[#dfe1e5] transition-colors text-xs font-medium cursor-pointer"
      >
        <Sparkles class="w-3.5 h-3.5 text-[#e2a53b]" />
        <span>Review</span>
      </button>

      <!-- Caveman level -->
      <label
        title="Caveman terse replies (/caveman)"
        class="flex items-center gap-1 px-2 py-1 rounded border border-[#393b40] bg-[#1e1f22] text-[#dfe1e5] hover:bg-[#2b2d30] text-xs font-medium cursor-pointer"
      >
        <span class="text-[#bcbec4]">Caveman</span>
        <select
          :value="project.caveman"
          @change="store.setAddonMode(project.id, 'caveman', ($event.target as HTMLSelectElement).value)"
          class="bg-[#1e1f22] text-[#3574f0] outline-none cursor-pointer"
        >
          <option v-for="l in CAVEMAN_LEVELS" :key="l" :value="l">{{ l }}</option>
        </select>
      </label>

      <!-- [RTK] toggle button -->
      <button
        type="button"
        @click="store.setAddonMode(project.id, 'rtk', project.rtk ? 'off' : 'on')"
        title="RTK compact shell output (/rtk on|off)"
        :class="[
          'flex items-center gap-1 px-2 py-1 rounded border transition-colors text-xs font-medium cursor-pointer',
          project.rtk
            ? 'bg-[#59a869]/20 text-[#59a869] border-[#59a869]'
            : 'bg-[#1e1f22] text-[#bcbec4] border-[#393b40] hover:bg-[#2b2d30] hover:text-[#dfe1e5]'
        ]"
      >
        <Terminal class="w-3.5 h-3.5" />
        <span>RTK</span>
      </button>

      <!-- Ponytail level -->
      <label
        title="Ponytail minimal-code mode (/ponytail)"
        class="flex items-center gap-1 px-2 py-1 rounded border border-[#393b40] bg-[#1e1f22] text-[#dfe1e5] hover:bg-[#2b2d30] text-xs font-medium cursor-pointer"
      >
        <span class="text-[#bcbec4]">Ponytail</span>
        <select
          :value="project.ponytail"
          @change="store.setAddonMode(project.id, 'ponytail', ($event.target as HTMLSelectElement).value)"
          class="bg-[#1e1f22] text-[#3574f0] outline-none cursor-pointer"
        >
          <option v-for="l in PONYTAIL_LEVELS" :key="l" :value="l">{{ l }}</option>
        </select>
      </label>
    </div>

    <div class="flex items-center gap-1.5">
      <!-- [▣ Phone] omp collab QR -->
      <button
        type="button"
        @click="collabOpen = true"
        title="Control this session from your phone (omp collab QR code)"
        class="flex items-center gap-1 px-2 py-1 rounded border border-[#393b40] bg-[#1e1f22] text-[#bcbec4] hover:bg-[#2b2d30] hover:text-[#dfe1e5] transition-colors text-xs font-medium cursor-pointer"
      >
        <QrCode class="w-3.5 h-3.5" />
        <span>Phone</span>
      </button>

      <!-- [⟲ New] button -->
      <button
        type="button"
        @click="handleNewSession"
        :disabled="project.isStreaming"
        title="Start new session (clear context)"
        class="flex items-center gap-1 px-2 py-1 rounded border border-[#393b40] bg-[#1e1f22] text-[#bcbec4] hover:bg-[#2b2d30] hover:text-[#dfe1e5] transition-colors text-xs font-medium cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
      >
        <RotateCcw class="w-3.5 h-3.5" />
        <span>New</span>
      </button>

      <!-- [■ Stop] button -->
      <button
        type="button"
        @click="handleStop"
        :disabled="!project.isStreaming"
        title="Stop / abort running turn (Esc)"
        :class="[
          'flex items-center gap-1 px-2 py-1 rounded border transition-colors text-xs font-medium cursor-pointer',
          project.isStreaming
            ? 'bg-[#e55765]/20 text-[#e55765] border-[#e55765] hover:bg-[#e55765]/30'
            : 'bg-[#1e1f22] text-[#6f737a] border-[#393b40] opacity-40 cursor-not-allowed'
        ]"
      >
        <Square class="w-3.5 h-3.5 fill-current" />
        <span>Stop</span>
      </button>
    </div>

    <CollabModal :open="collabOpen" :project-id="project.id" @close="collabOpen = false" />
  </div>
</template>
