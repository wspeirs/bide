import { ref, computed, watch } from 'vue';
import { defineStore } from 'pinia';
import { invoke, Channel } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type {
  ProjectState,
  ProjectInfo,
  GitStatus,
  RoleInfo,
  ActivityItem,
  AskQuestion,
  Worktree,
  Branches,
  WorkspaceConfig,
} from '../types';

export interface PaneSizes {
  left: number[];
  git: number;
}

function getBasename(dir: string): string {
  const parts = dir.replace(/\\/g, '/').split('/').filter(Boolean);
  return parts.pop() || dir;
}

const write = (k: string, v: string | null) => {
  try {
    v === null ? localStorage.removeItem(k) : localStorage.setItem(k, v);
  } catch {}
};

const read = (k: string) => {
  try {
    return localStorage.getItem(k);
  } catch {
    return null;
  }
};

function parsePanes(raw: string | null): PaneSizes | null {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw);
    if (parsed && Array.isArray(parsed.left) && typeof parsed.git === 'number') {
      return parsed as PaneSizes;
    }
    return null;
  } catch {
    return null;
  }
}

export const useProjectsStore = defineStore('projects', () => {
  const projects = ref<ProjectState[]>([]);
  const paneSizes = ref<PaneSizes>(parsePanes(read('bide_pane_sizes')) ?? { left: [20, 55, 25], git: 25 });
  const activeProjectId = ref<string | null>(read('bide_active_tab'));

  watch(paneSizes, (v) => write('bide_pane_sizes', JSON.stringify(v)), { deep: true, flush: 'sync' });
  watch(activeProjectId, (id) => write('bide_active_tab', id), { flush: 'sync' });

  let pollInterval: number | undefined = undefined;
  const subscribedChannels = new Map<string, Channel<Record<string, unknown>>>();
  let debugSeq = 0;

  const activeProject = computed<ProjectState | null>(() => {
    if (!activeProjectId.value) return null;
    return projects.value.find((p) => p.id === activeProjectId.value) ?? null;
  });

  function savePaneSizes(sizes: PaneSizes) {
    paneSizes.value = sizes;
  }

  function setActiveTab(id: string) {
    activeProjectId.value = id;
    refreshGitStatus(id);
    refreshState(id);
  }

  function startGitPolling() {
    clearInterval(pollInterval);
    pollInterval = window.setInterval(() => {
      if (activeProjectId.value) {
        refreshGitStatus(activeProjectId.value);
      }
    }, 5000);
  }

  function stopGitPolling() {
    clearInterval(pollInterval);
    pollInterval = undefined;
  }

  async function restoreProjects(): Promise<void> {
    try {
      const restored = await invoke<ProjectInfo[]>('projects_restore');
      if (!restored || restored.length === 0) return;

      for (const info of restored) {
        if (projects.value.some((p) => p.id === info.id)) continue;
        const project: ProjectState = {
          id: info.id,
          dir: info.dir,
          name: getBasename(info.dir),
          status: 'ready',
          restartPending: false,
          orchestrate: info.orchestrate ?? false,
          role: info.role ?? 'default',
          advisor: info.advisor ?? false,
          lastPrompt: null,
          activityLog: [],
          isStreaming: false,
          model: 'loading…',
          contextPercent: 0,
          pendingUiRequest: null,
          gitStatus: null,
          lastFetched: null,
          errorStderr: null,
          debugLog: [],
          debugOpen: false,
        };
        projects.value.push(project);
        setupSubscription(project.id);
      }

      if (!activeProjectId.value && projects.value.length > 0) {
        setActiveTab(projects.value[0].id);
      } else if (activeProjectId.value && !projects.value.some((p) => p.id === activeProjectId.value)) {
        if (projects.value.length > 0) {
          setActiveTab(projects.value[0].id);
        } else {
          activeProjectId.value = null;
        }
      }
    } catch (err) {
      console.error('Failed to restore projects:', err);
    }
  }

  async function openProject(chosenDir?: string): Promise<string | null> {
    let dir = chosenDir;
    if (!dir) {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Open Project Directory',
      });
      if (typeof selected === 'string') {
        dir = selected;
      } else {
        return null;
      }
    }

    try {
      const info = await invoke<ProjectInfo>('project_open', { dir });
      const existing = projects.value.find((p) => p.id === info.id);
      if (existing) {
        setActiveTab(existing.id);
        return existing.id;
      }

      const project: ProjectState = {
        id: info.id,
        dir: info.dir,
        name: getBasename(info.dir),
        status: 'ready',
        restartPending: false,
        orchestrate: info.orchestrate ?? false,
        role: info.role ?? 'default',
        advisor: info.advisor ?? false,
        lastPrompt: null,
        activityLog: [],
        isStreaming: false,
        model: 'loading…',
        contextPercent: 0,
        pendingUiRequest: null,
        gitStatus: null,
        lastFetched: null,
        errorStderr: null,
        debugLog: [],
        debugOpen: false,
      };

      projects.value.push(project);
      setActiveTab(project.id);
      setupSubscription(project.id);
      return project.id;
    } catch (err) {
      console.error('Failed to open project:', err);
      throw err;
    }
  }

  async function closeProject(id: string): Promise<void> {
    try {
      await invoke('project_close', { id });
    } catch (err) {
      console.error('Failed to close project backend:', err);
    }

    subscribedChannels.delete(id);
    const idx = projects.value.findIndex((p) => p.id === id);
    if (idx !== -1) {
      projects.value.splice(idx, 1);
    }

    if (activeProjectId.value === id) {
      if (projects.value.length > 0) {
        const nextIdx = Math.max(0, idx - 1);
        setActiveTab(projects.value[nextIdx].id);
      } else {
        activeProjectId.value = null;
      }
    }
  }

  async function closeActiveProject(): Promise<void> {
    if (activeProjectId.value) {
      await closeProject(activeProjectId.value);
    }
  }

  async function restartProject(id: string): Promise<void> {
    const project = projects.value.find((p) => p.id === id);
    if (!project) return;

    project.status = 'starting';
    project.isStreaming = false;
    project.errorStderr = null;

    try {
      await invoke<ProjectInfo>('project_restart', { id });
      project.status = 'ready';
      refreshState(id);
      refreshGitStatus(id);
    } catch (err) {
      project.status = 'error';
      const msg = typeof err === 'string' ? err : (err as Error)?.message || String(err);
      project.errorStderr = `Restart failed: ${msg}`;
      throw err;
    }
  }

  function setupSubscription(id: string) {
    if (subscribedChannels.has(id)) return;

    const channel = new Channel<Record<string, unknown>>((frame) => {
      handleFrame(id, frame);
    });
    subscribedChannels.set(id, channel);

    invoke('omp_subscribe', { id, channel }).catch((err) => {
      console.error(`Failed to subscribe channel for ${id}:`, err);
    });
  }

  function recordDebug(project: ProjectState, frame: Record<string, unknown>, type: string) {
    const out = type === 'rpc_out';
    const f = out ? (frame.frame as Record<string, unknown>) : frame;
    const t = typeof f?.type === 'string' ? f.type : '?';
    const d = new Date();
    project.debugLog.push({
      id: debugSeq++,
      time: `${d.toLocaleTimeString('en-GB')}.${String(d.getMilliseconds()).padStart(3, '0')}`,
      dir: out ? 'out' : 'in',
      kind: t === 'response' && typeof f.command === 'string' ? `response:${f.command}` : t,
      json: JSON.stringify(f),
    });
    if (project.debugLog.length > 500) project.debugLog.splice(0, project.debugLog.length - 500);
  }

  function handleFrame(id: string, frame: Record<string, unknown>) {
    const project = projects.value.find((p) => p.id === id);
    if (!project) return;

    const type = typeof frame.type === 'string' ? frame.type : '';
    recordDebug(project, frame, type);
    if (type === 'rpc_out') return;

    switch (type) {
      case 'ready':
        project.status = 'ready';
        refreshState(id);
        refreshGitStatus(id);
        break;

      case 'agent_start':
        project.status = 'busy';
        project.isStreaming = true;
        break;

      case 'agent_end': {
        project.isStreaming = false;
        const now = Date.now();
        for (const a of project.activityLog) {
          if (!a.endedAt && (a.type === 'thinking' || a.type === 'tool_call')) a.endedAt = now;
        }
        refreshState(id);
        break;
      }

      case 'message_update': {
        const ev = frame.assistantMessageEvent as Record<string, unknown> | undefined;
        if (!ev || typeof ev.type !== 'string') break;
        const now = Date.now();
        const log = project.activityLog;
        const add = (item: ActivityItem) => {
          log.push(item);
          return log[log.length - 1];
        };
        const last = () => log[log.length - 1];
        const ci = ev.contentIndex as number | undefined;
        const partialCall = () => {
          const content = (ev.partial as Record<string, unknown> | undefined)?.content as
            | Record<string, unknown>[]
            | undefined;
          const c = ci === undefined ? undefined : content?.[ci];
          return c && c.type === 'toolCall' ? c : undefined;
        };
        const generating = () =>
          [...log].reverse().find((a) => a.type === 'tool_call' && a.phase === 'generating' && a.contentIndex === ci);

        switch (ev.type) {
          case 'thinking_start':
            add({ id: crypto.randomUUID(), type: 'thinking', text: '', startedAt: now });
            break;

          case 'thinking_delta': {
            let t = last();
            if (!(t && t.type === 'thinking' && !t.endedAt)) {
              t = add({ id: crypto.randomUUID(), type: 'thinking', text: '', startedAt: now });
            }
            t.text = (t.text || '') + ((ev.delta as string) || '');
            break;
          }

          case 'thinking_end': {
            const t = last();
            if (t && t.type === 'thinking' && !t.endedAt) t.endedAt = now;
            break;
          }

          case 'text_delta': {
            const delta = (ev.delta as string) || '';
            const l = last();
            if (l && l.type === 'text') {
              l.text = (l.text || '') + delta;
            } else {
              add({ id: crypto.randomUUID(), type: 'text', text: delta, time: new Date().toLocaleTimeString() });
            }
            break;
          }

          case 'toolcall_start':
            add({
              id: crypto.randomUUID(),
              type: 'tool_call',
              tool: (partialCall()?.name as string) || 'tool',
              contentIndex: ci,
              phase: 'generating',
              argsChars: 0,
              startedAt: now,
            });
            break;

          case 'toolcall_delta': {
            const item = generating();
            if (!item) break;
            item.argsChars = (item.argsChars || 0) + ((ev.delta as string) || '').length;
            const pc = partialCall();
            if (pc) {
              item.tool = (pc.name as string) || item.tool;
              item.args = pc.arguments;
              const a = pc.arguments;
              if (a && typeof a === 'object' && 'i' in a && typeof a.i === 'string') item.intent = a.i;
            }
            break;
          }

          case 'toolcall_end': {
            const tc = ev.toolCall as Record<string, unknown> | undefined;
            if (!tc) break;
            const callId = (tc.id as string) || crypto.randomUUID();
            const item =
              generating() ??
              log.find((a) => a.callId === callId) ??
              add({ id: crypto.randomUUID(), type: 'tool_call', startedAt: now });
            item.callId = callId;
            item.tool = (tc.name as string) || item.tool || 'tool';
            item.args = tc.arguments;
            const a = tc.arguments;
            item.intent =
              (tc.intent as string) ||
              (a && typeof a === 'object' && 'i' in a && typeof a.i === 'string' ? a.i : undefined) ||
              item.intent;
            item.phase = 'running';
            break;
          }
        }
        break;
      }

      case 'tool_execution_start': {
        const callId = (frame.toolCallId as string) || crypto.randomUUID();
        const item =
          project.activityLog.find((a) => a.callId === callId) ??
          (() => {
            project.activityLog.push({ id: crypto.randomUUID(), type: 'tool_call', callId, startedAt: Date.now() });
            return project.activityLog[project.activityLog.length - 1];
          })();
        item.tool = (frame.toolName as string) || item.tool || 'tool';
        item.intent = (frame.intent as string) || item.intent;
        item.args = frame.args ?? item.args;
        item.phase = 'running';
        break;
      }

      case 'tool_execution_end': {
        const item = project.activityLog.find((a) => a.callId === (frame.toolCallId as string));
        if (item) {
          item.isError = Boolean(frame.isError);
          item.phase = 'done';
          item.endedAt = Date.now();
        }
        break;
      }

      case 'command_output': {
        const text = (frame.text as string) || '';
        project.activityLog.push({
          id: crypto.randomUUID(),
          type: 'command',
          text,
          time: new Date().toLocaleTimeString(),
        });
        break;
      }

      case 'extension_ui_request': {
        const method = (frame.method as string) || '';
        if (method === 'cancel') {
          if (project.pendingUiRequest?.id === frame.targetId) project.pendingUiRequest = null;
          break;
        }
        if (['select', 'confirm', 'input', 'editor', 'ask'].includes(method)) {
          project.pendingUiRequest = {
            id: (frame.id as string) || crypto.randomUUID(),
            method,
            title: (frame.title as string) || undefined,
            message: (frame.message as string) || undefined,
            options: Array.isArray(frame.options) ? (frame.options as string[]) : undefined,
            questions: Array.isArray(frame.questions) ? (frame.questions as AskQuestion[]) : undefined,
            defaultValue: (frame.defaultValue as string) || undefined,
          };
        }
        break;
      }

      case 'prompt_result': {
        const status = frame.status as string;
        if (status === 'error') {
          project.status = 'error';
          project.errorStderr = (frame.error as string) || 'Prompt execution error';
          project.isStreaming = false;
        } else if (frame.sessionSettled) {
          project.status = 'ready';
          project.isStreaming = false;
        }
        break;
      }

      case 'session_settled':
        project.status = 'ready';
        project.isStreaming = false;
        refreshGitStatus(id);
        break;

      case 'process_exited': {
        project.status = 'error';
        project.isStreaming = false;
        const lines = Array.isArray(frame.stderr) ? (frame.stderr as string[]) : [];
        const stderrText = lines.join('\n').trim();
        project.errorStderr = stderrText || 'omp process exited unexpectedly';
        break;
      }

      case 'model_changed':
      case 'thinking_level_changed':
        refreshState(id);
        break;
    }
  }

  async function sendPrompt(id: string, text: string, streamingBehavior?: string): Promise<void> {
    const project = projects.value.find((p) => p.id === id);
    if (!project) return;

    let promptText = text;
    if (project.orchestrate && !text.startsWith('orchestrate\n\n')) {
      promptText = `orchestrate\n\n${text}`;
    }

    project.lastPrompt = promptText;
    project.status = 'busy';
    project.isStreaming = true;
    project.errorStderr = null;

    try {
      await invoke('omp_prompt', {
        id,
        text: promptText,
        streamingBehavior: streamingBehavior ?? null,
      });
    } catch (err) {
      project.status = 'error';
      project.isStreaming = false;
      const msg = typeof err === 'string' ? err : (err as Error)?.message || String(err);
      project.errorStderr = msg;
      project.activityLog.push({
        id: crypto.randomUUID(),
        type: 'error',
        text: `Error sending prompt: ${msg}`,
        time: new Date().toLocaleTimeString(),
        isError: true,
      });
      throw err;
    }
  }

  async function abortPrompt(id: string): Promise<void> {
    try {
      await invoke('omp_abort', { id });
      const project = projects.value.find((p) => p.id === id);
      if (project) {
        project.isStreaming = false;
      }
    } catch (err) {
      console.error('Failed to abort prompt:', err);
    }
  }

  async function newSession(id: string): Promise<void> {
    const project = projects.value.find((p) => p.id === id);
    if (!project) return;

    try {
      await invoke('omp_new_session', { id });
      project.lastPrompt = null;
      project.activityLog = [];
      project.errorStderr = null;
      refreshState(id);
    } catch (err) {
      console.error('Failed to start new session:', err);
      throw err;
    }
  }

  function setOrchestrate(id: string, value: boolean) {
    const project = projects.value.find((p) => p.id === id);
    if (!project) return;
    project.orchestrate = value;
  }

  async function listRoles(id: string): Promise<RoleInfo[]> {
    try {
      return await invoke<RoleInfo[]>('omp_list_roles', { id });
    } catch (err) {
      console.error('Failed to list roles:', err);
      return [];
    }
  }

  async function setRole(id: string, role: string): Promise<void> {
    const project = projects.value.find((p) => p.id === id);
    if (!project) return;

    try {
      await invoke('omp_set_role', { id, role });
      project.role = role;
      refreshState(id);
    } catch (err) {
      console.error('Failed to set role:', err);
      throw err;
    }
  }

  async function toggleAdvisor(id: string): Promise<void> {
    const project = projects.value.find((p) => p.id === id);
    if (!project) return;

    const nextAdvisor = !project.advisor;
    const slash = nextAdvisor ? '/advisor on' : '/advisor off';

    try {
      await invoke('omp_command', { id, slash });
      project.advisor = nextAdvisor;
    } catch (err) {
      console.error('Failed to toggle advisor:', err);
      throw err;
    }
  }

  async function runReview(id: string): Promise<void> {
    try {
      await invoke('omp_command', { id, slash: '/review' });
    } catch (err) {
      console.error('Failed to run review:', err);
      throw err;
    }
  }

  function setGitError(id: string, err: unknown) {
    const project = projects.value.find((p) => p.id === id);
    if (!project) return;
    const msg = typeof err === 'string' ? err : (err as Error)?.message || String(err);
    project.errorStderr = msg;
  }

  async function respondUi(id: string, response: Record<string, unknown>): Promise<void> {
    const project = projects.value.find((p) => p.id === id);
    const reqId = project?.pendingUiRequest?.id;
    if (!project || !reqId) return;
    project.pendingUiRequest = null;
    const payload = { type: 'extension_ui_response', id: reqId, ...response };

    try {
      await invoke('omp_ui_respond', { id, response: payload });
    } catch (err) {
      console.error('Failed to respond to UI request:', err);
      throw err;
    }
  }

  async function refreshState(id: string): Promise<void> {
    const project = projects.value.find((p) => p.id === id);
    if (!project) return;

    try {
      const state = await invoke<Record<string, unknown>>('omp_get_state', { id });
      if (!state) return;

      if (state.model && typeof state.model === 'object') {
        const m = state.model as Record<string, unknown>;
        project.model = (m.name as string) || (m.id as string) || 'model';
      } else if (typeof state.model === 'string') {
        project.model = state.model;
      }

      if (state.contextUsage && typeof state.contextUsage === 'object') {
        const cu = state.contextUsage as Record<string, unknown>;
        if (typeof cu.percent === 'number') {
          project.contextPercent = Math.round(cu.percent * 10) / 10;
        }
      }

      if (typeof state.isStreaming === 'boolean') {
        project.isStreaming = state.isStreaming;
      }
      if (state.isSettled === true && project.status === 'busy') {
        project.status = 'ready';
      }
    } catch {
      // Ignore background state refresh errors
    }
  }

  async function refreshGitStatus(id: string): Promise<void> {
    const project = projects.value.find((p) => p.id === id);
    if (!project) return;

    try {
      const status = await invoke<GitStatus>('git_status', { id });
      project.gitStatus = status;
    } catch {
      project.gitStatus = null;
    }
  }

  async function gitStage(id: string, path: string): Promise<void> {
    try {
      await invoke('git_stage', { id, path });
      await refreshGitStatus(id);
    } catch (err) {
      setGitError(id, err);
      throw err;
    }
  }

  async function gitUnstage(id: string, path: string): Promise<void> {
    try {
      await invoke('git_unstage', { id, path });
      await refreshGitStatus(id);
    } catch (err) {
      setGitError(id, err);
      throw err;
    }
  }

  async function gitAddAll(id: string): Promise<void> {
    try {
      await invoke('git_add_all', { id });
      await refreshGitStatus(id);
    } catch (err) {
      setGitError(id, err);
      throw err;
    }
  }

  async function gitCommit(id: string, msg: string): Promise<void> {
    try {
      await invoke('git_commit', { id, msg });
      await refreshGitStatus(id);
    } catch (err) {
      setGitError(id, err);
      throw err;
    }
  }

  async function gitPush(id: string): Promise<void> {
    try {
      await invoke('git_push', { id });
      await refreshGitStatus(id);
    } catch (err) {
      setGitError(id, err);
      throw err;
    }
  }

  async function gitPull(id: string): Promise<void> {
    try {
      await invoke('git_pull', { id });
      await refreshGitStatus(id);
    } catch (err) {
      setGitError(id, err);
      throw err;
    }
  }

  async function gitFetch(id: string): Promise<void> {
    const project = projects.value.find((p) => p.id === id);
    try {
      await invoke('git_fetch', { id });
      if (project) {
        project.lastFetched = new Date();
      }
      await refreshGitStatus(id);
    } catch (err) {
      setGitError(id, err);
      throw err;
    }
  }

  async function getGitStatusText(id: string): Promise<string> {
    try {
      return await invoke<string>('git_status_text', { id });
    } catch (err) {
      const msg = typeof err === 'string' ? err : (err as Error)?.message || String(err);
      return msg;
    }
  }

  async function gitInit(id: string): Promise<void> {
    await invoke('git_init', { id });
    await refreshGitStatus(id);
  }

  async function listWorktrees(id: string): Promise<Worktree[]> {
    return await invoke<Worktree[]>('git_worktree_list', { id });
  }

  async function listBranches(id: string): Promise<Branches> {
    return await invoke<Branches>('git_branches', { id });
  }

  async function addWorktree(
    id: string,
    branch: string,
    newBranch: boolean,
    base: string | null,
  ): Promise<string> {
    return await invoke<string>('git_worktree_add', { id, branch, newBranch, base });
  }

  async function removeWorktree(id: string, path: string, force: boolean): Promise<void> {
    await invoke('git_worktree_remove', { id, path, force });
  }

  async function getWorkspaceConfig(id: string): Promise<WorkspaceConfig> {
    return await invoke<WorkspaceConfig>('workspace_config_get', { id });
  }

  async function setWorkspaceConfig(id: string, text: string): Promise<void> {
    await invoke('workspace_config_set', { id, text });
  }

  function listDir(id: string, rel: string): Promise<{ name: string; is_dir: boolean }[]> {
    return invoke('fs_list_dir', { id, rel });
  }

  return {
    projects,
    activeProjectId,
    paneSizes,
    activeProject,
    savePaneSizes,
    setActiveTab,
    startGitPolling,
    stopGitPolling,
    restoreProjects,
    openProject,
    closeProject,
    closeActiveProject,
    restartProject,
    handleFrame,
    sendPrompt,
    abortPrompt,
    newSession,
    setOrchestrate,
    listRoles,
    setRole,
    toggleAdvisor,
    runReview,
    respondUi,
    refreshState,
    refreshGitStatus,
    gitStage,
    gitUnstage,
    gitAddAll,
    gitCommit,
    gitPush,
    gitPull,
    gitFetch,
    getGitStatusText,
    gitInit,
    listWorktrees,
    listBranches,
    addWorktree,
    removeWorktree,
    getWorkspaceConfig,
    setWorkspaceConfig,
    listDir,
  };
});
