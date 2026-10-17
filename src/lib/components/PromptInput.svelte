<script lang="ts">
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

    if (e.key === 'Enter') {
      if (e.ctrlKey || e.metaKey) {
        // Ctrl+Enter / Cmd+Enter: steer while streaming or normal send
        e.preventDefault();
        send(project.isStreaming ? 'steer' : undefined);
      } else if (!e.shiftKey) {
        // Enter without shift: send (if streaming: followUp)
        e.preventDefault();
        send(project.isStreaming ? 'followUp' : undefined);
      }
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
</script>

<div class="flex flex-col h-full w-full bg-[#1e1f22] p-2.5 relative">
  <div class="flex-1 relative flex">
    <textarea
      bind:this={textareaRef}
      bind:value={text}
      onkeydown={handleKeyDown}
      disabled={project.status === 'starting'}
      placeholder={project.isStreaming
        ? 'Agent is working… (Enter for follow-up, Ctrl+Enter to steer, Esc to stop)'
        : 'Send a prompt… (Enter to send, Shift+Enter for newline)'}
      class="w-full h-full bg-[#1e1f22] border border-[#2b2d30] focus:border-[#3574f0] focus:ring-1 focus:ring-[#3574f0] rounded p-2.5 font-mono text-[13px] text-[#dfe1e5] placeholder-[#6f737a] resize-none outline-none leading-relaxed transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
    ></textarea>

    <div class="absolute right-2 bottom-2 flex items-center gap-2">
      <button
        type="button"
        onclick={() => send(project.isStreaming ? 'followUp' : undefined)}
        disabled={!text.trim() || project.status === 'starting'}
        title={project.isStreaming ? 'Send as follow-up' : 'Send prompt (Enter)'}
        class="flex items-center justify-center p-1.5 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
      >
        <SendHorizontal class="w-4 h-4" />
      </button>
    </div>
  </div>
</div>
