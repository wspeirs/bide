<script lang="ts">
  import { projectsStore } from '../stores/projects.svelte';
  import { Plus, X, Loader2, AlertTriangle, ArrowUp } from 'lucide-svelte';

  function onTabClick(id: string) {
    projectsStore.setActiveTab(id);
  }

  function onCloseClick(e: MouseEvent, id: string) {
    e.stopPropagation();
    projectsStore.closeProject(id);
  }

  function onNewTabClick() {
    projectsStore.openProject();
  }
</script>

<div class="flex h-9 w-full items-stretch bg-[#2b2d30] border-b border-[#2b2d30] select-none text-[13px] font-sans overflow-x-auto overflow-y-hidden">
  <div class="flex items-stretch flex-1 min-w-0">
    {#each projectsStore.projects as project (project.id)}
      {@const isActive = project.id === projectsStore.activeProjectId}
      <div
        role="tab"
        tabindex="0"
        aria-selected={isActive}
        onclick={() => onTabClick(project.id)}
        onkeydown={(e) => e.key === 'Enter' && onTabClick(project.id)}
        class="group relative flex items-center gap-2 px-3 border-r border-[#1e1f22] cursor-pointer max-w-[200px] shrink-0 transition-colors {isActive
          ? 'bg-[#1e1f22] text-[#dfe1e5] border-t-2 border-t-[#3574f0]'
          : 'bg-[#2b2d30] text-[#bcbec4] hover:bg-[#35373c] hover:text-[#dfe1e5]'}"
      >
        <!-- Status icon IN FRONT of project name -->
        <span class="flex items-center justify-center shrink-0 w-3.5 h-3.5">
          {#if project.status === 'busy'}
            <Loader2 class="w-3.5 h-3.5 animate-spin text-[#3574f0]" />
          {:else if project.status === 'ready'}
            <span class="w-2 h-2 rounded-full bg-[#59a869]"></span>
          {:else if project.status === 'error'}
            <AlertTriangle class="w-3.5 h-3.5 text-[#e55765]" />
          {:else}
            <!-- starting -->
            <span class="w-2 h-2 rounded-full border border-[#6f737a]"></span>
          {/if}
        </span>

        <!-- Tab title = folder basename -->
        <span class="truncate font-medium flex-1 text-left" title={project.dir}>
          {project.name}
        </span>

        <!-- ⬆ restart-pending marker after name -->
        {#if project.restartPending}
          <span title="Restarts after this turn" class="text-xs text-[#e2a53b] flex items-center">
            <ArrowUp class="w-3 h-3 stroke-[2.5]" />
          </span>
        {/if}

        <!-- Close button × -->
        <button
          type="button"
          onclick={(e) => onCloseClick(e, project.id)}
          title="Close tab"
          class="flex items-center justify-center w-4 h-4 rounded text-[#6f737a] hover:text-[#dfe1e5] hover:bg-[#393b40] shrink-0 transition-colors"
        >
          <X class="w-3 h-3" />
        </button>
      </div>
    {/each}

    <!-- + button at end of tab bar -->
    <button
      type="button"
      onclick={onNewTabClick}
      title="Open project directory (Ctrl+T)"
      class="flex items-center justify-center w-9 h-full text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#35373c] shrink-0 border-r border-[#1e1f22] transition-colors"
    >
      <Plus class="w-4 h-4" />
    </button>
  </div>
</div>
