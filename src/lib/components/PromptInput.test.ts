import { describe, it, expect, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { createTestingPinia } from '@pinia/testing';
import { nextTick } from 'vue';
import PromptInput from './PromptInput.vue';
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
    advisor: false,
    lastPrompt: null,
    activityLog: [],
    isStreaming: false,
    model: 'test-model',
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
});
