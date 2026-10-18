<script lang="ts">
  import type { ProjectState } from '../types';
  import { ChevronRight, ChevronDown, Wrench, CheckCircle2, AlertCircle, Loader2, Ban } from 'lucide-svelte';

  interface Props {
    project: ProjectState;
  }

  let { project }: Props = $props();

  let containerRef = $state<HTMLDivElement | null>(null);
  let userScrolledUp = $state(false);
  let expandedTools = $state<Record<string, boolean>>({});

  // Live clock so elapsed times of in-flight items tick while the agent works.
  let now = $state(Date.now());
  $effect(() => {
    if (!project.isStreaming) return;
    const t = setInterval(() => (now = Date.now()), 500);
    return () => clearInterval(t);
  });

  function toggleTool(id: string) {
    expandedTools[id] = !expandedTools[id];
  }

  function onScroll() {
    if (!containerRef) return;
    const { scrollTop, scrollHeight, clientHeight } = containerRef;
    userScrolledUp = scrollHeight - scrollTop - clientHeight > 40;
  }

  $effect(() => {
    // React to new items and to growth of the newest one (text, thinking, tool args)
    const last = project.activityLog[project.activityLog.length - 1];
    void project.activityLog.length;
    void last?.text?.length;
    void last?.argsChars;
    void project.isStreaming;

    if (!userScrolledUp && containerRef) {
      containerRef.scrollTop = containerRef.scrollHeight;
    }
  });

  function formatToolArgs(args: unknown): string {
    if (!args) return '';
    if (typeof args === 'string') return args;
    try {
      return JSON.stringify(args, null, 2);
    } catch {
      return String(args);
    }
  }

  function fmtDur(ms: number): string {
    if (ms < 1000) return `${Math.max(0, Math.round(ms))}ms`;
    if (ms < 10000) return `${(ms / 1000).toFixed(1)}s`;
    if (ms < 60000) return `${Math.round(ms / 1000)}s`;
    return `${Math.floor(ms / 60000)}m ${Math.round((ms % 60000) / 1000)}s`;
  }

  function fmtChars(n: number): string {
    return n < 1000 ? `${n}` : `${(n / 1000).toFixed(1)}k`;
  }
</script>

{#snippet timing(startedAt: number | undefined, endedAt: number | undefined)}
  {#if startedAt}
    <span>{new Date(startedAt).toLocaleTimeString('en-GB')}</span>
    <span class="text-[#bcbec4]">{fmtDur((endedAt ?? now) - startedAt)}</span>
  {/if}
{/snippet}

<div class="flex flex-col h-full w-full bg-[#1e1f22] select-text overflow-hidden">
  <div class="px-3 py-1 bg-[#252629] border-b border-[#2b2d30] text-[13px] font-mono uppercase tracking-wider text-[#6f737a] select-none flex items-center justify-between shrink-0">
    <span>Activity / Agent Console</span>
    {#if userScrolledUp}
      <button
        type="button"
        onclick={() => {
          userScrolledUp = false;
          if (containerRef) containerRef.scrollTop = containerRef.scrollHeight;
        }}
        class="text-[12px] text-[#3574f0] hover:underline cursor-pointer"
      >
        Scroll to bottom ↓
      </button>
    {/if}
  </div>

  <div
    bind:this={containerRef}
    onscroll={onScroll}
    class="flex-1 p-3 font-mono text-[15px] overflow-y-auto leading-relaxed space-y-3"
  >
    {#each project.activityLog as item (item.id)}
      {#if item.type === 'thinking'}
        {#if item.text || !item.endedAt}
          <div class="bg-[#252629]/50 border border-[#2b2d30] rounded p-2 text-neutral-400 italic font-mono text-[15px] whitespace-pre-wrap leading-relaxed">
            <div class="text-[12px] uppercase font-sans tracking-wider text-[#6f737a] not-italic mb-1 font-semibold flex items-center gap-1.5">
              <span class="w-1.5 h-1.5 rounded-full bg-[#3574f0]/60 {item.endedAt ? '' : 'animate-pulse'}"></span>
              Thinking
              <span class="ml-auto flex items-center gap-2 normal-case tracking-normal font-normal">
                {@render timing(item.startedAt, item.endedAt)}
              </span>
            </div>
            {item.text}
          </div>
        {/if}
      {:else if item.type === 'text'}
        <div class="font-mono text-[15px] text-[#dfe1e5] leading-relaxed whitespace-pre-wrap break-words">
          {item.text}
        </div>
      {:else if item.type === 'tool_call'}
        {@const isExpanded = !!expandedTools[item.id]}
        <div class="border border-[#2b2d30] bg-[#252629] rounded text-[15px] overflow-hidden">
          <button
            type="button"
            onclick={() => toggleTool(item.id)}
            class="w-full flex items-center gap-2 px-2.5 py-1.5 text-left hover:bg-[#2b2d30] transition-colors select-none text-[#bcbec4]"
          >
            {#if isExpanded}
              <ChevronDown class="w-3.5 h-3.5 text-[#6f737a] shrink-0" />
            {:else}
              <ChevronRight class="w-3.5 h-3.5 text-[#6f737a] shrink-0" />
            {/if}
            <Wrench class="w-3.5 h-3.5 text-[#e2a53b] shrink-0" />
            <span class="font-semibold text-[#dfe1e5]">{item.tool}</span>
            {#if item.intent}
              <span class="text-[#bcbec4] truncate max-w-[300px]">({item.intent})</span>
            {/if}
            <span class="ml-auto flex items-center gap-2 text-[12px] text-[#6f737a] shrink-0">
              {@render timing(item.startedAt, item.endedAt)}
              {#if item.endedAt && item.phase !== 'done'}
                <span class="text-[#e2a53b] flex items-center gap-1"><Ban class="w-3 h-3" /> stopped</span>
              {:else if item.phase === 'generating'}
                <span class="text-[#3574f0] flex items-center gap-1">
                  <Loader2 class="w-3 h-3 animate-spin" /> generating · {fmtChars(item.argsChars ?? 0)}
                </span>
              {:else if item.phase === 'running'}
                <span class="text-[#3574f0] flex items-center gap-1">
                  <Loader2 class="w-3 h-3 animate-spin" /> running
                </span>
              {:else if item.isError}
                <span class="text-[#e55765] flex items-center gap-1">
                  <AlertCircle class="w-3 h-3" /> error
                </span>
              {:else if item.isError === false}
                <span class="text-[#59a869] flex items-center gap-1">
                  <CheckCircle2 class="w-3 h-3" /> done
                </span>
              {/if}
            </span>
          </button>
          {#if isExpanded && item.args}
            <div class="p-2 border-t border-[#2b2d30] bg-[#1e1f22] text-[#bcbec4] overflow-x-auto">
              <pre class="m-0 font-mono text-[13px] whitespace-pre-wrap">{formatToolArgs(item.args)}</pre>
            </div>
          {/if}
        </div>
      {:else if item.type === 'command'}
        <div class="font-mono text-xs text-[#bcbec4] bg-[#252629] border border-[#2b2d30] rounded p-2 whitespace-pre-wrap">
          {item.text}
        </div>
      {:else if item.type === 'error'}
        <div class="font-mono text-xs text-[#e55765] bg-[#e55765]/10 border border-[#e55765]/30 rounded p-2 whitespace-pre-wrap">
          {item.text}
        </div>
      {/if}
    {/each}

    {#if project.activityLog.length === 0}
      <div class="text-[#6f737a] text-xs italic select-none py-4 text-center">
        No activity yet. Send a prompt below to start.
      </div>
    {/if}
  </div>
</div>
