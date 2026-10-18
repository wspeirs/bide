<script lang="ts">
  import { tick } from 'svelte';
  import { ArrowRight, ArrowLeft, Trash2 } from 'lucide-svelte';
  import type { ProjectState } from '../types';

  interface Props {
    project: ProjectState;
  }

  let { project }: Props = $props();

  let expanded = $state<Record<number, boolean>>({});
  let scroller = $state<HTMLDivElement | null>(null);
  let stick = true;

  function onScroll() {
    if (!scroller) return;
    stick = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 24;
  }

  $effect(() => {
    project.debugLog.length;
    if (stick) tick().then(() => scroller && (scroller.scrollTop = scroller.scrollHeight));
  });

  function pretty(json: string): string {
    try {
      return JSON.stringify(JSON.parse(json), null, 2);
    } catch {
      return json;
    }
  }

  function clear() {
    project.debugLog.length = 0;
    expanded = {};
  }
</script>

<div class="flex flex-col h-full w-full bg-[#1e1f22] select-text overflow-hidden">
  <div class="px-3 py-1 bg-[#252629] border-b border-[#2b2d30] text-[12px] font-mono uppercase tracking-wider text-[#6f737a] select-none flex items-center justify-between shrink-0">
    <span>Debug / JSON-RPC ({project.debugLog.length})</span>
    <button type="button" onclick={clear} title="Clear" class="hover:text-[#dfe1e5] cursor-pointer">
      <Trash2 class="w-3.5 h-3.5" />
    </button>
  </div>

  <div bind:this={scroller} onscroll={onScroll} class="flex-1 overflow-y-auto font-mono text-[12px] leading-relaxed">
    {#each project.debugLog as e (e.id)}
      <button
        type="button"
        onclick={() => (expanded[e.id] = !expanded[e.id])}
        class="w-full flex items-center gap-2 px-3 py-0.5 text-left hover:bg-[#2b2d30] cursor-pointer"
      >
        <span class="text-[#6f737a] shrink-0">{e.time}</span>
        {#if e.dir === 'out'}
          <ArrowRight class="w-3 h-3 text-[#e2a53b] shrink-0" />
        {:else}
          <ArrowLeft class="w-3 h-3 text-[#59a869] shrink-0" />
        {/if}
        <span class="text-[#3574f0] shrink-0">{e.kind}</span>
        <span class="truncate text-[#6f737a]">{e.json.slice(0, 200)}</span>
      </button>
      {#if expanded[e.id]}
        <pre class="m-0 px-6 py-1 bg-[#252629] text-[#bcbec4] whitespace-pre-wrap break-all">{pretty(e.json)}</pre>
      {/if}
    {/each}
  </div>
</div>
