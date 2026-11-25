export type ProjectStatus = 'busy' | 'ready' | 'error' | 'starting';

export interface ActivityItem {
  id: string;
  type: 'thinking' | 'text' | 'tool_call' | 'tool_result' | 'command' | 'error';
  text?: string;
  tool?: string;
  intent?: string;
  args?: Record<string, unknown> | unknown;
  output?: string;
  time?: string;
  isError?: boolean;
  /** Tool call id from omp (links toolcall_end / tool_execution_* frames). */
  callId?: string;
  /** Position of the tool call in the assistant message, used before the id is known. */
  contentIndex?: number;
  /** generating: model is still writing the call; running: executing; done: finished. */
  phase?: 'generating' | 'running' | 'done';
  /** Characters of tool arguments streamed so far (generation progress). */
  argsChars?: number;
  /** ms epoch */
  startedAt?: number;
  endedAt?: number;
}

export interface GitFileStatus {
  path: string;
  status: string;
  orig_path?: string | null;
}

export interface GitStatus {
  branch?: string | null;
  detached: boolean;
  upstream?: string | null;
  ahead: number;
  behind: number;
  staged: GitFileStatus[];
  unstaged: GitFileStatus[];
  untracked: string[];
}

export type ModelProfile = {
  modelRoles: Record<string, string>;
  defaultThinkingLevel?: string;
  modelFallbacks?: Record<string, string>;
};

export type ModelProfileScope = 'global' | 'local';
export type ModelProfileScopes = Record<ModelProfileScope, Record<string, ModelProfile>>;

export interface ModelInfo {
  provider: string;
  id: string;
  name: string;
  thinkingLevels?: string[];
  kind?: string;
}

export interface ProfileModelOptions {
  models: ModelInfo[];
  roles: string[];
}

export interface UpdateReport {
  success: boolean;
  oldVersion: string;
  newVersion: string;
  versionChanged: boolean;
  output: string;
  error?: string | null;
  restartsTriggered: string[];
  restartsPending: string[];
}

export interface AskQuestion {
  id: string;
  question: string;
  header?: string;
  options: { label: string; description?: string }[];
  multi?: boolean;
  recommended?: number;
}

export interface UiRequest {
  id: string;
  method: string; // 'select' | 'confirm' | 'input' | 'editor' | 'ask'
  title?: string;
  message?: string;
  options?: string[];
  questions?: AskQuestion[];
  defaultValue?: string;
  multi?: { checked: string[]; other?: string; count: number; uncertain: boolean };
}

export interface DebugEntry {
  id: number;
  time: string;
  dir: 'in' | 'out';
  kind: string;
  json: string;
}

export interface ProjectInfo {
  id: string;
  dir: string;
  session_file?: string | null;
  orchestrate?: boolean;
  role?: string;
  profile?: string | null;
  advisorModelKey?: string | null;
  advisorThinking?: string | null;
}

export interface PromptEntry {
  id: string;
  prompt: string;
  /** Final assistant text; null while running or when the run produced none. */
  response: string | null;
}

export interface SubagentInfo {
  id: string;
  agent: string;
  description?: string;
  status: 'running' | 'completed' | 'failed' | 'aborted';
  activityLog: ActivityItem[];
  startedAt: number;
  endedAt?: number;
}

export interface ProjectState {
  id: string;
  dir: string;
  name: string;
  status: ProjectStatus;
  restartPending: boolean;
  orchestrate: boolean;
  role: string;
  profile?: string;
  advisorModelKey?: string;
  advisorThinking?: string;
  history: PromptEntry[];
  subagents: SubagentInfo[];
  caveman: string;
  ponytail: string;
  rtk: boolean;
  activityLog: ActivityItem[];
  isStreaming: boolean;
  model: string;
  advisorModel?: string;
  advisorThinkingLevel?: string;
  /** `provider/modelId` of the live model ('' until known). */
  modelKey: string;
  thinking: string;
  contextPercent: number | null;
  contextWindow?: number;
  pendingUiRequest: UiRequest | null;
  gitStatus: GitStatus | null;
  lastFetched: Date | null;
  errorStderr: string | null;
  debugLog: DebugEntry[];
  debugOpen: boolean;
}

export interface Worktree {
  path: string;
  head: string;
  branch: string | null;
  detached: boolean;
  bare: boolean;
  locked: boolean;
  prunable: boolean;
  isMain: boolean;
  isCurrent: boolean;
}

export interface Branches {
  local: string[];
  remote: string[];
}

export interface WorkspaceConfig {
  text: string;
  source: 'workspace' | 'global' | 'none';
  warning: string | null;
}
