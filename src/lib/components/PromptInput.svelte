<script lang="ts">
  import { tick } from 'svelte';
  import { projectsStore } from '../stores/projects.svelte';
  import type { ProjectState } from '../types';
  import { SendHorizontal } from 'lucide-svelte';

  interface Props {
    project: ProjectState;
  }

  let { project }: Props = $props();

  let text = $state('');
  let textareaRef = $state<HTMLTextAreaElement | null>(null);
  $effect(() => {
    // Focus textarea when switching to this project or project id changes
    if (project.id && textareaRef) {
      textareaRef.focus();
    }
  });


  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      projectsStore.abortPrompt(project.id);
      return;
    }
  }

  async function send(behavior?: string) {
    const trimmed = text.trim();
    if (!trimmed) return;
    const promptToSend = text;
    text = '';

    try {
      await projectsStore.sendPrompt(project.id, promptToSend, behavior);
    } catch {
      // Restore prompt text on failure
      text = promptToSend;
    }
  }

  export function focus() {
    textareaRef?.focus();
  }

  /** Insert text at the textarea caret (replacing any selection), then refocus. */
  export async function insertAtCursor(s: string) {
    const el = textareaRef;
    const start = el?.selectionStart ?? text.length;
    const end = el?.selectionEnd ?? text.length;
    const prev = text[start - 1];
    const ins = (prev && !/\s/.test(prev) ? ' ' : '') + s + ' ';
    text = text.slice(0, start) + ins + text.slice(end);
    await tick();
    el?.focus();
    el?.setSelectionRange(start + ins.length, start + ins.length);
  }
</script>

<div class="flex flex-col h-full w-full bg-[#1e1f22] p-2.5 relative">
  <div class="flex-1 relative flex">
    <textarea
      bind:this={textareaRef}
      bind:value={text}
      onkeydown={handleKeyDown}
      disabled={project.status === 'starting'}
      placeholder={project.isStreaming
        ? 'Agent is working… (send icon for follow-up, Esc to stop)'
        : 'Send a prompt… (use the send icon)'}
      class="w-full h-full bg-[#1e1f22] border border-[#2b2d30] focus:border-[#3574f0] focus:ring-1 focus:ring-[#3574f0] rounded p-2.5 font-mono text-[15px] text-[#dfe1e5] placeholder-[#6f737a] resize-none outline-none leading-relaxed transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
    ></textarea>

    <div class="absolute right-2 bottom-2 flex items-center gap-2">
      <button
        type="button"
        onclick={() => send(project.isStreaming ? 'followUp' : undefined)}
        disabled={!text.trim() || project.status === 'starting'}
        title={project.isStreaming ? 'Send as follow-up' : 'Send prompt'}
        class="flex items-center justify-center p-1.5 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
      >
        <SendHorizontal class="w-4 h-4" />
      </button>
    </div>
  </div>
</div>
