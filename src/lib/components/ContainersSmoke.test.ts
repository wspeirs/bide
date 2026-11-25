import { describe, it, expect, beforeEach, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import { createTestingPinia } from '@pinia/testing';
import { nextTick, reactive } from 'vue';
import { SplitterGroup } from 'reka-ui';
import ProjectView from './ProjectView.vue';
import ActivityPane from './ActivityPane.vue';
import App from '../../App.vue';
import { useProjectsStore } from '../stores/projects';
import type { ProjectState } from '../types';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockImplementation((cmd: string, args?: { id?: string; name?: string | null }) => {
    if (cmd === 'projects_restore') return Promise.resolve([]);
    if (cmd === 'model_profiles_get') {
      const profile = { modelRoles: { default: 'a/b' } };
      return Promise.resolve({ global: { Work: profile, Fast: profile }, local: { Fast: profile, Local: profile } });
    }
    if (cmd === 'omp_set_profile') return Promise.resolve({ id: args?.id, profile: args?.name });
    if (cmd === 'omp_list_thinking_levels') return Promise.resolve(['off', 'high']);
    if (cmd === 'omp_list_models') return Promise.resolve([
      { provider: 'openai', id: 'gpt-4o', name: 'GPT-4o', thinkingLevels: ['off', 'high'] },
      { provider: 'openai', id: 'shared', name: 'Shared model', thinkingLevels: [] },
      { provider: 'openrouter', id: 'shared', name: 'Shared model', thinkingLevels: ['off', 'low', 'medium'] },
    ]);
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
    history: [{ id: 'h1', prompt: 'echo test', response: null }],
    subagents: [],
    caveman: 'off',
    ponytail: 'off',
    rtk: false,
    activityLog: [],
    isStreaming: false,
    model: 'gpt-4o',
    modelKey: 'openai/gpt-4o',
    thinking: 'off',
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

  it('selects distinct Custom primary and advisor models from provider groups and turns the advisor off', async ({ onTestFinished }) => {
    const project = reactive(makeProject({ profile: 'Work' }));
    const pinia = createTestingPinia({ createSpy: vi.fn, stubActions: false });
    const store = useProjectsStore(pinia);
    store.projects.push(project);
    const wrapper = mount(ProjectView, { attachTo: document.body, props: { project }, global: { plugins: [pinia] } });
    onTestFinished(() => wrapper.unmount());
    vi.mocked(store.setModel).mockImplementationOnce(async (_id, model, level) => {
      project.modelKey = model;
      project.model = 'Shared model';
      project.thinking = level ?? 'off';
    });
    vi.mocked(store.setAdvisor).mockImplementation(async (_id, model, level) => {
      project.advisorModelKey = model ?? undefined;
      project.advisorModel = model ? 'GPT-4o' : undefined;
      project.advisorThinking = level ?? undefined;
      project.advisorThinkingLevel = model ? 'off' : undefined;
    });
    const findDialog = () => document.body.querySelector<HTMLElement>('[role="dialog"]');
    const button = (dialog: Element, text: string) =>
      [...dialog.querySelectorAll('button')].find((candidate) => candidate.textContent?.trim() === text)!;
    const thinking = (dialog: Element) => dialog.querySelector<HTMLSelectElement>('label select')!;
    const levels = (dialog: Element) => [...thinking(dialog).options].map(({ value }) => value);
    const chooseThinking = (dialog: Element, level: string) => {
      thinking(dialog).value = level;
      thinking(dialog).dispatchEvent(new Event('change', { bubbles: true }));
    };
    await flushPromises();

    const picker = wrapper.get<HTMLSelectElement>('[aria-label="Model profile"]');
    expect(picker.findAll('option').map((option) => option.text())).toEqual(['Fast', 'Local', 'Work', 'Custom']);
    expect(picker.element.value).toBe('Work');
    const primarySelector = 'button[title^="Primary: "]';
    const advisorSelector = 'button[title^="Advisor: "]';
    expect(wrapper.find(primarySelector).exists()).toBe(false);
    expect(wrapper.find(advisorSelector).exists()).toBe(false);
    expect(wrapper.find('[aria-label="Thinking level"]').exists()).toBe(false);
    expect(findDialog()).toBeNull();

    await picker.setValue('');
    await flushPromises();
    expect(picker.element.value).toBe('');
    expect(findDialog()).toBeNull();
    const modelTrigger = wrapper.get(primarySelector);
    await modelTrigger.trigger('click');
    await flushPromises();
    const dialog = findDialog()!;
    expect(dialog).not.toBeNull();
    expect(document.getElementById(dialog.getAttribute('aria-labelledby')!)?.textContent?.trim()).toBe('Select primary model');
    expect(levels(dialog)).toEqual(['', 'off', 'high']);
    expect(thinking(dialog).value).toBe('off');

    const groups = [...dialog.querySelectorAll('details')];
    expect(groups.map((group) => [
      group.querySelector('summary')?.textContent?.trim().toLowerCase(),
      [...group.querySelectorAll('button')].map((button) => button.textContent?.trim()).sort(),
    ])).toEqual([
      ['openai', ['GPT-4o', 'Shared model']],
      ['openrouter', ['Shared model']],
    ]);
    const providerGroup = groups[1]!;
    expect(providerGroup.open).toBe(false);
    providerGroup.querySelector('summary')!.click();
    expect(providerGroup.open).toBe(true);
    providerGroup.querySelector('button')!.click();
    await flushPromises();
    expect(levels(dialog)).toEqual(['', 'off', 'low', 'medium']);
    expect(thinking(dialog).value).toBe('');
    chooseThinking(dialog, 'medium');
    expect(store.setModel).not.toHaveBeenCalled();
    button(dialog, 'Apply').click();
    await flushPromises();
    expect(store.setModel).toHaveBeenCalledExactlyOnceWith(project.id, 'openrouter/shared', 'medium');
    expect(findDialog()).toBeNull();
    expect(modelTrigger.text().toLowerCase()).toContain('openrouter');
    expect(wrapper.get('[title="Primary model and thinking level"]').text()).toContain('Shared model (medium)');

    await modelTrigger.trigger('click');
    await flushPromises();
    const reopened = findDialog();
    expect(reopened).not.toBeNull();
    expect(reopened!.querySelector('button[aria-pressed="true"]')?.textContent?.trim()).toBe('Shared model');
    expect(thinking(reopened!).value).toBe('medium');
    chooseThinking(reopened!, 'low');
    button(reopened!, 'Cancel').click();
    await flushPromises();
    expect(findDialog()).toBeNull();
    expect(store.setThinking).not.toHaveBeenCalled();
    expect(wrapper.get('[title="Primary model and thinking level"]').text()).toContain('Shared model (medium)');

    await modelTrigger.trigger('click');
    await flushPromises();
    const samePrimary = findDialog()!;
    expect(thinking(samePrimary).value).toBe('medium');
    chooseThinking(samePrimary, 'low');
    button(samePrimary, 'Apply').click();
    await flushPromises();
    expect(store.setThinking).toHaveBeenCalledExactlyOnceWith(project.id, 'low');
    expect(store.setModel).toHaveBeenCalledTimes(1);
    expect(wrapper.get('[title="Primary model and thinking level"]').text()).toContain('Shared model (low)');
    expect(wrapper.find('[aria-label="Thinking level"]').exists()).toBe(false);

    const advisorTrigger = wrapper.get(advisorSelector);
    expect(advisorTrigger.text()).toBe('Advisor: Off');
    await advisorTrigger.trigger('click');
    await flushPromises();
    const advisor = findDialog()!;
    expect(document.getElementById(advisor.getAttribute('aria-labelledby')!)?.textContent?.trim()).toBe('Select advisor model');
    expect(advisor.querySelector('button[aria-pressed="true"]')?.textContent?.trim()).toBe('Off');
    expect(thinking(advisor).disabled).toBe(true);
    const openai = advisor.querySelector('details')!;
    openai.querySelector('summary')!.click();
    const advisorOption = openai.querySelector('button')!;
    vi.mocked(store.setAdvisor).mockRejectedValueOnce(new Error('Advisor unavailable'));
    advisorOption.click();
    await flushPromises();
    expect(levels(advisor)).toEqual(['', 'off', 'high']);
    chooseThinking(advisor, 'high');
    expect(store.setAdvisor).not.toHaveBeenCalled();
    button(advisor, 'Apply').click();
    await flushPromises();
    expect(advisor.querySelector('[role="alert"]')?.textContent).toContain('Advisor unavailable');
    expect(advisorTrigger.text()).toBe('Advisor: Off');
    button(advisor, 'Apply').click();
    await flushPromises();
    expect(store.setAdvisor).toHaveBeenLastCalledWith(project.id, 'openai/gpt-4o', 'high');
    expect(wrapper.get('[title="Advisor model and thinking level"]').text()).toContain('GPT-4o (off)');
    expect(wrapper.get('[title="Primary model and thinking level"]').text()).toContain('Shared model (low)');

    await advisorTrigger.trigger('click');
    await flushPromises();
    const enabledAdvisor = findDialog()!;
    expect(enabledAdvisor.querySelector('button[aria-pressed="true"]')?.textContent?.trim()).toBe('GPT-4o');
    expect(thinking(enabledAdvisor).value).toBe('high');
    button(enabledAdvisor, 'Off').click();
    await flushPromises();
    expect(thinking(enabledAdvisor).disabled).toBe(true);
    expect(wrapper.get('[title="Advisor model and thinking level"]').text()).toContain('GPT-4o (off)');
    button(enabledAdvisor, 'Apply').click();
    await flushPromises();
    expect(store.setAdvisor).toHaveBeenLastCalledWith(project.id, null, null);
    expect(advisorTrigger.text()).toBe('Advisor: Off');
    expect(findDialog()).toBeNull();

    project.isStreaming = true;
    await nextTick();
    expect(picker.element.disabled).toBe(true);
    expect(modelTrigger.attributes('disabled')).toBeDefined();
    expect(advisorTrigger.attributes('disabled')).toBeDefined();

    await wrapper.get('[aria-label="Model profile settings"]').trigger('click');
    expect(store.modelProfilesOpen).toBe(true);
  });

  it('keeps Advisor Off available without a model catalog', async ({ onTestFinished }) => {
    const project = reactive(makeProject({ advisorModelKey: 'openai/gpt-4o', advisorModel: 'GPT-4o' }));
    const pinia = createTestingPinia({ createSpy: vi.fn, stubActions: false });
    const store = useProjectsStore(pinia);
    store.projects.push(project);
    vi.mocked(store.listModels).mockResolvedValueOnce([]);
    const wrapper = mount(ProjectView, { attachTo: document.body, props: { project }, global: { plugins: [pinia] } });
    onTestFinished(() => wrapper.unmount());
    await wrapper.get('button[title^="Advisor: "]').trigger('click');
    await flushPromises();
    const dialog = document.body.querySelector('[role="dialog"]')!;
    expect(dialog.textContent).toContain('No models available.');
    const off = [...dialog.querySelectorAll('button')].find((button) => button.textContent?.trim() === 'Off')!;
    expect(off.disabled).toBe(false);
    off.click();
    await nextTick();
    expect(store.setAdvisor).not.toHaveBeenCalled();
    [...dialog.querySelectorAll('button')].find((button) => button.textContent?.trim() === 'Apply')!.click();
    await flushPromises();
    expect(store.setAdvisor).toHaveBeenCalledExactlyOnceWith(project.id, null, null);
  });

  it('shows the chat model first and removes the advisor label when disabled', async () => {
    const project = reactive(makeProject({
      profile: 'Work', model: 'GPT-6.2 Astra', thinking: 'medium',
      advisorModel: 'Claude Opus 5.5', advisorThinkingLevel: 'high',
    }));
    const pinia = createTestingPinia({ createSpy: vi.fn, stubActions: false });
    const wrapper = mount(ProjectView, { props: { project }, global: { plugins: [pinia] } });
    expect(wrapper.text()).toContain('GPT-6.2 Astra (medium)·Advisor: Claude Opus 5.5 (high)·ctx');

    project.advisorModel = undefined;
    await nextTick();
    expect(wrapper.text()).toContain('GPT-6.2 Astra (medium)·ctx');
    expect(wrapper.text()).not.toContain('Claude Opus 5.5');
    wrapper.unmount();
  });

  it('shows the effective context window from OMP and clears it when usage is unavailable', async () => {
    const project = reactive(makeProject());
    const pinia = createTestingPinia({ createSpy: vi.fn, stubActions: false });
    const store = useProjectsStore(pinia);
    store.projects.push(project);
    const wrapper = mount(ProjectView, { props: { project }, global: { plugins: [pinia] } });

    for (const [contextUsage, label] of [
      [{ percent: 14, contextWindow: 500_000 }, '14.0%/500K'],
      [{ percent: 7, contextWindow: 1_000_000 }, '7.0%/1M'],
      [undefined, '?'],
    ] as const) {
      vi.mocked(invoke).mockResolvedValueOnce({ contextUsage });
      await store.refreshState(project.id);
      expect(wrapper.text()).toContain(`ctx ${label}`);
    }
    wrapper.unmount();
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

  it('ActivityPane shows full intent and tool output when chevron is clicked', async () => {
    const pinia = createTestingPinia({ createSpy: vi.fn, stubActions: false });
    const project = makeProject({
      activityLog: [
        {
          id: 'tc-1',
          type: 'tool_call',
          tool: 'wait',
          intent: 'Waiting up to 5m for ingest-daily-metrics to finish',
          args: {},
          output: 'ingest-daily-metrics completed',
          phase: 'done',
          isError: false,
        },
      ],
    });

    const wrapper = mount(ActivityPane, {
      props: { project },
      global: { plugins: [pinia] },
    });

    expect(wrapper.text()).not.toContain('ingest-daily-metrics completed');
    const btn = wrapper.findAll('button').find((b) => b.text().includes('wait'));
    expect(btn).toBeTruthy();
    await btn!.trigger('click');

    expect(wrapper.text()).toContain('Waiting up to 5m for ingest-daily-metrics to finish');
    expect(wrapper.text()).toContain('ingest-daily-metrics completed');
    wrapper.unmount();
  });

  it('ActivityPane shows live output tail for running tools without expanding', async () => {
    const pinia = createTestingPinia({ createSpy: vi.fn, stubActions: false });
    const lines = Array.from({ length: 15 }, (_, i) => `line ${i + 1}`).join('\n');
    const project = makeProject({
      activityLog: [
        {
          id: 'tc-run',
          type: 'tool_call',
          tool: 'bash',
          intent: 'Running backfill',
          args: { command: 'backfill.sh' },
          output: lines,
          phase: 'running',
        },
      ],
    });

    const wrapper = mount(ActivityPane, {
      props: { project },
      global: { plugins: [pinia] },
    });

    expect(wrapper.text()).toContain('… (3 earlier lines)');
    expect(wrapper.text()).toContain('line 15');
    expect(wrapper.text()).not.toContain('line 1\n');
    wrapper.unmount();
  });
});
