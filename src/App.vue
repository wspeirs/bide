<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue';
import { listen } from '@tauri-apps/api/event';
import { useProjectsStore } from './lib/stores/projects';
import TabBar from './lib/components/TabBar.vue';
import ProjectView from './lib/components/ProjectView.vue';
import UiRequestDialog from './lib/components/UiRequestDialog.vue';
import UpdateModal from './lib/components/UpdateModal.vue';
import { FolderOpen, Plus } from '@lucide/vue';

const store = useProjectsStore();
const updateModalOpen = ref(false);

function handleKeyDown(e: KeyboardEvent) {
  const isMod = e.ctrlKey || e.metaKey;

  // Ctrl+1 through Ctrl+9: switch to tab 1 through 9
  if (isMod && e.key >= '1' && e.key <= '9') {
    const index = parseInt(e.key, 10) - 1;
    if (index < store.projects.length) {
      e.preventDefault();
      store.setActiveTab(store.projects[index].id);
    }
    return;
  }

  // When Esc is pressed, call omp_abort if streaming
  if (e.key === 'Escape') {
    const active = store.activeProject;
    if (active && active.isStreaming) {
      e.preventDefault();
      store.abortPrompt(active.id);
      return;
    }
  }
}

let unlisten: (() => void) | undefined;
let isUnmounted = false;

onMounted(() => {
  store.restoreProjects();
  store.startGitPolling();

  listen<string>('menu', (event) => {
    const menuId = event.payload;
    if (menuId === 'new_tab') {
      store.openProject();
    } else if (menuId === 'close_tab') {
      store.closeActiveProject();
    } else if (menuId === 'update_omp') {
      updateModalOpen.value = true;
    }
  }).then((fn) => {
    if (isUnmounted) {
      fn();
    } else {
      unlisten = fn;
    }
  });

  window.addEventListener('keydown', handleKeyDown);
});

onBeforeUnmount(() => {
  isUnmounted = true;
  store.stopGitPolling();
  window.removeEventListener('keydown', handleKeyDown);
  if (unlisten) {
    unlisten();
    unlisten = undefined;
  }
});
</script>

<template>
  <main class="flex h-screen w-screen flex-col bg-[#1e1f22] text-[#dfe1e5] select-none font-sans overflow-hidden">
    <!-- Top Tab Bar -->
    <TabBar />

    <!-- Main View -->
    <div class="flex-1 w-full overflow-hidden relative">
      <template v-if="store.activeProject">
        <ProjectView :project="store.activeProject" />
        <UiRequestDialog :project="store.activeProject" />
      </template>
      <template v-else>
        <!-- Centered IntelliJ Empty State when no tabs open -->
        <div class="h-full w-full flex flex-col items-center justify-center p-8 text-center text-[#bcbec4]">
          <div class="flex flex-col items-center max-w-md gap-6 bg-[#252629]/40 border border-[#2b2d30] rounded-xl p-8 shadow-xl">
            <!-- bide logo / icon mark -->
            <div class="relative flex items-center justify-center">
              <div class="w-16 h-16 rounded-xl bg-[#252629] border border-[#393b40] flex items-center justify-center text-[#3574f0] shadow-md">
                <FolderOpen class="w-8 h-8" />
              </div>
              <span class="absolute -top-1 -right-1 w-3.5 h-3.5 rounded-full bg-[#59a869] border-2 border-[#1e1f22]"></span>
            </div>

            <div class="space-y-1.5">
              <h1 class="text-lg font-semibold text-[#dfe1e5] tracking-tight">Welcome to bide</h1>
              <p class="text-[15px] text-[#6f737a] leading-relaxed max-w-xs">
                Agent IDE powered by omp RPC. Open a workspace folder to begin.
              </p>
            </div>

            <button
              type="button"
              @click="store.openProject()"
              class="flex items-center gap-2 px-5 py-2.5 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium text-[15px] transition-colors shadow-sm cursor-pointer"
            >
              <Plus class="w-4 h-4" />
              <span>Open Project (Ctrl+T)</span>
            </button>

            <!-- Keyboard shortcut hints -->
            <div class="w-full pt-4 border-t border-[#2b2d30] flex flex-col gap-2 text-xs font-mono text-[#6f737a]">
              <div class="flex items-center justify-between gap-4">
                <span>Open project folder</span>
                <kbd class="px-2 py-0.5 rounded bg-[#2b2d30] border border-[#393b40] text-[#dfe1e5] text-[13px]">Ctrl+T</kbd>
              </div>
              <div class="flex items-center justify-between gap-4">
                <span>Close active tab</span>
                <kbd class="px-2 py-0.5 rounded bg-[#2b2d30] border border-[#393b40] text-[#dfe1e5] text-[13px]">Ctrl+W</kbd>
              </div>
              <div class="flex items-center justify-between gap-4">
                <span>Switch tab</span>
                <kbd class="px-2 py-0.5 rounded bg-[#2b2d30] border border-[#393b40] text-[#dfe1e5] text-[13px]">Ctrl+1..9</kbd>
              </div>
              <div class="flex items-center justify-between gap-4">
                <span>Stop / abort run</span>
                <kbd class="px-2 py-0.5 rounded bg-[#2b2d30] border border-[#393b40] text-[#dfe1e5] text-[13px]">Esc</kbd>
              </div>
            </div>
          </div>
        </div>
      </template>
    </div>

    <!-- Update omp Modal -->
    <UpdateModal :open="updateModalOpen" @close="updateModalOpen = false" />
  </main>
</template>
