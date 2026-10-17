<script lang="ts">
  import type { ProjectState } from '../types';
  import { GitBranch, Copy, Check } from 'lucide-svelte';

  interface Props {
    project: ProjectState;
  }

  let { project }: Props = $props();

  let copied = $state(false);
  let copyTimer: number | undefined = undefined;

  async function copyPath() {
    try {
      await navigator.clipboard.writeText(project.dir);
      copied = true;
      clearTimeout(copyTimer);
      copyTimer = window.setTimeout(() => {
        copied = false;
      }, 1500);
    } catch (err) {
      console.error('Failed to copy directory path:', err);
    }
  }
</script>

<div class="flex items-center justify-between px-3 py-1.5 bg-[#1e1f22] border-b border-[#2b2d30] text-xs font-mono text-[#bcbec4] select-none shrink-0 min-h-[28px]">
  <div class="flex items-center gap-3 overflow-hidden">
    <!-- Project dir path (click copies path) -->
    <button
      type="button"
      onclick={copyPath}
      class="flex items-center gap-1.5 hover:text-[#dfe1e5] transition-colors truncate text-left group"
      title="Click to copy full path"
    >
      <span class="truncate max-w-[320px] text-[#dfe1e5]">{project.dir}</span>
      {#if copied}
        <span class="text-[#59a869] flex items-center gap-1 text-[11px] font-sans">
          <Check class="w-3 h-3" /> Copied!
        </span>
      {:else}
        <Copy class="w-3 h-3 text-[#6f737a] group-hover:text-[#bcbec4] shrink-0" />
      {/if}
    </button>

    <!-- Git branch with icon -->
    {#if project.gitStatus}
      <div class="flex items-center gap-1 text-[#bcbec4] shrink-0" title="Git Branch">
        <GitBranch class="w-3 h-3 text-[#6f737a]" />
        <span>{project.gitStatus.detached ? 'detached HEAD' : project.gitStatus.branch || 'no branch'}</span>
      </div>
    {/if}
  </div>

  <div class="flex items-center gap-3 shrink-0">
    <!-- Model name and context % -->
    <div class="flex items-center gap-1.5 text-[#bcbec4]">
      <span class="text-[#dfe1e5]">{project.model}</span>
      <span class="text-[#6f737a]">·</span>
      <span>ctx {project.contextPercent}%</span>
    </div>

    <!-- Status indicator -->
    <div class="flex items-center gap-1.5 pl-2 border-l border-[#2b2d30]">
      {#if project.isStreaming}
        <span class="w-2 h-2 rounded-full bg-[#3574f0] animate-pulse"></span>
        <span class="text-[#3574f0] font-sans">streaming</span>
      {:else if project.status === 'ready'}
        <span class="w-2 h-2 rounded-full bg-[#59a869]"></span>
        <span class="text-[#59a869] font-sans">ready</span>
      {:else if project.status === 'error'}
        <span class="w-2 h-2 rounded-full bg-[#e55765]"></span>
        <span class="text-[#e55765] font-sans">error</span>
      {:else}
        <span class="w-2 h-2 rounded-full border border-[#6f737a]"></span>
        <span class="text-[#6f737a] font-sans">starting</span>
      {/if}
    </div>
  </div>
</div>
