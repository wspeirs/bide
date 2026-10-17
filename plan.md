# bide — Agent IDE on Tauri 2 + omp RPC

Desktop IDE shell for driving several `omp` agents at once. Each tab = one project = one
working directory = one long-lived `omp --mode rpc-ui --no-ui` child process. No editor,
no file tree (no left tray) for now: the IDE is the agent console plus basic git.

Reference: omp RPC protocol — <https://omp.sh/docs/rpc> (also `omp://rpc.md` locally).

---

## 1. Stack

| Layer | Choice | Why |
|---|---|---|
| Shell | Tauri 2 (Rust, `tokio`) | Requested. Native menu, dialogs, child processes. |
| omp transport | `omp-rpc` Rust crate from `can1357/oh-my-pi` (`sdk/rust/omp-rpc`, git dep) | Ships serde wire types, v2 negotiation, `rpc_chunk` reassembly, request correlation. Not on crates.io → pin git rev. Fallback: own ~200-line JSONL transport over `tokio::process` (see Phase 1 spike). |
| Git | `git` CLI via `tokio::process::Command` | Identical semantics to what the user types (`git add -u`, credential helpers, SSH agent, hooks). `git2` would need its own auth plumbing for push/pull. |
| UI | Svelte 5 + Vite + TypeScript (plain SPA, no SvelteKit) | Requested; SvelteKit routing/SSR is dead weight in a Tauri webview. |
| Styling | Tailwind CSS v4 + CSS variables for a dark "IntelliJ New UI / Darcula" palette | Utility classes for dense IDE layout; one theme file. |
| Split panes | `paneforge` | Resizable, persisted pane sizes for Svelte. |
| Widgets | `bits-ui` (headless: Select, Dialog, Switch, Tooltip, Toggle) | Accessible primitives; styled with Tailwind to look like IntelliJ. |
| Icons / fonts | `lucide-svelte`; Inter (UI), JetBrains Mono (prompt/thinking/paths) | IntelliJ look. |
| Tauri plugins | `tauri-plugin-dialog` (folder picker) | Native. Persistence is one small JSON file (§7), no store plugin needed. |

Skipped: markdown rendering of thinking (plain monospace `pre-wrap` first; add `marked` +
`dompurify` when wanted), syntax highlighting, LSP, file editor.

---

## 2. Window layout

```
┌──────────────────────────────────────────────────────────────────────────┐
│ File  Edit  About                                     (native menu bar)  │
├──────────────────────────────────────────────────────────────────────────┤
│ [⟳ bide ×] [● api-server ×] [⚠ infra ×] [+]                 (project tabs)│
├──────────────────────────────────────────────────────────────┬───────────┤
│ ~/src/bide   ⎇ main   claude-opus-4 · ctx 12%   ● streaming   │ GIT       │
├──────────────────────────────────────────────────────────────┤ ⎇ main ↑1↓0│
│ LAST PROMPT (read-only)                                       │ Changes   │
│ > refactor the git panel to …                                 │ M src/a.rs│
├──────────────────────────────────────────────────────────────┤  [+] [-]  │
│ THINKING / ACTIVITY (streaming, auto-scroll, read-only)       │ ?? new.ts │
│  thinking…                                                    │  [+]      │
│  ▸ tool: edit src/lib.rs                                      │           │
│  assistant text …                                             │ [Add all] │
│                                                               │ message:  │
│                                                               │ [_______] │
│                                                               │ [Commit]  │
├──────────────────────────────────────────────────────────────┤ [Status]  │
│ [⚡Orch] [Role: default ▾] [🛡Advisor] [Review] [⟲ New] [■ Stop]│ [Pull]    │
├──────────────────────────────────────────────────────────────┤ [Push]    │
│ PROMPT  (Enter = send, Shift+Enter = newline)                 │ [Fetch]   │
│                                                               │ fetched 3m│
└──────────────────────────────────────────────────────────────┴───────────┘
```

- Vertical split (paneforge) on the left column: last-prompt / thinking / controls+prompt.
- Right git panel is a fixed-but-resizable column; no left tray.
- Tab status icon, rendered **in front of the project name**:
  - **⟳ (spinning)** — active turn: from the moment a prompt/steer/review is admitted (or
    `agent_start` arrives) until `session_settled`. Covers streaming, tool calls, queued
    follow-ups, background jobs that will wake the agent, and a pending `UiRequestDialog`.
  - **green ●** — ready for more input: session settled (`isSettled` / `session_settled`).
  - **⚠ (red)** — omp exited/crashed or the last `prompt_result` was `status: "error"` (cleared
    on next prompt or restart). **○ (grey)** — process starting/restarting (before `ready`).
  - Restart-pending after Update (§4) is a separate small **⬆** after the name (tooltip
    "restarts after this turn"), so it never collides with ⟳.
  - Source of truth: per-tab `busy` flag in the projects store, set on admission/`agent_start`,
    cleared on `session_settled`; reconciled with `get_state.isSettled` on tab focus and after
    `ready` (covers missed frames and resumed sessions).
- Stop button is not in the requested control list but is the only way to `abort` a run; kept.

---

## 3. Architecture

### 3.1 Rust (`src-tauri/src/`)

| File | Responsibility |
|---|---|
| `main.rs` / `lib.rs` | Builder, plugins, native menu, `AppState { projects: Mutex<HashMap<ProjectId, Project>> }`, shutdown hook kills all children. |
| `omp.rs` | `OmpSession`: spawn, frame read loop, request/response correlation, forwarding events to the UI channel, shutdown. |
| `git.rs` | Thin wrappers over `git` CLI + porcelain v2 parser. |
| `projects.rs` | Open/close/restore tabs; `restart(id)`; pending-restart flag; hourly fetch timer. |
| `persist.rs` | Load/save `~/.config/bide/session.json` (§7). |
| `update.rs` | File → Update flow (§4). |
| `commands.rs` | `#[tauri::command]` surface (below). |

**Process per project**

```
omp --mode rpc-ui --no-ui [--resume <sessionFile>]      (cwd = project dir)
```

- `rpc-ui` so `/review`'s target menu and the `ask` tool reach the host as
  `extension_ui_request` (`select` / `editor` / `confirm` / `input` / `ask`); `--no-ui` keeps
  extensions headless. Plain `rpc` would suppress those dialogs.
- On `ready`: `negotiate_protocol {protocolVersion: 2}` (crate does this), then
  `set_event_filter { events: null, messageUpdates: "delta" }` (no full partial snapshots per
  token), `set_ask_dialog { enabled: true }`, `get_state`, `get_available_commands`; then
  re-apply per-tab session settings omp doesn't persist: role (`omp_set_role`) and advisor
  (`/advisor on` when enabled). Store `get_state.sessionFile` into `session.json` whenever it
  changes (spawn, `new_session`, resume).
- Stdout reader task: decode frame → route:
  - `response` with `id` → resolve pending oneshot.
  - everything else (events, `prompt_result`, `session_settled`, `queue_update`,
    `extension_ui_request`, `command_output`, `notice`, `extension_error`) → push to the
    project's `tauri::ipc::Channel<OmpFrame>` (per-tab channel, not global events, so tabs
    never see each other's traffic).
- Stderr → ring buffer (last 200 lines) shown in an error toast when the process dies.
- Shutdown: close stdin, keep draining stdout (the protocol requires it), wait 5 s, then kill.
- `restart(id)` — single path used by crash **Restart** button, app launch, and Update:
  shutdown (above) → spawn with `--resume <sessionFile>` → ready sequence. If the session file
  is gone or omp exits during resume (e.g. `Could not restore model`), spawn without `--resume`
  and toast "previous conversation could not be restored".

**Tauri commands**

```
project_open(dir) -> ProjectInfo           project_close(id)        projects_restore() -> [ProjectInfo]
omp_subscribe(id, channel: Channel<OmpFrame>)
omp_prompt(id, text, streaming_behavior?)  omp_abort(id)            omp_get_state(id)
omp_new_session(id)                        // clear context: new_session, persist new sessionFile
omp_command(id, slash: String)             // "/advisor on", "/review", …
app_update() -> UpdateReport               // File → Update (§4)
omp_ui_respond(id, response: Value)        // extension_ui_response passthrough
omp_list_roles(id) -> [RoleInfo]          // { name, selector } from project-effective modelRoles
omp_set_role(id, role) -> Model           // resolve selector → set_model (+ set_thinking_level)
git_status(id) -> GitStatus   git_stage(id, path)   git_unstage(id, path)   git_add_all(id)
git_commit(id, msg)   git_push(id)   git_pull(id)   git_fetch(id)   git_status_text(id) -> String
```

### 3.2 Frontend (`src/`)

| File | Responsibility |
|---|---|
| `App.svelte` | Tab bar + active `ProjectView`; listens for menu events. |
| `lib/stores/projects.svelte.ts` | Svelte 5 runes store: tabs, active id, per-project state (last prompt, activity log, streaming flag, model, ctx %, git status, pending UI request). Reduces `OmpFrame`s. |
| `lib/components/TabBar.svelte` | Tabs with status icon before the name (⟳ busy / green ● ready / ⚠ error / ○ starting), ⬆ restart-pending marker, close ×, `+` opens folder picker. |
| `lib/components/ProjectView.svelte` | paneforge layout of the pieces below. |
| `lib/components/CwdBar.svelte` | Path (click = copy), branch, model, context %, streaming indicator. |
| `lib/components/LastPrompt.svelte` | Read-only monospace view of last sent prompt. |
| `lib/components/ActivityPane.svelte` | Streams thinking deltas (dim italic), assistant text, one-line tool events; auto-scroll unless user scrolled up. |
| `lib/components/ControlStrip.svelte` | Orchestrate toggle, Role select (bits-ui `Select`), Advisor toggle, Review button, Stop. |
| `lib/components/PromptInput.svelte` | Textarea; Enter send, Shift+Enter newline, Ctrl+Enter steer while streaming, Esc = abort. |
| `lib/components/GitPanel.svelte` | Changed files with +/-, Add all, commit box, Status/Push/Pull/Fetch, last-fetch age. |
| `lib/components/StatusModal.svelte` | `git status` output in a modal (monospace). |
| `lib/components/UiRequestDialog.svelte` | Renders `extension_ui_request` (`select`, `confirm`, `input`, `editor`, `ask`), sends `extension_ui_response`; closes on `cancel` with matching `targetId`. |
| `styles/theme.css` | Darcula-ish CSS variables + Tailwind `@theme`. |

---

## 4. Feature → protocol mapping

| Feature | Implementation |
|---|---|
| Send prompt | `prompt { message }`. While streaming: `streamingBehavior: "followUp"` (Enter) or `"steer"` (Ctrl+Enter); omp rejects a bare prompt during streaming. Completion via `prompt_result` (same `id`); run done at `session_settled`. |
| Last prompt pane | Set locally on send (the text actually sent, including any `orchestrate` prefix). |
| Thinking pane | `message_update.assistantMessageEvent` `thinking_*` deltas; also `text_*` deltas and `tool_execution_start/end` one-liners so the user sees the answer. Group by `messageId`. Cap buffer (e.g. last 5k lines) to keep the DOM light. |
| **Orchestrate** toggle | omp magic keyword: when on, outgoing prompt = `"orchestrate\n\n" + text`. Keyword must be lowercase, standalone prose (`omp://magic-keywords.md`); needs `task` tool enabled. Per-tab, persisted. |
| **Agent** drop-down = model role | Options: chat roles from `omp config get modelRoles --json`, run with `cwd` = project dir so project config layers apply; re-read each time the drop-down opens (config changes live). Shown: `default`, `smol`, `slow`, `plan`, `task`, `commit`, `vision`, `advisor` and any custom role; hidden: model-kind roles (`image`, `web`, `speech`, `dictation`, `judge`) and background roles (`tiny`, `memory`). Item label = role, sub-label = selector. On select, Rust resolves the selector: follow `@role` aliases (cycle guard); split comma lists and try each in order; strip a trailing `:<level>` only when it is a valid `ThinkingLevel` (model ids like `llama3:8b` keep their colon); split `provider/modelId` on the first `/`; send `set_model { provider, modelId }` — first success wins; then `set_thinking_level` if a level was given. Failure (all candidates `Model not found`) → toast, selection reverts. Session-scoped: never writes `config.yml`. Displayed value: role whose resolved model equals `get_state.model`, else `custom` (e.g. after `model_changed` from fallback). Per-tab, persisted; re-applied after spawn/restart. |
| **Advisor** toggle | `prompt { message: "/advisor on" \| "/advisor off" }` (builtin, session-scoped, answers with `agentInvoked: false`). Tooltip shows `/advisor status` output (`command_output`). Requires `modelRoles.advisor` or a `slow` role; show the `no_model` text if returned. |
| **Review** button | `prompt { message: "/review" }`. Its target menu arrives as an `extension_ui_request` `select` → `UiRequestDialog`. Review output streams into the activity pane. |
| Stop | `abort` (Esc). |
| **⟲ New** (clear context) | `new_session` → fresh conversation in the same omp process; then `get_state` and persist the new `sessionFile`. Clears the tab's last-prompt and activity panes. Disabled while streaming (a session transition would abort the run). Role/advisor survive (same process). Old transcript stays on disk. |
| **Update** (File menu) | Rust runs `omp --version`, then `omp update` (stdin null, output captured into a modal with a spinner), then `omp --version` again. Non-zero exit → modal shows stderr, nothing restarts. Version unchanged → "already up to date", nothing restarts. Version changed → for each tab: if `get_state.isSettled` (no run, nothing queued, no background jobs) → `restart(id)` now; else set `restart_pending` (⬆ marker) and `restart(id)` on the next `session_settled` frame. Settled, not `prompt_result`, so background jobs (async `task`, auto-backgrounded `bash`) aren't killed mid-flight. Prompt input is disabled during a restart and re-enabled at the new `ready`. Menu item disabled while an update runs. |
| Header info | `get_state` after `agent_end`, `model_changed`, and on tab focus: `model`, `contextUsage.percent`, `isStreaming`. |
| Tab ↔ process | One `OmpSession` per tab; `cwd` set at spawn; never shared. |

## 5. Git panel mapping

| Control | Command |
|---|---|
| File list | `git status --porcelain=v2 --branch -z` → staged / unstaged / untracked, branch, ahead/behind. Refresh on: tab focus, after any git action, after `agent_end`, and a 5 s debounced poll while tab is visible. |
| `+` on a file | `git add -- <path>` |
| `-` on a file | `git restore --staged -- <path>` |
| Add all | `git add -u` (exactly as requested: tracked files only; untracked still need `+`). |
| Commit | `git commit -m <msg>`; disabled when nothing staged or message empty. Hook failures shown in a toast with stderr. |
| Status | `git status` (human output) in `StatusModal`. |
| Push | `git push` (no args; uses upstream). Error toast w/ stderr if no upstream. |
| Pull | `git pull` (respects user's `pull.rebase` config). |
| Fetch | `git fetch --prune`. Auto: on tab open and every hour per project (`tokio::time::interval`, cancelled on tab close). Panel shows "fetched Nm ago". |

All git commands: `GIT_TERMINAL_PROMPT=0` so a missing credential fails fast instead of hanging
the child; `cwd` = project dir; non-git directory → panel shows "Not a git repository" + `git init`
hint (no init button).

## 6. Menu (native Tauri 2 `Menu`)

- **File**: New Project Tab… (`Ctrl+T`, folder picker), Close Tab (`Ctrl+W`), separator, Update
  omp (§4), separator, Quit (`Ctrl+Q`).
- **Edit**: predefined Undo / Redo / Cut / Copy / Paste / Select All (needed for clipboard
  shortcuts in the webview on macOS).
- **About**: About bide (predefined about item with version).

Menu events → `app.emit("menu", id)` → `App.svelte`.

## 7. Persistence

`~/.config/bide/session.json` (fixed path as requested; created with the directory on first save):

```json
{
  "version": 1,
  "activeTab": "uuid",
  "paneSizes": { "left": [15, 60, 25], "git": 22 },
  "tabs": [
    { "id": "uuid", "dir": "/home/me/src/bide", "sessionFile": "/home/me/.omp/agent/sessions/…jsonl",
      "orchestrate": false, "role": "default", "advisor": false }
  ]
}
```

- Written atomically (`session.json.tmp` + `rename`) on: tab open/close/reorder, toggle/role
  change, `sessionFile` change, pane resize (debounced 500 ms).
- On launch: for each tab, `restart(id)` semantics — spawn with `--resume <sessionFile>`, so every
  conversation continues where it left off. Missing/corrupt file → start with no tabs and keep a
  `session.json.bak` copy of the bad file.
- `serde` + `std::fs`; home via `std::env::home_dir()`.

---

## 8. Phases

Each phase ends green: `cargo clippy -- -D warnings`, `cargo test`, `npm run check`
(svelte-check), `npm run build`, then a manual smoke in `cargo tauri dev`.

### Phase 0 — Scaffold
1. `npm create tauri-app` (Svelte + TS, Vite), Tauri 2; app id `dev.bide.app`.
2. Add Tailwind v4, paneforge, bits-ui, lucide-svelte, fonts; `theme.css`.
3. Add `tauri-plugin-dialog`; capabilities JSON.
4. Native menu (File/Edit/About) wired to a frontend `menu` event.
- **Done when**: window opens with menu, empty tab bar, `+` opens folder picker.

### Phase 1 — omp spike + session core (Rust)
1. Spike: add `omp-rpc` git dep; confirm it builds and exposes spawn-with-cwd, raw command
   send, and an event stream. If it doesn't fit Tauri's async model, write `omp.rs` transport
   directly (JSONL over `tokio::process`, v2 negotiate, `rpc_chunk` reassembly per spec).
2. Spike: run `omp --mode rpc-ui --no-ui` by hand; capture frames for `prompt`, `/advisor on`,
   `/review`, `set_model`, `set_thinking_level`, `new_session`. Confirm `--resume <path>` works in
   RPC mode (fallback: spawn fresh then `switch_session { sessionPath }`). Save captures as test
   fixtures in `src-tauri/tests/fixtures/`.
3. `OmpSession` spawn/route/shutdown; `restart(id)` with resume + fallback; `persist.rs`;
   Tauri commands `omp_*`.
4. Unit tests: frame routing (response vs event vs ui_request) from fixtures; role-selector
   resolution (alias chain, alias cycle, comma list, `:level` suffix vs `llama3:8b`); chunk reassembly
   if hand-rolled.
   Also `persist.rs` round-trip + corrupt-file handling.
- **Done when**: a test harness opens two sessions in two temp dirs and both answer prompts
  concurrently; killing the harness and re-running resumes both conversations.

### Phase 2 — Tabs + agent console UI  (parallel with Phase 3 after Phase 1 API is fixed)
1. Projects store + `TabBar` + `ProjectView` layout with paneforge.
2. `CwdBar`, `LastPrompt`, `ActivityPane` (thinking/text/tool deltas), `PromptInput`.
3. `ControlStrip`: Orchestrate, Role, Advisor, Review, New, Stop per §4.
4. `UiRequestDialog` for `select` / `confirm` / `input` / `editor` / `ask` / `cancel`.
5. Persist tabs + per-tab toggles to `session.json`; restore + resume on launch.
6. File → Update: `update.rs`, progress modal, ⬆ pending marker, deferred restart on `session_settled`.
7. Tab status icons (§2): ⟳ while a turn is active, green ● when ready, ⚠ / ○ for error / starting.
- **Done when**: two tabs on two dirs stream simultaneously; switching tabs keeps each
  stream intact; a busy tab shows ⟳ before its name and flips to a green ● at `session_settled`;
  `/review` menu appears as a dialog; New clears context; app relaunch resumes
  both conversations; Update with one tab busy restarts the idle tab immediately and the busy
  one only after it settles (simulate a version change with a stub `omp` wrapper on `PATH`).

### Phase 3 — Git panel
1. `git.rs` wrappers + porcelain v2 parser (+ unit tests on captured porcelain output:
   renames, untracked, staged+unstaged same file, detached HEAD, no upstream).
2. `GitPanel`, `StatusModal`, commit box, toasts for stderr.
3. Background fetch on open + hourly; refresh triggers per §5.
- **Done when**: stage/unstage/add-all/commit/push/pull/fetch work against a scratch repo
  with a local bare remote.

### Phase 4 — Polish
1. Keyboard shortcuts (§3.2, §6), focus management (prompt focused on tab switch).
2. Process-exit UX (restart), error toasts, empty states.
3. Theme pass for IntelliJ feel: 13px UI font, 1px `#2b2d30` borders, tool-window headers.
- **Done when**: full smoke: 3 tabs, concurrent prompts, advisor on in one, orchestrate in
  another, review in third, commit+push from git panel, quit cleanly (no orphan `omp`
  processes — check `pgrep omp`).

---

## 9. Risks

- **`omp-rpc` crate API unknown** until Phase 1 spike; fallback transport is specified.
- **Role switch is session-only** and bypasses omp's own role resolver (no RPC command takes a
  role alias), so selector parsing must mirror omp's: alias, comma-list, thinking suffix.
- **Advisor needs a model role**; without `modelRoles.advisor`/`slow` it reports `no_model`.
- **Orphaned children** on crash: also set `kill_on_drop(true)` on each `Child`.
- **`omp update` replaces the binary under running processes**; running sessions keep the old
  code until restarted, which is the point of the deferred restart. A tab that never settles
  (stuck run) stays pending until the user hits Stop.
- **Large frames**: v2 chunking required for big `agent_end`/tool output; activity buffer cap
  prevents webview slowdowns.
- **Concurrent omp processes** share `~/.omp` config/state; omp is designed for multiple
  sessions, but per-project session dirs keep transcripts separate.

## 10. Decisions

- App launch resumes every tab's last conversation (`--resume <sessionFile>` from
  `~/.config/bide/session.json`); **⟲ New** starts a fresh one.
- Review = omp's `/review` slash command.
- Agent drop-down = model role, session-scoped.
