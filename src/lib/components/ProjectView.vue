<script setup lang="ts">
import { ref } from 'vue';
import { SplitterGroup, SplitterPanel, SplitterResizeHandle } from 'reka-ui';
import ErrorBanner from './ErrorBanner.vue';
import StatusBar from './StatusBar.vue';
import DebugPane from './DebugPane.vue';
import LastPrompt from './LastPrompt.vue';
import ActivityPane from './ActivityPane.vue';
import ControlStrip from './ControlStrip.vue';
import PromptInput from './PromptInput.vue';
import GitPanel from './GitPanel.vue';
import type { ProjectState } from '../types';
import { useProjectsStore } from '../stores/projects';

defineProps<{
  project: ProjectState;
}>();

const store = useProjectsStore();

const promptRef = ref<InstanceType<typeof PromptInput> | null>(null);

function onLeftLayoutChange(sizes: number[]) {
  // With the debug pane open there are 4 sizes; don't persist those over the 3-pane layout.
  if (sizes.length === 3) {
    store.savePaneSizes({
      left: sizes,
      git: store.paneSizes.git,
    });
  }
}

function onMainLayoutChange(sizes: number[]) {
  if (sizes.length >= 2) {
    store.savePaneSizes({
      left: store.paneSizes.left,
      git: sizes[1],
    });
  }
}
</script>

<template>
  <div class="flex flex-col h-full w-full bg-[#1e1f22] overflow-hidden select-none">
    <!-- Error Banner (process exit / error status with last stderr + restart) -->
    <ErrorBanner :project="project" />

    <!-- Main Horizontal Split: Left Column (Chat/Prompt) vs Right Column (Git) -->
    <div class="flex-1 w-full overflow-hidden">
      <SplitterGroup direction="horizontal" @layout="onMainLayoutChange" class="h-full w-full">
        <!-- Left Column -->
        <SplitterPanel
          :id="'left-column'"
          :order="1"
          :default-size="100 - store.paneSizes.git"
          :min-size="40"
          class="h-full overflow-hidden flex flex-col"
        >
          <SplitterGroup direction="vertical" @layout="onLeftLayoutChange" class="h-full w-full">
            <!-- Top: LastPrompt -->
            <SplitterPanel
              id="left-last-prompt"
              :order="1"
              :default-size="store.paneSizes.left[0] ?? 20"
              :min-size="8"
              :collapsible="true"
              :collapsed-size="0"
              class="overflow-hidden"
            >
              <LastPrompt :project="project" />
            </SplitterPanel>

            <SplitterResizeHandle class="h-1 bg-[#2b2d30] hover:bg-[#3574f0] transition-colors cursor-row-resize shrink-0" />

            <!-- Middle: ActivityPane -->
            <SplitterPanel
              id="left-activity"
              :order="2"
              :default-size="store.paneSizes.left[1] ?? 55"
              :min-size="20"
              class="overflow-hidden"
            >
              <ActivityPane :project="project" />
            </SplitterPanel>

            <SplitterResizeHandle class="h-1 bg-[#2b2d30] hover:bg-[#3574f0] transition-colors cursor-row-resize shrink-0" />

            <!-- Bottom: ControlStrip + PromptInput -->
            <SplitterPanel
              id="left-prompt"
              :order="3"
              :default-size="store.paneSizes.left[2] ?? 25"
              :min-size="15"
              class="overflow-hidden flex flex-col"
            >
              <ControlStrip :project="project" />
              <div class="flex-1 overflow-hidden">
                <PromptInput :project="project" ref="promptRef" />
              </div>
            </SplitterPanel>

            <template v-if="project.debugOpen">
              <SplitterResizeHandle class="h-1 bg-[#2b2d30] hover:bg-[#3574f0] transition-colors cursor-row-resize shrink-0" />

              <!-- Debug: JSON-RPC log, under the prompt -->
              <SplitterPanel
                id="left-debug"
                :order="4"
                :default-size="30"
                :min-size="10"
                class="overflow-hidden"
              >
                <DebugPane :project="project" />
              </SplitterPanel>
            </template>
          </SplitterGroup>
        </SplitterPanel>

        <!-- Resizer between Left and Git column -->
        <SplitterResizeHandle class="w-1 bg-[#2b2d30] hover:bg-[#3574f0] transition-colors cursor-col-resize shrink-0" />

        <!-- Right Column: GitPanel -->
        <SplitterPanel
          :id="'right-git'"
          :order="2"
          :default-size="store.paneSizes.git ?? 25"
          :min-size="15"
          :max-size="50"
          class="h-full overflow-hidden"
        >
          <GitPanel :project="project" @pickFile="(p) => promptRef?.insertAtCursor('@' + p)" />
        </SplitterPanel>
      </SplitterGroup>
    </div>

    <!-- Bottom StatusBar: model, context, run status -->
    <StatusBar :project="project" />
  </div>
</template>
