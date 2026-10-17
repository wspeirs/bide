<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import { Plus, FolderOpen } from 'lucide-svelte';

  let selectedFolder = $state<string | null>(null);
  let statusMessage = $state<string>('Ready');

  async function handleNewTab() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Open Project Directory',
      });
      if (selected && typeof selected === 'string') {
        selectedFolder = selected;
        statusMessage = `Opened project directory: ${selected}`;
      }
    } catch (err) {
      console.error('Failed to open project folder:', err);
      statusMessage = `Failed to open directory: ${err}`;
    }
  }

  function handleCloseTab() {
    selectedFolder = null;
    statusMessage = 'Project tab closed';
  }

  onMount(() => {
    let unlisten: (() => void) | undefined;

    listen<string>('menu', (event) => {
      const menuId = event.payload;
      if (menuId === 'new_tab') {
        handleNewTab();
      } else if (menuId === 'close_tab') {
        handleCloseTab();
      } else if (menuId === 'update_omp') {
        statusMessage = 'Update omp requested from menu';
      }
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  });
</script>

<main class="flex h-screen w-screen flex-col bg-[#1e1f22] text-[#dfe1e5] select-none font-sans">
  <!-- Tab bar -->
  <div class="flex h-9 items-center border-b border-[#2b2d30] bg-[#2b2d30] px-2 gap-1 text-[13px]">
    {#if selectedFolder}
      <div class="flex items-center gap-2 px-3 py-1 bg-[#1e1f22] border-t-2 border-t-[#3574f0] text-[#dfe1e5] rounded-t text-xs">
        <span class="inline-block h-2 w-2 rounded-full bg-[#59a869]"></span>
        <span class="font-medium truncate max-w-xs">{selectedFolder.split('/').filter(Boolean).pop() || selectedFolder}</span>
        <button
          type="button"
          class="ml-1 text-[#6f737a] hover:text-[#dfe1e5] transition-colors"
          onclick={handleCloseTab}
          aria-label="Close Tab"
        >
          ×
        </button>
      </div>
    {:else}
      <div class="text-[#6f737a] italic text-xs px-2">No open projects</div>
    {/if}

    <button
      type="button"
      onclick={handleNewTab}
      class="flex h-6 w-6 items-center justify-center rounded text-[#868a91] hover:bg-[#35373c] hover:text-[#dfe1e5] transition-colors"
      title="New Project Tab (Ctrl+T)"
      aria-label="New Project Tab"
    >
      <Plus size={15} />
    </button>
  </div>

  <!-- Content area placeholder -->
  <div class="flex flex-1 items-center justify-center bg-[#1e1f22] p-6 text-center">
    {#if selectedFolder}
      <div class="flex flex-col items-center gap-2">
        <div class="text-sm text-[#bcbec4]">Active Project:</div>
        <div class="font-mono text-xs bg-[#252629] border border-[#2b2d30] px-3 py-1.5 rounded text-[#dfe1e5]">
          {selectedFolder}
        </div>
      </div>
    {:else}
      <div class="flex flex-col items-center gap-3 text-[#6f737a]">
        <FolderOpen size={40} class="stroke-1 text-[#43454a]" />
        <div class="text-sm">
          Click <kbd class="px-1.5 py-0.5 text-xs bg-[#252629] border border-[#2b2d30] rounded text-[#bcbec4]">+</kbd> or press <kbd class="px-1.5 py-0.5 text-xs bg-[#252629] border border-[#2b2d30] rounded text-[#bcbec4]">Ctrl+T</kbd> to open a project directory
        </div>
      </div>
    {/if}
  </div>

  <!-- Status bar -->
  <div class="flex h-6 items-center border-t border-[#2b2d30] bg-[#252629] px-3 text-[11px] text-[#868a91]">
    <span>{statusMessage}</span>
  </div>
</main>
