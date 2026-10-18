<script lang="ts">
  import { untrack } from 'svelte';
  import { projectsStore } from '../stores/projects.svelte';
  import type { ProjectState } from '../types';
  import { ChevronRight, ChevronDown, Folder, File, RefreshCw } from 'lucide-svelte';

  interface Props {
    project: ProjectState;
    /** Called with the project-relative path when a file is clicked. */
    onPick: (relPath: string) => void;
  }

  let { project, onPick }: Props = $props();

  interface Entry {
    name: string;
    is_dir: boolean;
  }

  // Lazily loaded directory listings, keyed by project-relative dir ('' = root).
  let children = $state<Record<string, Entry[]>>({});
  let expanded = $state<Record<string, boolean>>({});
  let error = $state<string | null>(null);

  async function load(rel: string) {
    try {
      children[rel] = await projectsStore.listDir(project.id, rel);
      error = null;
    } catch (err) {
      error = typeof err === 'string' ? err : (err as Error)?.message || String(err);
    }
  }

  function reloadAll() {
    Object.keys(children).forEach(load);
  }

  let loadedFor = '';
  // New project tab: start over. Git status polling (every 5s) doubles as a change signal.
  $effect(() => {
    project.gitStatus;
    const id = project.id;
    untrack(() => {
      if (loadedFor !== id) {
        loadedFor = id;
        children = {};
        expanded = {};
        load('');
      } else {
        reloadAll();
      }
    });
  });

  interface Row {
    rel: string;
    name: string;
    is_dir: boolean;
    depth: number;
  }

  let rows = $derived.by(() => {
    const out: Row[] = [];
    const walk = (dir: string, depth: number) => {
      for (const e of children[dir] ?? []) {
        const rel = dir ? `${dir}/${e.name}` : e.name;
        out.push({ rel, name: e.name, is_dir: e.is_dir, depth });
        if (e.is_dir && expanded[rel]) walk(rel, depth + 1);
      }
    };
    walk('', 0);
    return out;
  });

  function click(r: Row) {
    if (!r.is_dir) return onPick(r.rel);
    expanded[r.rel] = !expanded[r.rel];
    if (expanded[r.rel] && !children[r.rel]) load(r.rel);
  }
</script>

<div class="flex flex-col h-full w-full overflow-hidden">
  <div class="px-3 py-1 bg-[#252629] border-b border-[#2b2d30] text-[12px] font-mono uppercase tracking-wider text-[#6f737a] flex items-center justify-between shrink-0">
    <span>Files</span>
    <button type="button" onclick={reloadAll} title="Refresh" class="hover:text-[#dfe1e5] cursor-pointer">
      <RefreshCw class="w-3.5 h-3.5" />
    </button>
  </div>

  {#if error}
    <div class="px-3 py-1 text-[#e55765] font-mono text-[13px]">{error}</div>
  {/if}

  <div class="flex-1 overflow-y-auto py-1 font-mono text-[14px]">
    {#each rows as r (r.rel)}
      <button
        type="button"
        onclick={() => click(r)}
        title={r.is_dir ? r.rel : `Insert @${r.rel} into prompt`}
        style="padding-left: {8 + r.depth * 14}px"
        class="w-full flex items-center gap-1 pr-2 py-0.5 text-left text-[#dfe1e5] hover:bg-[#2b2d30] cursor-pointer"
      >
        {#if r.is_dir}
          {#if expanded[r.rel]}
            <ChevronDown class="w-3 h-3 text-[#6f737a] shrink-0" />
          {:else}
            <ChevronRight class="w-3 h-3 text-[#6f737a] shrink-0" />
          {/if}
          <Folder class="w-3.5 h-3.5 text-[#e2a53b] shrink-0" />
        {:else}
          <span class="w-3 shrink-0"></span>
          <File class="w-3.5 h-3.5 text-[#6f737a] shrink-0" />
        {/if}
        <span class="truncate">{r.name}</span>
      </button>
    {/each}
  </div>
</div>
