import { describe, it, expect, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { createTestingPinia } from '@pinia/testing';
import { nextTick } from 'vue';
import PromptInput from './PromptInput.vue';
import { useProjectsStore } from '../stores/projects';
import type { ProjectState } from '../types';

vi.mock('@tauri-apps/api/core', () => {
  return {
    invoke: vi.fn().mockResolvedValue(undefined),
    Channel: class {
      constructor(public onmessage?: (msg: unknown) => void) {}
    },
  };
});

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));

function createTestProject(id = 'proj-1'): ProjectState {
  return {
    id,
    dir: '/test/dir',
    name: 'dir',
    status: 'ready',
    restartPending: false,
    orchestrate: false,
    role: 'default',
    history: [],
    subagents: [],
    caveman: 'off',
    ponytail: 'off',
    rtk: false,
    activityLog: [],
    isStreaming: false,
    model: 'test-model',
    modelKey: 'test/test-model',
    thinking: 'off',
    contextPercent: 0,
    pendingUiRequest: null,
    gitStatus: null,
    lastFetched: null,
    errorStderr: null,
    debugLog: [],
    debugOpen: false,
  };
}

describe('PromptInput', () => {
  it('focuses textarea on mount and when project id changes', async () => {
    const project = createTestProject('proj-1');
    const pinia = createTestingPinia({ createSpy: vi.fn, stubActions: false });

    const wrapper = mount(PromptInput, {
      props: { project },
      attachTo: document.body,
      global: {
        plugins: [pinia],
      },
    });

    await nextTick();
    const textarea = wrapper.find('textarea').element;
    expect(document.activeElement).toBe(textarea);

    (textarea as HTMLTextAreaElement).blur();
    expect(document.activeElement).not.toBe(textarea);

    await wrapper.setProps({
      project: createTestProject('proj-2'),
    });
    await nextTick();
    expect(document.activeElement).toBe(textarea);

    wrapper.unmount();
  });

  it('cycles profiles with Ctrl+P and changes thinking only in Custom', async () => {
    const pinia = createTestingPinia({ createSpy: vi.fn });
    const store = useProjectsStore(pinia);
    const project = createTestProject();
    const wrapper = mount(PromptInput, { props: { project }, global: { plugins: [pinia] } });
    const textarea = wrapper.get('textarea');

    await textarea.trigger('keydown', { key: 'p', ctrlKey: true });
    await textarea.trigger('keydown', { key: 'Tab', shiftKey: true });
    expect(store.cycleProfile).toHaveBeenCalledWith(project.id);
    expect(store.cycleThinking).toHaveBeenCalledTimes(1);

    await wrapper.setProps({ project: { ...project, profile: 'Work' } });
    const shiftTab = new KeyboardEvent('keydown', { key: 'Tab', shiftKey: true, cancelable: true });
    textarea.element.dispatchEvent(shiftTab);
    expect(shiftTab.defaultPrevented).toBe(false);
    expect(store.cycleThinking).toHaveBeenCalledTimes(1);

    await wrapper.setProps({ project: { ...project, isStreaming: true } });
    await textarea.trigger('keydown', { key: 'p', ctrlKey: true });
    expect(store.cycleProfile).toHaveBeenCalledTimes(1);
    wrapper.unmount();
  });
});
