<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { useProjectsStore } from '../stores/projects';
import type { ProjectState } from '../types';
import { ChevronRight, ChevronDown, Folder, File, RefreshCw } from '@lucide/vue';

interface Props {
  project: ProjectState;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  (e: 'pick', relPath: string): void;
}>();

const store = useProjectsStore();

interface Entry {
  name: string;
  is_dir: boolean;
}

// Lazily loaded directory listings, keyed by project-relative dir ('' = root).
const children = ref<Record<string, Entry[]>>({});
const expanded = ref<Record<string, boolean>>({});
const error = ref<string | null>(null);

async function load(rel: string) {
  try {
    children.value[rel] = await store.listDir(props.project.id, rel);
    error.value = null;
  } catch (err) {
    error.value = typeof err === 'string' ? err : (err as Error)?.message || String(err);
  }
}

function reloadAll() {
  Object.keys(children.value).forEach(load);
}

let loadedFor = '';
// New project tab: start over. Git status polling (every 5s) doubles as a change signal.
watch(
  () => [props.project.id, props.project.gitStatus],
  ([newId]) => {
    if (loadedFor !== newId) {
      loadedFor = (newId as string) || '';
      children.value = {};
      expanded.value = {};
      load('');
    } else {
      reloadAll();
    }
  },
  { immediate: true }
);

interface Row {
  rel: string;
  name: string;
  is_dir: boolean;
  depth: number;
}

const rows = computed<Row[]>(() => {
  const out: Row[] = [];
  const walk = (dir: string, depth: number) => {
    for (const e of children.value[dir] ?? []) {
      const rel = dir ? `${dir}/${e.name}` : e.name;
      out.push({ rel, name: e.name, is_dir: e.is_dir, depth });
      if (e.is_dir && expanded.value[rel]) walk(rel, depth + 1);
    }
  };
  walk('', 0);
  return out;
});

function click(r: Row) {
  if (!r.is_dir) {
    emit('pick', r.rel);
    return;
  }
  expanded.value[r.rel] = !expanded.value[r.rel];
  if (expanded.value[r.rel] && !children.value[r.rel]) {
    load(r.rel);
  }
}
</script>

<template>
  <div class="flex flex-col h-full w-full overflow-hidden">
    <div class="px-3 py-1 bg-[#252629] border-b border-[#2b2d30] text-[12px] font-mono uppercase tracking-wider text-[#6f737a] flex items-center justify-between shrink-0">
      <span>Files</span>
      <button type="button" @click="reloadAll" title="Refresh" class="hover:text-[#dfe1e5] cursor-pointer">
        <RefreshCw class="w-3.5 h-3.5" />
      </button>
    </div>

    <div v-if="error" class="px-3 py-1 text-[#e55765] font-mono text-[13px]">{{ error }}</div>

    <div class="flex-1 overflow-y-auto py-1 font-mono text-[14px]">
      <button
        v-for="r in rows"
        :key="r.rel"
        type="button"
        @click="click(r)"
        :title="r.is_dir ? r.rel : `Insert @${r.rel} into prompt`"
        :style="{ paddingLeft: `${8 + r.depth * 14}px` }"
        class="w-full flex items-center gap-1 pr-2 py-0.5 text-left text-[#dfe1e5] hover:bg-[#2b2d30] cursor-pointer"
      >
        <template v-if="r.is_dir">
          <ChevronDown v-if="expanded[r.rel]" class="w-3 h-3 text-[#6f737a] shrink-0" />
          <ChevronRight v-else class="w-3 h-3 text-[#6f737a] shrink-0" />
          <Folder class="w-3.5 h-3.5 text-[#e2a53b] shrink-0" />
        </template>
        <template v-else>
          <span class="w-3 shrink-0"></span>
          <File class="w-3.5 h-3.5 text-[#6f737a] shrink-0" />
        </template>
        <span class="truncate">{{ r.name }}</span>
      </button>
    </div>
  </div>
</template>
