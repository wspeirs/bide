import { describe, it, expect, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { createTestingPinia } from '@pinia/testing';
import UiRequestDialog from './UiRequestDialog.vue';
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
describe('UiRequestDialog', () => {
  function makeProject(pendingUiRequest: ProjectState['pendingUiRequest'] = null): ProjectState {
    return {
      id: 'proj-1',
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
      pendingUiRequest,
      gitStatus: null,
      lastFetched: null,
      errorStderr: null,
      debugLog: [],
      debugOpen: false,
    };
  }

  // 1. single-select question: Submit disabled until an option or non-blank custom text is given.
  it('single-select question: Submit disabled until an option or non-blank custom text is given', async () => {
    const project = makeProject({
      id: 'req-1',
      method: 'ask',
      questions: [
        {
          id: 'q1',
          question: 'Pick one',
          options: [{ label: 'Option A' }, { label: 'Option B' }],
          multi: false,
        },
      ],
    });

    const wrapper = mount(UiRequestDialog, {
      props: { project },
      global: {
        plugins: [createTestingPinia({ createSpy: vi.fn, stubActions: false })],
      },
    });

    const submitBtn = wrapper.find('button[type="submit"]');
    expect(submitBtn.exists()).toBe(true);
    expect(submitBtn.attributes('disabled')).toBeDefined();

    // Selecting an option enables submit
    const optionBtns = wrapper.findAll('button[type="button"]').filter((b) => b.text().includes('Option A'));
    await optionBtns[0].trigger('click');
    expect(submitBtn.attributes('disabled')).toBeUndefined();

    // Deselecting disables submit again
    await optionBtns[0].trigger('click');
    expect(submitBtn.attributes('disabled')).toBeDefined();

    // Whitespace only in custom input leaves submit disabled
    const customInput = wrapper.find('input[placeholder="Other (type your own)…"]');
    await customInput.setValue('   ');
    expect(submitBtn.attributes('disabled')).toBeDefined();

    // Non-blank custom text enables submit
    await customInput.setValue('Custom text');
    expect(submitBtn.attributes('disabled')).toBeUndefined();
  });

  // 2. multi-select may be submitted empty.
  it('multi-select may be submitted empty', async () => {
    const project = makeProject({
      id: 'req-2',
      method: 'ask',
      questions: [
        {
          id: 'q1',
          question: 'Pick any',
          options: [{ label: 'Option A' }, { label: 'Option B' }],
          multi: true,
        },
      ],
    });

    const wrapper = mount(UiRequestDialog, {
      props: { project },
      global: {
        plugins: [createTestingPinia({ createSpy: vi.fn, stubActions: false })],
      },
    });

    const submitBtn = wrapper.find('button[type="submit"]');
    expect(submitBtn.exists()).toBe(true);
    expect(submitBtn.attributes('disabled')).toBeUndefined();

    const store = useProjectsStore();
    const respondUiSpy = vi.spyOn(store, 'respondUi');

    await wrapper.find('form').trigger('submit');
    expect(respondUiSpy).toHaveBeenCalledWith('proj-1', {
      answers: [
        {
          id: 'q1',
          selectedOptions: [],
        },
      ],
    });
  });

  // 3. answers are in question order with matching id, customInput trimmed and omitted when blank.
  it('answers are in question order with matching id, customInput trimmed and omitted when blank', async () => {
    const project = makeProject({
      id: 'req-3',
      method: 'ask',
      questions: [
        {
          id: 'q1',
          question: 'Question 1',
          options: [{ label: 'Q1A' }, { label: 'Q1B' }],
          multi: true,
        },
        {
          id: 'q2',
          question: 'Question 2',
          options: [{ label: 'Q2A' }, { label: 'Q2B' }],
          multi: false,
        },
        {
          id: 'q3',
          question: 'Question 3',
          options: [{ label: 'Q3A' }],
          multi: true,
        },
      ],
    });

    const wrapper = mount(UiRequestDialog, {
      props: { project },
      global: {
        plugins: [createTestingPinia({ createSpy: vi.fn, stubActions: false })],
      },
    });

    const store = useProjectsStore();
    const respondUiSpy = vi.spyOn(store, 'respondUi');

    // Select Q1A for q1, and give custom input with padding whitespace
    const q1A = wrapper.findAll('button[type="button"]').find((b) => b.text().includes('Q1A'))!;
    await q1A.trigger('click');

    const inputs = wrapper.findAll('input[placeholder="Other (type your own)…"]');
    await inputs[0].setValue('  custom for q1  ');

    // For q2, select Q2A (custom blank)
    const q2A = wrapper.findAll('button[type="button"]').find((b) => b.text().includes('Q2A'))!;
    await q2A.trigger('click');
    await inputs[1].setValue('   '); // whitespace only -> should be omitted

    // q3 left empty

    await wrapper.find('form').trigger('submit');

    expect(respondUiSpy).toHaveBeenCalledWith('proj-1', {
      answers: [
        {
          id: 'q1',
          selectedOptions: ['Q1A'],
          customInput: 'custom for q1',
        },
        {
          id: 'q2',
          selectedOptions: ['Q2A'],
        },
        {
          id: 'q3',
          selectedOptions: [],
        },
      ],
    });
  });

  // 4. single-select: picking an option clears custom text and vice versa.
  it('single-select: picking an option clears custom text and vice versa', async () => {
    const project = makeProject({
      id: 'req-4',
      method: 'ask',
      questions: [
        {
          id: 'q1',
          question: 'Single Choice',
          options: [{ label: 'Option 1' }, { label: 'Option 2' }],
          multi: false,
        },
      ],
    });

    const wrapper = mount(UiRequestDialog, {
      props: { project },
      global: {
        plugins: [createTestingPinia({ createSpy: vi.fn, stubActions: false })],
      },
    });

    const opt1Btn = wrapper.findAll('button[type="button"]').find((b) => b.text().includes('Option 1'))!;
    const input = wrapper.find<HTMLInputElement>('input[placeholder="Other (type your own)…"]');

    // 1. Pick option 1
    await opt1Btn.trigger('click');
    expect(opt1Btn.text()).toContain('● Option 1');

    // 2. Type non-blank in custom -> clears option
    await input.setValue('custom answer');
    expect(opt1Btn.text()).toContain('○ Option 1');
    expect(input.element.value).toBe('custom answer');

    // 3. Pick option 1 again -> clears custom text
    await opt1Btn.trigger('click');
    expect(opt1Btn.text()).toContain('● Option 1');
    expect(input.element.value).toBe('');
  });

  // 5. changing pendingUiRequest resets all selections.
  it('changing pendingUiRequest resets all selections', async () => {
    const project = makeProject({
      id: 'req-5a',
      defaultValue: 'initial-val',
      method: 'input',
    });

    const wrapper = mount(UiRequestDialog, {
      props: { project },
      global: {
        plugins: [createTestingPinia({ createSpy: vi.fn, stubActions: false })],
      },
    });

    const input = wrapper.find<HTMLInputElement>('input[placeholder="Enter value…"]');
    expect(input.element.value).toBe('initial-val');
    await input.setValue('modified-val');
    expect(input.element.value).toBe('modified-val');

    // Switch to another request
    const updatedProject = makeProject({
      id: 'req-5b',
      defaultValue: 'next-val',
      method: 'input',
    });
    await wrapper.setProps({ project: updatedProject });

    expect(input.element.value).toBe('next-val');

    // Switch to ask request, select something, then change request
    const askProject = makeProject({
      id: 'req-5c',
      method: 'ask',
      questions: [
        {
          id: 'q1',
          question: 'Q',
          options: [{ label: 'A' }],
          multi: false,
        },
      ],
    });
    await wrapper.setProps({ project: askProject });

    const btnA = wrapper.findAll('button[type="button"]').find((b) => b.text().includes('A'))!;
    await btnA.trigger('click');
    expect(btnA.text()).toContain('● A');

    const customInput = wrapper.find<HTMLInputElement>('input[placeholder="Other (type your own)…"]');
    await customInput.setValue('some custom');

    // Change pendingUiRequest again
    const askProject2 = makeProject({
      id: 'req-5d',
      method: 'ask',
      questions: [
        {
          id: 'q1',
          question: 'Q',
          options: [{ label: 'A' }],
          multi: false,
        },
      ],
    });
    await wrapper.setProps({ project: askProject2 });

    const btnAAfter = wrapper.findAll('button[type="button"]').find((b) => b.text().includes('A'))!;
    expect(btnAAfter.text()).toContain('○ A');
    const customAfter = wrapper.find<HTMLInputElement>('input[placeholder="Other (type your own)…"]');
    expect(customAfter.element.value).toBe('');
  });
});
