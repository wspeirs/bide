<script lang="ts">
  import { PaneGroup, Pane, PaneResizer } from 'paneforge';
  import ErrorBanner from './ErrorBanner.svelte';
  import StatusBar from './StatusBar.svelte';
  import DebugPane from './DebugPane.svelte';
  import LastPrompt from './LastPrompt.svelte';
  import ActivityPane from './ActivityPane.svelte';
  import ControlStrip from './ControlStrip.svelte';
  import PromptInput from './PromptInput.svelte';
  import GitPanel from './GitPanel.svelte';
  import type { ProjectState } from '../types';
  import { projectsStore } from '../stores/projects.svelte';

  interface Props {
    project: ProjectState;
  }

  let { project }: Props = $props();

  let promptRef = $state<{ insertAtCursor: (s: string) => void }>();
  function onLeftLayoutChange(sizes: number[]) {
    // With the debug pane open there are 4 sizes; don't persist those over the 3-pane layout.
    if (sizes.length === 3) {
      projectsStore.savePaneSizes({
        left: sizes,
        git: projectsStore.paneSizes.git,
      });
    }
  }

  function onMainLayoutChange(sizes: number[]) {
    if (sizes.length >= 2) {
      projectsStore.savePaneSizes({
        left: projectsStore.paneSizes.left,
        git: sizes[1],
      });
    }
  }
</script>

<div class="flex flex-col h-full w-full bg-[#1e1f22] overflow-hidden select-none">
  <!-- Error Banner (process exit / error status with last stderr + restart) -->
  <ErrorBanner {project} />


  <!-- Main Horizontal Split: Left Column (Chat/Prompt) vs Right Column (Git) -->
  <div class="flex-1 w-full overflow-hidden">
    <PaneGroup direction="horizontal" onLayoutChange={onMainLayoutChange} class="h-full w-full">
      <!-- Left Column -->
      <Pane defaultSize={100 - projectsStore.paneSizes.git} minSize={40} class="h-full overflow-hidden flex flex-col">
        <PaneGroup direction="vertical" onLayoutChange={onLeftLayoutChange} class="h-full w-full">
          <!-- Top: LastPrompt -->
          <Pane
            id="left-last-prompt"
            order={1}
            defaultSize={projectsStore.paneSizes.left[0] ?? 20}
            minSize={8}
            collapsible={true}
            collapsedSize={0}
            class="overflow-hidden"
          >
            <LastPrompt {project} />
          </Pane>

          <PaneResizer class="h-1 bg-[#2b2d30] hover:bg-[#3574f0] transition-colors cursor-row-resize shrink-0" />

          <!-- Middle: ActivityPane -->
          <Pane id="left-activity" order={2} defaultSize={projectsStore.paneSizes.left[1] ?? 55} minSize={20} class="overflow-hidden">
            <ActivityPane {project} />
          </Pane>

          <PaneResizer class="h-1 bg-[#2b2d30] hover:bg-[#3574f0] transition-colors cursor-row-resize shrink-0" />

          <!-- Bottom: ControlStrip + PromptInput -->
          <Pane id="left-prompt" order={3} defaultSize={projectsStore.paneSizes.left[2] ?? 25} minSize={15} class="overflow-hidden flex flex-col">
            <ControlStrip {project} />
            <div class="flex-1 overflow-hidden">
              <PromptInput {project} bind:this={promptRef} />
            </div>
          </Pane>

          {#if project.debugOpen}
            <PaneResizer class="h-1 bg-[#2b2d30] hover:bg-[#3574f0] transition-colors cursor-row-resize shrink-0" />

            <!-- Debug: JSON-RPC log, under the prompt -->
            <Pane id="left-debug" order={4} defaultSize={30} minSize={10} class="overflow-hidden">
              <DebugPane {project} />
            </Pane>
          {/if}
        </PaneGroup>
      </Pane>

      <!-- Resizer between Left and Git column -->
      <PaneResizer class="w-1 bg-[#2b2d30] hover:bg-[#3574f0] transition-colors cursor-col-resize shrink-0" />

      <!-- Right Column: GitPanel -->
      <Pane
        defaultSize={projectsStore.paneSizes.git ?? 25}
        minSize={15}
        maxSize={50}
        class="h-full overflow-hidden"
      >
        <GitPanel {project} onPickFile={(p) => promptRef?.insertAtCursor('@' + p)} />
      </Pane>
    </PaneGroup>
  </div>

  <!-- Bottom StatusBar: model, context, run status -->
  <StatusBar {project} />
</div>
