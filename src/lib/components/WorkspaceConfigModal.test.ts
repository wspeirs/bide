import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils';
import { createTestingPinia } from '@pinia/testing';
import { invoke } from '@tauri-apps/api/core';
import WorkspaceConfigModal from './WorkspaceConfigModal.vue';
import type { ProjectState } from '../types';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  Channel: class {
    constructor(public onmessage?: (msg: unknown) => void) {}
  },
}));

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

describe('WorkspaceConfigModal', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd: string) => {
      if (cmd === 'workspace_config_get') {
        return Promise.resolve({ text: 'symbolPreset: nerd\n', source: 'workspace', warning: null });
      }
      if (cmd === 'workspace_env_get') {
        return Promise.resolve({ GEMINI_API_KEY: 'existing-key' });
      }
      return Promise.resolve(undefined);
    });
  });

  it('renders Environment Variables tab with known names datalist and saves env pairs', async () => {
    const wrapper = mount(WorkspaceConfigModal, {
      props: {
        open: true,
        project: createTestProject(),
      },
      global: {
        plugins: [createTestingPinia({ createSpy: vi.fn, stubActions: false })],
      },
    });

    await flushPromises();

    // Switch to Environment Variables tab
    const envTabBtn = wrapper.findAll('button').find((b) => b.text().includes('Environment Variables'));
    expect(envTabBtn).toBeDefined();
    await envTabBtn!.trigger('click');

    // Known env vars datalist includes OPENROUTER_API_KEY and GEMINI_API_KEY
    const options = wrapper.findAll('datalist#known-omp-env-vars option').map((o) => o.attributes('value'));
    expect(options).toContain('OPENROUTER_API_KEY');
    expect(options).toContain('GEMINI_API_KEY');

    // Existing row is populated
    const nameInputs = wrapper.findAll('input[list="known-omp-env-vars"]');
    expect(nameInputs).toHaveLength(1);
    expect((nameInputs[0].element as HTMLInputElement).value).toBe('GEMINI_API_KEY');

    // Add a new variable row
    const addBtn = wrapper.findAll('button').find((b) => b.text().includes('Add variable'));
    await addBtn!.trigger('click');

    const updatedNameInputs = wrapper.findAll('input[list="known-omp-env-vars"]');
    const valueInputs = wrapper.findAll('input[placeholder="Value"]');
    expect(updatedNameInputs).toHaveLength(2);

    await updatedNameInputs[1].setValue('OPENROUTER_API_KEY');
    await valueInputs[1].setValue('or-secret');

    // Save
    const saveBtn = wrapper.findAll('button').find((b) => b.text() === 'Save');
    await saveBtn!.trigger('click');
    await flushPromises();

    expect(invoke).toHaveBeenCalledTimes(3);
    expect(invoke).toHaveBeenLastCalledWith('workspace_save', {
      id: 'proj-1',
      text: null,
      env: {
        GEMINI_API_KEY: 'existing-key',
        OPENROUTER_API_KEY: 'or-secret',
      },
    });
  });

  function mountModal() {
    return mount(WorkspaceConfigModal, {
      props: { open: true, project: createTestProject() },
      global: { plugins: [createTestingPinia({ createSpy: vi.fn, stubActions: false })] },
    });
  }

  function button(wrapper: VueWrapper, label: string) {
    return wrapper.findAll('button').find((b) => b.text().includes(label))!;
  }

  it('saves only config when only config was edited', async () => {
    const wrapper = mountModal();
    await flushPromises();
    await wrapper.find('textarea').setValue('symbolPreset: ascii\n');
    await button(wrapper, 'Save').trigger('click');
    await flushPromises();

    expect(invoke).toHaveBeenLastCalledWith('workspace_save', { id: 'proj-1', text: 'symbolPreset: ascii\n', env: null });
  });

  it('saves config and env edits in a single call', async () => {
    const wrapper = mountModal();
    await flushPromises();
    await wrapper.find('textarea').setValue('symbolPreset: ascii\n');
    await button(wrapper, 'Environment Variables').trigger('click');
    await wrapper.find('input[placeholder="Value"]').setValue('new-key');
    await button(wrapper, 'Save').trigger('click');
    await flushPromises();

    const saves = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === 'workspace_save');
    expect(saves).toEqual([
      ['workspace_save', { id: 'proj-1', text: 'symbolPreset: ascii\n', env: { GEMINI_API_KEY: 'new-key' } }],
    ]);
  });

  it('rejects PATH before calling the backend', async () => {
    const wrapper = mountModal();
    await flushPromises();
    await button(wrapper, 'Environment Variables').trigger('click');
    await wrapper.find('input[list="known-omp-env-vars"]').setValue('path');
    await button(wrapper, 'Save').trigger('click');
    await flushPromises();

    expect(invoke).not.toHaveBeenCalledWith('workspace_save', expect.anything());
    expect(wrapper.text()).toContain('PATH cannot be set per repository.');
  });

  it('keeps config editable and savable when the env file fails to load', async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'workspace_config_get') {
        return Promise.resolve({ text: 'symbolPreset: nerd\n', source: 'workspace', warning: null });
      }
      if (cmd === 'workspace_env_get') return Promise.reject('Failed to parse workspace env file: bad');
      return Promise.resolve(undefined);
    });
    const wrapper = mountModal();
    await flushPromises();

    expect(wrapper.text()).toContain('Failed to parse workspace env file: bad');
    expect(wrapper.find('textarea').attributes('disabled')).toBeUndefined();
    await button(wrapper, 'Save').trigger('click');
    await flushPromises();

    expect(invoke).toHaveBeenLastCalledWith('workspace_save', { id: 'proj-1', text: 'symbolPreset: nerd\n', env: null });
  });
});
