import { invoke, Channel } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type {
  ProjectState,
  ProjectStatus,
  ProjectInfo,
  GitStatus,
  RoleInfo,
  ActivityItem,
  UiRequest,
  AskQuestion,
  Worktree,
  Branches,
  WorkspaceConfig,
} from '../types';

function getBasename(dir: string): string {
  const parts = dir.replace(/\\/g, '/').split('/').filter(Boolean);
  return parts.pop() || dir;
}

class ProjectsStore {
  projects = $state<ProjectState[]>([]);
  activeProjectId = $state<string | null>(null);
  paneSizes = $state<{ left: number[]; git: number }>({ left: [20, 55, 25], git: 25 });

  private pollInterval: number | undefined = undefined;
  private subscribedChannels = new Map<string, Channel<Record<string, unknown>>>();

  get activeProject(): ProjectState | null {
    if (!this.activeProjectId) return null;
    return this.projects.find((p) => p.id === this.activeProjectId) ?? null;
  }

  constructor() {
    this.loadLocalPreferences();
    this.startGitPolling();
  }

  private loadLocalPreferences() {
    try {
      const savedPanes = localStorage.getItem('bide_pane_sizes');
      if (savedPanes) {
        this.paneSizes = JSON.parse(savedPanes);
      }
      const savedActive = localStorage.getItem('bide_active_tab');
      if (savedActive) {
        this.activeProjectId = savedActive;
      }
    } catch {
      // Ignore local storage errors
    }
  }

  savePaneSizes(sizes: { left: number[]; git: number }) {
    this.paneSizes = sizes;
    try {
      localStorage.setItem('bide_pane_sizes', JSON.stringify(sizes));
    } catch {
      // Ignore
    }
  }

  setActiveTab(id: string) {
    this.activeProjectId = id;
    try {
      localStorage.setItem('bide_active_tab', id);
    } catch {
      // Ignore
    }
    this.refreshGitStatus(id);
    this.refreshState(id);
  }

  private startGitPolling() {
    clearInterval(this.pollInterval);
    this.pollInterval = window.setInterval(() => {
      if (this.activeProjectId) {
        this.refreshGitStatus(this.activeProjectId);
      }
    }, 5000);
  }

  async restoreProjects(): Promise<void> {
    try {
      const restored = await invoke<ProjectInfo[]>('projects_restore');
      if (!restored || restored.length === 0) return;

      for (const info of restored) {
        if (this.projects.some((p) => p.id === info.id)) continue;
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
        this.projects.push(project);
        this.setupSubscription(project.id);
      }

      if (!this.activeProjectId && this.projects.length > 0) {
        this.setActiveTab(this.projects[0].id);
      } else if (this.activeProjectId && !this.projects.some((p) => p.id === this.activeProjectId)) {
        if (this.projects.length > 0) {
          this.setActiveTab(this.projects[0].id);
        } else {
          this.activeProjectId = null;
        }
      }
    } catch (err) {
      console.error('Failed to restore projects:', err);
    }
  }

  async openProject(chosenDir?: string): Promise<string | null> {
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
      const existing = this.projects.find((p) => p.id === info.id);
      if (existing) {
        this.setActiveTab(existing.id);
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

      this.projects.push(project);
      this.setActiveTab(project.id);
      this.setupSubscription(project.id);
      return project.id;
    } catch (err) {
      console.error('Failed to open project:', err);
      throw err;
    }
  }

  async closeProject(id: string): Promise<void> {
    try {
      await invoke('project_close', { id });
    } catch (err) {
      console.error('Failed to close project backend:', err);
    }

    this.subscribedChannels.delete(id);
    const idx = this.projects.findIndex((p) => p.id === id);
    if (idx !== -1) {
      this.projects.splice(idx, 1);
    }

    if (this.activeProjectId === id) {
      if (this.projects.length > 0) {
        const nextIdx = Math.max(0, idx - 1);
        this.setActiveTab(this.projects[nextIdx].id);
      } else {
        this.activeProjectId = null;
        try {
          localStorage.removeItem('bide_active_tab');
        } catch {
          // Ignore
        }
      }
    }
  }
  async closeActiveProject(): Promise<void> {
    if (this.activeProjectId) {
      await this.closeProject(this.activeProjectId);
    }
  }

  async restartProject(id: string): Promise<void> {
    const project = this.projects.find((p) => p.id === id);
    if (!project) return;

    project.status = 'starting';
    project.isStreaming = false;
    project.errorStderr = null;

    try {
      await invoke<ProjectInfo>('project_restart', { id });
      project.status = 'ready';
      this.refreshState(id);
      this.refreshGitStatus(id);
    } catch (err) {
      project.status = 'error';
      const msg = typeof err === 'string' ? err : (err as Error)?.message || String(err);
      project.errorStderr = `Restart failed: ${msg}`;
      throw err;
    }
  }


  private setupSubscription(id: string) {
    if (this.subscribedChannels.has(id)) return;

    const channel = new Channel<Record<string, unknown>>((frame) => {
      this.handleFrame(id, frame);
    });
    this.subscribedChannels.set(id, channel);

    invoke('omp_subscribe', { id, channel }).catch((err) => {
      console.error(`Failed to subscribe channel for ${id}:`, err);
    });
  }
  private debugSeq = 0;

  /** Keep a ring buffer of raw RPC frames (both directions) for the debug pane. */
  private recordDebug(project: ProjectState, frame: Record<string, unknown>, type: string) {
    const out = type === 'rpc_out';
    const f = out ? (frame.frame as Record<string, unknown>) : frame;
    const t = typeof f?.type === 'string' ? f.type : '?';
    const d = new Date();
    project.debugLog.push({
      id: this.debugSeq++,
      time: `${d.toLocaleTimeString('en-GB')}.${String(d.getMilliseconds()).padStart(3, '0')}`,
      dir: out ? 'out' : 'in',
      kind: t === 'response' && typeof f.command === 'string' ? `response:${f.command}` : t,
      json: JSON.stringify(f),
    });
    if (project.debugLog.length > 500) project.debugLog.splice(0, project.debugLog.length - 500);
  }


  private handleFrame(id: string, frame: Record<string, unknown>) {
    const project = this.projects.find((p) => p.id === id);
    if (!project) return;

    const type = typeof frame.type === 'string' ? frame.type : '';
    this.recordDebug(project, frame, type);
    if (type === 'rpc_out') return;


    switch (type) {
      case 'ready':
        project.status = 'ready';
        this.refreshState(id);
        this.refreshGitStatus(id);
        break;

      case 'agent_start':
        project.status = 'busy';
        project.isStreaming = true;
        break;

      case 'agent_end': {
        project.isStreaming = false;
        // Close anything left open (e.g. aborted turn) so nothing shows as running forever.
        const now = Date.now();
        for (const a of project.activityLog) {
          if (!a.endedAt && (a.type === 'thinking' || a.type === 'tool_call')) a.endedAt = now;
        }
        this.refreshState(id);
        break;
      }

      case 'message_update': {
        const ev = frame.assistantMessageEvent as Record<string, unknown> | undefined;
        if (!ev || typeof ev.type !== 'string') break;
        const now = Date.now();
        const log = project.activityLog;
        // Push, then return the reactive proxy so later mutations trigger updates.
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
        // The call still being written at this content index.
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
          // omp timed out / aborted the dialog; close it
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
        this.refreshGitStatus(id);
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
        this.refreshState(id);
        break;
    }
  }

  async sendPrompt(id: string, text: string, streamingBehavior?: string): Promise<void> {
    const project = this.projects.find((p) => p.id === id);
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

  async abortPrompt(id: string): Promise<void> {
    try {
      await invoke('omp_abort', { id });
      const project = this.projects.find((p) => p.id === id);
      if (project) {
        project.isStreaming = false;
      }
    } catch (err) {
      console.error('Failed to abort prompt:', err);
    }
  }

  async newSession(id: string): Promise<void> {
    const project = this.projects.find((p) => p.id === id);
    if (!project) return;

    try {
      await invoke('omp_new_session', { id });
      project.lastPrompt = null;
      project.activityLog = [];
      project.errorStderr = null;
      this.refreshState(id);
    } catch (err) {
      console.error('Failed to start new session:', err);
      throw err;
    }
  }

  setOrchestrate(id: string, value: boolean) {
    const project = this.projects.find((p) => p.id === id);
    if (!project) return;
    project.orchestrate = value;
  }

  async listRoles(id: string): Promise<RoleInfo[]> {
    try {
      return await invoke<RoleInfo[]>('omp_list_roles', { id });
    } catch (err) {
      console.error('Failed to list roles:', err);
      return [];
    }
  }

  async setRole(id: string, role: string): Promise<void> {
    const project = this.projects.find((p) => p.id === id);
    if (!project) return;

    try {
      await invoke('omp_set_role', { id, role });
      project.role = role;
      this.refreshState(id);
    } catch (err) {
      console.error('Failed to set role:', err);
      throw err;
    }
  }

  async toggleAdvisor(id: string): Promise<void> {
    const project = this.projects.find((p) => p.id === id);
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

  async runReview(id: string): Promise<void> {
    try {
      await invoke('omp_command', { id, slash: '/review' });
    } catch (err) {
      console.error('Failed to run review:', err);
      throw err;
    }
  }

  async respondUi(id: string, response: Record<string, unknown>): Promise<void> {
    const project = this.projects.find((p) => p.id === id);
    const reqId = project?.pendingUiRequest?.id;
    if (!project || !reqId) return;
    project.pendingUiRequest = null;
    response = { type: 'extension_ui_response', id: reqId, ...response };

    try {
      await invoke('omp_ui_respond', { id, response });
    } catch (err) {
      console.error('Failed to respond to UI request:', err);
      throw err;
    }
  }

  async refreshState(id: string): Promise<void> {
    const project = this.projects.find((p) => p.id === id);
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

  async refreshGitStatus(id: string): Promise<void> {
    const project = this.projects.find((p) => p.id === id);
    if (!project) return;

    try {
      const status = await invoke<GitStatus>('git_status', { id });
      project.gitStatus = status;
    } catch {
      project.gitStatus = null;
    }
  }

  async gitStage(id: string, path: string): Promise<void> {
    try {
      await invoke('git_stage', { id, path });
      await this.refreshGitStatus(id);
    } catch (err) {
      this.setGitError(id, err);
      throw err;
    }
  }

  async gitUnstage(id: string, path: string): Promise<void> {
    try {
      await invoke('git_unstage', { id, path });
      await this.refreshGitStatus(id);
    } catch (err) {
      this.setGitError(id, err);
      throw err;
    }
  }

  async gitAddAll(id: string): Promise<void> {
    try {
      await invoke('git_add_all', { id });
      await this.refreshGitStatus(id);
    } catch (err) {
      this.setGitError(id, err);
      throw err;
    }
  }

  async gitCommit(id: string, msg: string): Promise<void> {
    try {
      await invoke('git_commit', { id, msg });
      await this.refreshGitStatus(id);
    } catch (err) {
      this.setGitError(id, err);
      throw err;
    }
  }

  async gitPush(id: string): Promise<void> {
    try {
      await invoke('git_push', { id });
      await this.refreshGitStatus(id);
    } catch (err) {
      this.setGitError(id, err);
      throw err;
    }
  }

  async gitPull(id: string): Promise<void> {
    try {
      await invoke('git_pull', { id });
      await this.refreshGitStatus(id);
    } catch (err) {
      this.setGitError(id, err);
      throw err;
    }
  }

  async gitFetch(id: string): Promise<void> {
    const project = this.projects.find((p) => p.id === id);
    try {
      await invoke('git_fetch', { id });
      if (project) {
        project.lastFetched = new Date();
      }
      await this.refreshGitStatus(id);
    } catch (err) {
      this.setGitError(id, err);
      throw err;
    }
  }

  async getGitStatusText(id: string): Promise<string> {
    try {
      return await invoke<string>('git_status_text', { id });
    } catch (err) {
      const msg = typeof err === 'string' ? err : (err as Error)?.message || String(err);
      return msg;
    }
  }

  async gitInit(id: string): Promise<void> {
    await invoke('git_init', { id });
    await this.refreshGitStatus(id);
  }

  async listWorktrees(id: string): Promise<Worktree[]> {
    return await invoke<Worktree[]>('git_worktree_list', { id });
  }

  async listBranches(id: string): Promise<Branches> {
    return await invoke<Branches>('git_branches', { id });
  }

  async addWorktree(
    id: string,
    branch: string,
    newBranch: boolean,
    base: string | null,
  ): Promise<string> {
    return await invoke<string>('git_worktree_add', { id, branch, newBranch, base });
  }

  async removeWorktree(id: string, path: string, force: boolean): Promise<void> {
    await invoke('git_worktree_remove', { id, path, force });
  }

  async getWorkspaceConfig(id: string): Promise<WorkspaceConfig> {
    return await invoke<WorkspaceConfig>('workspace_config_get', { id });
  }

  async setWorkspaceConfig(id: string, text: string): Promise<void> {
    await invoke('workspace_config_set', { id, text });
  }

  listDir(id: string, rel: string): Promise<{ name: string; is_dir: boolean }[]> {
    return invoke('fs_list_dir', { id, rel });
  }

  private setGitError(id: string, err: unknown) {
    const project = this.projects.find((p) => p.id === id);
    if (!project) return;
    const msg = typeof err === 'string' ? err : (err as Error)?.message || String(err);
    project.errorStderr = msg;
  }
}

export const projectsStore = new ProjectsStore();
