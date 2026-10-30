<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { SplitterGroup, SplitterPanel, SplitterResizeHandle } from 'reka-ui';
import { confirm } from '@tauri-apps/plugin-dialog';
import {
  GitBranch,
  ArrowUp,
  ArrowDown,
  Plus,
  Minus,
  RefreshCw,
  Download,
  Upload,
  FileText,
  AlertCircle,
  X,
  Copy,
  Check,
  Settings,
  ExternalLink,
  Trash2,
} from '@lucide/vue';
import { useProjectsStore } from '../stores/projects';
import type { ProjectState, Worktree, Branches } from '../types';
import StatusModal from './StatusModal.vue';
import WorkspaceConfigModal from './WorkspaceConfigModal.vue';
import FileTree from './FileTree.vue';

const props = defineProps<{
  project: ProjectState;
}>();

const emit = defineEmits<{
  (e: 'pickFile', relPath: string): void;
}>();

const store = useProjectsStore();

const commitMessage = ref('');
const isCommitting = ref(false);
const isActionRunning = ref(false);
const gitActionError = ref<string | null>(null);

const statusModalOpen = ref(false);
const statusModalText = ref('');
const configModalOpen = ref(false);

const worktrees = ref<Worktree[]>([]);
const wtError = ref<string | null>(null);
const wtBusy = ref(false);
const wtFormOpen = ref(false);
const wtNew = ref(true);
const wtBranch = ref('');
const wtBase = ref('');
const branches = ref<Branches>({ local: [], remote: [] });

const gitStatus = computed(() => props.project.gitStatus);

const copied = ref(false);
let copyTimer: number | undefined = undefined;

async function copyPath() {
  try {
    await navigator.clipboard.writeText(props.project.dir);
    copied.value = true;
    clearTimeout(copyTimer);
    copyTimer = window.setTimeout(() => (copied.value = false), 1500);
  } catch (err) {
    console.error('Failed to copy directory path:', err);
  }
}

function timeAgo(date: Date | null): string {
  if (!date) return 'never';
  const seconds = Math.floor((Date.now() - date.getTime()) / 1000);
  if (seconds < 60) return 'just now';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ago`;
}

const fetchedAgo = computed(() => timeAgo(props.project.lastFetched));

async function loadWorktrees() {
  const id = props.project.id;
  try {
    const result = await store.listWorktrees(id);
    if (id !== props.project.id) return;
    worktrees.value = result.filter((w) => !w.bare);
  } catch (err) {
    if (id !== props.project.id) return;
    worktrees.value = [];
    wtError.value = typeof err === 'string' ? err : (err as Error)?.message || String(err);
  }
}

const isGit = computed(() => props.project.gitStatus !== null);
const currentBranch = computed(() => props.project.gitStatus?.branch ?? null);

watch(
  () => [props.project.id, currentBranch.value, isGit.value] as const,
  ([_id, _branch, git]) => {
    wtError.value = null;
    if (git) loadWorktrees();
    else worktrees.value = [];
  },
  { immediate: true }
);

async function toggleWorktreeForm() {
  wtFormOpen.value = !wtFormOpen.value;
  if (wtFormOpen.value) {
    try {
      branches.value = await store.listBranches(props.project.id);
    } catch (err) {
      wtError.value = typeof err === 'string' ? err : (err as Error)?.message || String(err);
    }
  }
}

async function handleCreateWorktree() {
  if (wtBusy.value || !wtBranch.value.trim()) return;
  wtBusy.value = true;
  wtError.value = null;
  try {
    const branch = wtBranch.value.trim();
    const base = wtNew.value && wtBase.value.trim() ? wtBase.value.trim() : null;
    const path = await store.addWorktree(props.project.id, branch, wtNew.value, base);
    await store.openProject(path);
    wtBranch.value = '';
    wtBase.value = '';
    wtFormOpen.value = false;
  } catch (err) {
    wtError.value = typeof err === 'string' ? err : (err as Error)?.message || String(err);
  } finally {
    wtBusy.value = false;
    await loadWorktrees();
  }
}

async function handleOpenWorktree(wt: Worktree) {
  try {
    const existing = store.projects.find((p) => p.dir === wt.path);
    if (existing) {
      store.setActiveTab(existing.id);
    } else {
      await store.openProject(wt.path);
    }
  } catch (err) {
    wtError.value = typeof err === 'string' ? err : (err as Error)?.message || String(err);
  }
}

async function handleRemoveWorktree(wt: Worktree) {
  const confirmed = await confirm(`Remove worktree ${wt.path}?`, {
    title: 'Remove worktree',
    kind: 'warning',
  });
  if (!confirmed) return;

  const toClose = store.projects.filter((p) => p.dir === wt.path);
  for (const tab of toClose) {
    await store.closeProject(tab.id);
  }

  try {
    await store.removeWorktree(props.project.id, wt.path, false);
  } catch (err) {
    const errStr = typeof err === 'string' ? err : (err as Error)?.message || String(err);
    const forceConfirmed = await confirm(
      `${errStr}\n\nForce remove? Uncommitted changes in that worktree will be lost.`,
      { title: 'Force remove worktree', kind: 'warning' },
    );
    if (forceConfirmed) {
      try {
        await store.removeWorktree(props.project.id, wt.path, true);
      } catch (forceErr) {
        wtError.value = typeof forceErr === 'string' ? forceErr : (forceErr as Error)?.message || String(forceErr);
      }
    }
  }
  await loadWorktrees();
}

async function handleStage(path: string) {
  try {
    gitActionError.value = null;
    await store.gitStage(props.project.id, path);
  } catch (err) {
    gitActionError.value = typeof err === 'string' ? err : (err as Error)?.message || String(err);
  }
}

async function handleUnstage(path: string) {
  try {
    gitActionError.value = null;
    await store.gitUnstage(props.project.id, path);
  } catch (err) {
    gitActionError.value = typeof err === 'string' ? err : (err as Error)?.message || String(err);
  }
}

async function handleAddAll() {
  try {
    gitActionError.value = null;
    await store.gitAddAll(props.project.id);
  } catch (err) {
    gitActionError.value = typeof err === 'string' ? err : (err as Error)?.message || String(err);
  }
}

async function handleCommit() {
  if (!commitMessage.value.trim()) return;
  isCommitting.value = true;
  gitActionError.value = null;
  try {
    await store.gitCommit(props.project.id, commitMessage.value.trim());
    commitMessage.value = '';
  } catch (err) {
    gitActionError.value = typeof err === 'string' ? err : (err as Error)?.message || String(err);
  } finally {
    isCommitting.value = false;
  }
}

async function handlePull() {
  isActionRunning.value = true;
  gitActionError.value = null;
  try {
    await store.gitPull(props.project.id);
  } catch (err) {
    gitActionError.value = typeof err === 'string' ? err : (err as Error)?.message || String(err);
  } finally {
    isActionRunning.value = false;
  }
}

async function handlePush() {
  isActionRunning.value = true;
  gitActionError.value = null;
  try {
    await store.gitPush(props.project.id);
  } catch (err) {
    gitActionError.value = typeof err === 'string' ? err : (err as Error)?.message || String(err);
  } finally {
    isActionRunning.value = false;
  }
}

async function handleFetch() {
  isActionRunning.value = true;
  gitActionError.value = null;
  try {
    await store.gitFetch(props.project.id);
  } catch (err) {
    gitActionError.value = typeof err === 'string' ? err : (err as Error)?.message || String(err);
  } finally {
    isActionRunning.value = false;
  }
}

async function handleShowStatus() {
  try {
    const text = await store.getGitStatusText(props.project.id);
    statusModalText.value = text;
    statusModalOpen.value = true;
  } catch (err) {
    gitActionError.value = typeof err === 'string' ? err : (err as Error)?.message || String(err);
  }
}

async function handleInit() {
  isActionRunning.value = true;
  gitActionError.value = null;
  try {
    await store.gitInit(props.project.id);
  } catch (err) {
    gitActionError.value = typeof err === 'string' ? err : (err as Error)?.message || String(err);
  } finally {
    isActionRunning.value = false;
  }
}
</script>

<template>
  <div class="flex flex-col h-full w-full bg-[#1e1f22] select-none text-xs font-sans overflow-hidden border-l border-[#2b2d30]">
    <!-- Header: GIT, Branch, Ahead/Behind -->
    <div class="flex flex-col gap-1 px-3 py-2 bg-[#252629] border-b border-[#2b2d30] shrink-0 font-medium">
      <!-- Project dir path (click copies path) + settings icon -->
      <div class="flex items-center justify-between gap-1">
        <button
          type="button"
          @click="copyPath"
          class="flex items-center gap-1.5 min-w-0 text-left group hover:text-white transition-colors cursor-pointer"
          title="Click to copy full path"
        >
          <span class="truncate font-mono text-[#dfe1e5]">{{ project.dir }}</span>
          <Check v-if="copied" class="w-3 h-3 text-[#59a869] shrink-0" />
          <Copy v-else class="w-3 h-3 text-[#6f737a] group-hover:text-[#bcbec4] shrink-0" />
        </button>
        <button
          type="button"
          @click="configModalOpen = true"
          class="p-1 rounded text-[#6f737a] hover:text-[#dfe1e5] hover:bg-[#35373c] transition-colors cursor-pointer shrink-0"
          title="Workspace omp config"
        >
          <Settings class="w-3.5 h-3.5" />
        </button>
      </div>
      <div class="flex items-center justify-between gap-2">
        <div class="flex items-center gap-2 overflow-hidden">
          <GitBranch class="w-3.5 h-3.5 text-[#3574f0] shrink-0" />
          <span class="truncate text-[#bcbec4] font-mono text-[13px]" :title="gitStatus?.branch || ''">
            {{ gitStatus ? (gitStatus.detached ? 'detached HEAD' : gitStatus.branch || 'no branch') : 'no repository' }}
          </span>
        </div>

        <div v-if="gitStatus" class="flex items-center gap-2 text-[13px] font-mono shrink-0">
          <span v-if="gitStatus.ahead > 0" class="flex items-center text-[#59a869]" :title="`${gitStatus.ahead} commits ahead`">
            <ArrowUp class="w-3 h-3" />{{ gitStatus.ahead }}
          </span>
          <span v-if="gitStatus.behind > 0" class="flex items-center text-[#e2a53b]" :title="`${gitStatus.behind} commits behind`">
            <ArrowDown class="w-3 h-3" />{{ gitStatus.behind }}
          </span>
          <span v-if="gitStatus.ahead === 0 && gitStatus.behind === 0" class="text-[#6f737a]" title="In sync with remote">
            ↑0 ↓0
          </span>
        </div>
      </div>
    </div>

    <!-- Not a git repo: just the file tree, plus a button to initialise one -->
    <template v-if="!gitStatus">
      <div class="flex flex-col gap-2 px-3 py-2 border-b border-[#2b2d30] shrink-0">
        <button
          type="button"
          @click="handleInit"
          :disabled="isActionRunning"
          title="Run git init . in the project directory"
          class="flex items-center justify-center gap-1.5 py-1.5 px-3 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium text-xs disabled:opacity-50 transition-colors cursor-pointer"
        >
          <GitBranch class="w-3.5 h-3.5" />
          <span>Initialize git repository</span>
        </button>
        <span v-if="gitActionError" class="font-mono text-[13px] text-[#e55765] break-words">{{ gitActionError }}</span>
      </div>
      <div class="flex-1 overflow-hidden">
        <FileTree :project="project" @pick="(p) => emit('pickFile', p)" />
      </div>
    </template>

    <SplitterGroup v-else direction="vertical" class="flex-1 w-full overflow-hidden">
      <!-- Top: changes (modified / deleted / untracked), commit, remote actions -->
      <SplitterPanel
        :id="'git-changes'"
        :order="1"
        :default-size="50"
        :min-size="20"
        class="flex flex-col overflow-hidden"
      >
        <!-- Error banner if any -->
        <div
          v-if="gitActionError"
          class="px-3 py-2 bg-[#e55765]/10 border-b border-[#e55765]/30 text-[#e55765] text-xs flex items-start justify-between gap-2 shrink-0"
        >
          <div class="flex items-start gap-1.5 flex-1 break-words">
            <AlertCircle class="w-3.5 h-3.5 shrink-0 mt-0.5" />
            <span class="font-mono text-[13px]">{{ gitActionError }}</span>
          </div>
          <button
            type="button"
            @click="gitActionError = null"
            class="text-[#e55765] hover:text-white shrink-0 p-0.5 cursor-pointer"
          >
            <X class="w-3.5 h-3.5" />
          </button>
        </div>

        <!-- Changes file list -->
        <div class="flex-1 overflow-y-auto p-2 space-y-3 font-mono text-xs">
          <!-- Staged Files -->
          <div class="space-y-1">
            <div class="flex items-center justify-between text-[13px] font-sans font-semibold uppercase tracking-wider text-[#6f737a] px-1">
              <span>Staged Changes ({{ gitStatus.staged.length }})</span>
            </div>
            <div v-if="gitStatus.staged.length === 0" class="text-[13px] text-[#6f737a] italic px-1">
              No staged changes
            </div>
            <template v-else>
              <div
                v-for="f in gitStatus.staged"
                :key="f.path"
                class="flex items-center justify-between px-1.5 py-1 rounded hover:bg-[#252629] group transition-colors font-mono text-[15px]"
              >
                <div class="flex items-center gap-1.5 truncate flex-1 mr-2" :title="f.orig_path ? `${f.orig_path} -> ${f.path}` : f.path">
                  <span
                    :class="[
                      'w-3 text-center font-bold',
                      f.status === 'D' ? 'text-[#e55765]' : f.status === 'A' ? 'text-[#59a869]' : 'text-[#3574f0]'
                    ]"
                  >
                    {{ f.status }}
                  </span>
                  <span class="truncate text-[#dfe1e5]">{{ f.path }}</span>
                </div>
                <button
                  type="button"
                  @click="handleUnstage(f.path)"
                  title="Unstage file (-)"
                  class="flex items-center justify-center w-5 h-5 rounded hover:bg-[#35373c] text-[#6f737a] hover:text-[#dfe1e5] shrink-0 transition-colors cursor-pointer"
                >
                  <Minus class="w-3 h-3" />
                </button>
              </div>
            </template>
          </div>

          <!-- Unstaged Files -->
          <div class="space-y-1">
            <div class="flex items-center justify-between text-[13px] font-sans font-semibold uppercase tracking-wider text-[#6f737a] px-1">
              <span>Changes ({{ gitStatus.unstaged.length }})</span>
              <button
                v-if="gitStatus.unstaged.length > 0"
                type="button"
                @click="handleAddAll"
                title="Add all tracked changes (git add -u)"
                class="text-[12px] text-[#3574f0] hover:underline normal-case font-normal cursor-pointer"
              >
                Add all
              </button>
            </div>
            <div v-if="gitStatus.unstaged.length === 0" class="text-[13px] text-[#6f737a] italic px-1">
              No unstaged changes
            </div>
            <template v-else>
              <div
                v-for="f in gitStatus.unstaged"
                :key="f.path"
                class="flex items-center justify-between px-1.5 py-1 rounded hover:bg-[#252629] group transition-colors font-mono text-[15px]"
              >
                <div class="flex items-center gap-1.5 truncate flex-1 mr-2" :title="f.path">
                  <span
                    :class="[
                      'w-3 text-center font-bold',
                      f.status === 'D' ? 'text-[#e55765]' : 'text-[#e2a53b]'
                    ]"
                  >
                    {{ f.status }}
                  </span>
                  <span class="truncate text-[#dfe1e5]">{{ f.path }}</span>
                </div>
                <button
                  type="button"
                  @click="handleStage(f.path)"
                  title="Stage file (+)"
                  class="flex items-center justify-center w-5 h-5 rounded hover:bg-[#35373c] text-[#6f737a] hover:text-[#dfe1e5] shrink-0 transition-colors cursor-pointer"
                >
                  <Plus class="w-3 h-3" />
                </button>
              </div>
            </template>
          </div>

          <!-- Untracked Files -->
          <div class="space-y-1">
            <div class="flex items-center justify-between text-[13px] font-sans font-semibold uppercase tracking-wider text-[#6f737a] px-1">
              <span>Untracked ({{ gitStatus.untracked.length }})</span>
            </div>
            <div v-if="gitStatus.untracked.length === 0" class="text-[13px] text-[#6f737a] italic px-1">
              No untracked files
            </div>
            <template v-else>
              <div
                v-for="p in gitStatus.untracked"
                :key="p"
                class="flex items-center justify-between px-1.5 py-1 rounded hover:bg-[#252629] group transition-colors font-mono text-[15px]"
              >
                <div class="flex items-center gap-1.5 truncate flex-1 mr-2" :title="p">
                  <span class="w-3 text-center font-bold text-[#6f737a]">?</span>
                  <span class="truncate text-[#dfe1e5]">{{ p }}</span>
                </div>
                <button
                  type="button"
                  @click="handleStage(p)"
                  title="Stage file (+)"
                  class="flex items-center justify-center w-5 h-5 rounded hover:bg-[#35373c] text-[#6f737a] hover:text-[#dfe1e5] shrink-0 transition-colors cursor-pointer"
                >
                  <Plus class="w-3 h-3" />
                </button>
              </div>
            </template>
          </div>
        </div>

        <!-- Commit Box -->
        <div class="p-2.5 bg-[#252629] border-t border-[#2b2d30] space-y-2 shrink-0">
          <textarea
            v-model="commitMessage"
            rows="2"
            placeholder="Commit message…"
            class="w-full p-2 rounded bg-[#1e1f22] border border-[#393b40] focus:border-[#3574f0] text-xs font-mono text-[#dfe1e5] placeholder-[#6f737a] resize-none outline-none leading-relaxed"
          ></textarea>

          <button
            type="button"
            @click="handleCommit"
            :disabled="isCommitting || !commitMessage.trim() || gitStatus.staged.length === 0"
            class="w-full py-1.5 px-3 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium text-xs disabled:opacity-40 disabled:cursor-not-allowed transition-colors cursor-pointer"
          >
            {{ isCommitting ? 'Committing…' : 'Commit' }}
          </button>
        </div>

        <!-- Action buttons row: [Status], [Pull], [Push], [Fetch] -->
        <div class="p-2 bg-[#1e1f22] border-t border-[#2b2d30] shrink-0 space-y-2">
          <div class="grid grid-cols-4 gap-1.5">
            <button
              type="button"
              @click="handleShowStatus"
              title="View raw git status output"
              class="flex items-center justify-center gap-1 py-1 px-1.5 rounded bg-[#252629] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors text-[13px] cursor-pointer"
            >
              <FileText class="w-3 h-3" />
              <span>Status</span>
            </button>

            <button
              type="button"
              @click="handlePull"
              :disabled="isActionRunning"
              title="Pull from remote (git pull)"
              class="flex items-center justify-center gap-1 py-1 px-1.5 rounded bg-[#252629] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors text-[13px] cursor-pointer disabled:opacity-50"
            >
              <Download class="w-3 h-3" />
              <span>Pull</span>
            </button>

            <button
              type="button"
              @click="handlePush"
              :disabled="isActionRunning"
              title="Push to remote (git push)"
              class="flex items-center justify-center gap-1 py-1 px-1.5 rounded bg-[#252629] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors text-[13px] cursor-pointer disabled:opacity-50"
            >
              <Upload class="w-3 h-3" />
              <span>Push</span>
            </button>

            <button
              type="button"
              @click="handleFetch"
              :disabled="isActionRunning"
              title="Fetch from remote (git fetch --prune)"
              class="flex items-center justify-center gap-1 py-1 px-1.5 rounded bg-[#252629] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors text-[13px] cursor-pointer disabled:opacity-50"
            >
              <RefreshCw :class="['w-3 h-3', isActionRunning ? 'animate-spin' : '']" />
              <span>Fetch</span>
            </button>
          </div>

          <!-- Footer: fetched status -->
          <div class="text-[13px] text-[#6f737a] text-center font-mono">
            fetched {{ fetchedAgo }}
          </div>
        </div>
      </SplitterPanel>

      <SplitterResizeHandle class="h-1 bg-[#2b2d30] hover:bg-[#3574f0] transition-colors cursor-row-resize shrink-0" />

      <!-- Middle: worktrees -->
      <SplitterPanel
        :id="'git-worktrees'"
        :order="2"
        :default-size="20"
        :min-size="10"
        class="flex flex-col overflow-hidden"
      >
        <!-- Header row -->
        <div class="flex items-center justify-between text-[13px] font-sans font-semibold uppercase tracking-wider text-[#6f737a] px-3 py-2 bg-[#252629] border-b border-[#2b2d30] shrink-0">
          <span>Worktrees ({{ worktrees.length }})</span>
          <button
            type="button"
            @click="toggleWorktreeForm"
            title="Create new worktree"
            class="flex items-center justify-center p-1 rounded hover:bg-[#35373c] text-[#bcbec4] hover:text-white transition-colors cursor-pointer"
          >
            <Plus class="w-3.5 h-3.5" />
          </button>
        </div>

        <div
          v-if="wtError"
          class="px-3 py-1.5 bg-[#e55765]/10 border-b border-[#e55765]/30 text-[#e55765] text-xs flex items-start justify-between gap-2 shrink-0"
        >
          <div class="flex items-start gap-1.5 flex-1 break-words">
            <AlertCircle class="w-3.5 h-3.5 shrink-0 mt-0.5" />
            <span class="font-mono text-[12px]">{{ wtError }}</span>
          </div>
          <button
            type="button"
            @click="wtError = null"
            class="text-[#e55765] hover:text-white shrink-0 p-0.5 cursor-pointer"
          >
            <X class="w-3.5 h-3.5" />
          </button>
        </div>

        <div v-if="wtFormOpen" class="p-2.5 bg-[#252629] border-b border-[#2b2d30] space-y-2 shrink-0">
          <div class="flex items-center gap-1">
            <button
              type="button"
              @click="wtNew = true"
              :class="[
                'flex-1 py-1 px-2 rounded text-xs font-medium transition-colors cursor-pointer',
                wtNew ? 'bg-[#3574f0] text-white' : 'bg-[#1e1f22] text-[#bcbec4] hover:text-[#dfe1e5]'
              ]"
            >
              New branch
            </button>
            <button
              type="button"
              @click="wtNew = false"
              :class="[
                'flex-1 py-1 px-2 rounded text-xs font-medium transition-colors cursor-pointer',
                !wtNew ? 'bg-[#3574f0] text-white' : 'bg-[#1e1f22] text-[#bcbec4] hover:text-[#dfe1e5]'
              ]"
            >
              Existing branch
            </button>
          </div>

          <div class="space-y-1">
            <input
              type="text"
              v-model="wtBranch"
              :placeholder="wtNew ? 'Branch name' : 'Select or type branch'"
              :list="!wtNew ? 'wt-local-branches' : undefined"
              class="w-full px-2 py-1 bg-[#1e1f22] border border-[#393b40] rounded text-[#bcbec4] text-xs focus:outline-none focus:border-[#3574f0]"
            />
            <datalist v-if="!wtNew" id="wt-local-branches">
              <option v-for="b in branches.local" :key="b" :value="b"></option>
            </datalist>
          </div>

          <div v-if="wtNew" class="space-y-1">
            <input
              type="text"
              v-model="wtBase"
              placeholder="Base (default HEAD)"
              list="wt-all-branches"
              class="w-full px-2 py-1 bg-[#1e1f22] border border-[#393b40] rounded text-[#bcbec4] text-xs focus:outline-none focus:border-[#3574f0]"
            />
            <datalist id="wt-all-branches">
              <option v-for="b in [...branches.local, ...branches.remote]" :key="b" :value="b"></option>
            </datalist>
          </div>

          <div class="flex items-center justify-end gap-2 pt-1">
            <button
              type="button"
              @click="wtFormOpen = false"
              class="px-2.5 py-1 rounded border border-[#393b40] text-[#bcbec4] hover:text-white transition-colors cursor-pointer"
            >
              Cancel
            </button>
            <button
              type="button"
              @click="handleCreateWorktree"
              :disabled="wtBusy || !wtBranch.trim()"
              class="px-2.5 py-1 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer"
            >
              {{ wtBusy ? 'Creating…' : 'Create' }}
            </button>
          </div>
        </div>

        <!-- Worktrees list -->
        <div class="flex-1 overflow-y-auto p-1.5 space-y-1">
          <div v-if="worktrees.length === 0" class="text-[12px] text-[#6f737a] italic px-1 py-0.5">
            No worktrees
          </div>
          <template v-else>
            <div
              v-for="wt in worktrees"
              :key="wt.path"
              class="flex items-center justify-between px-1.5 py-1 rounded hover:bg-[#252629] group transition-colors text-xs"
              :title="wt.path"
            >
              <div class="flex items-center gap-1.5 min-w-0 flex-1 mr-2">
                <GitBranch class="w-3.5 h-3.5 text-[#6f737a] shrink-0" />
                <span class="truncate font-mono text-[#dfe1e5] text-[13px]">
                  {{ wt.branch ?? `detached ${wt.head.slice(0, 7)}` }}
                </span>
                <div class="flex items-center gap-1 shrink-0">
                  <span v-if="wt.isMain" class="px-1 py-0.5 rounded bg-[#35373c] text-[#9da0a8] text-[10px] font-sans">
                    main
                  </span>
                  <span v-if="wt.isCurrent" class="px-1 py-0.5 rounded bg-[#3574f0]/20 text-[#3574f0] text-[10px] font-sans">
                    current
                  </span>
                  <span v-if="wt.locked" class="px-1 py-0.5 rounded bg-[#e2a53b]/20 text-[#e2a53b] text-[10px] font-sans">
                    locked
                  </span>
                  <span v-if="wt.prunable" class="px-1 py-0.5 rounded bg-[#e55765]/20 text-[#e55765] text-[10px] font-sans">
                    prunable
                  </span>
                </div>
              </div>

              <div class="flex items-center gap-1 shrink-0">
                <button
                  v-if="!wt.isCurrent && !wt.prunable"
                  type="button"
                  @click="handleOpenWorktree(wt)"
                  title="Open worktree"
                  class="flex items-center justify-center w-5 h-5 rounded hover:bg-[#35373c] text-[#6f737a] hover:text-[#dfe1e5] transition-colors cursor-pointer"
                >
                  <ExternalLink class="w-3.5 h-3.5" />
                </button>

                <button
                  v-if="!wt.isMain && !wt.isCurrent"
                  type="button"
                  @click="handleRemoveWorktree(wt)"
                  title="Remove worktree"
                  class="flex items-center justify-center w-5 h-5 rounded hover:bg-[#35373c] text-[#6f737a] hover:text-[#e55765] transition-colors cursor-pointer"
                >
                  <Trash2 class="w-3.5 h-3.5" />
                </button>
              </div>
            </div>
          </template>
        </div>
      </SplitterPanel>

      <SplitterResizeHandle class="h-1 bg-[#2b2d30] hover:bg-[#3574f0] transition-colors cursor-row-resize shrink-0" />

      <!-- Bottom: file browser -->
      <SplitterPanel
        :id="'git-files'"
        :order="3"
        :default-size="30"
        :min-size="15"
        class="overflow-hidden"
      >
        <FileTree :project="project" @pick="(p) => emit('pickFile', p)" />
      </SplitterPanel>
    </SplitterGroup>
  </div>

  <StatusModal
    :open="statusModalOpen"
    :text="statusModalText"
    @close="statusModalOpen = false"
  />

  <WorkspaceConfigModal
    :open="configModalOpen"
    :project="project"
    @close="configModalOpen = false"
  />
</template>
