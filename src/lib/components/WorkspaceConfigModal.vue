<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { X, Plus, Trash2, Eye, EyeOff } from '@lucide/vue';
import { useProjectsStore } from '../stores/projects';
import type { ProjectState } from '../types';

interface EnvRow {
  name: string;
  value: string;
  revealed: boolean;
}

const KNOWN_OMP_ENV_VARS: Array<{ name: string; description: string }> = [
  // Core Providers
  { name: 'GEMINI_API_KEY', description: 'Google Gemini models' },
  { name: 'ANTHROPIC_API_KEY', description: 'Anthropic Claude models' },
  { name: 'ANTHROPIC_OAUTH_TOKEN', description: 'Anthropic OAuth (takes precedence over API key)' },
  { name: 'OPENAI_API_KEY', description: 'OpenAI GPT models' },
  { name: 'OPENROUTER_API_KEY', description: 'OpenRouter aggregated models' },
  { name: 'COPILOT_GITHUB_TOKEN', description: 'GitHub Copilot' },
  { name: 'AZURE_OPENAI_API_KEY', description: 'Azure OpenAI models' },
  { name: 'GROQ_API_KEY', description: 'Groq models' },
  { name: 'CEREBRAS_API_KEY', description: 'Cerebras models' },
  { name: 'XAI_API_KEY', description: 'xAI Grok models' },
  { name: 'MISTRAL_API_KEY', description: 'Mistral models' },
  { name: 'ZAI_API_KEY', description: 'z.ai models (ZhipuAI/GLM)' },
  { name: 'KILO_API_KEY', description: 'Kilo Gateway models' },
  { name: 'MINIMAX_API_KEY', description: 'MiniMax models' },
  { name: 'OPENCODE_API_KEY', description: 'OpenCode Zen/OpenCode Go models' },
  { name: 'CURSOR_ACCESS_TOKEN', description: 'Cursor AI models' },
  { name: 'CLINE_API_KEY', description: 'ClinePass subscription models' },
  { name: 'AI_GATEWAY_API_KEY', description: 'Vercel AI Gateway' },
  { name: 'STEPFUN_API_KEY', description: 'StepFun Step models' },
  { name: 'HELMCODE_API_KEY', description: 'Helmcode private EU inference' },
  { name: 'WAFER_SERVERLESS_API_KEY', description: 'Wafer Serverless' },
  { name: 'YOLO_AUTO_API_KEY', description: 'Yolo-Auto flat-rate Qwen models' },
  { name: 'SINGULARITYAPI_DEV_API_KEY', description: 'SingularityAPI universal gateway' },
  { name: 'SINGULARITYAPI_TECH_API_KEY', description: 'SingularityAPI reserved DeepSeek lanes' },
  { name: 'SCW_SECRET_KEY', description: 'Scaleway Generative APIs models' },
  { name: 'OTARI_API_KEY', description: 'Otari AI gateway (workspace models)' },
  { name: 'OTARI_BASE_URL', description: 'Otari self-hosted API root' },
  { name: 'UMANS_API_KEY', description: 'Umans AI models' },
  { name: 'UMANS_AI_CODING_PLAN_API_KEY', description: 'Umans AI Coding Plan models' },
  { name: 'ABLITERATION_API_KEY', description: 'Abliteration uncensored GLM models' },
  { name: 'COMMAND_CODE_API_KEY', description: 'Command Code Provider API models' },
  { name: 'CHARM_HYPER_API_KEY', description: 'Charm Hyper inference gateway models' },
  // Foundry / mTLS
  { name: 'CLAUDE_CODE_USE_FOUNDRY', description: 'Enable Anthropic Foundry mode' },
  { name: 'FOUNDRY_BASE_URL', description: 'Anthropic Foundry base URL' },
  { name: 'ANTHROPIC_FOUNDRY_API_KEY', description: 'Anthropic Foundry Bearer token' },
  { name: 'ANTHROPIC_CUSTOM_HEADERS', description: 'Extra headers for custom ANTHROPIC_BASE_URL' },
  // Cloud Providers
  { name: 'AWS_PROFILE', description: 'AWS Bedrock profile' },
  { name: 'AWS_ACCESS_KEY_ID', description: 'AWS Bedrock access key ID' },
  { name: 'AWS_SECRET_ACCESS_KEY', description: 'AWS Bedrock secret key' },
  { name: 'GOOGLE_CLOUD_PROJECT', description: 'Google Vertex AI project' },
  { name: 'GOOGLE_CLOUD_LOCATION', description: 'Google Vertex AI location' },
  { name: 'GOOGLE_APPLICATION_CREDENTIALS', description: 'Vertex AI service account JSON path' },
  // Search & Tools
  { name: 'EXA_API_KEY', description: 'Exa web search' },
  { name: 'BRAVE_API_KEY', description: 'Brave web search' },
  { name: 'PERPLEXITY_API_KEY', description: 'Perplexity web search' },
  { name: 'TAVILY_API_KEY', description: 'Tavily web search' },
  { name: 'TINYFISH_API_KEY', description: 'TinyFish web search' },
  { name: 'KEENABLE_API_KEY', description: 'Keenable web search' },
  { name: 'SOFYA_API_KEY', description: 'Sofya web search' },
  { name: 'FIRECRAWL_API_KEY', description: 'Firecrawl web search + fetch' },
  { name: 'ANTHROPIC_SEARCH_API_KEY', description: 'Anthropic web search override' },
  { name: 'ANTHROPIC_SEARCH_BASE_URL', description: 'Anthropic web search base URL' },
  { name: 'TYPESAFE_API_KEY', description: 'TypeSafe System One judgments' },
  // Configuration
  { name: 'OMP_PROFILE', description: 'Named profile for isolated agent state' },
  { name: 'PI_CODING_AGENT_DIR', description: 'Session storage directory' },
  { name: 'PI_SMOL_MODEL', description: 'Override smol/fast model' },
  { name: 'PI_SLOW_MODEL', description: 'Override slow/reasoning model' },
  { name: 'PI_PLAN_MODEL', description: 'Override planning model' },
];

const props = defineProps<{
  open: boolean;
  project: ProjectState;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

const store = useProjectsStore();

const activeTab = ref<'config' | 'env'>('config');
const text = ref('');
const source = ref<'workspace' | 'global' | 'none'>('none');
const warning = ref<string | null>(null);
const envRows = ref<EnvRow[]>([]);
const configDirty = ref(false);
const envDirty = ref(false);
const error = ref<string | null>(null);
const loading = ref(false);
const saving = ref(false);
const configLoadFailed = ref(false);
const envLoadFailed = ref(false);

// Save only what was edited (one backend call → one restart). With nothing edited, Save applies the visible tab.
const saveConfig = computed(
  () =>
    !warning.value &&
    !configLoadFailed.value &&
    (configDirty.value || (activeTab.value === 'config' && !envDirty.value))
);
// An unreadable env file is only overwritten after an explicit edit.
const saveEnv = computed(
  () => envDirty.value || (activeTab.value === 'env' && !configDirty.value && !envLoadFailed.value)
);
const canSave = computed(() => !saving.value && !loading.value && (saveConfig.value || saveEnv.value));

function errText(err: unknown): string {
  return typeof err === 'string' ? err : (err as Error)?.message || String(err);
}

function handleClose() {
  emit('close');
}

function handleKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    handleClose();
  }
}

function addEnvRow() {
  envRows.value.push({ name: '', value: '', revealed: false });
  envDirty.value = true;
}

function removeEnvRow(idx: number) {
  envRows.value.splice(idx, 1);
  envDirty.value = true;
}

function buildEnvMap(): Record<string, string> {
  const out: Record<string, string> = {};
  for (const row of envRows.value) {
    const name = row.name.trim();
    if (!name && !row.value) continue;
    if (!name) {
      throw new Error('Environment variable name cannot be empty when a value is set.');
    }
    if (name.includes('=') || /[\s\0]/.test(name)) {
      throw new Error(`Invalid environment variable name "${name}": must not contain '=', whitespace, or null bytes.`);
    }
    if (name.toUpperCase() === 'PATH') {
      throw new Error('PATH cannot be set per repository.');
    }
    if (row.value.includes('\0')) {
      throw new Error(`Value of "${name}" must not contain null bytes.`);
    }
    if (Object.prototype.hasOwnProperty.call(out, name)) {
      throw new Error(`Duplicate environment variable "${name}".`);
    }
    out[name] = row.value;
  }
  return out;
}

async function handleSave() {
  saving.value = true;
  error.value = null;
  try {
    const envMap = saveEnv.value ? buildEnvMap() : null;
    await store.saveWorkspace(props.project.id, saveConfig.value ? text.value : null, envMap);
    handleClose();
  } catch (err) {
    error.value = errText(err);
  } finally {
    saving.value = false;
  }
}

watch(
  () => [props.open, props.project.id] as const,
  ([open, id], _, onCleanup) => {
    if (open) {
      let cancelled = false;
      onCleanup(() => {
        cancelled = true;
      });
      loading.value = true;
      configLoadFailed.value = false;
      envLoadFailed.value = false;
      error.value = null;
      activeTab.value = 'config';
      text.value = '';
      source.value = 'none';
      warning.value = null;
      envRows.value = [];
      configDirty.value = false;
      envDirty.value = false;

      Promise.allSettled([store.getWorkspaceConfig(id), store.getWorkspaceEnv(id)]).then(([cfg, env]) => {
        if (cancelled) return;
        const errors: string[] = [];
        if (cfg.status === 'fulfilled') {
          text.value = cfg.value.text;
          source.value = cfg.value.source;
          warning.value = cfg.value.warning;
        } else {
          configLoadFailed.value = true;
          errors.push(errText(cfg.reason));
        }
        if (env.status === 'fulfilled') {
          envRows.value = Object.entries(env.value).map(([name, value]) => ({ name, value, revealed: false }));
        } else {
          envLoadFailed.value = true;
          errors.push(`${errText(env.reason)} — saving environment variables will replace the file.`);
        }
        error.value = errors.length ? errors.join('\n') : null;
        loading.value = false;
      });
    }
  },
  { immediate: true }
);
</script>

<template>
  <div
    v-if="open"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs select-none"
    @keydown="handleKeyDown"
  >
    <button
      type="button"
      class="fixed inset-0 cursor-default bg-transparent border-none p-0 outline-none"
      aria-label="Close dialog"
      @click="handleClose"
    ></button>

    <div class="relative w-full max-w-3xl h-[80vh] flex flex-col bg-[#1e1f22] border border-[#393b40] rounded shadow-2xl overflow-hidden z-10 text-xs font-sans">
      <!-- Header -->
      <div class="flex items-start justify-between px-4 py-2.5 bg-[#252629] border-b border-[#2b2d30] shrink-0">
        <div class="flex flex-col gap-0.5">
          <span class="font-semibold text-[#dfe1e5] text-sm">omp config — {{ project.name }}</span>
          <span class="text-[11px] text-[#6f737a]">
            <template v-if="activeTab === 'env'">
              Shared by all worktrees of this repo. Stored outside the repo in ~/.config/bide/workspaces/ and passed to omp.
            </template>
            <template v-else-if="source === 'workspace'">
              Shared by all worktrees of this repo. Written to .omp/config.yml (git-excluded) when a workspace opens.
            </template>
            <template v-else-if="source === 'global'">
              No workspace config yet — starting from your global omp config. Nothing is written until you save.
            </template>
            <template v-else>
              No workspace config yet.
            </template>
          </span>
        </div>
        <button
          type="button"
          class="p-1 rounded text-[#6f737a] hover:text-[#dfe1e5] hover:bg-[#35373c] transition-colors cursor-pointer"
          @click="handleClose"
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <!-- Tabs -->
      <div class="flex items-center gap-1 px-4 pt-2 bg-[#252629] border-b border-[#2b2d30] shrink-0">
        <button
          type="button"
          class="px-3 py-1.5 text-xs font-medium rounded-t border-b-2 transition-colors cursor-pointer"
          :class="
            activeTab === 'config'
              ? 'text-[#dfe1e5] border-[#3574f0] bg-[#1e1f22]'
              : 'text-[#6f737a] border-transparent hover:text-[#bcbec4]'
          "
          @click="activeTab = 'config'"
        >
          config.yml
        </button>
        <button
          type="button"
          class="px-3 py-1.5 text-xs font-medium rounded-t border-b-2 transition-colors cursor-pointer flex items-center gap-1.5"
          :class="
            activeTab === 'env'
              ? 'text-[#dfe1e5] border-[#3574f0] bg-[#1e1f22]'
              : 'text-[#6f737a] border-transparent hover:text-[#bcbec4]'
          "
          @click="activeTab = 'env'"
        >
          <span>Environment Variables</span>
          <span
            v-if="envRows.length > 0"
            class="px-1.5 py-0.2 rounded-full text-[10px] bg-[#35373c] text-[#bcbec4]"
          >
            {{ envRows.length }}
          </span>
        </button>
      </div>

      <!-- Warning if tracked (config tab only) -->
      <div
        v-if="activeTab === 'config' && warning"
        class="px-4 py-2 bg-[#e55765]/10 border-b border-[#e55765]/30 text-[#e55765] text-xs shrink-0"
      >
        {{ warning }}
      </div>

      <!-- Body -->
      <div class="flex-1 p-3 flex flex-col overflow-hidden bg-[#1e1f22]">
        <div v-if="loading" class="flex-1 flex items-center justify-center text-[#6f737a]">
          Loading config...
        </div>

        <!-- config.yml editor -->
        <textarea
          v-else-if="activeTab === 'config'"
          v-model="text"
          :disabled="Boolean(warning) || saving || configLoadFailed"
          spellcheck="false"
          class="flex-1 w-full h-full p-2.5 bg-[#141416] border border-[#393b40] rounded text-[#bcbec4] font-mono text-xs resize-none focus:outline-none focus:border-[#3574f0] select-text disabled:opacity-50 disabled:cursor-not-allowed leading-relaxed"
          placeholder="# Per-workspace omp config (YAML format)"
          @input="configDirty = true"
        ></textarea>

        <!-- Environment Variables editor -->
        <div v-else class="flex-1 flex flex-col overflow-hidden gap-2">
          <datalist id="known-omp-env-vars">
            <option
              v-for="item in KNOWN_OMP_ENV_VARS"
              :key="item.name"
              :value="item.name"
            >
              {{ item.description }}
            </option>
          </datalist>

          <div class="flex-1 overflow-y-auto space-y-2 pr-1">
            <div
              v-if="envRows.length === 0"
              class="p-6 border border-dashed border-[#393b40] rounded text-center text-[#6f737a] select-text"
            >
              No repository environment variables configured yet. Add one below (for example
              <code class="text-[#bcbec4] font-mono">GEMINI_API_KEY</code> or
              <code class="text-[#bcbec4] font-mono">OPENROUTER_API_KEY</code>).
            </div>

            <div
              v-for="(row, idx) in envRows"
              :key="idx"
              class="flex items-center gap-2"
            >
              <input
                v-model="row.name"
                type="text"
                list="known-omp-env-vars"
                spellcheck="false"
                autocomplete="off"
                :disabled="saving"
                placeholder="NAME (e.g. GEMINI_API_KEY)"
                class="w-[42%] px-2.5 py-1.5 bg-[#141416] border border-[#393b40] rounded text-[#dfe1e5] font-mono text-xs focus:outline-none focus:border-[#3574f0] select-text"
                @input="envDirty = true"
              />
              <div class="relative flex-1 flex items-center">
                <input
                  v-model="row.value"
                  :type="row.revealed ? 'text' : 'password'"
                  spellcheck="false"
                  autocomplete="off"
                  :disabled="saving"
                  placeholder="Value"
                  class="w-full pl-2.5 pr-8 py-1.5 bg-[#141416] border border-[#393b40] rounded text-[#bcbec4] font-mono text-xs focus:outline-none focus:border-[#3574f0] select-text"
                  @input="envDirty = true"
                />
                <button
                  type="button"
                  class="absolute right-1.5 p-1 rounded text-[#6f737a] hover:text-[#dfe1e5] cursor-pointer"
                  :title="row.revealed ? 'Hide value' : 'Show value'"
                  @click="row.revealed = !row.revealed"
                >
                  <EyeOff v-if="row.revealed" class="w-3.5 h-3.5" />
                  <Eye v-else class="w-3.5 h-3.5" />
                </button>
              </div>
              <button
                type="button"
                class="p-1.5 rounded text-[#6f737a] hover:text-[#e55765] hover:bg-[#e55765]/10 transition-colors cursor-pointer"
                title="Remove variable"
                @click="removeEnvRow(idx)"
              >
                <Trash2 class="w-3.5 h-3.5" />
              </button>
            </div>
          </div>

          <div class="pt-1 shrink-0">
            <button
              type="button"
              class="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors cursor-pointer"
              @click="addEnvRow"
            >
              <Plus class="w-3.5 h-3.5" />
              <span>Add variable</span>
            </button>
          </div>
        </div>
      </div>

      <!-- Error message if save failed -->
      <div
        v-if="error"
        class="px-4 py-1.5 bg-[#e55765]/10 border-t border-[#e55765]/30 text-[#e55765] text-xs font-mono shrink-0 whitespace-pre-line"
      >
        {{ error }}
      </div>

      <!-- Footer -->
      <div class="px-4 py-2.5 bg-[#252629] border-t border-[#2b2d30] flex items-center justify-between shrink-0">
        <span class="text-[11px] text-[#6f737a]">
          Saving restarts this repo's omp sessions (after any running turn).
        </span>
        <div class="flex items-center gap-2">
          <button
            type="button"
            class="px-3 py-1.5 rounded border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors cursor-pointer"
            @click="handleClose"
          >
            Cancel
          </button>
          <button
            type="button"
            :disabled="!canSave"
            class="px-3 py-1.5 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer"
            @click="handleSave"
          >
            {{ saving ? 'Saving...' : 'Save' }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
