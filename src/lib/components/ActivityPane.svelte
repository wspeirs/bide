<script lang="ts">
  import type { ProjectState, ActivityItem } from '../types';
  import { ChevronRight, ChevronDown, Wrench, CheckCircle2, AlertCircle } from 'lucide-svelte';

  interface Props {
    project: ProjectState;
  }

  let { project }: Props = $props();

  let containerRef = $state<HTMLDivElement | null>(null);
  let userScrolledUp = $state(false);
  let expandedTools = $state<Record<string, boolean>>({});

  function toggleTool(id: string) {
    expandedTools[id] = !expandedTools[id];
  }

  function onScroll() {
    if (!containerRef) return;
    const { scrollTop, scrollHeight, clientHeight } = containerRef;
    userScrolledUp = scrollHeight - scrollTop - clientHeight > 40;
  }

  $effect(() => {
    // React to activityLog or thinking changes
    const _len = project.activityLog.length;
    const _t = project.thinking.length;
    const _streaming = project.isStreaming;

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
</script>

<div class="flex flex-col h-full w-full bg-[#1e1f22] select-text overflow-hidden">
  <div class="px-3 py-1 bg-[#252629] border-b border-[#2b2d30] text-[11px] font-mono uppercase tracking-wider text-[#6f737a] select-none flex items-center justify-between shrink-0">
    <span>Activity / Agent Console</span>
    {#if userScrolledUp}
      <button
        type="button"
        onclick={() => {
          userScrolledUp = false;
          if (containerRef) containerRef.scrollTop = containerRef.scrollHeight;
        }}
        class="text-[10px] text-[#3574f0] hover:underline cursor-pointer"
      >
        Scroll to bottom ↓
      </button>
    {/if}
  </div>

  <div
    bind:this={containerRef}
    onscroll={onScroll}
    class="flex-1 p-3 font-mono text-[13px] overflow-y-auto leading-relaxed space-y-3"
  >
    <!-- Thinking section if present -->
    {#if project.thinking}
      <div class="bg-[#252629]/50 border border-[#2b2d30] rounded p-2 text-neutral-400 italic font-mono text-[13px] whitespace-pre-wrap leading-relaxed">
        <div class="text-[10px] uppercase font-sans tracking-wider text-[#6f737a] not-italic mb-1 font-semibold flex items-center gap-1.5">
          <span class="w-1.5 h-1.5 rounded-full bg-[#3574f0]/60"></span>
          Thinking
        </div>
        {project.thinking}
      </div>
    {/if}

    <!-- Activity Log items -->
    {#each project.activityLog as item (item.id)}
      {#if item.type === 'text'}
        <div class="font-mono text-[13px] text-[#dfe1e5] leading-relaxed whitespace-pre-wrap break-words">
          {item.text}
        </div>
      {:else if item.type === 'tool_call'}
        {@const isExpanded = !!expandedTools[item.id]}
        <div class="border border-[#2b2d30] bg-[#252629] rounded text-[13px] overflow-hidden">
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
            <span class="ml-auto text-[10px] text-[#6f737a]">
              {#if item.isError}
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
              <pre class="m-0 font-mono text-[11px] whitespace-pre-wrap">{formatToolArgs(item.args)}</pre>
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

    {#if project.activityLog.length === 0 && !project.thinking}
      <div class="text-[#6f737a] text-xs italic select-none py-4 text-center">
        No activity yet. Send a prompt below to start.
      </div>
    {/if}
  </div>
</div>
