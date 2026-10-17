<script lang="ts">
  import { PaneGroup, Pane, PaneResizer } from 'paneforge';
  import CwdBar from './CwdBar.svelte';
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

  function onLeftLayoutChange(sizes: number[]) {
    if (sizes.length >= 3) {
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
  <!-- Top CwdBar -->
  <CwdBar {project} />

  <!-- Main Horizontal Split: Left Column (Chat/Prompt) vs Right Column (Git) -->
  <div class="flex-1 w-full overflow-hidden">
    <PaneGroup direction="horizontal" onLayoutChange={onMainLayoutChange} class="h-full w-full">
      <!-- Left Column -->
      <Pane defaultSize={100 - projectsStore.paneSizes.git} minSize={40} class="h-full overflow-hidden flex flex-col">
        <PaneGroup direction="vertical" onLayoutChange={onLeftLayoutChange} class="h-full w-full">
          <!-- Top: LastPrompt -->
          <Pane
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
          <Pane defaultSize={projectsStore.paneSizes.left[1] ?? 55} minSize={20} class="overflow-hidden">
            <ActivityPane {project} />
          </Pane>

          <PaneResizer class="h-1 bg-[#2b2d30] hover:bg-[#3574f0] transition-colors cursor-row-resize shrink-0" />

          <!-- Bottom: ControlStrip + PromptInput -->
          <Pane defaultSize={projectsStore.paneSizes.left[2] ?? 25} minSize={15} class="overflow-hidden flex flex-col">
            <ControlStrip {project} />
            <div class="flex-1 overflow-hidden">
              <PromptInput {project} />
            </div>
          </Pane>
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
        <GitPanel {project} />
      </Pane>
    </PaneGroup>
  </div>
</div>
