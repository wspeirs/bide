<script lang="ts">
  import { projectsStore } from '../stores/projects.svelte';
  import type { ProjectState, Worktree, Branches } from '../types';
  import StatusModal from './StatusModal.svelte';
  import WorkspaceConfigModal from './WorkspaceConfigModal.svelte';
  import FileTree from './FileTree.svelte';
  import { PaneGroup, Pane, PaneResizer } from 'paneforge';
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
  } from 'lucide-svelte';

  interface Props {
    project: ProjectState;
    /** Called with a project-relative path when a file in the browser is clicked. */
    onPickFile: (relPath: string) => void;
  }

  let { project, onPickFile }: Props = $props();

  let commitMessage = $state('');
  let isCommitting = $state(false);
  let isActionRunning = $state(false);
  let gitActionError = $state<string | null>(null);

  let statusModalOpen = $state(false);
  let statusModalText = $state('');
  let configModalOpen = $state(false);

  let worktrees = $state<Worktree[]>([]);
  let wtError = $state<string | null>(null);
  let wtBusy = $state(false);
  let wtFormOpen = $state(false);
  let wtNew = $state(true);
  let wtBranch = $state('');
  let wtBase = $state('');
  let branches = $state<Branches>({ local: [], remote: [] });

  let gitStatus = $derived(project.gitStatus);

  let copied = $state(false);
  let copyTimer: number | undefined = undefined;

  async function copyPath() {
    try {
      await navigator.clipboard.writeText(project.dir);
      copied = true;
      clearTimeout(copyTimer);
      copyTimer = window.setTimeout(() => (copied = false), 1500);
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

  let fetchedAgo = $derived(timeAgo(project.lastFetched));

  async function loadWorktrees() {
    const id = project.id;
    try {
      const result = await projectsStore.listWorktrees(id);
      if (id !== project.id) return;
      worktrees = result.filter((w) => !w.bare);
    } catch (err) {
      if (id !== project.id) return;
      worktrees = [];
      wtError = typeof err === 'string' ? err : (err as Error)?.message || String(err);
    }
  }

  // Primitives only: `gitStatus` is replaced on every 5s poll and would re-run the effect.
  let isGit = $derived(project.gitStatus !== null);
  let currentBranch = $derived(project.gitStatus?.branch ?? null);

  $effect(() => {
    // Reload on tab switch, repo init, or checkout.
    void project.id;
    void currentBranch;
    wtError = null;
    if (isGit) loadWorktrees();
    else worktrees = [];
  });

  async function toggleWorktreeForm() {
    wtFormOpen = !wtFormOpen;
    if (wtFormOpen) {
      try {
        branches = await projectsStore.listBranches(project.id);
      } catch (err) {
        wtError = typeof err === 'string' ? err : (err as Error)?.message || String(err);
      }
    }
  }

  async function handleCreateWorktree() {
    if (wtBusy || !wtBranch.trim()) return;
    wtBusy = true;
    wtError = null;
    try {
      const branch = wtBranch.trim();
      const base = wtNew && wtBase.trim() ? wtBase.trim() : null;
      const path = await projectsStore.addWorktree(project.id, branch, wtNew, base);
      await projectsStore.openProject(path);
      wtBranch = '';
      wtBase = '';
      wtFormOpen = false;
    } catch (err) {
      wtError = typeof err === 'string' ? err : (err as Error)?.message || String(err);
    } finally {
      wtBusy = false;
      await loadWorktrees();
    }
  }

  async function handleOpenWorktree(wt: Worktree) {
    try {
      const existing = projectsStore.projects.find((p) => p.dir === wt.path);
      if (existing) {
        projectsStore.setActiveTab(existing.id);
      } else {
        await projectsStore.openProject(wt.path);
      }
    } catch (err) {
      wtError = typeof err === 'string' ? err : (err as Error)?.message || String(err);
    }
  }

  async function handleRemoveWorktree(wt: Worktree) {
    const confirmed = await confirm(`Remove worktree ${wt.path}?`, {
      title: 'Remove worktree',
      kind: 'warning',
    });
    if (!confirmed) return;

    const toClose = projectsStore.projects.filter((p) => p.dir === wt.path);
    for (const tab of toClose) {
      await projectsStore.closeProject(tab.id);
    }

    try {
      await projectsStore.removeWorktree(project.id, wt.path, false);
    } catch (err) {
      const errStr = typeof err === 'string' ? err : (err as Error)?.message || String(err);
      const forceConfirmed = await confirm(
        `${errStr}\n\nForce remove? Uncommitted changes in that worktree will be lost.`,
        { title: 'Force remove worktree', kind: 'warning' },
      );
      if (forceConfirmed) {
        try {
          await projectsStore.removeWorktree(project.id, wt.path, true);
        } catch (forceErr) {
          wtError = typeof forceErr === 'string' ? forceErr : (forceErr as Error)?.message || String(forceErr);
        }
      }
    }
    await loadWorktrees();
  }

  async function handleStage(path: string) {
    try {
      gitActionError = null;
      await projectsStore.gitStage(project.id, path);
    } catch (err) {
      gitActionError = typeof err === 'string' ? err : (err as Error)?.message || String(err);
    }
  }

  async function handleUnstage(path: string) {
    try {
      gitActionError = null;
      await projectsStore.gitUnstage(project.id, path);
    } catch (err) {
      gitActionError = typeof err === 'string' ? err : (err as Error)?.message || String(err);
    }
  }

  async function handleAddAll() {
    try {
      gitActionError = null;
      await projectsStore.gitAddAll(project.id);
    } catch (err) {
      gitActionError = typeof err === 'string' ? err : (err as Error)?.message || String(err);
    }
  }

  async function handleCommit() {
    if (!commitMessage.trim()) return;
    isCommitting = true;
    gitActionError = null;
    try {
      await projectsStore.gitCommit(project.id, commitMessage.trim());
      commitMessage = '';
    } catch (err) {
      gitActionError = typeof err === 'string' ? err : (err as Error)?.message || String(err);
    } finally {
      isCommitting = false;
    }
  }

  async function handlePull() {
    isActionRunning = true;
    gitActionError = null;
    try {
      await projectsStore.gitPull(project.id);
    } catch (err) {
      gitActionError = typeof err === 'string' ? err : (err as Error)?.message || String(err);
    } finally {
      isActionRunning = false;
    }
  }

  async function handlePush() {
    isActionRunning = true;
    gitActionError = null;
    try {
      await projectsStore.gitPush(project.id);
    } catch (err) {
      gitActionError = typeof err === 'string' ? err : (err as Error)?.message || String(err);
    } finally {
      isActionRunning = false;
    }
  }

  async function handleFetch() {
    isActionRunning = true;
    gitActionError = null;
    try {
      await projectsStore.gitFetch(project.id);
    } catch (err) {
      gitActionError = typeof err === 'string' ? err : (err as Error)?.message || String(err);
    } finally {
      isActionRunning = false;
    }
  }

  async function handleShowStatus() {
    try {
      const text = await projectsStore.getGitStatusText(project.id);
      statusModalText = text;
      statusModalOpen = true;
    } catch (err) {
      gitActionError = typeof err === 'string' ? err : (err as Error)?.message || String(err);
    }
  }

  async function handleInit() {
    isActionRunning = true;
    gitActionError = null;
    try {
      await projectsStore.gitInit(project.id);
    } catch (err) {
      gitActionError = typeof err === 'string' ? err : (err as Error)?.message || String(err);
    } finally {
      isActionRunning = false;
    }
  }
</script>

<div class="flex flex-col h-full w-full bg-[#1e1f22] select-none text-xs font-sans overflow-hidden border-l border-[#2b2d30]">
  <!-- Header: GIT, Branch, Ahead/Behind -->
  <div class="flex flex-col gap-1 px-3 py-2 bg-[#252629] border-b border-[#2b2d30] shrink-0 font-medium">
    <!-- Project dir path (click copies path) + settings icon -->
    <div class="flex items-center justify-between gap-1">
      <button
        type="button"
        onclick={copyPath}
        class="flex items-center gap-1.5 min-w-0 text-left group hover:text-white transition-colors cursor-pointer"
        title="Click to copy full path"
      >
        <span class="truncate font-mono text-[#dfe1e5]">{project.dir}</span>
        {#if copied}
          <Check class="w-3 h-3 text-[#59a869] shrink-0" />
        {:else}
          <Copy class="w-3 h-3 text-[#6f737a] group-hover:text-[#bcbec4] shrink-0" />
        {/if}
      </button>
      <button
        type="button"
        onclick={() => (configModalOpen = true)}
        class="p-1 rounded text-[#6f737a] hover:text-[#dfe1e5] hover:bg-[#35373c] transition-colors cursor-pointer shrink-0"
        title="Workspace omp config"
      >
        <Settings class="w-3.5 h-3.5" />
      </button>
    </div>
    <div class="flex items-center justify-between gap-2">
      <div class="flex items-center gap-2 overflow-hidden">
        <GitBranch class="w-3.5 h-3.5 text-[#3574f0] shrink-0" />
        <span class="truncate text-[#bcbec4] font-mono text-[13px]" title={gitStatus?.branch || ''}>
          {gitStatus ? (gitStatus.detached ? 'detached HEAD' : gitStatus.branch || 'no branch') : 'no repository'}
        </span>
      </div>

    {#if gitStatus}
      <div class="flex items-center gap-2 text-[13px] font-mono shrink-0">
        {#if gitStatus.ahead > 0}
          <span class="flex items-center text-[#59a869]" title={`${gitStatus.ahead} commits ahead`}>
            <ArrowUp class="w-3 h-3" />{gitStatus.ahead}
          </span>
        {/if}
        {#if gitStatus.behind > 0}
          <span class="flex items-center text-[#e2a53b]" title={`${gitStatus.behind} commits behind`}>
            <ArrowDown class="w-3 h-3" />{gitStatus.behind}
          </span>
        {/if}
        {#if gitStatus.ahead === 0 && gitStatus.behind === 0}
          <span class="text-[#6f737a]" title="In sync with remote">↑0 ↓0</span>
        {/if}
      </div>
    {/if}
    </div>
  </div>

  {#if !gitStatus}
    <!-- Not a git repo: just the file tree, plus a button to initialise one -->
    <div class="flex flex-col gap-2 px-3 py-2 border-b border-[#2b2d30] shrink-0">
      <button
        type="button"
        onclick={handleInit}
        disabled={isActionRunning}
        title="Run git init . in the project directory"
        class="flex items-center justify-center gap-1.5 py-1.5 px-3 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium text-xs disabled:opacity-50 transition-colors cursor-pointer"
      >
        <GitBranch class="w-3.5 h-3.5" />
        <span>Initialize git repository</span>
      </button>
      {#if gitActionError}
        <span class="font-mono text-[13px] text-[#e55765] break-words">{gitActionError}</span>
      {/if}
    </div>
    <div class="flex-1 overflow-hidden">
      <FileTree {project} onPick={onPickFile} />
    </div>
  {:else}
    <PaneGroup direction="vertical" class="flex-1 w-full overflow-hidden">
      <!-- Top: changes (modified / deleted / untracked), commit, remote actions -->
      <Pane id="git-changes" order={1} defaultSize={50} minSize={20} class="flex flex-col overflow-hidden">
    <!-- Error banner if any -->
    {#if gitActionError}
      <div class="px-3 py-2 bg-[#e55765]/10 border-b border-[#e55765]/30 text-[#e55765] text-xs flex items-start justify-between gap-2 shrink-0">
        <div class="flex items-start gap-1.5 flex-1 break-words">
          <AlertCircle class="w-3.5 h-3.5 shrink-0 mt-0.5" />
          <span class="font-mono text-[13px]">{gitActionError}</span>
        </div>
        <button
          type="button"
          onclick={() => (gitActionError = null)}
          class="text-[#e55765] hover:text-white shrink-0 p-0.5 cursor-pointer"
        >
          <X class="w-3.5 h-3.5" />
        </button>
      </div>
    {/if}

    <!-- Changes file list -->
    <div class="flex-1 overflow-y-auto p-2 space-y-3 font-mono text-xs">
      <!-- Staged Files -->
      <div class="space-y-1">
        <div class="flex items-center justify-between text-[13px] font-sans font-semibold uppercase tracking-wider text-[#6f737a] px-1">
          <span>Staged Changes ({gitStatus.staged.length})</span>
        </div>
        {#if gitStatus.staged.length === 0}
          <div class="text-[13px] text-[#6f737a] italic px-1">No staged changes</div>
        {:else}
          {#each gitStatus.staged as f (f.path)}
            <div class="flex items-center justify-between px-1.5 py-1 rounded hover:bg-[#252629] group transition-colors font-mono text-[15px]">
              <div class="flex items-center gap-1.5 truncate flex-1 mr-2" title={f.orig_path ? `${f.orig_path} -> ${f.path}` : f.path}>
                <span class="w-3 text-center font-bold {f.status === 'D' ? 'text-[#e55765]' : f.status === 'A' ? 'text-[#59a869]' : 'text-[#3574f0]'}">
                  {f.status}
                </span>
                <span class="truncate text-[#dfe1e5]">{f.path}</span>
              </div>
              <button
                type="button"
                onclick={() => handleUnstage(f.path)}
                title="Unstage file (-)"
                class="flex items-center justify-center w-5 h-5 rounded hover:bg-[#35373c] text-[#6f737a] hover:text-[#dfe1e5] shrink-0 transition-colors cursor-pointer"
              >
                <Minus class="w-3 h-3" />
              </button>
            </div>
          {/each}
        {/if}
      </div>

      <!-- Unstaged Files -->
      <div class="space-y-1">
        <div class="flex items-center justify-between text-[13px] font-sans font-semibold uppercase tracking-wider text-[#6f737a] px-1">
          <span>Changes ({gitStatus.unstaged.length})</span>
          {#if gitStatus.unstaged.length > 0}
            <button
              type="button"
              onclick={handleAddAll}
              title="Add all tracked changes (git add -u)"
              class="text-[12px] text-[#3574f0] hover:underline normal-case font-normal cursor-pointer"
            >
              Add all
            </button>
          {/if}
        </div>
        {#if gitStatus.unstaged.length === 0}
          <div class="text-[13px] text-[#6f737a] italic px-1">No unstaged changes</div>
        {:else}
          {#each gitStatus.unstaged as f (f.path)}
            <div class="flex items-center justify-between px-1.5 py-1 rounded hover:bg-[#252629] group transition-colors font-mono text-[15px]">
              <div class="flex items-center gap-1.5 truncate flex-1 mr-2" title={f.path}>
                <span class="w-3 text-center font-bold {f.status === 'D' ? 'text-[#e55765]' : 'text-[#e2a53b]'}">
                  {f.status}
                </span>
                <span class="truncate text-[#dfe1e5]">{f.path}</span>
              </div>
              <button
                type="button"
                onclick={() => handleStage(f.path)}
                title="Stage file (+)"
                class="flex items-center justify-center w-5 h-5 rounded hover:bg-[#35373c] text-[#6f737a] hover:text-[#dfe1e5] shrink-0 transition-colors cursor-pointer"
              >
                <Plus class="w-3 h-3" />
              </button>
            </div>
          {/each}
        {/if}
      </div>

      <!-- Untracked Files -->
      <div class="space-y-1">
        <div class="flex items-center justify-between text-[13px] font-sans font-semibold uppercase tracking-wider text-[#6f737a] px-1">
          <span>Untracked ({gitStatus.untracked.length})</span>
        </div>
        {#if gitStatus.untracked.length === 0}
          <div class="text-[13px] text-[#6f737a] italic px-1">No untracked files</div>
        {:else}
          {#each gitStatus.untracked as p (p)}
            <div class="flex items-center justify-between px-1.5 py-1 rounded hover:bg-[#252629] group transition-colors font-mono text-[15px]">
              <div class="flex items-center gap-1.5 truncate flex-1 mr-2" title={p}>
                <span class="w-3 text-center font-bold text-[#6f737a]">?</span>
                <span class="truncate text-[#dfe1e5]">{p}</span>
              </div>
              <button
                type="button"
                onclick={() => handleStage(p)}
                title="Stage file (+)"
                class="flex items-center justify-center w-5 h-5 rounded hover:bg-[#35373c] text-[#6f737a] hover:text-[#dfe1e5] shrink-0 transition-colors cursor-pointer"
              >
                <Plus class="w-3 h-3" />
              </button>
            </div>
          {/each}
        {/if}
      </div>
    </div>

    <!-- Commit Box -->
    <div class="p-2.5 bg-[#252629] border-t border-[#2b2d30] space-y-2 shrink-0">
      <textarea
        bind:value={commitMessage}
        rows={2}
        placeholder="Commit message…"
        class="w-full p-2 rounded bg-[#1e1f22] border border-[#393b40] focus:border-[#3574f0] text-xs font-mono text-[#dfe1e5] placeholder-[#6f737a] resize-none outline-none leading-relaxed"
      ></textarea>

      <button
        type="button"
        onclick={handleCommit}
        disabled={isCommitting || !commitMessage.trim() || gitStatus.staged.length === 0}
        class="w-full py-1.5 px-3 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium text-xs disabled:opacity-40 disabled:cursor-not-allowed transition-colors cursor-pointer"
      >
        {isCommitting ? 'Committing…' : 'Commit'}
      </button>
    </div>

    <!-- Action buttons row: [Status], [Pull], [Push], [Fetch] -->
    <div class="p-2 bg-[#1e1f22] border-t border-[#2b2d30] shrink-0 space-y-2">
      <div class="grid grid-cols-4 gap-1.5">
        <button
          type="button"
          onclick={handleShowStatus}
          title="View raw git status output"
          class="flex items-center justify-center gap-1 py-1 px-1.5 rounded bg-[#252629] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors text-[13px] cursor-pointer"
        >
          <FileText class="w-3 h-3" />
          <span>Status</span>
        </button>

        <button
          type="button"
          onclick={handlePull}
          disabled={isActionRunning}
          title="Pull from remote (git pull)"
          class="flex items-center justify-center gap-1 py-1 px-1.5 rounded bg-[#252629] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors text-[13px] cursor-pointer disabled:opacity-50"
        >
          <Download class="w-3 h-3" />
          <span>Pull</span>
        </button>

        <button
          type="button"
          onclick={handlePush}
          disabled={isActionRunning}
          title="Push to remote (git push)"
          class="flex items-center justify-center gap-1 py-1 px-1.5 rounded bg-[#252629] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors text-[13px] cursor-pointer disabled:opacity-50"
        >
          <Upload class="w-3 h-3" />
          <span>Push</span>
        </button>

        <button
          type="button"
          onclick={handleFetch}
          disabled={isActionRunning}
          title="Fetch from remote (git fetch --prune)"
          class="flex items-center justify-center gap-1 py-1 px-1.5 rounded bg-[#252629] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors text-[13px] cursor-pointer disabled:opacity-50"
        >
          <RefreshCw class="w-3 h-3 {isActionRunning ? 'animate-spin' : ''}" />
          <span>Fetch</span>
        </button>
      </div>

      <!-- Footer: fetched status -->
      <div class="text-[13px] text-[#6f737a] text-center font-mono">
        fetched {fetchedAgo}
      </div>
    </div>
      </Pane>

      <PaneResizer class="h-1 bg-[#2b2d30] hover:bg-[#3574f0] transition-colors cursor-row-resize shrink-0" />

      <!-- Middle: worktrees -->
      <Pane id="git-worktrees" order={2} defaultSize={20} minSize={10} class="flex flex-col overflow-hidden">
        <!-- Header row -->
        <div class="flex items-center justify-between text-[13px] font-sans font-semibold uppercase tracking-wider text-[#6f737a] px-3 py-2 bg-[#252629] border-b border-[#2b2d30] shrink-0">
          <span>Worktrees ({worktrees.length})</span>
          <button
            type="button"
            onclick={toggleWorktreeForm}
            title="Create new worktree"
            class="flex items-center justify-center p-1 rounded hover:bg-[#35373c] text-[#bcbec4] hover:text-white transition-colors cursor-pointer"
          >
            <Plus class="w-3.5 h-3.5" />
          </button>
        </div>

        {#if wtError}
          <div class="px-3 py-1.5 bg-[#e55765]/10 border-b border-[#e55765]/30 text-[#e55765] text-xs flex items-start justify-between gap-2 shrink-0">
            <div class="flex items-start gap-1.5 flex-1 break-words">
              <AlertCircle class="w-3.5 h-3.5 shrink-0 mt-0.5" />
              <span class="font-mono text-[12px]">{wtError}</span>
            </div>
            <button
              type="button"
              onclick={() => (wtError = null)}
              class="text-[#e55765] hover:text-white shrink-0 p-0.5 cursor-pointer"
            >
              <X class="w-3.5 h-3.5" />
            </button>
          </div>
        {/if}

        {#if wtFormOpen}
          <div class="p-2.5 bg-[#252629] border-b border-[#2b2d30] space-y-2 shrink-0">
            <div class="flex items-center gap-1">
              <button
                type="button"
                onclick={() => (wtNew = true)}
                class="flex-1 py-1 px-2 rounded text-xs font-medium transition-colors cursor-pointer {wtNew ? 'bg-[#3574f0] text-white' : 'bg-[#1e1f22] text-[#bcbec4] hover:text-[#dfe1e5]'}"
              >
                New branch
              </button>
              <button
                type="button"
                onclick={() => (wtNew = false)}
                class="flex-1 py-1 px-2 rounded text-xs font-medium transition-colors cursor-pointer {!wtNew ? 'bg-[#3574f0] text-white' : 'bg-[#1e1f22] text-[#bcbec4] hover:text-[#dfe1e5]'}"
              >
                Existing branch
              </button>
            </div>

            <div class="space-y-1">
              <input
                type="text"
                bind:value={wtBranch}
                placeholder={wtNew ? 'Branch name' : 'Select or type branch'}
                list={!wtNew ? 'wt-local-branches' : undefined}
                class="w-full px-2 py-1 bg-[#1e1f22] border border-[#393b40] rounded text-[#bcbec4] text-xs focus:outline-none focus:border-[#3574f0]"
              />
              {#if !wtNew}
                <datalist id="wt-local-branches">
                  {#each branches.local as b}
                    <option value={b}></option>
                  {/each}
                </datalist>
              {/if}
            </div>

            {#if wtNew}
              <div class="space-y-1">
                <input
                  type="text"
                  bind:value={wtBase}
                  placeholder="Base (default HEAD)"
                  list="wt-all-branches"
                  class="w-full px-2 py-1 bg-[#1e1f22] border border-[#393b40] rounded text-[#bcbec4] text-xs focus:outline-none focus:border-[#3574f0]"
                />
                <datalist id="wt-all-branches">
                  {#each [...branches.local, ...branches.remote] as b}
                    <option value={b}></option>
                  {/each}
                </datalist>
              </div>
            {/if}

            <div class="flex items-center justify-end gap-2 pt-1">
              <button
                type="button"
                onclick={() => (wtFormOpen = false)}
                class="px-2.5 py-1 rounded border border-[#393b40] text-[#bcbec4] hover:text-white transition-colors cursor-pointer"
              >
                Cancel
              </button>
              <button
                type="button"
                onclick={handleCreateWorktree}
                disabled={wtBusy || !wtBranch.trim()}
                class="px-2.5 py-1 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer"
              >
                {wtBusy ? 'Creating…' : 'Create'}
              </button>
            </div>
          </div>
        {/if}

        <!-- Worktrees list -->
        <div class="flex-1 overflow-y-auto p-1.5 space-y-1">
          {#if worktrees.length === 0}
            <div class="text-[12px] text-[#6f737a] italic px-1 py-0.5">No worktrees</div>
          {:else}
            {#each worktrees as wt (wt.path)}
              <div
                class="flex items-center justify-between px-1.5 py-1 rounded hover:bg-[#252629] group transition-colors text-xs"
                title={wt.path}
              >
                <div class="flex items-center gap-1.5 min-w-0 flex-1 mr-2">
                  <GitBranch class="w-3.5 h-3.5 text-[#6f737a] shrink-0" />
                  <span class="truncate font-mono text-[#dfe1e5] text-[13px]">
                    {wt.branch ?? `detached ${wt.head.slice(0, 7)}`}
                  </span>
                  <div class="flex items-center gap-1 shrink-0">
                    {#if wt.isMain}
                      <span class="px-1 py-0.5 rounded bg-[#35373c] text-[#9da0a8] text-[10px] font-sans">main</span>
                    {/if}
                    {#if wt.isCurrent}
                      <span class="px-1 py-0.5 rounded bg-[#3574f0]/20 text-[#3574f0] text-[10px] font-sans">current</span>
                    {/if}
                    {#if wt.locked}
                      <span class="px-1 py-0.5 rounded bg-[#e2a53b]/20 text-[#e2a53b] text-[10px] font-sans">locked</span>
                    {/if}
                    {#if wt.prunable}
                      <span class="px-1 py-0.5 rounded bg-[#e55765]/20 text-[#e55765] text-[10px] font-sans">prunable</span>
                    {/if}
                  </div>
                </div>

                <div class="flex items-center gap-1 shrink-0">
                  {#if !wt.isCurrent && !wt.prunable}
                    <button
                      type="button"
                      onclick={() => handleOpenWorktree(wt)}
                      title="Open worktree"
                      class="flex items-center justify-center w-5 h-5 rounded hover:bg-[#35373c] text-[#6f737a] hover:text-[#dfe1e5] transition-colors cursor-pointer"
                    >
                      <ExternalLink class="w-3.5 h-3.5" />
                    </button>
                  {/if}

                  {#if !wt.isMain && !wt.isCurrent}
                    <button
                      type="button"
                      onclick={() => handleRemoveWorktree(wt)}
                      title="Remove worktree"
                      class="flex items-center justify-center w-5 h-5 rounded hover:bg-[#35373c] text-[#6f737a] hover:text-[#e55765] transition-colors cursor-pointer"
                    >
                      <Trash2 class="w-3.5 h-3.5" />
                    </button>
                  {/if}
                </div>
              </div>
            {/each}
          {/if}
        </div>
      </Pane>

      <PaneResizer class="h-1 bg-[#2b2d30] hover:bg-[#3574f0] transition-colors cursor-row-resize shrink-0" />

      <!-- Bottom: file browser -->
      <Pane id="git-files" order={3} defaultSize={30} minSize={15} class="overflow-hidden">
        <FileTree {project} onPick={onPickFile} />
      </Pane>
    </PaneGroup>
  {/if}
</div>

<StatusModal
  open={statusModalOpen}
  text={statusModalText}
  onClose={() => (statusModalOpen = false)}
/>

<WorkspaceConfigModal
  open={configModalOpen}
  {project}
  onClose={() => (configModalOpen = false)}
/>
