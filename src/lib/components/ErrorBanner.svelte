<script lang="ts">
  import { projectsStore } from '../stores/projects.svelte';
  import type { ProjectState } from '../types';
  import { AlertTriangle, RotateCcw, X } from 'lucide-svelte';

  interface Props {
    project: ProjectState;
  }

  let { project }: Props = $props();

  let isRestarting = $state(false);

  async function handleRestart() {
    isRestarting = true;
    try {
      await projectsStore.restartProject(project.id);
    } catch {
      // Error is set in project.errorStderr by store
    } finally {
      isRestarting = false;
    }
  }

  function handleDismiss() {
    project.errorStderr = null;
  }
</script>

{#if project.errorStderr || project.status === 'error'}
  <div class="flex items-start justify-between gap-3 px-3.5 py-2.5 bg-[#3a1d1d] border-b border-[#e55765]/40 text-[#dfe1e5] shrink-0 font-sans text-[13px] animate-in fade-in duration-150">
    <div class="flex items-start gap-2.5 min-w-0 flex-1">
      <AlertTriangle class="w-4 h-4 text-[#e55765] shrink-0 mt-0.5" />
      <div class="flex flex-col min-w-0 flex-1 gap-1">
        <div class="flex items-center gap-2">
          <span class="font-semibold text-[#f87171]">Process Error</span>
          <span class="text-[11px] text-[#6f737a] font-mono">
            {project.status === 'error' ? 'omp process died or error status' : 'turn error'}
          </span>
        </div>
        {#if project.errorStderr}
          <pre class="m-0 p-2 rounded bg-[#1e1f22]/80 border border-[#2b2d30] font-mono text-[11px] text-[#fca5a5] whitespace-pre-wrap break-all max-h-32 overflow-y-auto leading-relaxed select-text">{project.errorStderr}</pre>
        {:else}
          <span class="text-xs text-[#bcbec4]">Process exited with an error.</span>
        {/if}
      </div>
    </div>

    <div class="flex items-center gap-2 shrink-0">
      <button
        type="button"
        onclick={handleRestart}
        disabled={isRestarting}
        title="Restart omp session (--resume)"
        class="flex items-center gap-1.5 px-3 py-1.5 rounded bg-[#e55765] hover:bg-[#e55765]/90 text-white font-medium text-xs transition-colors disabled:opacity-50 cursor-pointer shadow-sm"
      >
        <RotateCcw class="w-3.5 h-3.5 {isRestarting ? 'animate-spin' : ''}" />
        <span>{isRestarting ? 'Restarting…' : 'Restart'}</span>
      </button>

      <button
        type="button"
        onclick={handleDismiss}
        title="Dismiss banner"
        class="p-1 rounded text-[#6f737a] hover:text-[#dfe1e5] hover:bg-[#252629] transition-colors cursor-pointer"
      >
        <X class="w-4 h-4" />
      </button>
    </div>
  </div>
{/if}
