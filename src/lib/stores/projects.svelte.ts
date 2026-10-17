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
        const project: ProjectState = {
          id: info.id,
          dir: info.dir,
          name: getBasename(info.dir),
          status: 'starting',
          restartPending: false,
          orchestrate: info.orchestrate ?? false,
          role: info.role ?? 'default',
          advisor: info.advisor ?? false,
          lastPrompt: null,
          thinking: '',
          activityLog: [],
          isStreaming: false,
          model: 'loading…',
          contextPercent: 0,
          pendingUiRequest: null,
          gitStatus: null,
          lastFetched: null,
          errorStderr: null,
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
        status: 'starting',
        restartPending: false,
        orchestrate: info.orchestrate ?? false,
        role: info.role ?? 'default',
        advisor: info.advisor ?? false,
        lastPrompt: null,
        thinking: '',
        activityLog: [],
        isStreaming: false,
        model: 'loading…',
        contextPercent: 0,
        pendingUiRequest: null,
        gitStatus: null,
        lastFetched: null,
        errorStderr: null,
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

  private handleFrame(id: string, frame: Record<string, unknown>) {
    const project = this.projects.find((p) => p.id === id);
    if (!project) return;

    const type = typeof frame.type === 'string' ? frame.type : '';

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

      case 'agent_end':
        project.isStreaming = false;
        this.refreshState(id);
        break;

      case 'message_update': {
        const ev = frame.assistantMessageEvent as Record<string, unknown> | undefined;
        if (ev && typeof ev.type === 'string') {
          if (ev.type === 'thinking_delta') {
            project.thinking += (ev.delta as string) || '';
          } else if (ev.type === 'text_delta') {
            const delta = (ev.delta as string) || '';
            const last = project.activityLog[project.activityLog.length - 1];
            if (last && last.type === 'text') {
              last.text = (last.text || '') + delta;
            } else {
              project.activityLog.push({
                id: crypto.randomUUID(),
                type: 'text',
                text: delta,
                time: new Date().toLocaleTimeString(),
              });
            }
          } else if (ev.type === 'toolcall_end' && ev.toolCall) {
            const tc = ev.toolCall as Record<string, unknown>;
            const toolId = (tc.id as string) || crypto.randomUUID();
            const existing = project.activityLog.find((a) => a.id === toolId);
            if (!existing) {
              project.activityLog.push({
                id: toolId,
                type: 'tool_call',
                tool: (tc.name as string) || 'tool',
                intent: (tc.intent as string) || undefined,
                args: tc.arguments,
                time: new Date().toLocaleTimeString(),
              });
            }
          }
        }
        break;
      }

      case 'tool_execution_start': {
        const toolCallId = (frame.toolCallId as string) || crypto.randomUUID();
        const existing = project.activityLog.find((a) => a.id === toolCallId);
        if (existing) {
          existing.intent = (frame.intent as string) || existing.intent;
          existing.args = frame.args ?? existing.args;
        } else {
          project.activityLog.push({
            id: toolCallId,
            type: 'tool_call',
            tool: (frame.toolName as string) || 'tool',
            intent: (frame.intent as string) || undefined,
            args: frame.args,
            time: new Date().toLocaleTimeString(),
          });
        }
        break;
      }

      case 'tool_execution_end': {
        const toolCallId = frame.toolCallId as string;
        const existing = project.activityLog.find((a) => a.id === toolCallId);
        if (existing) {
          existing.isError = Boolean(frame.isError);
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
        const noiseMethods = ['setStatus', 'setWidget', 'notify'];
        if (!noiseMethods.includes(method)) {
          project.pendingUiRequest = {
            id: (frame.id as string) || crypto.randomUUID(),
            method: method || 'select',
            title: (frame.title as string) || undefined,
            message: (frame.message as string) || undefined,
            options: Array.isArray(frame.options) ? (frame.options as string[]) : undefined,
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
      project.thinking = '';
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
    if (project) {
      project.pendingUiRequest = null;
    }

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

  private setGitError(id: string, err: unknown) {
    const project = this.projects.find((p) => p.id === id);
    if (!project) return;
    const msg = typeof err === 'string' ? err : (err as Error)?.message || String(err);
    project.errorStderr = msg;
  }
}

export const projectsStore = new ProjectsStore();
