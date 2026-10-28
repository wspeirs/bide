<script lang="ts">
  import { X } from 'lucide-svelte';
  import { projectsStore } from '../stores/projects.svelte';
  import type { ProjectState } from '../types';

  interface Props {
    open: boolean;
    project: ProjectState;
    onClose: () => void;
  }

  let { open, project, onClose }: Props = $props();

  let text = $state('');
  let source = $state<'workspace' | 'global' | 'none'>('none');
  let warning = $state<string | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let saving = $state(false);
  let loadFailed = $state(false);

  $effect(() => {
    if (open) {
      const id = project.id;
      let cancelled = false;
      loading = true;
      loadFailed = false;
      error = null;
      text = '';
      source = 'none';
      warning = null;
      projectsStore
        .getWorkspaceConfig(id)
        .then((cfg) => {
          if (cancelled) return;
          text = cfg.text;
          source = cfg.source;
          warning = cfg.warning;
        })
        .catch((err) => {
          if (cancelled) return;
          loadFailed = true;
          error = typeof err === 'string' ? err : (err as Error)?.message || String(err);
        })
        .finally(() => {
          if (!cancelled) loading = false;
        });
      return () => {
        cancelled = true;
      };
    }
  });


  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      onClose();
    }
  }

  async function handleSave() {
    saving = true;
    error = null;
    try {
      await projectsStore.setWorkspaceConfig(project.id, text);
      onClose();
    } catch (err) {
      error = typeof err === 'string' ? err : (err as Error)?.message || String(err);
    } finally {
      saving = false;
    }
  }
</script>

{#if open}
  <div
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    onkeydown={handleKeyDown}
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs select-none"
  >
    <button
      type="button"
      class="fixed inset-0 cursor-default bg-transparent border-none p-0 outline-none"
      onclick={onClose}
      aria-label="Close dialog"
    ></button>

    <div class="relative w-full max-w-3xl h-[80vh] flex flex-col bg-[#1e1f22] border border-[#393b40] rounded shadow-2xl overflow-hidden z-10 text-xs font-sans">
      <!-- Header -->
      <div class="flex items-start justify-between px-4 py-2.5 bg-[#252629] border-b border-[#2b2d30] shrink-0">
        <div class="flex flex-col gap-0.5">
          <span class="font-semibold text-[#dfe1e5] text-sm">omp config — {project.name}</span>
          <span class="text-[11px] text-[#6f737a]">
            {#if source === 'workspace'}
              Shared by all worktrees of this repo. Written to .omp/config.yml (git-excluded) when a workspace opens.
            {:else if source === 'global'}
              No workspace config yet — starting from your global omp config. Nothing is written until you save.
            {:else}
              No workspace config yet.
            {/if}
          </span>
        </div>
        <button
          type="button"
          onclick={onClose}
          class="p-1 rounded text-[#6f737a] hover:text-[#dfe1e5] hover:bg-[#35373c] transition-colors cursor-pointer"
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <!-- Warning if tracked -->
      {#if warning}
        <div class="px-4 py-2 bg-[#e55765]/10 border-b border-[#e55765]/30 text-[#e55765] text-xs shrink-0">
          {warning}
        </div>
      {/if}

      <!-- Body / Editor -->
      <div class="flex-1 p-3 flex flex-col overflow-hidden bg-[#1e1f22]">
        {#if loading}
          <div class="flex-1 flex items-center justify-center text-[#6f737a]">
            Loading config...
          </div>
        {:else}
          <textarea
            bind:value={text}
            disabled={Boolean(warning) || saving || loadFailed}
            spellcheck="false"
            class="flex-1 w-full h-full p-2.5 bg-[#141416] border border-[#393b40] rounded text-[#bcbec4] font-mono text-xs resize-none focus:outline-none focus:border-[#3574f0] select-text disabled:opacity-50 disabled:cursor-not-allowed leading-relaxed"
            placeholder="# Per-workspace omp config (YAML format)"
          ></textarea>
        {/if}
      </div>

      <!-- Error message if save failed -->
      {#if error}
        <div class="px-4 py-1.5 bg-[#e55765]/10 border-t border-[#e55765]/30 text-[#e55765] text-xs font-mono shrink-0">
          {error}
        </div>
      {/if}

      <!-- Footer -->
      <div class="px-4 py-2.5 bg-[#252629] border-t border-[#2b2d30] flex items-center justify-between shrink-0">
        <span class="text-[11px] text-[#6f737a]">
          Saving restarts this repo's omp sessions (after any running turn).
        </span>
        <div class="flex items-center gap-2">
          <button
            type="button"
            onclick={onClose}
            class="px-3 py-1.5 rounded border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors cursor-pointer"
          >
            Cancel
          </button>
          <button
            type="button"
            onclick={handleSave}
            disabled={Boolean(warning) || saving || loading || loadFailed}
            class="px-3 py-1.5 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer"
          >
            {saving ? 'Saving...' : 'Save'}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}
