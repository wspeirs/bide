<script lang="ts">
  import { projectsStore } from '../stores/projects.svelte';
  import type { ProjectState } from '../types';
  import StatusModal from './StatusModal.svelte';
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
  } from 'lucide-svelte';

  interface Props {
    project: ProjectState;
  }

  let { project }: Props = $props();

  let commitMessage = $state('');
  let isCommitting = $state(false);
  let isActionRunning = $state(false);
  let gitActionError = $state<string | null>(null);

  let statusModalOpen = $state(false);
  let statusModalText = $state('');

  let gitStatus = $derived(project.gitStatus);

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
</script>

<div class="flex flex-col h-full w-full bg-[#1e1f22] select-none text-xs font-sans overflow-hidden border-l border-[#2b2d30]">
  <!-- Header: GIT, Branch, Ahead/Behind -->
  <div class="flex items-center justify-between px-3 py-2 bg-[#252629] border-b border-[#2b2d30] shrink-0 font-medium">
    <div class="flex items-center gap-2 overflow-hidden">
      <GitBranch class="w-3.5 h-3.5 text-[#3574f0] shrink-0" />
      <span class="font-bold tracking-wide uppercase text-[11px] text-[#dfe1e5]">Git</span>
      {#if gitStatus}
        <span class="text-[#6f737a]">·</span>
        <span class="truncate text-[#bcbec4] font-mono text-[11px]" title={gitStatus.branch || ''}>
          {gitStatus.detached ? 'detached' : gitStatus.branch || 'no branch'}
        </span>
      {/if}
    </div>

    {#if gitStatus}
      <div class="flex items-center gap-2 text-[11px] font-mono shrink-0">
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

  {#if !gitStatus}
    <!-- Not a git repo state -->
    <div class="flex-1 flex flex-col items-center justify-center p-6 text-center text-[#6f737a] gap-2">
      <GitBranch class="w-8 h-8 opacity-40 text-[#6f737a]" />
      <div class="font-medium text-[#bcbec4]">Not a git repository</div>
      <div class="text-[11px]">Run <code class="font-mono text-[#dfe1e5]">git init</code> in terminal to enable source control.</div>
    </div>
  {:else}
    <!-- Error banner if any -->
    {#if gitActionError}
      <div class="px-3 py-2 bg-[#e55765]/10 border-b border-[#e55765]/30 text-[#e55765] text-xs flex items-start justify-between gap-2 shrink-0">
        <div class="flex items-start gap-1.5 flex-1 break-words">
          <AlertCircle class="w-3.5 h-3.5 shrink-0 mt-0.5" />
          <span class="font-mono text-[11px]">{gitActionError}</span>
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
        <div class="flex items-center justify-between text-[11px] font-sans font-semibold uppercase tracking-wider text-[#6f737a] px-1">
          <span>Staged Changes ({gitStatus.staged.length})</span>
        </div>
        {#if gitStatus.staged.length === 0}
          <div class="text-[11px] text-[#6f737a] italic px-1">No staged changes</div>
        {:else}
          {#each gitStatus.staged as f (f.path)}
            <div class="flex items-center justify-between px-1.5 py-1 rounded hover:bg-[#252629] group transition-colors font-mono text-[13px]">
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
        <div class="flex items-center justify-between text-[11px] font-sans font-semibold uppercase tracking-wider text-[#6f737a] px-1">
          <span>Changes ({gitStatus.unstaged.length})</span>
          {#if gitStatus.unstaged.length > 0}
            <button
              type="button"
              onclick={handleAddAll}
              title="Add all tracked changes (git add -u)"
              class="text-[10px] text-[#3574f0] hover:underline normal-case font-normal cursor-pointer"
            >
              Add all
            </button>
          {/if}
        </div>
        {#if gitStatus.unstaged.length === 0}
          <div class="text-[11px] text-[#6f737a] italic px-1">No unstaged changes</div>
        {:else}
          {#each gitStatus.unstaged as f (f.path)}
            <div class="flex items-center justify-between px-1.5 py-1 rounded hover:bg-[#252629] group transition-colors font-mono text-[13px]">
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
        <div class="flex items-center justify-between text-[11px] font-sans font-semibold uppercase tracking-wider text-[#6f737a] px-1">
          <span>Untracked ({gitStatus.untracked.length})</span>
        </div>
        {#if gitStatus.untracked.length === 0}
          <div class="text-[11px] text-[#6f737a] italic px-1">No untracked files</div>
        {:else}
          {#each gitStatus.untracked as p (p)}
            <div class="flex items-center justify-between px-1.5 py-1 rounded hover:bg-[#252629] group transition-colors font-mono text-[13px]">
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
          class="flex items-center justify-center gap-1 py-1 px-1.5 rounded bg-[#252629] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors text-[11px] cursor-pointer"
        >
          <FileText class="w-3 h-3" />
          <span>Status</span>
        </button>

        <button
          type="button"
          onclick={handlePull}
          disabled={isActionRunning}
          title="Pull from remote (git pull)"
          class="flex items-center justify-center gap-1 py-1 px-1.5 rounded bg-[#252629] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors text-[11px] cursor-pointer disabled:opacity-50"
        >
          <Download class="w-3 h-3" />
          <span>Pull</span>
        </button>

        <button
          type="button"
          onclick={handlePush}
          disabled={isActionRunning}
          title="Push to remote (git push)"
          class="flex items-center justify-center gap-1 py-1 px-1.5 rounded bg-[#252629] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors text-[11px] cursor-pointer disabled:opacity-50"
        >
          <Upload class="w-3 h-3" />
          <span>Push</span>
        </button>

        <button
          type="button"
          onclick={handleFetch}
          disabled={isActionRunning}
          title="Fetch from remote (git fetch --prune)"
          class="flex items-center justify-center gap-1 py-1 px-1.5 rounded bg-[#252629] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors text-[11px] cursor-pointer disabled:opacity-50"
        >
          <RefreshCw class="w-3 h-3 {isActionRunning ? 'animate-spin' : ''}" />
          <span>Fetch</span>
        </button>
      </div>

      <!-- Footer: fetched status -->
      <div class="text-[11px] text-[#6f737a] text-center font-mono">
        fetched {fetchedAgo}
      </div>
    </div>
  {/if}
</div>

<StatusModal
  open={statusModalOpen}
  text={statusModalText}
  onClose={() => (statusModalOpen = false)}
/>
