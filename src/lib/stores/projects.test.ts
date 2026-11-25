/// <reference types="node" />
import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { setActivePinia, createPinia } from 'pinia';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { useProjectsStore } from './projects';
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

import { invoke } from '@tauri-apps/api/core';

describe('useProjectsStore', () => {
  beforeEach(() => {
    localStorage.clear();
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

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

  function mockInvoke(responses: Record<string, unknown>) {
    vi.mocked(invoke).mockImplementation((cmd: string) => Promise.resolve(responses[cmd]));
  }

  describe('1. Store frame handling', () => {
    it('fixture 07 (select) opens a dialog with its options', () => {
      const store = useProjectsStore();
      const project = createTestProject('proj-1');
      store.projects.push(project);

      const filePath = resolve(process.cwd(), 'src-tauri/tests/fixtures/07_extension_ui_request_review_select.jsonl');
      const lines = readFileSync(filePath, 'utf-8').trim().split('\n');
      for (const line of lines) {
        if (!line.trim()) continue;
        store.handleFrame('proj-1', JSON.parse(line));
      }

      expect(project.pendingUiRequest).not.toBeNull();
      expect(project.pendingUiRequest?.method).toBe('select');
      expect(project.pendingUiRequest?.title).toBe('Review Mode');
      expect(project.pendingUiRequest?.options).toEqual([
        '1. Review against a base branch (PR Style)',
        '2. Review uncommitted changes',
        '3. Review a specific commit',
        '4. Review a specific PR',
        '5. Custom review instructions',
      ]);
    });

    it('fixture 08 (setStatus/notify/setWidget) opens nothing', () => {
      const store = useProjectsStore();
      const project = createTestProject('proj-1');
      store.projects.push(project);

      const filePath = resolve(process.cwd(), 'src-tauri/tests/fixtures/08_extension_ui_noise_nonui.jsonl');
      const lines = readFileSync(filePath, 'utf-8').trim().split('\n');
      for (const line of lines) {
        if (!line.trim()) continue;
        store.handleFrame('proj-1', JSON.parse(line));
      }

      expect(project.pendingUiRequest).toBeNull();
      expect(project.caveman).toBe('off');
      expect(project.rtk).toBe(true);
      expect(project.ponytail).toBe('lite');
    });

    it('non-dialog methods (open_url, set_editor_text) open nothing', () => {
      const store = useProjectsStore();
      const project = createTestProject('proj-1');
      store.projects.push(project);

      store.handleFrame('proj-1', {
        type: 'extension_ui_request',
        id: 'req-1',
        method: 'open_url',
        url: 'https://example.com',
      });
      expect(project.pendingUiRequest).toBeNull();

      store.handleFrame('proj-1', {
        type: 'extension_ui_request',
        id: 'req-2',
        method: 'set_editor_text',
        text: 'hello',
      });
      expect(project.pendingUiRequest).toBeNull();
    });

    it('an ask frame yields pendingUiRequest.questions', () => {
      const store = useProjectsStore();
      const project = createTestProject('proj-1');
      store.projects.push(project);

      const questions = [
        { id: 'q1', question: 'Continue?', options: [{ label: 'Yes' }, { label: 'No' }] },
      ];
      store.handleFrame('proj-1', {
        type: 'extension_ui_request',
        id: 'req-ask',
        method: 'ask',
        questions,
      });

      expect(project.pendingUiRequest).not.toBeNull();
      expect(project.pendingUiRequest?.method).toBe('ask');
      expect(project.pendingUiRequest?.questions).toEqual(questions);
    });

    it('cancel with matching targetId clears the request, a non-matching one does not', () => {
      const store = useProjectsStore();
      const project = createTestProject('proj-1');
      store.projects.push(project);

      store.handleFrame('proj-1', {
        type: 'extension_ui_request',
        id: 'req-dialog',
        method: 'confirm',
        message: 'Are you sure?',
      });
      expect(project.pendingUiRequest?.id).toBe('req-dialog');

      // Non-matching targetId
      store.handleFrame('proj-1', {
        type: 'extension_ui_request',
        method: 'cancel',
        targetId: 'different-id',
      });
      expect(project.pendingUiRequest?.id).toBe('req-dialog');

      // Matching targetId
      store.handleFrame('proj-1', {
        type: 'extension_ui_request',
        method: 'cancel',
        targetId: 'req-dialog',
      });
      expect(project.pendingUiRequest).toBeNull();
    });

    it('a collab-prompt message_start records history; other custom messages do not', () => {
      const store = useProjectsStore();
      const project = createTestProject('proj-1');
      store.projects.push(project);

      store.handleFrame('proj-1', { type: 'message_start', message: { role: 'custom', customType: 'xdev-mount-notice', content: 'notice' } });
      expect(project.history).toEqual([]);

      store.handleFrame('proj-1', { type: 'agent_start' });
      store.handleFrame('proj-1', { type: 'message_start', message: { role: 'custom', customType: 'collab-prompt', content: 'from phone', details: { from: 'guest' } } });

      store.handleFrame('proj-1', { type: 'agent_start' });
      store.handleFrame('proj-1', {
        type: 'message_start',
        message: { role: 'custom', customType: 'collab-prompt', content: [{ type: 'text', text: 'with image' }, { type: 'image', data: 'AA==', mimeType: 'image/png' }] },
      });
      expect(project.history.map((e) => e.prompt)).toEqual(['from phone', 'with image']);
    });

    it('run lifecycle', () => {
      const store = useProjectsStore();
      const project = createTestProject('proj-1');
      project.activityLog.push({ id: 'old-1', type: 'text', text: 'old' });
      project.subagents.push({ id: 'old-sa', agent: 'explore', status: 'completed', activityLog: [], startedAt: 1 });
      store.projects.push(project);

      store.handleFrame('proj-1', { type: 'agent_start' });
      store.handleFrame('proj-1', {
        type: 'message_start',
        message: { role: 'user', attribution: 'user', content: [{ type: 'text', text: 'orchestrate\n\nfix it' }] },
      });
      expect(project.activityLog).toEqual([]);
      expect(project.subagents).toEqual([]);
      expect(project.history).toHaveLength(1);
      expect(project.history[0].prompt).toBe('fix it');

      store.handleFrame('proj-1', {
        type: 'message_start',
        message: { role: 'user', attribution: 'agent', content: 'ignored' },
      });
      expect(project.history).toHaveLength(1);
      expect(project.history[0].prompt).toBe('fix it');

      store.handleFrame('proj-1', {
        type: 'message_start',
        message: { role: 'user', attribution: 'user', content: 'also this' },
      });
      expect(project.history[0].prompt).toBe('fix it\n\nalso this');

      store.handleFrame('proj-1', {
        type: 'message_update',
        assistantMessageEvent: { type: 'text_delta', delta: 'thinking aloud' },
      });
      store.handleFrame('proj-1', {
        type: 'message_update',
        assistantMessageEvent: { type: 'toolcall_end', toolCall: { id: 't1', name: 'read', arguments: {} } },
      });
      store.handleFrame('proj-1', {
        type: 'message_update',
        assistantMessageEvent: { type: 'text_delta', delta: '| a |\n|---|' },
      });
      store.handleFrame('proj-1', { type: 'agent_end' });
      expect(project.history[0].response).toBe('| a |\n|---|');
    });

    it('subagents', () => {
      const store = useProjectsStore();
      const project = createTestProject('proj-1');
      store.projects.push(project);

      store.handleFrame('proj-1', {
        type: 'subagent_lifecycle',
        payload: { id: '0-Explore', agent: 'explore', description: 'scan', status: 'started', index: 0 },
      });
      expect(project.subagents).toHaveLength(1);
      expect(project.subagents[0].status).toBe('running');

      store.handleFrame('proj-1', {
        type: 'subagent_event',
        payload: {
          id: '0-Explore',
          event: { type: 'message_update', assistantMessageEvent: { type: 'thinking_delta', delta: 'hmm' } },
        },
      });
      expect(project.subagents[0].activityLog).toHaveLength(1);
      expect(project.subagents[0].activityLog[0]).toMatchObject({ type: 'thinking', text: 'hmm' });
      expect(project.activityLog).toEqual([]);

      store.handleFrame('proj-1', {
        type: 'subagent_lifecycle',
        payload: { id: '0-Explore', agent: 'explore', status: 'completed', index: 0 },
      });
      expect(project.subagents[0].status).toBe('completed');
      expect(project.subagents[0].endedAt).toBeDefined();
    });

    it('multi-select loop', async () => {
      const store = useProjectsStore();
      const project = createTestProject('proj-1');
      project.activityLog.push({
        id: 'ask-1',
        type: 'tool_call',
        tool: 'ask',
        phase: 'running',
        args: {
          html: '<form><fieldset><legend>4. Which</legend><label><input type="checkbox" name="e" value="a" checked> Aug 13 (prior)</label><label><input type="checkbox" name="e" value="b"> Nov 20 (upcoming)</label></fieldset></form>',
        },
      });
      store.projects.push(project);

      const options = ['Aug 13 (prior)', 'Nov 20 (upcoming)', 'Other (type your own)', '\uf00c Done selecting'];
      store.handleFrame('proj-1', {
        type: 'extension_ui_request',
        id: 'r1',
        method: 'select',
        title: '(1 selected) 4. Which',
        options,
      });
      expect(project.pendingUiRequest?.title).toBe('4. Which');
      expect(project.pendingUiRequest?.multi?.checked).toEqual(['Aug 13 (prior)']);
      expect(project.pendingUiRequest?.multi?.uncertain).toBe(false);

      await store.respondUi('proj-1', { value: 'Nov 20 (upcoming)' });
      store.handleFrame('proj-1', {
        type: 'extension_ui_request',
        id: 'r2',
        method: 'select',
        title: '(2 selected) 4. Which',
        options,
      });
      expect(project.pendingUiRequest?.multi?.checked).toEqual(['Aug 13 (prior)', 'Nov 20 (upcoming)']);

      await store.respondUi('proj-1', { value: 'Aug 13 (prior)' });
      store.handleFrame('proj-1', {
        type: 'extension_ui_request',
        id: 'r3',
        method: 'select',
        title: '(1 selected) 4. Which',
        options,
      });
      expect(project.pendingUiRequest?.multi?.checked).toEqual(['Nov 20 (upcoming)']);

      await store.respondUi('proj-1', { value: 'Other (type your own)' });
      store.handleFrame('proj-1', {
        type: 'extension_ui_request',
        id: 'r4',
        method: 'input',
        title: '4. Which',
      });
      await store.respondUi('proj-1', { value: ' late ' });
      store.handleFrame('proj-1', {
        type: 'extension_ui_request',
        id: 'r5',
        method: 'select',
        title: '(2 selected) 4. Which',
        options,
      });
      expect(project.pendingUiRequest?.multi?.other).toBe('late');
      expect(project.pendingUiRequest?.multi?.uncertain).toBe(false);
    });
  });

  describe('2. respondUi wire format', () => {
    it('invoke receives type: "extension_ui_response" and pending request id; confirm sends confirmed', async () => {
      const store = useProjectsStore();
      const project = createTestProject('proj-1');
      project.pendingUiRequest = {
        id: 'req-confirm',
        method: 'confirm',
        message: 'Confirm action?',
      };
      store.projects.push(project);

      await store.respondUi('proj-1', { confirmed: true });

      expect(invoke).toHaveBeenCalledWith('omp_ui_respond', {
        id: 'proj-1',
        response: {
          type: 'extension_ui_response',
          id: 'req-confirm',
          confirmed: true,
        },
      });
      expect(project.pendingUiRequest).toBeNull();
    });

    it('ask sends answers', async () => {
      const store = useProjectsStore();
      const project = createTestProject('proj-1');
      project.pendingUiRequest = {
        id: 'req-ask',
        method: 'ask',
        questions: [{ id: 'q1', question: 'Pick', options: [{ label: 'A' }] }],
      };
      store.projects.push(project);

      const answers = [{ id: 'q1', selected: ['A'] }];
      await store.respondUi('proj-1', { answers });

      expect(invoke).toHaveBeenCalledWith('omp_ui_respond', {
        id: 'proj-1',
        response: {
          type: 'extension_ui_response',
          id: 'req-ask',
          answers,
        },
      });
      expect(project.pendingUiRequest).toBeNull();
    });

    it('a second call with no pending request sends nothing', async () => {
      const store = useProjectsStore();
      const project = createTestProject('proj-1');
      project.pendingUiRequest = {
        id: 'req-once',
        method: 'confirm',
      };
      store.projects.push(project);

      await store.respondUi('proj-1', { confirmed: true });
      expect(invoke).toHaveBeenCalledTimes(1);

      // Second call when pendingUiRequest is already null
      await store.respondUi('proj-1', { confirmed: true });
      expect(invoke).toHaveBeenCalledTimes(1);
    });
  });

  describe('4. Prefs persistence, and nothing else', () => {
    it('setting paneSizes writes bide_pane_sizes as JSON; setting activeProjectId writes bide_active_tab as plain string; setting to null removes the key', () => {
      const store = useProjectsStore();

      store.savePaneSizes({ left: [25, 50, 25], git: 30 });
      expect(localStorage.getItem('bide_pane_sizes')).toBe(JSON.stringify({ left: [25, 50, 25], git: 30 }));

      store.activeProjectId = 'proj-abc';
      expect(localStorage.getItem('bide_active_tab')).toBe('proj-abc');

      store.activeProjectId = null;
      expect(localStorage.getItem('bide_active_tab')).toBeNull();
    });

    it('a new Pinia over the same storage reads both back', () => {
      localStorage.setItem('bide_pane_sizes', JSON.stringify({ left: [15, 60, 25], git: 28 }));
      localStorage.setItem('bide_active_tab', 'proj-restored');

      setActivePinia(createPinia());
      const store = useProjectsStore();

      expect(store.paneSizes).toEqual({ left: [15, 60, 25], git: 28 });
      expect(store.activeProjectId).toBe('proj-restored');
    });

    it('shows the restored active tab’s current model and thinking', async () => {
      localStorage.setItem('bide_active_tab', 'proj-restored');
      mockInvoke({
        projects_restore: [{ id: 'proj-restored', dir: '/test/dir', profile: null }],
        omp_get_state: { model: { provider: 'openai', id: 'gpt-4.1', name: 'GPT-4.1' } },
      });
      const store = useProjectsStore();

      await store.restoreProjects();

      await vi.waitFor(() => {
        expect(store.activeProject?.modelKey).toBe('openai/gpt-4.1');
        expect(store.activeProject?.thinking).toBe('off');
      });
    });

    it('corrupt JSON in bide_pane_sizes falls back to defaults instead of throwing', () => {
      localStorage.setItem('bide_pane_sizes', '{ corrupt: json');

      setActivePinia(createPinia());
      let store: { paneSizes: unknown } | undefined;
      expect(() => {
        store = useProjectsStore();
      }).not.toThrow();
      expect(store?.paneSizes).toEqual({ left: [20, 55, 25], git: 25 });
    });

    it('after opening a project and feeding it frames, Object.keys(localStorage) is a subset of those two keys', async () => {
      vi.mocked(invoke).mockImplementation(async (cmd) =>
        cmd === 'project_open' ? { id: 'proj-stream', dir: '/x/proj-stream' } : undefined
      );
      const store = useProjectsStore();
      await store.openProject('/x/proj-stream');

      // Feed multiple frames (message deltas, thinking, commands, debug)
      store.handleFrame('proj-stream', {
        type: 'message_update',
        assistantMessageEvent: { type: 'thinking_start' },
      });
      store.handleFrame('proj-stream', {
        type: 'message_update',
        assistantMessageEvent: { type: 'thinking_delta', delta: 'pondering...' },
      });
      store.handleFrame('proj-stream', {
        type: 'message_update',
        assistantMessageEvent: { type: 'thinking_end' },
      });
      store.handleFrame('proj-stream', {
        type: 'command_output',
        text: 'npm test',
      });

      const keys = Object.keys(localStorage);
      expect(keys).toContain('bide_active_tab');
      const allowedKeys = new Set(['bide_pane_sizes', 'bide_active_tab']);
      for (const k of keys) {
        expect(allowedKeys.has(k)).toBe(true);
      }
    });
  });

  describe('5. Polling lifecycle', () => {
    it('startGitPolling() creates exactly one interval (calling twice does not stack), stopGitPolling() clears it', () => {
      vi.useFakeTimers();
      const store = useProjectsStore();
      const project = createTestProject('proj-1');
      store.projects.push(project);
      store.activeProjectId = 'proj-1';

      store.startGitPolling();
      store.startGitPolling(); // Second call should not double-stack

      vi.advanceTimersByTime(5000);
      expect(invoke).toHaveBeenCalledWith('git_status', { id: 'proj-1' });
      expect(invoke).toHaveBeenCalledTimes(1);

      vi.advanceTimersByTime(5000);
      expect(invoke).toHaveBeenCalledTimes(2);

      store.stopGitPolling();
      vi.advanceTimersByTime(10000);
      expect(invoke).toHaveBeenCalledTimes(2); // No further calls
    });
  });

  describe('6. Ctrl+P / Shift+Tab cycling', () => {
    it('cycleThinking advances and wraps from the last level to the first', async () => {
      const store = useProjectsStore();
      const project = createTestProject();
      store.projects.push(project);
      mockInvoke({ omp_list_thinking_levels: ['off', 'low', 'high'] });

      project.thinking = 'low';
      await store.cycleThinking('proj-1');
      expect(invoke).toHaveBeenCalledWith('omp_set_thinking', { id: 'proj-1', level: 'high' });
      expect(project.thinking).toBe('high');

      await store.cycleThinking('proj-1');
      expect(invoke).toHaveBeenCalledWith('omp_set_thinking', { id: 'proj-1', level: 'off' });
      expect(project.thinking).toBe('off');
    });

    it('cycles sorted, distinct profile names with Custom last', async () => {
      const store = useProjectsStore();
      const project = createTestProject();
      store.projects.push(project);
      const profile = { modelRoles: { default: 'a/b' } };
      vi.mocked(invoke).mockImplementation((cmd, args) => {
        if (cmd === 'model_profiles_get') {
          return Promise.resolve({ global: { Work: profile, Fast: profile }, local: { Fast: profile, Local: profile } });
        }
        if (cmd === 'omp_set_profile' && args && 'name' in args) {
          return Promise.resolve({ id: project.id, profile: args.name });
        }
        return Promise.resolve(undefined);
      });

      for (const name of ['Fast', 'Local', 'Work', null]) {
        await store.cycleProfile(project.id);
        expect(invoke).toHaveBeenCalledWith('omp_set_profile', { id: project.id, name });
        expect(project.profile).toBe(name ?? undefined);
      }
    });

    it('refreshes affected tabs and the picker revision after saving', async () => {
      const store = useProjectsStore();
      const changed = { ...createTestProject('changed'), profile: 'Work' };
      const untouched = { ...createTestProject('untouched'), profile: 'Work' };
      store.projects.push(changed, untouched);
      mockInvoke({ model_profile_save: { updated: [{ id: changed.id, profile: null }], warning: null } });

      await store.saveModelProfile(undefined, 'global', 'Work', null);

      expect(store.profileRevision).toBe(1);
      expect(changed.profile).toBeUndefined();
      expect(untouched.profile).toBe('Work');
      expect(invoke).toHaveBeenCalledWith('omp_get_state', { id: changed.id });
      expect(invoke).not.toHaveBeenCalledWith('omp_get_state', { id: untouched.id });
    });

    it('keeps the current selection after a failed switch and allows another try', async () => {
      const store = useProjectsStore();
      const project = createTestProject();
      store.projects.push(project);
      vi.mocked(invoke).mockRejectedValueOnce('model unavailable');

      await store.setProfile(project.id, 'Work');
      expect(project.profile).toBeUndefined();
      expect(project.activityLog.at(-1)?.text).toContain('model unavailable');

      mockInvoke({ omp_set_profile: { id: project.id, profile: 'Work' } });
      await store.setProfile(project.id, 'Work');
      expect(project.profile).toBe('Work');
    });
  });

  it('keeps the requested advisor effort separate from the live status and clears it when disabled', async () => {
    const store = useProjectsStore();
    const project = createTestProject();
    store.projects.push(project);

    const model = 'openrouter/vendor/model';
    await store.setAdvisor(project.id, model, 'high');
    expect(invoke).toHaveBeenLastCalledWith('omp_set_advisor', { id: project.id, model, level: 'high' });
    expect(project.advisorModelKey).toBe(model);
    expect(project.advisorThinking).toBe('high');

    store.handleFrame(project.id, {
      type: 'extension_ui_request', method: 'setStatus', statusKey: 'bide-advisor-thinking', statusText: 'medium',
    });
    expect(project.advisorThinkingLevel).toBe('medium');

    await store.setAdvisor(project.id, null);
    expect(invoke).toHaveBeenLastCalledWith('omp_set_advisor', { id: project.id, model: null, level: null });
    expect(project.advisorModelKey).toBeUndefined();
    expect(project.advisorThinking).toBeUndefined();
  });

  describe('7. Tool execution output', () => {
    it('captures partialResult and final result text on tool_call items', () => {
      const store = useProjectsStore();
      const project = createTestProject();
      store.projects.push(project);

      store.handleFrame('proj-1', {
        type: 'tool_execution_start',
        toolCallId: 'call-1',
        toolName: 'bash',
        args: { command: 'echo hi' },
        intent: 'Running echo',
      });
      store.handleFrame('proj-1', {
        type: 'tool_execution_update',
        toolCallId: 'call-1',
        partialResult: { content: [{ type: 'text', text: 'hi\n' }] },
      });
      expect(project.activityLog[0].output).toBe('hi\n');

      store.handleFrame('proj-1', {
        type: 'tool_execution_end',
        toolCallId: 'call-1',
        result: { content: [{ type: 'text', text: 'hi\ndone\n' }], details: { wallTimeMs: 5 } },
        isError: false,
      });
      expect(project.activityLog[0].phase).toBe('done');
      expect(project.activityLog[0].output).toBe('hi\ndone\n');
    });

    it('extracts background job progress from partialResult.details.jobs when content is empty', () => {
      const store = useProjectsStore();
      const project = createTestProject();
      store.projects.push(project);

      store.handleFrame('proj-1', {
        type: 'tool_execution_start',
        toolCallId: 'call-wait',
        toolName: 'wait',
        args: { ids: ['bg_18'] },
        intent: 'Waiting up to 5m for bg_18',
      });

      store.handleFrame('proj-1', {
        type: 'tool_execution_update',
        toolCallId: 'call-wait',
        partialResult: {
          content: [{ type: 'text', text: '' }],
          details: {
            op: 'wait',
            jobs: [
              {
                id: 'bg_18',
                type: 'bash',
                status: 'running',
                label: 'Schedule 13 backfill',
                durationMs: 394_000,
                stage: 'files copied',
                widget: {
                  state: 'live',
                  title: 'Schedule 13 backfill',
                  stage: 'files copied',
                  blocks: [
                    {
                      kind: 'stat',
                      items: [
                        { label: 'files copied', text: '4083' },
                        { label: 'files/s', text: '10' },
                        { label: 'days this year', text: '249' },
                      ],
                    },
                    { kind: 'text', text: '2023' },
                  ],
                },
              },
            ],
          },
        },
      });

      expect(project.activityLog[0].output).toBe(
        [
          'bg_18 (bash) · Schedule 13 backfill · files copied · running · 6m34s',
          '  files copied 4083 · files/s 10 · days this year 249',
          '  2023',
        ].join('\n')
      );
    });
  });
});
