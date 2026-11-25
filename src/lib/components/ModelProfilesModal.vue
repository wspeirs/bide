<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { Trash2, X } from '@lucide/vue';
import {
  DialogContent, DialogDescription, DialogOverlay, DialogPortal, DialogRoot, DialogTitle,
  TabsContent, TabsList, TabsRoot, TabsTrigger,
} from 'reka-ui';
import { useProjectsStore } from '../stores/projects';
import type { ModelInfo, ModelProfile, ModelProfileScope, ModelProfileScopes, ProfileModelOptions } from '../types';
import ModelPicker from './ModelPicker.vue';

const store = useProjectsStore();
const scopes = ['global', 'local'] as const;
const scope = ref<ModelProfileScope>('global');
const projectId = computed(() => store.activeProject?.id);
const profiles = ref<ModelProfileScopes | null>(null);
const options = ref<ProfileModelOptions>();
const catalogError = ref('');
const catalogLoading = ref(false);
const modelsByKey = computed(() => new Map(options.value?.models.map(model => [`${model.provider}/${model.id}`, model])));
const loading = ref(false);
const loadError = ref('');
const detachedSaveError = ref('');
let opener: HTMLElement | null = null;
const saving = ref(false);

const thinkingLevels = new Set(['off', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max', 'auto']);
const slots = ['primary', 'fallback'] as const;
type ModelSlot = typeof slots[number];
type RoleRow = { role: string; primary: string; fallback: string };
const picker = ref<{ row: RoleRow; slot: ModelSlot }>();
const stagedModel = ref<string | null>(null);
let pickerOpener: HTMLButtonElement | undefined;

function parts(selector: string) {
  const colon = selector.lastIndexOf(':');
  const level = selector.slice(colon + 1);
  return colon >= 0 && thinkingLevels.has(level) && !modelsByKey.value.has(selector)
    ? { model: selector.slice(0, colon), level }
    : { model: selector, level: '' };
}

function makeDraft(name = '', profile?: ModelProfile) {
  const roles = new Set(['default', 'advisor', ...Object.keys(profile?.modelRoles ?? {})]);
  return {
    selected: name,
    name,
    defaultThinking: profile?.defaultThinkingLevel,
    roles: [...roles].map((role): RoleRow => ({
      role,
      primary: profile?.modelRoles[role] ?? '',
      fallback: profile?.modelFallbacks?.[role] ?? '',
    })),
    deleting: false,
    error: '',
  };
}

const drafts = ref({ global: makeDraft(), local: makeDraft() });
const draft = computed(() => drafts.value[scope.value]);
const availableRoles = computed(() => options.value?.roles.filter(role =>
  !draft.value.roles.some(row => row.role === role)) ?? []);

function roleLabel(role: string) {
  if (role === 'default') return 'Primary chat';
  return role[0]!.toUpperCase() + role.slice(1);
}

function slotLabel(row: RoleRow, slot: ModelSlot) {
  return `${roleLabel(row.role)}${slot === 'fallback' ? ' fallback' : ''}`;
}

function roleAccepts(role: string, { kind }: ModelInfo) {
  switch (role) {
    case 'image': return kind === 'image';
    case 'speech': return kind === 'tts';
    case 'dictation': return kind === 'stt';
    case 'web': return kind === 'search' || kind === 'chat';
    case 'judge': return kind === 'judge' || kind === 'tiny' || kind === 'chat';
    case 'tiny':
    case 'memory': return kind === 'tiny' || kind === 'chat';
    default: return !kind || kind === 'chat';
  }
}

const pickerModels = computed(() => {
  const role = picker.value?.row.role;
  return role ? options.value?.models.filter(model => roleAccepts(role, model)) ?? [] : [];
});
const emptyPickerLabel = computed(() => picker.value?.slot === 'fallback' ? 'None'
  : picker.value?.row.role === 'advisor' ? 'Off' : undefined);

function modelLabel(row: RoleRow, slot: ModelSlot) {
  const selector = row[slot];
  if (!selector) return slot === 'fallback' ? 'Add fallback model' : row.role === 'advisor' ? 'Off' : 'Select model';
  const defaultThinking = row.role === 'default' && slot === 'primary' ? draft.value.defaultThinking : undefined;
  return defaultThinking && !parts(selector).level ? `${selector}:${defaultThinking}` : selector;
}

function levelFor(row: RoleRow, slot: ModelSlot) {
  return parts(row[slot]).level || (row.role === 'default' && slot === 'primary' ? draft.value.defaultThinking ?? '' : '');
}

function levelsFor(row: RoleRow, slot: ModelSlot) {
  const level = levelFor(row, slot);
  const levels = modelsByKey.value.get(parts(row[slot]).model)?.thinkingLevels ?? [];
  return level && !levels.includes(level) ? [...levels, level] : levels;
}

function setThinking(row: RoleRow, slot: ModelSlot, event: Event) {
  const level = (event.target as HTMLSelectElement).value;
  const { model } = parts(row[slot]);
  if (row.role === 'default' && slot === 'primary') {
    draft.value.defaultThinking = undefined;
  }
  row[slot] = level ? `${model}:${level}` : model;
}

function addRole(event: Event) {
  const select = event.target as HTMLSelectElement;
  if (availableRoles.value.includes(select.value)) {
    draft.value.roles.push({ role: select.value, primary: '', fallback: '' });
  }
  select.value = '';
}

function openPicker(row: RoleRow, slot: ModelSlot, event: MouseEvent) {
  pickerOpener = event.currentTarget as HTMLButtonElement;
  picker.value = { row, slot };
  stagedModel.value = parts(row[slot]).model || null;
}

function applyPickedModel() {
  const target = picker.value;
  if (!target) return;
  const { row, slot } = target;
  const model = stagedModel.value;
  if (model !== (parts(row[slot]).model || null)) {
    row[slot] = model ?? '';
    if (row.role === 'default' && slot === 'primary') draft.value.defaultThinking = undefined;
    if (slot === 'primary' && !model) row.fallback = '';
  }
  picker.value = undefined;
}

function restorePickerFocus(event: Event) {
  if (!pickerOpener?.isConnected) return;
  event.preventDefault();
  pickerOpener.focus();
}
const names = computed(() => Object.keys(profiles.value?.[scope.value] ?? {}).sort((a, b) => a.localeCompare(b)));
const replacesGlobal = computed(() => scope.value === 'local'
  && Object.hasOwn(profiles.value?.global ?? {}, draft.value.name.trim()));
const retry = ref(0);

watch([() => store.modelProfilesOpen, projectId, retry], async ([open, id], [wasOpen, oldId], onCleanup) => {
  if (!open) return;
  picker.value = undefined;
  if (!wasOpen) {
    opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    detachedSaveError.value = '';
    scope.value = 'global';
    drafts.value = { global: makeDraft(), local: makeDraft() };
  } else if (id !== oldId) {
    drafts.value.local = makeDraft();
    if (!id) scope.value = 'global';
  }

  let current = true;
  onCleanup(() => { current = false; });
  profiles.value = null;
  options.value = undefined;
  catalogError.value = '';
  catalogLoading.value = true;
  loading.value = true;
  loadError.value = '';
  void store.getProfileModelOptions(id).then((result) => {
    if (current) options.value = result;
  }).catch((err) => {
    if (current) catalogError.value = `Could not load the model catalog: ${err}`;
  }).finally(() => {
    if (current) catalogLoading.value = false;
  });
  try {
    const result = await store.getModelProfiles(id);
    if (current) profiles.value = result;
  } catch (err) {
    if (current) loadError.value = `Could not load model profiles: ${err}`;
  } finally {
    if (current) loading.value = false;
  }
}, { immediate: true, flush: 'sync' });

function selectProfile(event: Event) {
  const name = (event.target as HTMLSelectElement).value;
  drafts.value[scope.value] = makeDraft(name, name ? profiles.value?.[scope.value][name] : undefined);
}

function close() {
  if (!saving.value) store.modelProfilesOpen = false;
}

function restoreFocus(event: Event) {
  if (!opener?.isConnected) return;
  event.preventDefault();
  opener.focus();
}

async function save(remove = false) {
  const currentScope = scope.value;
  const currentDraft = draft.value;
  const id = projectId.value;
  const name = currentDraft.name.trim();
  if (saving.value || !profiles.value || (currentScope === 'local' && !id)) return;
  if (remove !== currentDraft.deleting || (remove && !currentDraft.selected)) return;
  detachedSaveError.value = '';
  currentDraft.error = '';

  let profile: ModelProfile | null = null;
  if (!remove) {
    if (!name || !currentDraft.roles[0]?.primary) {
      currentDraft.error = 'Enter a profile name and select a default model.';
      return;
    }
    if (!currentDraft.selected && Object.hasOwn(profiles.value[currentScope], name)) {
      currentDraft.error = 'That name already exists in this tab. Select the existing profile to edit it.';
      return;
    }
    const modelRoles: Record<string, string> = Object.create(null);
    const modelFallbacks: Record<string, string> = Object.create(null);
    for (const { role, primary, fallback } of currentDraft.roles) {
      if (!primary) {
        if (role === 'advisor') continue;
        currentDraft.error = `Select a model for ${roleLabel(role)}, or remove the role.`;
        return;
      }
      modelRoles[role] = primary;
      if (fallback) modelFallbacks[role] = fallback;
    }
    profile = {
      modelRoles,
      defaultThinkingLevel: currentDraft.defaultThinking,
      modelFallbacks: Object.keys(modelFallbacks).length ? modelFallbacks : undefined,
    };
  }

  saving.value = true;
  try {
    const warning = await store.saveModelProfile(id, currentScope, name, profile);
    if (drafts.value[currentScope] === currentDraft) {
      drafts.value[currentScope] = profile ? makeDraft(name, profile) : makeDraft();
    }
    if (id === projectId.value && profiles.value) {
      if (profile) {
        profiles.value[currentScope] = { ...profiles.value[currentScope], [name]: profile };
      } else {
        delete profiles.value[currentScope][name];
      }
    } else if (currentScope === 'global' || id === projectId.value) {
      retry.value++;
    }
    if (warning) detachedSaveError.value = warning;
  } catch (err) {
    const error = `Could not ${remove ? 'delete' : 'save'} “${name}”: ${err}`;
    if (drafts.value[currentScope] === currentDraft) currentDraft.error = error;
    else detachedSaveError.value = error;
  } finally {
    saving.value = false;
  }
}

const inputClass = 'min-w-0 w-full px-2.5 py-1.5 bg-[#141416] border border-[#393b40] rounded text-[#dfe1e5] placeholder:text-[#9da0a8] focus:outline-none focus:border-[#3574f0]';
const buttonClass = 'px-3 py-1.5 rounded border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed';
</script>

<template>
  <DialogRoot :open="store.modelProfilesOpen" @update:open="!$event && close()">
    <DialogPortal>
      <DialogOverlay class="fixed inset-0 z-50 bg-black/60 backdrop-blur-xs" />
      <DialogContent
        @close-auto-focus="restoreFocus"
        class="fixed top-1/2 left-1/2 z-50 -translate-x-1/2 -translate-y-1/2 w-[calc(100%-2rem)] max-w-xl max-h-[85vh] flex flex-col overflow-hidden bg-[#1e1f22] text-[#dfe1e5] border border-[#393b40] rounded shadow-2xl text-xs font-sans"
      >
        <header class="flex items-start justify-between gap-3 px-4 py-2.5 bg-[#252629] border-b border-[#2b2d30] shrink-0">
          <div class="space-y-0.5">
            <DialogTitle class="font-semibold text-sm">Model profiles</DialogTitle>
            <DialogDescription class="text-[#bcbec4]">
              Assign a model and thinking level to each role. Fallbacks use omp’s retry policy for provider errors, including quota exhaustion, rate limits, and temporary outages.
            </DialogDescription>
          </div>
          <button
            type="button" aria-label="Close model profiles" :disabled="saving"
            class="p-1 rounded text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#35373c] cursor-pointer disabled:opacity-50"
            @click="close"
          >
            <X class="w-4 h-4" />
          </button>
        </header>
        <p v-if="detachedSaveError" role="alert" class="px-4 py-2 bg-[#e55765]/10 text-[#e55765] break-words">{{ detachedSaveError }}</p>

        <TabsRoot v-model="scope" class="flex flex-col min-h-0">
          <TabsList aria-label="Profile scope" class="flex items-center gap-1 px-4 pt-2 bg-[#252629] border-b border-[#2b2d30] shrink-0">
            <TabsTrigger
              v-for="tab in scopes" :key="tab" :value="tab" :disabled="saving || (tab === 'local' && !projectId)"
              class="px-3 py-1.5 font-medium border-b-2 border-transparent text-[#bcbec4] hover:text-[#dfe1e5] data-[state=active]:border-[#3574f0] data-[state=active]:bg-[#1e1f22] data-[state=active]:text-[#dfe1e5] cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {{ tab === 'global' ? 'Global' : 'Local' }}
            </TabsTrigger>
          </TabsList>

          <TabsContent v-for="tab in scopes" :key="tab" :value="tab" class="min-h-0 flex flex-col data-[state=inactive]:hidden">
            <div class="px-4 pt-3 pb-2 text-[#bcbec4]">
              <template v-if="tab === 'local'">
                Profiles for {{ store.activeProject?.name }}. A local profile with the same name replaces the entire global profile, including its overrides.
              </template>
              <template v-else>
                Profiles available in all projects unless replaced by a local profile with the same name.
                <span v-if="!projectId">Open a project to manage local profiles.</span>
              </template>
            </div>
            <p v-if="loading" role="status" class="px-4 pb-4 text-[#bcbec4]">Loading model profiles…</p>
            <div v-else-if="loadError" class="px-4 pb-4 space-y-2">
              <p role="alert" class="text-[#e55765]">{{ loadError }}</p>
              <button type="button" :class="buttonClass" @click="retry++">Retry</button>
            </div>
            <form v-else-if="profiles" class="min-h-0 flex flex-col" @submit.prevent="save()">
              <div class="min-h-0 overflow-y-auto">
                <fieldset :disabled="saving" class="min-w-0 space-y-3 px-4 py-2 disabled:opacity-60">
                  <label class="block space-y-1">
                    <span class="font-medium">Profile</span>
                    <select :value="draft.selected" :class="inputClass" @change="selectProfile">
                      <option value="">New profile</option>
                      <option v-for="name in names" :key="name" :value="name">{{ name }}</option>
                    </select>
                  </label>
                  <label class="block space-y-1">
                    <span class="font-medium">Name</span>
                    <input v-model="draft.name" :readonly="Boolean(draft.selected)" :class="inputClass" required />
                  </label>
                  <p v-if="replacesGlobal" class="text-[#bcbec4]">This profile replaces the global “{{ draft.name.trim() }}” profile in this project.</p>
                  <div v-if="catalogError" class="space-y-2">
                    <p role="alert" class="text-[#e55765]">{{ catalogError }}</p>
                    <button type="button" :class="buttonClass" @click="retry++">Retry catalog</button>
                  </div>
                  <div v-for="(row, index) in draft.roles" :key="row.role" class="space-y-2 border-t border-[#393b40] pt-2">
                    <div class="flex items-center justify-between">
                      <span class="font-medium">{{ roleLabel(row.role) }}</span>
                      <button
                        v-if="index > 1" type="button" :aria-label="`Remove ${roleLabel(row.role)} role override`"
                        class="p-1 rounded text-[#bcbec4] hover:text-[#e55765] hover:bg-[#e55765]/10 cursor-pointer"
                        @click="draft.roles.splice(index, 1)"
                      ><Trash2 class="w-3.5 h-3.5" /></button>
                    </div>
                    <div v-for="slot in slots" :key="slot" class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
                      <button
                        type="button" aria-haspopup="dialog" :aria-label="`${slotLabel(row, slot)} model: ${modelLabel(row, slot)}`"
                        :disabled="slot === 'fallback' && !row.primary"
                        :title="modelLabel(row, slot)" :class="[buttonClass, 'min-w-0 wrap-anywhere text-left']"
                        @click="openPicker(row, slot, $event)"
                      >{{ slot === 'fallback' && row.fallback ? 'Fallback: ' : '' }}{{ modelLabel(row, slot) }}</button>
                      <label class="flex items-center gap-2">
                        Thinking
                        <select
                          :aria-label="`${slotLabel(row, slot)} thinking level`" :value="levelFor(row, slot)"
                          :disabled="!row[slot] || !levelsFor(row, slot).length"
                          class="w-24 px-2 py-1 rounded border border-[#393b40] bg-[#1e1f22] focus-visible:outline-[#3574f0] disabled:opacity-50"
                          @change="setThinking(row, slot, $event)"
                        >
                          <option value="">Default</option>
                          <option v-for="level in levelsFor(row, slot)" :key="level" :value="level">{{ level }}</option>
                        </select>
                      </label>
                    </div>
                  </div>
                  <select aria-label="Add role override" :disabled="!availableRoles.length" :class="inputClass" value="" @change="addRole">
                    <option value="" disabled>Add role override…</option>
                    <option v-for="role in availableRoles" :key="role" :value="role">{{ roleLabel(role) }}</option>
                  </select>
                  <p class="text-[#bcbec4]">Leave Advisor off to disable it. Unassigned chat roles use the primary chat model; specialized and background roles keep their omp defaults.</p>
                </fieldset>
              </div>

              <p v-if="draft.error" role="alert" class="px-4 py-2 bg-[#e55765]/10 border-t border-[#e55765]/30 text-[#e55765] break-words">{{ draft.error }}</p>
              <footer class="flex shrink-0 flex-wrap items-center justify-between gap-2 px-4 py-2.5 bg-[#252629] border-t border-[#2b2d30]">
                <template v-if="draft.deleting">
                  <span class="text-[#bcbec4]">Delete “{{ draft.selected }}” from {{ scope }} profiles?</span>
                  <div class="flex gap-2">
                    <button type="button" :class="buttonClass" :disabled="saving" @click="draft.deleting = false">Cancel</button>
                    <button type="button" :class="[buttonClass, 'text-[#e55765]']" :disabled="saving" @click="save(true)">{{ saving ? 'Deleting…' : 'Confirm delete' }}</button>
                  </div>
                </template>
                <template v-else>
                  <button v-if="draft.selected" type="button" :class="[buttonClass, 'text-[#e55765]']" :disabled="saving" @click="draft.deleting = true">Delete profile</button>
                  <button type="submit" :disabled="saving" class="ml-auto px-3 py-1.5 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer">
                    {{ saving ? 'Saving…' : 'Save profile' }}
                  </button>
                </template>
              </footer>
            </form>
          </TabsContent>
        </TabsRoot>
        <ModelPicker
          :open="!!picker" :title="picker ? `Select ${slotLabel(picker.row, picker.slot).toLowerCase()} model` : 'Select model'"
          :model="stagedModel" :models="pickerModels" :empty-label="emptyPickerLabel"
          :loading="catalogLoading" :error="catalogError"
          :cannot-apply="stagedModel === null && !emptyPickerLabel"
          @update:open="!$event && (picker = undefined)" @select="stagedModel = $event"
          @apply="applyPickedModel" @close-auto-focus="restorePickerFocus"
        />
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
