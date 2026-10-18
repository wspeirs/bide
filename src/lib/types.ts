export type ProjectStatus = 'busy' | 'ready' | 'error' | 'starting';

export interface ActivityItem {
  id: string;
  type: 'thinking' | 'text' | 'tool_call' | 'tool_result' | 'command' | 'error';
  text?: string;
  tool?: string;
  intent?: string;
  args?: Record<string, unknown> | unknown;
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

export interface RoleInfo {
  name: string;
  selector: string;
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

export interface UiRequest {
  id: string;
  method: string; // 'select' | 'confirm' | 'input' | 'editor' | 'ask'
  title?: string;
  message?: string;
  options?: string[];
  defaultValue?: string;
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
  advisor?: boolean;
}

export interface ProjectState {
  id: string;
  dir: string;
  name: string;
  status: ProjectStatus;
  restartPending: boolean;
  orchestrate: boolean;
  role: string;
  advisor: boolean;
  lastPrompt: string | null;
  activityLog: ActivityItem[];
  isStreaming: boolean;
  model: string;
  contextPercent: number;
  pendingUiRequest: UiRequest | null;
  gitStatus: GitStatus | null;
  lastFetched: Date | null;
  errorStderr: string | null;
  debugLog: DebugEntry[];
  debugOpen: boolean;
}
