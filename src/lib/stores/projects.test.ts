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
});
