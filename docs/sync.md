OpenAgents Sync Model

Scope
- This document audits how data flows across the three layers — Bridge (Rust), Mobile (Expo), and Desktop (Tauri) — and how they stay in sync using Convex and Codex JSONL rollouts. It also recommends the steady‑state model and phased improvements.

Summary
- Source(s) of truth:
  - Threads/Messages: Codex JSONL files are the canonical resume log; Convex mirrors them for live sync and queries.
  - Projects/Skills: Files under `~/.openagents/{projects,skills}` are canonical; Bridge mirrors them to Convex and keeps them in sync with FS watchers.
- Live ingestion (today):
  - When a run is triggered through our Bridge, the Bridge streams Codex JSONL and writes normalized rows directly into Convex in real time (threads/messages).
  - Mobile/Desktop UIs subscribe to Convex queries and render live updates.
  - Desktop (Tauri) optimistically persists the user message to Convex (`messages:create`) before enqueuing the run so the message appears immediately in the UI.
  - If `resumeId` is not provided, the Bridge uses `last` by default so Codex continues the current session.
- Historical ingestion:
  - On startup, the Bridge backfills recent Codex JSONL sessions into Convex; an on‑demand WS control can trigger a larger backfill.
- What’s not covered yet (now addressed):
  - Continuous FS watching of the Codex sessions directory for external agent runs (e.g., “base Claude Code”) after startup. Implemented via a `notify` watcher that tails `~/.codex/sessions` and upserts stream rows into Convex. Tests pending (see coverage doc).

Architecture At A Glance
- Bridge (Axum WS + Convex client)
  - WebSocket server at `ws://<host>:8787/ws` (control plane + JSONL broadcast)
  - Spawns Codex and forwards stdout/stderr lines to all clients
  - Writes key events to Convex: threads upsert, messages insert, reasoning, command cards
  - Filesystem→Convex sync for Projects and Skills with watchers (enabled by default)
  - Historical backfill from JSONL sessions into Convex at startup (+ on demand)
- Mobile (Expo)
  - Uses Convex React for queries/subscriptions (threads, messages, projects, skills)
  - Uses Bridge WS only for control (e.g., `run.submit`) and non‑Convex utilities
- Desktop (Tauri)
  - Bundles/launches a local Convex backend (sidecar) and auto‑deploys functions
  - Provides Rust commands that query/subscribe to Convex and emit UI events
  - On send: `enqueue_run` first calls `messages:create` (user message), then `runs:enqueue`

Key Codepaths (by layer)
- Bridge
  - WebSocket and process orchestration: `crates/codex-bridge/src/ws.rs`
  - Projects (FS schema + IO): `crates/codex-bridge/src/projects.rs:1`
  - Skills (FS schema + IO): `crates/codex-bridge/src/skills.rs:1`
  - JSONL history parsing: `crates/codex-bridge/src/history.rs:1`
  - Convex write points (threads/messages during stream):
    - Helpers in `crates/codex-bridge/src/convex_write.rs` (upsert/finalize, log compaction)
    - The stdout forwarder in `ws.rs` invokes those helpers when mapping assistant/reasoning and tool rows
  - FS→Convex sync (Projects/Skills):
    - `crates/codex-bridge/src/watchers.rs` (sync + notify watchers)
  - Historical backfill (Codex JSONL → Convex):
    - Startup backfill queue: `crates/codex-bridge/src/watchers.rs` (`enqueue_historical_on_start`)
    - On‑demand WS control `{"control":"convex.backfill"}`: handler in `ws.rs`
  - Run submission (Bridge spawns Codex and writes stdin): control `run.submit` handled in `ws.rs` using `codex_runner.rs`

- Convex (functions + schema)
  - Schema and indexes: `convex/schema.ts:1`
  - Threads: list, byId, create, upsertFromStream → `convex/threads.ts:1`
  - Messages: forThread, create, byId, countForThread → `convex/messages.ts:1`
  - Projects: list, byId, upsertFromFs, remove → `convex/projects.ts:1`
  - Skills: listAll, listByScope, upsertFromFs, removeByScope, bulkUpsertFromFs → `convex/skills.ts:1`

- Mobile (Expo)
  - Convex Provider (URL derived from Bridge host): `expo/providers/convex.tsx:1`
  - Bridge WS provider (control plane): `expo/providers/ws.tsx:1`
  - Convex thread UI (queries + send): `expo/app/convex/thread/[id].tsx:1`
  - Projects provider (Convex + local store): `expo/providers/projects.tsx:1`
  - Skills provider (Convex + local store): `expo/providers/skills.tsx:1`

  - Desktop (Tauri)
  - Convex sidecar lifecycle + auto‑deploy: `tauri/src-tauri/src/bridge.rs`
  - Commands to list/subscribe threads/messages: `tauri/src-tauri/src/convex.rs`, `subscriptions.rs`
  - Bridge bootstrap and `bridge:ready` event: `tauri/src-tauri/src/bridge.rs`

Data Model Conventions
- Threads
  - `threads.threadId` is a stable key used by `messages.threadId`.
  - For app‑created threads, `threadId` is set to the Convex document `_id` string for simplicity.
  - For imported Codex JSONL sessions, `threadId` is set to the Codex `thread_id` value.
  - Queries always use `messages:forThread({ threadId })` regardless of whether the value came from a Convex doc `_id` or an external Codex id.
- Messages
  - Inserted with `{ threadId, role?, kind, text?, data?, ts }` and indexed by `(threadId, ts)`.
  - Server queries filter out preface/system entries in Desktop; Mobile hides “preface” heuristically in UI.
- Projects/Skills
  - Filesystem is canonical; Bridge mirrors into Convex via sync functions and watchers; removal in FS removes Convex rows.

Live Flow (Bridge‑originated runs)
1) App enqueues a run in Convex for UI immediacy: `runs:enqueue` (writes the user message immediately so the transcript updates) — `convex/runs.ts:1`.
2) App sends `{"control":"run.submit", ...}` over Bridge WS with the Convex thread doc id and optional project — `expo/app/convex/thread/[id].tsx:52`.
3) Bridge spawns Codex with working dir/project context and writes a one‑line JSON config + the user text to stdin. See `crates/codex-bridge/src/main.rs:720`.
4) As Codex streams JSONL events, the Bridge forwards lines to WS clients and mirrors agent/user/reasoning/command items into Convex messages — `crates/codex-bridge/src/main.rs:1196`.
5) UIs subscribe to Convex and update live; no direct REST to the Bridge is used.

Historical Flow (external or prior runs)
- On start, the Bridge scans Codex sessions (`~/.codex/sessions`) and upserts the newest threads/messages into Convex — `crates/codex-bridge/src/main.rs:1458`.
- At any time, a WS control `{ "control": "convex.backfill" }` triggers a larger parse + upsert pass — `crates/codex-bridge/src/main.rs:594`.

What’s Working Well
- Low‑latency live updates: runs submitted through our Bridge appear instantly in Mobile/Desktop via Convex subscriptions.
- Projects/Skills: FS→Convex sync is complete with watchers for both personal and registry scopes; project‑scoped `skills/` are mirrored per project root.
- Desktop sidecar: Convex bundled and auto‑deployed; subscriptions are handled natively in Rust and emitted to the webview.

Observed Gaps / Issues
- External agents bypassing our Bridge
  - If a user runs another CLI that writes Codex JSONL, those files are not continuously mirrored unless the Bridge restarts (startup backfill) or a manual backfill is triggered.
  - Impact: Mobile/Desktop won’t see those threads/messages live.
- Subscription hiccups (reported)
  - Early iterations subscribed to thread lists and then performed per‑row `countForThread` client loops. We replaced this with a server‑side aggregate query `threads:listWithCounts` to reduce load and flapping. See recent commits and Tauri code using `subscribe("threads:listWithCounts", …)`.
  - On Mobile, message subscriptions rely on `thread.threadId`. For brand‑new threads, we fall back to the doc `_id` until `threadId` is patched; code handles this by passing `thread?.threadId || id` — `expo/app/convex/thread/[id].tsx:17`.
- Stale docs
  - Older docs referenced a spool/ingester; current Bridge writes to Convex directly. Comments in code reflect this (“spool/mirror removed”).

Decision: What Should Be Canonical for Sync?
- Recommended steady state: Hybrid with Convex as the live sync backbone
  - Keep Codex JSONL as the canonical resume log.
  - Continue writing live runs (through our Bridge) directly into Convex for low latency.
  - Add a lightweight file watcher for `~/.codex/sessions` to capture external runs in near‑real‑time (see Plan below). This makes Convex the always‑on “sync bus” regardless of which agent produced the JSONL.

Alternatives Considered
- “Convex only” for everything
  - Pros: single store for queries/sync; fewer moving parts for UIs
  - Cons: Loses compatibility with third‑party tools and Codex’s resume semantics unless we also write JSONL; not viable while agents save to disk natively.
- “Filesystem only” with clients parsing JSONL
  - Pros: one canonical store (disk)
  - Cons: Reimplement querying, pagination, and live subscriptions; weaker multi‑client sync story; higher coupling to JSONL schema

Recommended Plan (Phased)
- P0 (done)
  - Live ingest to Convex for Bridge‑originated runs (threads/messages)
  - FS→Convex sync for Projects/Skills with watchers
  - Startup + on‑demand backfill for historical JSONL
- P1 (short‑term)
  - Add a sessions watcher: monitor `~/.codex/sessions` for creates/modifies and incrementally parse appended lines (track file offsets) → write to Convex. Idempotent by (threadId, ts, role/kind, text hash).
  - Surface a WS control to toggle and report watcher status (e.g., `{ "control": "sessions.watch_status" }`).
  - Mobile/Desktop: keep existing Convex subscriptions; no UI changes required.
- P2 (polish)
  - Add a lightweight dedupe layer in Convex mutations to avoid duplicate messages on re‑parse.
  - Normalize thread title updates and `resumeId` handling in `threads:upsertFromStream`.
  - Add vector/full‑text indexes for message search.

Operational Defaults
- Ports
  - Bridge WS: `ws://<host>:8787/ws`
  - Convex (Bridge‑managed): `http://<host>:7788` by default; set `OPENAGENTS_CONVEX_PORT` to change
  - Desktop sidecar also respects `OPENAGENTS_CONVEX_PORT` and binds to `OPENAGENTS_CONVEX_INTERFACE` (defaults to `0.0.0.0`).
- Bridge flags (Codex spawn)
  - Defaults to full access unless overridden; see permissions doc. The app still prepends a JSON config line to stdin for belt‑and‑suspenders.
- No REST to Bridge
  - Control is strictly via WS messages (e.g., `run.submit`, `convex.status`, `convex.backfill`, project save/delete). Do not add HTTP endpoints.

Quick References (files)
- Bridge provider (WS): `expo/providers/ws.tsx:1`
- Convex provider (URL derivation): `expo/providers/convex.tsx:1`
- Thread screen (send/subscribe): `expo/app/convex/thread/[id].tsx:1`
- Bridge JSONL → Convex writes: `crates/codex-bridge/src/convex_write.rs` (called from `ws.rs`)
- Projects/Skills/Sessions watchers: `crates/codex-bridge/src/watchers.rs`
- Historical backfill entry: `crates/codex-bridge/src/watchers.rs` (`enqueue_historical_on_start`)

FAQ
- Q: Why do some threads use a Convex doc `_id` as `threadId` and others a UUID from Codex?
  - A: New app‑created threads use the doc `_id` for simplicity. Imported Codex threads keep the external `thread_id`. `messages.threadId` always matches the corresponding thread’s `threadId`, so subscriptions work uniformly.
- Q: Why did we stop writing to a “spool” on disk?
  - A: We now write directly to Convex from the Bridge for lower latency and simpler ops. If Convex is down, we still have on‑demand backfill and can add a sessions watcher to close the remaining gap.
- Q: Should mobile/desktop read JSONL directly?
  - A: No. They read from Convex (subscriptions) and use Bridge only for control messages. JSONL remains for resume and import.

Structure Recommendations (reduce file bloat)
- Goal: improve maintainability and avoid oversized single files by splitting domains and responsibilities. The structure below keeps syncing logic cohesive and testable.

- Bridge (Rust)
  - Split `crates/codex-bridge/src/main.rs` into modules:
    - `ws.rs` — Axum WebSocket server + control routing
    - `codex_runner.rs` — spawn/respawn, stdin/stdout forwarding, config preface
    - `convex_write.rs` — helpers to map JSONL → Convex mutations (threads/messages)
    - `fs_watch.rs` — Projects/Skills watchers (reuse notify wiring for sessions watcher)
    - `history_scan.rs` — Codex sessions scan/parse utilities (from `history.rs`)
    - `sessions_watch.rs` — new: incremental watcher for `~/.codex/sessions` (track offsets; idempotent upserts)
  - Move `projects.rs` and `skills.rs` under `crates/codex-bridge/src/fs_sync/` to group FS‑backed models.
  - Optional shared crate: `crates/openagents-sync/` with JSONL parsing + Convex write helpers used by both Bridge and (future) side tools.

- Convex (functions)
  - Group by domain folders with barrel exports:
    - `convex/threads/queries.ts`, `convex/threads/mutations.ts`
    - `convex/messages/queries.ts`, `convex/messages/mutations.ts`, `convex/messages/aggregates.ts`
    - `convex/projects/*.ts`, `convex/skills/*.ts`
  - Keep `convex/schema.ts` at root; avoid mega files by splitting domain logic.

- Mobile (Expo)
  - Create domain providers under `expo/providers/convex/` (threads.tsx, messages.tsx) so `expo/app/*` screens stay thin.
  - Split `expo/app/convex/thread/[id].tsx` into smaller components: `MessageList.tsx`, `Composer.tsx`, `ThreadMeta.tsx` in `expo/components/thread/`.

- Desktop (Tauri)
  - Split `tauri/src-tauri/src/lib.rs` into modules:
    - `convex.rs` (sidecar, deploy, status emitter)
    - `bridge.rs` (bridge bootstrap/status)
    - `subscriptions.rs` (thread/message subscriptions)
    - `commands.rs` (invoke handlers)

- Docs
  - Move sync docs into `docs/sync/`:
    - `docs/sync/overview.md` (this page’s high‑level content)
    - `docs/sync/bridge-to-convex.md` (live ingestion mapping)
    - `docs/sync/jsonl-backfill.md` (historical import)
    - `docs/sync/sessions-watcher.md` (design + ops)
  - Keep short single‑purpose pages; link from a brief `docs/sync.md` index.

- Hygiene
  - Cap file sizes (~500–800 lines) and extract modules once exceeded.
  - Add lightweight unit tests for JSONL→Convex mappers and history parsing.
  - Prefer feature‑flagged modules for experimental watchers to limit main surface.
