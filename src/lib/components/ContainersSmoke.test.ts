import { describe, it, expect, beforeEach, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { createTestingPinia } from '@pinia/testing';
import { nextTick, reactive } from 'vue';
import { SplitterGroup } from 'reka-ui';
import ProjectView from './ProjectView.vue';
import App from '../../App.vue';
import { useProjectsStore } from '../stores/projects';
import type { ProjectState } from '../types';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockImplementation((cmd: string) => {
    if (cmd === 'projects_restore') return Promise.resolve([]);
    if (cmd === 'omp_list_roles') return Promise.resolve([]);
    if (cmd === 'git_worktree_list') return Promise.resolve([]);
    if (cmd === 'git_branches') return Promise.resolve({ local: [], remote: [] });
    if (cmd === 'fs_list_dir') return Promise.resolve([]);
    return Promise.resolve(undefined);
  }),
  Channel: class {
    constructor(public onmessage?: (msg: unknown) => void) {}
  },
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn().mockResolvedValue(null),
  confirm: vi.fn().mockResolvedValue(true),
}));

// ResizeObserver stub for reka-ui Splitter
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}

beforeEach(() => {
  vi.stubGlobal('ResizeObserver', ResizeObserverStub);
  localStorage.clear();
});

function makeProject(overrides: Partial<ProjectState> = {}): ProjectState {
  return {
    id: 'test-proj-1',
    dir: '/tmp/test-project',
    name: 'test-project',
    status: 'ready',
    restartPending: false,
    orchestrate: false,
    role: 'default',
    advisor: false,
    lastPrompt: 'echo test',
    activityLog: [],
    isStreaming: false,
    model: 'gpt-4o',
    contextPercent: 12.5,
    pendingUiRequest: null,
    gitStatus: {
      detached: false,
      branch: 'main',
      ahead: 0,
      behind: 0,
      staged: [],
      unstaged: [],
      untracked: [],
    },
    lastFetched: null,
    errorStderr: null,
    debugLog: [],
    debugOpen: false,
    ...overrides,
  };
}

describe('Containers smoke tests', () => {
  it('mounts ProjectView without throwing, renders splitters and panels', async () => {
    const project = reactive(makeProject());
    const pinia = createTestingPinia({ createSpy: vi.fn, stubActions: false });
    const store = useProjectsStore(pinia);
    store.projects.push(project);
    store.activeProjectId = project.id;

    const wrapper = mount(ProjectView, {
      props: { project },
      global: { plugins: [pinia] },
    });

    expect(wrapper.exists()).toBe(true);
    // Debug pane is closed by default
    expect(wrapper.find('#left-debug').exists()).toBe(false);

    // Splitter persistence checks
    const splitterGroups = wrapper.findAllComponents(SplitterGroup);
    const verticalGroup = splitterGroups.find((g) => g.props('direction') === 'vertical');
    expect(verticalGroup).toBeDefined();

    verticalGroup!.vm.$emit('layout', [30, 40, 30]);
    expect(store.paneSizes.left).toEqual([30, 40, 30]);

    verticalGroup!.vm.$emit('layout', [10, 40, 20, 30]);
    expect(store.paneSizes.left).toEqual([30, 40, 30]);

    const horizontalGroup = splitterGroups.find((g) => g.props('direction') === 'horizontal');
    expect(horizontalGroup).toBeDefined();

    horizontalGroup!.vm.$emit('layout', [70, 30]);
    expect(store.paneSizes.git).toBe(30);

    // Toggle debugOpen to true: mounts 4th panel
    project.debugOpen = true;
    await nextTick();
    expect(wrapper.find('#left-debug').exists()).toBe(true);

    // Toggle debugOpen back to false
    project.debugOpen = false;
    await nextTick();
    expect(wrapper.find('#left-debug').exists()).toBe(false);
  });

  it('mounts App in empty state and transitions when project is active', async () => {
    const pinia = createTestingPinia({ createSpy: vi.fn, stubActions: false });
    const store = useProjectsStore(pinia);

    const wrapper = mount(App, {
      global: { plugins: [pinia] },
    });

    expect(wrapper.exists()).toBe(true);
    // Initially empty state
    expect(wrapper.text()).toContain('Welcome to bide');

    // Add active project
    const project = makeProject();
    store.projects.push(project);
    store.activeProjectId = project.id;
    await nextTick();

    // Now renders ProjectView instead of empty state
    expect(wrapper.findComponent(ProjectView).exists()).toBe(true);
    expect(wrapper.text()).not.toContain('Welcome to bide');
    wrapper.unmount();
  });
});
