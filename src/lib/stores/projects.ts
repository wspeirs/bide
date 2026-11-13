import { ref, computed, watch } from 'vue';
import { defineStore } from 'pinia';
import { invoke, Channel } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type {
  ProjectState,
  ProjectInfo,
  GitStatus,
  RoleInfo,
  ModelInfo,
  ActivityItem,
  AskQuestion,
  Worktree,
  Branches,
  WorkspaceConfig,
  PromptEntry,
  SubagentInfo,
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

function contentText(content: unknown): string {
  return typeof content === 'string'
    ? content
    : Array.isArray(content)
      ? content
          .filter((p) => p && typeof p === 'object' && (p as Record<string, unknown>).type === 'text')
          .map((p) => String((p as Record<string, unknown>).text ?? ''))
          .join('\n')
      : '';
}

function promptText(msg: Record<string, unknown> | undefined): string | null {
  const isUser = msg?.role === 'user' && msg?.attribution !== 'agent';
  const isCollab = msg?.role === 'custom' && msg?.customType === 'collab-prompt';
  if (!isUser && !isCollab) return null;
  return contentText(msg?.content).replace(/^orchestrate\n\n/, '');
}

export function finalResponse(log: ActivityItem[]): string {
  let lastTool = -1;
  for (let i = log.length - 1; i >= 0; i--) {
    if (log[i].type === 'tool_call') {
      lastTool = i;
      break;
    }
  }
  return log
    .slice(lastTool + 1)
    .filter((a) => a.type === 'text')
    .map((a) => a.text ?? '')
    .join('\n\n')
    .trim();
}

function closeOpenItems(log: ActivityItem[]) {
  const now = Date.now();
  for (const a of log) {
    if (!a.endedAt && (a.type === 'thinking' || a.type === 'tool_call')) a.endedAt = now;
  }
}

function formatDurationMs(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return '';
  if (ms < 1000) return `${Math.round(ms)}ms`;
  const totalSec = Math.floor(ms / 1000);
  const h = Math.floor(totalSec / 3600);
  const m = Math.floor((totalSec % 3600) / 60);
  const s = totalSec % 60;
  if (h > 0) return `${h}h${m}m${s}s`;
  if (m > 0) return `${m}m${s}s`;
  return `${s}s`;
}

function formatWidgetBlocks(blocks: unknown[], stage?: string): string[] {
  const lines: string[] = [];
  for (const b of blocks) {
    if (!b || typeof b !== 'object') continue;
    const block = b as Record<string, unknown>;
    if (block.kind === 'stat' && Array.isArray(block.items)) {
      const stats = block.items
        .map((it) => {
          if (!it || typeof it !== 'object') return '';
          const item = it as Record<string, unknown>;
          const label = typeof item.label === 'string' ? item.label.trim() : '';
          const text = typeof item.text === 'string' ? item.text.trim() : '';
          const trend =
            item.trend && typeof item.trend === 'object' && typeof (item.trend as Record<string, unknown>).text === 'string'
              ? ` ${(item.trend as Record<string, unknown>).text}`
              : '';
          return [label, text].filter(Boolean).join(' ') + trend;
        })
        .filter(Boolean);
      if (stats.length) lines.push(stats.join(' · '));
    } else if (block.kind === 'text' && typeof block.text === 'string') {
      const t = block.text.trim();
      if (t && t !== stage) lines.push(t);
    } else if (block.kind === 'bars' && Array.isArray(block.items)) {
      for (const it of block.items) {
        if (!it || typeof it !== 'object') continue;
        const item = it as Record<string, unknown>;
        const label = typeof item.label === 'string' ? item.label.trim() : '';
        const text = typeof item.text === 'string' ? item.text.trim() : '';
        const line = [label, text].filter(Boolean).join(': ');
        if (line) lines.push(line);
      }
    } else if (block.kind === 'grid' && Array.isArray(block.items)) {
      const items = block.items
        .map((it) => (it && typeof it === 'object' && typeof (it as Record<string, unknown>).label === 'string' ? (it as Record<string, unknown>).label : ''))
        .filter(Boolean);
      if (items.length) lines.push(items.join(' · '));
    }
  }
  return lines;
}

function formatJobProgress(raw: unknown): string | undefined {
  if (!raw || typeof raw !== 'object') return undefined;
  const j = raw as Record<string, unknown>;
  const id = typeof j.id === 'string' ? j.id : '';
  const type = typeof j.type === 'string' ? j.type : '';
  const status = typeof j.status === 'string' ? j.status : '';
  const widget = j.widget && typeof j.widget === 'object' ? (j.widget as Record<string, unknown>) : undefined;
  const title =
    (typeof widget?.title === 'string' && widget.title.trim()) ||
    (typeof j.intent === 'string' && j.intent.trim()) ||
    (typeof j.label === 'string' && j.label.split(/\r?\n/, 1)[0]?.trim()) ||
    '';
  const stage =
    (typeof widget?.stage === 'string' && widget.stage.trim()) ||
    (typeof j.stage === 'string' && j.stage.trim()) ||
    '';
  const pct = typeof j.completionPercent === 'number' && Number.isFinite(j.completionPercent) ? `${j.completionPercent}%` : '';
  const dur = typeof j.durationMs === 'number' ? formatDurationMs(j.durationMs) : '';
  const exit = typeof j.exitCode === 'number' && j.exitCode !== 0 ? `exit ${j.exitCode}` : '';

  const headParts = [
    id && type ? `${id} (${type})` : id || type,
    title && title !== id ? title : '',
    stage && stage !== title ? stage : '',
    status,
    pct,
    exit,
    dur,
  ].filter(Boolean);

  const lines: string[] = [];
  if (headParts.length) lines.push(headParts.join(' · '));

  if (widget && Array.isArray(widget.blocks)) {
    for (const line of formatWidgetBlocks(widget.blocks, stage)) {
      lines.push(`  ${line}`);
    }
  }

  const body =
    (typeof j.errorText === 'string' && j.errorText.trim()) ||
    (typeof j.resultText === 'string' && j.resultText.trim()) ||
    '';
  if (body) {
    for (const line of body.split(/\r?\n/)) {
      if (line.trim()) lines.push(`  ${line}`);
    }
  }

  return lines.length ? lines.join('\n') : undefined;
}

function extractDetailsOutput(details: unknown): string | undefined {
  if (!details || typeof details !== 'object') return undefined;
  const d = details as Record<string, unknown>;
  const sections: string[] = [];

  if (Array.isArray(d.jobs)) {
    const jobLines = d.jobs.map(formatJobProgress).filter((s): s is string => Boolean(s));
    if (jobLines.length) sections.push(jobLines.join('\n'));
  }

  if (Array.isArray(d.agents)) {
    const agentLines = d.agents
      .map((a) => {
        if (!a || typeof a !== 'object') return undefined;
        const ag = a as Record<string, unknown>;
        const parts = [
          typeof ag.id === 'string' ? ag.id : '',
          typeof ag.agent === 'string' ? ag.agent : '',
          typeof ag.activity === 'string' ? ag.activity : '',
          typeof ag.status === 'string' ? ag.status : '',
        ].filter(Boolean);
        return parts.length ? parts.join(' · ') : undefined;
      })
      .filter((s): s is string => Boolean(s));
    if (agentLines.length) sections.push(agentLines.join('\n'));
  }

  return sections.length ? sections.join('\n') : undefined;
}

function extractToolOutput(res: unknown): string | undefined {
  if (res == null) return undefined;
  if (typeof res === 'string') return res;
  if (typeof res === 'object' && 'content' in res && Array.isArray(res.content)) {
    const parts = res.content
      .map((c) => {
        if (c && typeof c === 'object' && 'type' in c && c.type === 'text' && 'text' in c && typeof c.text === 'string') {
          return c.text;
        }
        return JSON.stringify(c);
      })
      .filter(Boolean);
    if (parts.length) return parts.join('\n');
    if ('details' in res) {
      const detailsOut = extractDetailsOutput((res as Record<string, unknown>).details);
      if (detailsOut !== undefined) return detailsOut;
    }
    return undefined;
  }
  try {
    return JSON.stringify(res, null, 2);
  } catch {
    return String(res);
  }
}

function applyAgentEvent(log: ActivityItem[], frame: Record<string, unknown>) {
  switch (frame.type) {
    case 'message_update': {
      const ev = frame.assistantMessageEvent as Record<string, unknown> | undefined;
      if (!ev || typeof ev.type !== 'string') break;
      const now = Date.now();
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
        log.find((a) => a.callId === callId) ??
        (() => {
          log.push({ id: crypto.randomUUID(), type: 'tool_call', callId, startedAt: Date.now() });
          return log[log.length - 1];
        })();
      item.tool = (frame.toolName as string) || item.tool || 'tool';
      item.intent = (frame.intent as string) || item.intent;
      item.args = frame.args ?? item.args;
      item.phase = 'running';
      break;
    }

    case 'tool_execution_update': {
      const item = log.find((a) => a.callId === (frame.toolCallId as string));
      if (item) {
        const out = extractToolOutput(frame.partialResult);
        if (out !== undefined) item.output = out;
      }
      break;
    }

    case 'tool_execution_end': {
      const item = log.find((a) => a.callId === (frame.toolCallId as string));
      if (item) {
        item.isError = Boolean(frame.isError);
        item.phase = 'done';
        item.endedAt = Date.now();
        const out = extractToolOutput(frame.result);
        if (out !== undefined) item.output = out;
      }
      break;
    }
  }
}

function applyStatus(project: ProjectState, key: unknown, text: unknown) {
  const plain = String(text ?? '').replace(/\x1b\[[0-9;]*m/g, '').trim();
  if (key === 'caveman') {
    project.caveman = plain.match(/caveman:\s*([a-z]+)/i)?.[1].toLowerCase() ?? 'off';
  } else if (key === 'rtk') {
    project.rtk = /rtk:\s*on/i.test(plain);
  } else if (key === 'ponytail') {
    project.ponytail = plain.match(/ponytail:.*?([a-z]+)\s*$/i)?.[1].toLowerCase() ?? 'off';
  }
}

const SELECTED_PREFIX = /^\((\d+) selected\) /;
const DONE_OPTION = /Done selecting$/;
const OTHER_OPTION = 'Other (type your own)';

function askPrechecked(log: ActivityItem[], options: string[]): string[] {
  let html: string | null = null;
  for (let i = log.length - 1; i >= 0; i--) {
    const a = log[i];
    if (a.type === 'tool_call' && a.tool === 'ask' && a.phase !== 'done') {
      const args = a.args as Record<string, unknown> | undefined;
      if (typeof args?.html === 'string') {
        html = args.html;
        break;
      }
    }
  }
  if (!html) return [];
  const doc = new DOMParser().parseFromString(html, 'text/html');
  const result: string[] = [];
  for (const input of doc.querySelectorAll('input[type=checkbox][checked]')) {
    const label = (input.closest('label')?.textContent ?? '').replace(/\s+/g, ' ').trim();
    if (options.includes(label)) result.push(label);
  }
  return result;
}

export const useProjectsStore = defineStore('projects', () => {
  const projects = ref<ProjectState[]>([]);
  const paneSizes = ref<PaneSizes>(parsePanes(read('bide_pane_sizes')) ?? { left: [20, 55, 25], git: 25 });
  const activeProjectId = ref<string | null>(read('bide_active_tab'));

  watch(paneSizes, (v) => write('bide_pane_sizes', JSON.stringify(v)), { deep: true, flush: 'sync' });
  watch(activeProjectId, (id) => write('bide_active_tab', id), { flush: 'sync' });

  let pollInterval: number | undefined = undefined;
  const subscribedChannels = new Map<string, Channel<Record<string, unknown>>>();
  const runStarting = new Set<string>();
  interface MultiLoop {
    base: string;
    checked: Set<string>;
    other?: string;
    lastPick?: string;
  }
  const multiLoops = new Map<string, MultiLoop>();
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
          history: [],
          subagents: [],
          caveman: 'off',
          ponytail: 'off',
          rtk: false,
          activityLog: [],
          isStreaming: false,
          model: 'loading…',
          modelKey: '',
          thinking: '',
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
        loadHistory(project.id);
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
        history: [],
        subagents: [],
        caveman: 'off',
        ponytail: 'off',
        rtk: false,
        activityLog: [],
        isStreaming: false,
        model: 'loading…',
        modelKey: '',
        thinking: '',
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
      loadHistory(project.id);
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

  function upsertSubagent(project: ProjectState, p: Record<string, unknown>): SubagentInfo {
    const sid = String(p.id);
    const existing = project.subagents.find((s) => s.id === sid);
    if (existing) {
      if (typeof p.agent === 'string' && p.agent) existing.agent = p.agent;
      if (typeof p.description === 'string') existing.description = p.description;
      return existing;
    }
    project.subagents.push({
      id: sid,
      agent: (p.agent as string) || 'agent',
      description: p.description as string | undefined,
      status: 'running',
      activityLog: [],
      startedAt: Date.now(),
    });
    return project.subagents[project.subagents.length - 1];
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
        runStarting.add(id);
        break;

      case 'agent_end': {
        project.isStreaming = false;
        closeOpenItems(project.activityLog);
        runStarting.delete(id);
        if (project.history.length) {
          project.history[project.history.length - 1].response = finalResponse(project.activityLog) || null;
        }
        refreshState(id);
        break;
      }

      case 'message_start': {
        const text = promptText(frame.message as Record<string, unknown> | undefined);
        if (text === null) break;
        if (runStarting.delete(id) || project.history.length === 0) {
          project.activityLog = [];
          project.subagents = [];
          project.history.push({ id: crypto.randomUUID(), prompt: text, response: null });
        } else {
          project.history[project.history.length - 1].prompt += '\n\n' + text;
        }
        break;
      }

      case 'message_update':
      case 'tool_execution_start':
      case 'tool_execution_update':
      case 'tool_execution_end':
        applyAgentEvent(project.activityLog, frame);
        break;

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
        if (method === 'setStatus') {
          applyStatus(project, frame.statusKey, frame.statusText);
          break;
        }
        if (method === 'cancel') {
          if (project.pendingUiRequest?.id === frame.targetId) project.pendingUiRequest = null;
          break;
        }
        if (['select', 'confirm', 'input', 'editor', 'ask'].includes(method)) {
          if (method !== 'select' && method !== 'input') {
            multiLoops.delete(id);
          }
          if (method === 'select') {
            const raw = String(frame.title ?? '');
            const m = SELECTED_PREFIX.exec(raw);
            const base = m ? raw.slice(m[0].length) : raw;
            const options = (frame.options as string[]) ?? [];
            let st = multiLoops.get(id);
            if (!st || st.base !== base) {
              st = { base, checked: new Set(askPrechecked(project.activityLog, options)) };
            } else {
              if (st.lastPick === OTHER_OPTION) {
                st.other = undefined;
              } else if (st.lastPick && !DONE_OPTION.test(st.lastPick)) {
                if (st.checked.has(st.lastPick)) st.checked.delete(st.lastPick);
                else st.checked.add(st.lastPick);
              }
              st.lastPick = undefined;
            }
            multiLoops.set(id, st);
            const isMulti = !!m || options.some((o) => DONE_OPTION.test(o));
            const count = m ? Number(m[1]) : 0;
            project.pendingUiRequest = {
              id: (frame.id as string) || crypto.randomUUID(),
              method,
              title: base || undefined,
              message: (frame.message as string) || undefined,
              options: Array.isArray(frame.options) ? (frame.options as string[]) : undefined,
              questions: Array.isArray(frame.questions) ? (frame.questions as AskQuestion[]) : undefined,
              defaultValue: (frame.defaultValue as string) || undefined,
              ...(isMulti
                ? {
                    multi: {
                      checked: [...st.checked],
                      other: st.other,
                      count,
                      uncertain: count !== st.checked.size + (st.other ? 1 : 0),
                    },
                  }
                : {}),
            };
          } else {
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
        }
        break;
      }

      case 'subagent_lifecycle': {
        const p = (frame.payload ?? {}) as Record<string, unknown>;
        const sa = upsertSubagent(project, p);
        const status = String(p.status ?? '');
        if (status === 'started') {
          sa.status = 'running';
        } else {
          sa.status = status as SubagentInfo['status'];
          sa.endedAt = Date.now();
          closeOpenItems(sa.activityLog);
        }
        break;
      }

      case 'subagent_event': {
        const p = (frame.payload ?? {}) as { id: string; event: Record<string, unknown> };
        const sa = upsertSubagent(project, { id: p.id });
        if (p.event) {
          applyAgentEvent(sa.activityLog, p.event);
          if (p.event.type === 'agent_end') closeOpenItems(sa.activityLog);
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
      project.history = [];
      project.subagents = [];
      multiLoops.delete(id);
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

  async function listModels(id: string): Promise<ModelInfo[]> {
    try {
      return await invoke<ModelInfo[]>('omp_list_models', { id });
    } catch (err) {
      console.error('Failed to list models:', err);
      return [];
    }
  }

  async function listThinkingLevels(id: string): Promise<string[]> {
    try {
      return await invoke<string[]>('omp_list_thinking_levels', { id });
    } catch (err) {
      console.error('Failed to list thinking levels:', err);
      return [];
    }
  }

  /** `model` is `provider/modelId`; switches the tab to the custom role. */
  async function setModel(id: string, model: string): Promise<void> {
    const project = projects.value.find((p) => p.id === id);
    if (!project) return;
    try {
      await invoke('omp_set_model', { id, model });
      project.role = 'custom';
      refreshState(id);
    } catch (err) {
      console.error('Failed to set model:', err);
      throw err;
    }
  }

  async function setThinking(id: string, level: string): Promise<void> {
    const project = projects.value.find((p) => p.id === id);
    if (!project) return;
    try {
      await invoke('omp_set_thinking', { id, level });
      project.thinking = level;
      refreshState(id);
    } catch (err) {
      console.error('Failed to set thinking level:', err);
      throw err;
    }
  }

  /** Ctrl+P: next role in list order (custom → first role). */
  async function cycleRole(id: string): Promise<void> {
    const project = projects.value.find((p) => p.id === id);
    const roles = await listRoles(id);
    if (!project || roles.length === 0) return;
    const idx = roles.findIndex((r) => r.name === project.role);
    await setRole(id, roles[(idx + 1) % roles.length].name);
  }

  /** Shift+Tab: next thinking level the live model supports, wrapping to the first. */
  async function cycleThinking(id: string): Promise<void> {
    const project = projects.value.find((p) => p.id === id);
    const levels = await listThinkingLevels(id);
    if (!project || levels.length === 0) return;
    const idx = levels.indexOf(project.thinking);
    await setThinking(id, levels[(idx + 1) % levels.length]);
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

  async function collabLink(id: string): Promise<string> {
    return await invoke<string>('omp_collab_link', { id });
  }

  function setGitError(id: string, err: unknown) {
    const project = projects.value.find((p) => p.id === id);
    if (!project) return;
    const msg = typeof err === 'string' ? err : (err as Error)?.message || String(err);
    project.errorStderr = msg;
  }

  async function respondUi(id: string, response: Record<string, unknown>): Promise<void> {
    const project = projects.value.find((p) => p.id === id);
    const req = project?.pendingUiRequest;
    const reqId = req?.id;
    if (!project || !reqId) return;
    const st = multiLoops.get(id);
    if (response.cancelled) {
      multiLoops.delete(id);
    } else if (req?.method === 'select' && typeof response.value === 'string') {
      if (st) {
        st.lastPick = response.value;
        if (DONE_OPTION.test(response.value)) multiLoops.delete(id);
      }
    } else if (req?.method === 'input' && st?.lastPick === OTHER_OPTION) {
      st.other = String(response.value ?? '').trim() || undefined;
      st.lastPick = undefined;
    }
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
        project.modelKey = m.provider && m.id ? `${m.provider}/${m.id}` : '';
      } else if (typeof state.model === 'string') {
        project.model = state.model;
      }
      if (typeof state.thinkingLevel === 'string') {
        project.thinking = state.thinkingLevel;
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
      const st = await invoke<Record<string, string>>('omp_ext_statuses', { id });
      for (const [k, v] of Object.entries(st ?? {})) applyStatus(project, k, v);
    } catch {
      // Ignore background state refresh errors
    }
  }

  async function loadHistory(id: string): Promise<void> {
    try {
      const res = await invoke<unknown>('omp_get_messages', { id });
      const msgs = Array.isArray(res) ? res : (res as Record<string, unknown> | null)?.messages;
      if (!Array.isArray(msgs)) return;
      const built: PromptEntry[] = [];
      for (const raw of msgs) {
        const m = raw as Record<string, unknown> | undefined;
        const prompt = promptText(m);
        if (prompt !== null) {
          built.push({ id: crypto.randomUUID(), prompt, response: null });
        } else if (m?.role === 'assistant' && built.length > 0) {
          const txt = contentText(m.content).trim();
          if (txt) built[built.length - 1].response = txt;
        }
      }
      const project = projects.value.find((p) => p.id === id);
      if (project) project.history = built;
    } catch {
      // Ignore background history load errors
    }
  }

  async function setAddonMode(id: string, name: 'caveman' | 'ponytail' | 'rtk', value: string): Promise<void> {
    const project = projects.value.find((p) => p.id === id);
    if (!project) return;
    try {
      await invoke('omp_command', { id, slash: `/${name} ${value}` });
      if (name === 'rtk') project.rtk = value === 'on';
      else project[name] = value;
    } catch (err) {
      console.error(`Failed to set ${name}:`, err);
      throw err;
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

  async function getWorkspaceEnv(id: string): Promise<Record<string, string>> {
    return await invoke<Record<string, string>>('workspace_env_get', { id });
  }

  /** null = leave that part unchanged; the repo's sessions restart once. */
  async function saveWorkspace(id: string, text: string | null, env: Record<string, string> | null): Promise<void> {
    await invoke('workspace_save', { id, text, env });
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
    listModels,
    listThinkingLevels,
    setModel,
    setThinking,
    cycleRole,
    cycleThinking,
    toggleAdvisor,
    runReview,
    collabLink,
    respondUi,
    refreshState,
    loadHistory,
    setAddonMode,
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
    getWorkspaceEnv,
    saveWorkspace,
    listDir,
  };
});
