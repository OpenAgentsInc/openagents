# Long‑Running Master Tasks (Brainstorm)

This app can wrap Codex CLI to run long jobs that span hundreds of calls and many hours. Below is a concrete, repo‑aware plan to support a master task that splits into chunks, runs continuously, and exposes full state/controls in the UI.

## Relevant Code (Today)
- Event bridge and proto runner: `src-tauri/src/lib.rs`
  - Stream reader → UI events: `handle_proto_event` and reader loop (session + deltas) (src-tauri/src/lib.rs:1300)
  - Session start/refresh (new chat): `new_chat_session` (src-tauri/src/lib.rs:940)
  - Submit messages: `submit_chat` (src-tauri/src/lib.rs:960)
  - Recent chats + rollout scan: `list_recent_chats` (src-tauri/src/lib.rs:722)
  - Off‑record requests (private proto messages): `send_offrecord` + capture in reader (src-tauri/src/lib.rs:1182, src-tauri/src/lib.rs:1120)
- UI & state: `src/app.rs`
  - Stream listener mapping to transcript: (src/app.rs:150)
  - Sidebar: chats, status, event log; “New chat” (src/app.rs:200)
  - Chat input + autoscroll (src/app.rs:440)
- Persistence you can reuse
  - Rollouts JSONL (Codex): `~/.codex/sessions/...` (docs/codex/core-rollout.md)
  - Sidecar titles: `.title` files (written next to rollouts) (src-tauri/src/lib.rs:714)

## User‑Visible Concept
- Master Task: a single, named job (e.g., “Convert Figma doc into screens”) that:
  - Owns a queue of Subtasks (chunks)
  - Runs unattended across time (pause/resume)
  - Streams results back to UI and commits intermediate artifacts
  - Has a clear stop condition (acceptance criteria OR budget exhausted)

## Proposed Files & Persistence
- `~/.codex/master-tasks/*.task.json` (single source of truth)
  - id, name, status, created_at, updated_at
  - autonomy_budget { approvals: never/auto, sandbox: danger/full, max_turns, max_token_budget }
  - stop_conditions { done_regex | checklist | callback }
  - queue: [ { id, title, status, inputs, session_id?, rollout_path?, last_error? } ]
  - metrics: { turns, tokens_in, tokens_out, wall_clock }
- Sidecars that already exist remain: rollout `.title` for quick listing.

## Orchestration Engine (Wrapper)
- New module: `src-tauri/src/master.rs` (not yet created; planned)
  - State machine per master task
  - Pulls next Subtask → ensures a Codex session (existing `new_chat_session` / `submit_chat`)
  - Writes a small “control prompt” at the start of each subtask to set role, autonomy, and stop hints
  - Tracks per‑turn events via the existing stream; updates metrics + queue state
  - Pause/Resume: toggles a task flag the engine honors between turns
  - Backoff/retry rules when tool errors or rate limits hit (simple jitter + max tries)

## Chunking & Planning
- Start: User enters master goal, optional acceptance criteria, autonomy budget
- “Planner” pass (off‑record): create a to‑do list as Subtasks
  - Send a single private prompt via `send_offrecord`: “Break this goal into 8–15 atomic steps … Return JSON list.”
  - Validate JSON and seed `queue[]`
- During execution: allow dynamic replanning
  - When a subtask finishes early/late, run another off‑record prompt to adjust remaining steps

## UI Additions (Fully Transparent)
- Left sidebar: new “Master Tasks” section
  - List tasks with status chips (running, paused, done, error)
  - Button: New master task
- Main panel (when a task is selected)
  - Title + controls: Pause/Resume, Cancel, Autonomy level
  - Checklist of Subtasks with statuses; current one shows live transcript
  - Events strip (we already have the raw log) pinned to the right
  - Budgets: turns, token, wall‑clock progress bars
- Chat area remains; for a master task, the transcript is scoped to the active subtask/session

## Process Model
- One subtask → one live Codex session (reusing current proto bridge)
- When a subtask completes by stop rule (e.g., “code compiled without errors”), mark it done and advance
- If a subtask needs manual input, surface “Awaiting input” and pause engine (user can type or skip)

## Stop Conditions (Examples)
- Acceptance checklist satisfied (all checks green)
- Code builds + test suite passes
- Diff size / file count threshold hit
- Time/turn/token budget exceeded

## Safety & Approval Strategy
- Autonomy flag feeds existing config knobs we already pass:
  - `approval_policy=never` for unattended
  - `sandbox_mode=danger-full-access` (can be edited per user)
- For risky steps, insert a “review gate” subtask that blocks on user approval

## Where to Hook in Code
- Engine runs alongside stream logic in `src-tauri/src/lib.rs`; minimal hooks:
  - Emit UI event when a subtask starts/completes (new small `UiStreamEvent` like `PlanUpdate` or `TaskUpdate`)
  - Reuse `send_offrecord` for planning/summarizing and keep the UI uncluttered (we already filter those events)
  - Use `session_configured` to attach session_id to the subtask (src-tauri/src/lib.rs:1315)
- UI listens in `src/app.rs` and updates master task panel; user can pause/skip

## Minimal Milestones
1) Data model + file: `.task.json` loader/saver; CRUD in Tauri commands
2) Sidebar + master task panel UI (list, detail, controls)
3) Planner off‑record prompt for initial queue
4) Runner loop: one subtask end‑to‑end with budgets, pause/resume
5) Error paths + retry; metrics and stop rules
6) Polish: background notifications, sidecar titles for subtasks, deep links into rollouts

## Risks & Mitigations
- Runaway autonomy → budgets & gates
- Ambiguous success → encode concrete checks (e.g., run tests, grep logs)
- Restarts → persist engine state and resume ticker from `.task.json`

## Why this slots in cleanly
- We already have:
  - Proto runtime, event mapping, and off‑record channel
  - Rollout persistence + session discovery
  - UI scaffolding for status, logs, chats, and collapsible detail
- The engine needs no core‑Codex changes; it’s a wrapper that drives sessions with a transparent UI and durable state.

