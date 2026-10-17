<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { projectsStore } from './lib/stores/projects.svelte';
  import TabBar from './lib/components/TabBar.svelte';
  import ProjectView from './lib/components/ProjectView.svelte';
  import UiRequestDialog from './lib/components/UiRequestDialog.svelte';
  import UpdateModal from './lib/components/UpdateModal.svelte';
  import { FolderOpen, Plus } from 'lucide-svelte';

  let updateModalOpen = $state(false);

  function handleKeyDown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 't') {
      e.preventDefault();
      projectsStore.openProject();
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'w') {
      if (projectsStore.activeProjectId) {
        e.preventDefault();
        projectsStore.closeProject(projectsStore.activeProjectId);
      }
    }
  }

  onMount(() => {
    projectsStore.restoreProjects();

    let unlisten: (() => void) | undefined;
    listen<string>('menu', (event) => {
      const menuId = event.payload;
      if (menuId === 'new_tab') {
        projectsStore.openProject();
      } else if (menuId === 'close_tab') {
        if (projectsStore.activeProjectId) {
          projectsStore.closeProject(projectsStore.activeProjectId);
        }
      } else if (menuId === 'update_omp') {
        updateModalOpen = true;
      }
    }).then((fn) => {
      unlisten = fn;
    });

    window.addEventListener('keydown', handleKeyDown);

    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      if (unlisten) {
        unlisten();
      }
    };
  });
</script>

<main class="flex h-screen w-screen flex-col bg-[#1e1f22] text-[#dfe1e5] select-none font-sans overflow-hidden">
  <!-- Top Tab Bar -->
  <TabBar />

  <!-- Main View -->
  <div class="flex-1 w-full overflow-hidden relative">
    {#if projectsStore.activeProject}
      <ProjectView project={projectsStore.activeProject} />
      <UiRequestDialog project={projectsStore.activeProject} />
    {:else}
      <!-- Empty State when no tabs open -->
      <div class="h-full w-full flex flex-col items-center justify-center p-8 text-center text-[#bcbec4]">
        <div class="flex flex-col items-center max-w-sm gap-4">
          <div class="w-16 h-16 rounded-2xl bg-[#252629] border border-[#2b2d30] flex items-center justify-center text-[#3574f0] shadow-lg">
            <FolderOpen class="w-8 h-8" />
          </div>

          <div class="space-y-1">
            <h2 class="text-base font-semibold text-[#dfe1e5]">No Project Open</h2>
            <p class="text-xs text-[#6f737a] leading-relaxed">
              Open a project directory to launch an omp agent session and start working.
            </p>
          </div>

          <button
            type="button"
            onclick={() => projectsStore.openProject()}
            class="flex items-center gap-2 px-4 py-2 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium text-xs transition-colors shadow-sm cursor-pointer"
          >
            <Plus class="w-4 h-4" />
            <span>Open Project Directory</span>
          </button>

          <div class="text-[11px] text-[#6f737a] font-mono">
            Press <kbd class="px-1.5 py-0.5 rounded bg-[#2b2d30] border border-[#393b40] text-[#dfe1e5]">Ctrl+T</kbd> to open
          </div>
        </div>
      </div>
    {/if}
  </div>

  <!-- Update omp Modal -->
  <UpdateModal open={updateModalOpen} onClose={() => (updateModalOpen = false)} />
</main>
