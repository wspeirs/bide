<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { useProjectsStore } from '../stores/projects';
import type { ProjectState, ModelInfo } from '../types';
import { Users, RotateCcw, Square, Settings, Sparkles, QrCode, Terminal } from '@lucide/vue';
import CollabModal from './CollabModal.vue';
import ModelPicker from './ModelPicker.vue';

const CAVEMAN_LEVELS = ['off', 'lite', 'full', 'ultra', 'wenyan'] as const;
const PONYTAIL_LEVELS = ['off', 'lite', 'full', 'ultra'] as const;
const MODEL_ROLES = ['Primary', 'Advisor'] as const;
type ModelRole = typeof MODEL_ROLES[number];

const props = defineProps<{
  project: ProjectState;
}>();

const store = useProjectsStore();

const profileNames = ref<string[]>([]);
const collabOpen = ref(false);
const models = ref<ModelInfo[]>([]);
const openModelRole = ref<ModelRole | null>(null);
const modelsLoading = ref(false);
const modelSaving = ref(false);
const modelError = ref('');
const draftModel = ref<string | null>(null);
const draftThinking = ref('');
const isCustom = computed(() => props.project.profile === undefined);
const modelRoles = computed(() => isCustom.value ? MODEL_ROLES : []);
const isSwitching = computed(() => store.profileSwitching.has(props.project.id));
const modelEditingDisabled = computed(() => isSwitching.value || modelSaving.value ||
  props.project.isStreaming || props.project.status === 'busy');
const draftModelInfo = computed(() =>
  models.value.find((model) => `${model.provider}/${model.id}` === draftModel.value));
const draftThinkingLevels = computed(() => draftModelInfo.value?.thinkingLevels ?? []);
const selectedThinking = computed(() =>
  draftThinkingLevels.value.includes(draftThinking.value) ? draftThinking.value : '');
const cannotApply = computed(() => modelEditingDisabled.value ||
  (draftModel.value === null ? openModelRole.value === 'Primary' : modelsLoading.value));

function modelKeyFor(role: ModelRole) {
  return (role === 'Primary' ? props.project.modelKey : props.project.advisorModelKey) ?? null;
}

function thinkingFor(role: ModelRole) {
  return (role === 'Primary' ? props.project.thinking : props.project.advisorThinking) ?? '';
}

function modelLabelFor(role: ModelRole) {
  const key = modelKeyFor(role);
  if (!key) return role === 'Advisor' ? 'Off' : 'Select model';
  const name = role === 'Primary' ? props.project.model : props.project.advisorModel;
  if (name) return `${name} · ${key.split('/')[0]}`;
  return role === 'Advisor' ? `Unavailable (${key})` : key;
}

watch(
  [() => props.project.id, () => store.profileRevision],
  async ([id], previous, onCleanup) => {
    if (id !== previous?.[0]) profileNames.value = [];
    let stale = false;
    onCleanup(() => { stale = true; });
    const names = await store.listProfileNames(id);
    if (!stale && names) profileNames.value = names;
  },
  { immediate: true },
);

watch(
  [() => props.project.id, isCustom],
  async ([id, custom], previous, onCleanup) => {
    if (id !== previous?.[0]) models.value = [];
    openModelRole.value = null;
    modelsLoading.value = custom && models.value.length === 0;
    if (!modelsLoading.value) return;
    let stale = false;
    onCleanup(() => { stale = true; });
    const list = await store.listModels(id);
    if (!stale) {
      models.value = list;
      modelsLoading.value = false;
    }
  },
  { immediate: true },
);

watch(isSwitching, (switching) => {
  if (switching) openModelRole.value = null;
});

function updateModelPicker(role: ModelRole, open: boolean) {
  if (modelSaving.value) return;
  if (open) {
    if (!isCustom.value || modelEditingDisabled.value) return;
    draftModel.value = modelKeyFor(role);
    draftThinking.value = thinkingFor(role);
    modelError.value = '';
    openModelRole.value = role;
  } else if (openModelRole.value === role) {
    openModelRole.value = null;
  }
}

function toggleOrchestrate() {
  store.setOrchestrate(props.project.id, !props.project.orchestrate);
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

function onProfileChange(e: Event) {
  const select = e.target as HTMLSelectElement;
  const name = select.value || null;
  select.value = props.project.profile ?? '';
  store.setProfile(props.project.id, name);
}

function stageModel(key: string | null) {
  if (modelEditingDisabled.value || key === draftModel.value) return;
  draftModel.value = key;
  const role = openModelRole.value;
  draftThinking.value = role && key === modelKeyFor(role) ? thinkingFor(role) : '';
  modelError.value = '';
}

async function applyModel(role: ModelRole) {
  if (!isCustom.value || cannotApply.value || openModelRole.value !== role) return;
  const key = draftModel.value;
  const level = selectedThinking.value || null;
  const id = props.project.id;
  modelError.value = '';
  modelSaving.value = true;
  try {
    const sameModel = key === modelKeyFor(role);
    if (role === 'Advisor') {
      if (!sameModel || (key && ((level ?? '') !== thinkingFor(role) || !props.project.advisorModel))) {
        await store.setAdvisor(id, key, level);
      }
    } else if (key !== null) {
      if (!sameModel) await store.setModel(id, key, level);
      else if (level && level !== thinkingFor(role)) await store.setThinking(id, level);
    }
    if (props.project.id === id) openModelRole.value = null;
  } catch (err) {
    if (props.project.id === id) {
      modelError.value = `Failed to select model: ${err instanceof Error ? err.message : String(err)}`;
    }
  } finally {
    modelSaving.value = false;
  }
}
</script>

<template>
  <div class="relative flex items-center justify-between px-3 py-1.5 bg-[#252629] border-t border-b border-[#2b2d30] text-xs font-sans select-none shrink-0">
    <div class="flex items-center gap-1.5 flex-wrap">
      <!-- [Orchestrate] toggle button -->
      <button
        type="button"
        @click="toggleOrchestrate"
        :aria-pressed="Boolean(project.orchestrate)"
        title="Use the primary model to orchestrate with omp’s configured subagent models"
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

      <select
        :value="project.profile ?? ''"
        :disabled="modelEditingDisabled"
        @change="onProfileChange"
        aria-label="Model profile"
        title="Model profile (Ctrl+P in the prompt cycles profiles)"
        class="max-w-64 px-2 py-1 rounded border border-[#393b40] bg-[#1e1f22] text-[#dfe1e5] hover:bg-[#2b2d30] text-xs font-medium cursor-pointer focus-visible:outline-[#3574f0] disabled:opacity-50 disabled:cursor-default"
      >
        <option v-if="project.profile !== undefined && !profileNames.includes(project.profile)" :value="project.profile">{{ project.profile }}</option>
        <option v-for="name in profileNames" :key="name" :value="name">{{ name }}</option>
        <option value="">Custom</option>
      </select>
      <button
        type="button"
        title="Model profile settings"
        aria-label="Model profile settings"
        @click="store.modelProfilesOpen = true"
        class="flex items-center px-1.5 py-1 rounded border border-[#393b40] bg-[#1e1f22] text-[#bcbec4] hover:bg-[#2b2d30] hover:text-[#dfe1e5] transition-colors cursor-pointer focus-visible:outline-[#3574f0]"
      >
        <Settings class="w-3.5 h-3.5" />
      </button>

      <ModelPicker
        v-for="role in modelRoles" :key="`${project.id}:${role}`"
        :open="openModelRole === role" :title="`Select ${role.toLowerCase()} model`"
        :models="models" :model="draftModel" :empty-label="role === 'Advisor' ? 'Off' : undefined"
        :loading="modelsLoading" :disabled="modelEditingDisabled" :saving="modelSaving"
        :error="modelError" :cannot-apply="cannotApply"
        @update:open="updateModelPicker(role, $event)" @select="stageModel" @apply="applyModel(role)"
      >
        <template #trigger>
          <button
            type="button" :disabled="modelEditingDisabled" :title="`${role}: ${modelLabelFor(role)}`"
            class="max-w-48 truncate px-2 py-1 rounded border border-[#393b40] bg-[#1e1f22] text-[#dfe1e5] hover:bg-[#2b2d30] text-xs font-medium cursor-pointer focus-visible:outline-[#3574f0] disabled:opacity-50"
          >
            {{ role }}: {{ modelLabelFor(role) }}
          </button>
        </template>
        <template #thinking>
          <label class="flex items-center gap-2">
            Thinking level
            <select
              :value="selectedThinking"
              :disabled="modelEditingDisabled || !draftThinkingLevels.length"
              @change="draftThinking = ($event.target as HTMLSelectElement).value"
              class="px-2 py-1 rounded border border-[#393b40] bg-[#1e1f22] focus-visible:outline-[#3574f0] disabled:opacity-50"
            >
              <option value="">{{ role === 'Primary' ? 'Keep current' : 'Default' }}</option>
              <option v-for="level in draftThinkingLevels" :key="level" :value="level">{{ level }}</option>
            </select>
          </label>
        </template>
      </ModelPicker>

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
