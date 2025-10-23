# Projects and Threads State — Current Behavior

This document explains how the mobile app and the Rust bridge currently manage state for projects and threads (formerly “sessions”). It focuses on where state lives, how it flows on send/receive, and what the UI shows today. It also notes the small UI addition you want (showing the active project name under the thread title) and how we’ll wire it with Zustand.

## Overview
- Live stream: A lightweight in‑memory + AsyncStorage log that renders the current thread feed.
- History: A separate Zustand store that fetches recent threads and individual transcripts from the bridge over HTTP.
- Projects: A persisted projects list with a single active project; message sends include project context and optional working directory.
- Bridge: Spawns the Codex CLI, forwards stdout/stderr over WebSocket, and exposes `/history` and `/thread` for historical browsing.

## Live Thread Feed (in‑memory + persisted)
- Source: `expo/lib/log-store.ts`
  - Keeps a `Map<number, LogDetail>` in memory and a referentially stable `snapshot: LogDetail[]` used by `useSyncExternalStore`.
  - Persists the log array to `AsyncStorage` on a short debounce.
  - Key exports used by the Session screen: `subscribe`, `putLog`, `getAllLogs`, `loadLogs`.
- UI: The Session screen (`expo/app/session/index.tsx`) parses each incoming JSONL line and renders rows/cards accordingly.
  - It also shows a transient “Working: Ns” status under the most recent user message until the first assistant content of the turn arrives.

## History and Thread Transcripts (Zustand)
- Store: `expo/lib/threads-store.ts`
  - `history: HistoryItem[]` and `thread: Record<string, ThreadResponse>` live in a small Zustand store.
  - `loadHistory(wsUrl)`: `GET /history` via the bridge, caches to `AsyncStorage`, and updates `history`.
  - `loadThread(wsUrl, id, path?)`: `GET /thread?id=…` (and optional `path=`) to fetch the parsed transcript for a single thread file.
  - The store is read from the Drawer “History” list and the Thread detail view.
- Bridge: `crates/codex-bridge/src/history.rs`
  - `/history` scans `~/.codex/sessions` for the latest JSONL files in the new event format and returns a compact list (id, path, mtime, title, snippet).
  - `/thread` parses one JSONL file and returns `{ title, items, instructions? }` using the new Codex shapes (`response_item`, `event_msg`) and the legacy `item.*` fallback.

## Projects (Zustand + AsyncStorage) and Sending
- Persistence: `expo/lib/projects-store.ts`
  - Stores projects in `AsyncStorage` as a map keyed by `id`, plus an `activeId`.
  - API: `listProjects`, `getActiveProject`, `setActiveProject`, `upsertProject`, `removeProject`.
- Provider: `expo/providers/projects.tsx`
  - Hydrates from storage and exposes `projects`, `activeProject`, and actions (`setActive`, `save`, `del`).
  - `sendForProject(project, userText)` prepares and sends a payload via the WebSocket provider:
    - First line: a JSON config with approvals/sandbox and optionally:
      - `cd`: the project’s `workingDir` (if set).
      - `project`: `{ id, name, repo, agent_file }` for downstream context.
      - `resume`: pulled once from `ws.resumeNextId` when resuming a thread.
    - Body: a short human preface describing the environment and (if present) the active project, followed by the user’s message. The preface is controlled by `attachPreface` from the WS provider.

## WebSocket Provider and Preferences
- Source: `expo/providers/ws.tsx`
  - Manages connection, exposes `send`, and persists user preferences (`wsUrl`, `readOnly`, `networkEnabled`, `approvals`, `attachPreface`).
  - `resumeNextId`: a one‑shot resume id that `ProjectsProvider` consumes on the next send.

## Working Directory Semantics (Bridge + App)
- If an active project has `workingDir`, `sendForProject` includes `cd` in the first‑line JSON.
- Bridge behavior (`crates/codex-bridge/src/main.rs`):
  - Parses the first‑line JSON for `cd` and `resume` on each incoming payload.
  - Expands `~`/`~/` (`expand_home`) and respawns the Codex CLI with that working directory.
  - If no `cd` is provided, it uses a default detection heuristic (`detect_repo_root`) that prefers the nearest ancestor containing both `expo/` and `crates/`; otherwise it falls back to the current process directory.
  - The bridge captures `thread.started` events to remember the last `thread_id` for subsequent implicit resumes.

Today, if you are not actively in a project, no `cd` is sent by the app; the bridge picks the default repo root. You asked for an explicit default working dir when there is no active project — that would be an application‑level change (e.g., a WS preference like “Default working dir”) rather than relying on the bridge heuristic.

## What the UI Shows Today
- Drawer History shows a list of thread titles with timestamps (no inline project labels yet).
- Thread detail renders the parsed items; instructions are only shown in detail when present.
- Live Session view shows a transient “Working: Ns” counter directly beneath the user’s message until the first assistant content for that turn arrives.
- There is no separate visual indicator for the active project in the live feed or history — the project context only appears inside the human preface of the sent payload.

## Proposed Small Addition (Subtitle)
- Add a small, subdued subtitle under each thread title showing the active project name when that thread was created under a project.
- State wiring with Zustand:
  - On send, the `ProjectsProvider` knows the active project; when the next `thread.started` event arrives in the live feed, record a mapping `threadId → projectId` in a small store (e.g., extend `useThreads` with `threadProject: Record<string, ProjectId>` and `setThreadProject`).
  - When rendering History rows and Thread detail headers, consult this mapping and the current projects list to display the project name if available.
  - For historical files not created in this app session, we can later infer the project by heuristics (e.g., matching `workingDir` or repo), but the first increment is to track new threads going forward.

## Path Forward (no code changes yet)
1. Add a minimal `threadProject` slice to the threads store (Zustand) and wire it in the Session feed when `thread.started` is seen after a send.
2. Render the project name as a secondary line under the thread title in History items and in the thread detail header.
3. Add an optional WS preference “Default working dir (when no project)” and include it as `cd` only when no active project is selected; otherwise omit and let the bridge default.
4. Keep the current preface behavior, but do not surface project details in History rows — only the subtitle name.

If you’re happy with this plan, I’ll implement the subtitle and the small Zustand mapping next.

## New Thread Button Loads Same Thread First Press — Root Cause & Plan

Observed: Pressing the “New thread” button takes you to the thread screen but the next send continues the previous thread. Only after pressing it a second time does it clear out and start fresh.

Root cause
- UI action: The header button triggers `clearLog()` and navigates to `/thread?focus=1` (see `expo/components/app-header.tsx`). This clears the in‑memory feed only; it does not inform the bridge to start a new thread.
- Bridge default: The bridge remembers the last `thread_id` it saw (`last_thread_id`) and on subsequent prompts will implicitly resume that thread unless the payload explicitly requests otherwise. See `crates/codex-bridge/src/main.rs`:
  - `extract_resume_from_ws_payload` reads `resume` from the first JSON line.
  - If `resume` is missing, it falls back to `last_thread_id` and respawns Codex with `exec resume <id>`.
  - If `resume` is "new" or "none", it suppresses resume and starts a fresh thread.
- Sender behavior: `ProjectsProvider.sendForProject()` only includes a `resume` field when `ws.resumeNextId` is set; otherwise it omits `resume`, which triggers the bridge’s implicit resume of the last thread.

Why it seems to work on the second press
- The first “New thread” clears only the UI log. The next user send still omits `resume`, so the bridge resumes the prior thread. Pressing again and then sending after some flows where `resumeNextId` may have been set from a prior explicit action (e.g., continuing from history) can mask the issue, but fundamentally the first press never sent `resume: "new"` to the bridge.

Plan to fix
1. On “New thread” button press, set a one‑shot resume flag so the very next send includes `{"resume":"new"}`:
   - Call `ws.setResumeNextId('new')` before navigation or immediately after `clearLog()` in `AppHeader.onNewChat`.
   - Leave everything else intact; `ProjectsProvider.sendForProject()` will include `resume:"new"` once and then clear it.
2. Optional: also set an “ephemeral” UI state so the Session screen header/title reflects “New thread” immediately; this is already effectively the case because the feed is cleared.
3. Longer‑term guard: consider changing the bridge fallback to require explicit `resume` rather than implicit last‑thread resume (breaking change), or keep default behavior but ensure all New‑thread actions always set `resume:"new"`.

Relevant code pointers
- `expo/components/app-header.tsx` (`onNewChat` uses `clearLog()` + `router.push('/thread?focus=1')`).
- `expo/providers/ws.tsx` provides `resumeNextId` and `setResumeNextId`.
- `expo/providers/projects.tsx` composes the first‑line JSON, including `resume` when `resumeNextId` is set.
- `crates/codex-bridge/src/main.rs`:
  - `extract_resume_from_ws_payload()` parsing,
  - resume decision logic and `last_thread_id` fallback,
  - stdout handler capturing `thread.started` to update `last_thread_id`.
