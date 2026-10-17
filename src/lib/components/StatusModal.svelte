<script lang="ts">
  import { X, Copy, Check } from 'lucide-svelte';

  interface Props {
    open: boolean;
    text: string;
    onClose: () => void;
  }

  let { open, text, onClose }: Props = $props();

  let copied = $state(false);
  let copyTimer: number | undefined = undefined;

  async function copyText() {
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
      clearTimeout(copyTimer);
      copyTimer = window.setTimeout(() => {
        copied = false;
      }, 1500);
    } catch (err) {
      console.error('Failed to copy git status text:', err);
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      onClose();
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

    <div class="relative w-full max-w-2xl max-h-[80vh] flex flex-col bg-[#1e1f22] border border-[#393b40] rounded shadow-2xl overflow-hidden z-10 text-xs font-sans">
      <div class="flex items-center justify-between px-4 py-2.5 bg-[#252629] border-b border-[#2b2d30] shrink-0">
        <span class="font-semibold text-[#dfe1e5]">Git Status</span>
        <div class="flex items-center gap-2">
          <button
            type="button"
            onclick={copyText}
            class="flex items-center gap-1 px-2 py-1 rounded bg-[#1e1f22] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] transition-colors"
          >
            {#if copied}
              <Check class="w-3.5 h-3.5 text-[#59a869]" />
              <span class="text-[#59a869]">Copied</span>
            {:else}
              <Copy class="w-3.5 h-3.5" />
              <span>Copy</span>
            {/if}
          </button>
          <button
            type="button"
            onclick={onClose}
            class="p-1 rounded text-[#6f737a] hover:text-[#dfe1e5] hover:bg-[#35373c] transition-colors"
          >
            <X class="w-4 h-4" />
          </button>
        </div>
      </div>

      <div class="flex-1 p-4 overflow-y-auto select-text font-mono text-xs bg-[#1e1f22] text-[#bcbec4]">
        <pre class="m-0 whitespace-pre-wrap font-mono leading-relaxed">{text || 'No output.'}</pre>
      </div>

      <div class="px-4 py-2 bg-[#252629] border-t border-[#2b2d30] flex justify-end shrink-0">
        <button
          type="button"
          onclick={onClose}
          class="px-3 py-1.5 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium transition-colors"
        >
          Close
        </button>
      </div>
    </div>
  </div>
{/if}
