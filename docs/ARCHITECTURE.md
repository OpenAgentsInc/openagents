OpenAgents Architecture

Overview
- Goal: a mobile and desktop command center for coding agents. The system streams Codex (or compatible) JSONL output into a local Convex backend for realtime sync across clients, while keeping the on‑disk JSONL as the canonical resume.
- Layers:
  - Desktop (Tauri): local app with a Convex sidecar and a Rust backend that exposes commands and subscriptions to the webview.
  - Mobile (Expo): React Native app using Expo Router. Talks to Convex for data; uses the bridge only for control (e.g., run.submit).
  - Bridge (Rust): Axum WebSocket server that spawns the Codex CLI, forwards stdout/stderr lines to clients, and mirrors agent state to Convex for sync. Also watches Projects/Skills folders and Codex sessions on disk.
  - Convex: local database + functions; the source of truth for synced threads/messages in the apps (mirrors JSONL and FS state).

High‑Level Data Flow
- Run submission (user → agent):
  1) Client sends a control message over the bridge WS: `{ "control": "run.submit", threadDocId, text, projectId?, resumeId? }`.
  2) Bridge spawns a short‑lived Codex child process (optionally CD to the project’s workingDir and optionally resume) and writes a small JSON preface + the user text to stdin, then closes stdin.
  3) As Codex emits JSONL, the bridge parses key events and writes normalized rows into Convex (assistant messages, reasoning, tool rows).
  4) Apps subscribe to Convex queries and render updates in realtime.

- Desktop (Tauri) send path specifics:
  - The Convex function `runs:enqueue` persists the user message; the Tauri backend does not insert a duplicate.
  - The bridge defaults `resumeId` to `last` when not provided, ensuring Codex continues the latest session in that thread.

- Streaming (agent → apps):
  - The bridge writes partial assistant/reason text via `messages:upsertStreamed` and finalizes via `messages:finalizeStreamed` at turn completion. Desktop and mobile subscribe and show live text growth.

- Projects/Skills sync (FS → Convex):
  - The bridge watches `~/.openagents/{projects,skills}` and a repo registry `./skills/` and mirrors valid entries into Convex (`projects:upsertFromFs`, `skills:upsertFromFs`). Deletions are removed by scope.

- External sessions tailer (JSONL → Convex):
  - The bridge watches `~/.codex/sessions` and mirrors the latest assistant/reason text of external runs into Convex so new threads appear and stream live across devices.

Components & Responsibilities

Bridge (Rust, crates/codex-bridge)
- Entry: `crates/codex-bridge/src/main.rs` (thin). Most logic is in modules:
  - `ws.rs`: Axum WebSocket route (`/ws`), socket loop, control dispatch, and the stdout/stderr forwarder mapping JSONL into Convex writes.
  - `codex_runner.rs`: spawn/respawn of the Codex CLI with full‑access defaults and JSON output mode; wrapper for stdin/stdout/stderr handles.
  - `convex_write.rs`: streaming write helpers (upsert/finalize) and log compaction for large deltas.
  - `watchers.rs`: notify‑based watchers for Projects/Skills and the sessions tailer; one‑shot sync functions.
  - `controls.rs`: tolerant control parser for WS payloads.
  - `state.rs`: global AppState (broadcast channel, child I/O, trackers).
  - `bootstrap.rs`: Convex local backend lifecycle (optional for the CLI mode).
  - `util.rs`: now_ms, path expansion, repo root detection, misc helpers.

- Control plane (WebSocket only; no REST): see `controls.rs` for supported verbs:
  - `bridge.status`, `convex.status`, `projects`, `skills`, `project.save`, `project.delete`, `convex.backfill`, `interrupt`, `run.submit`.

- JSONL → Convex mapping (selected examples):
  - `agent_message.delta` and `reasoning.delta` → `messages:upsertStreamed` with partial=true.
  - Final agent message or reasoning item → `messages:finalizeStreamed`; if no partial existed, insert a snapshot `messages:create`.
  - Tool rows via `item.*`:
    - `command_execution` → kind `cmd` (payload JSON in `text`)
    - `file_change` → kind `file`
    - `web_search` → kind `search`
    - `mcp_tool_call` → kind `mcp`
    - `todo_list` → kind `todo`

Desktop (Tauri)
- Rust backend modules (`tauri/src-tauri/src/`):
  - `bridge.rs`: starts Convex sidecar and emits `convex:local_status` & `bridge:ready` events; spawns the CLI bridge in dev.
  - `convex.rs`: pure mapping helpers and commands (list threads/messages, count, mapping/hide rules).
  - `subscriptions.rs`: live Convex subscriptions, emitting `convex:threads` and `convex:messages` to the webview.
  - `commands.rs`: mutations (`runs:enqueue`, `threads:create`). User message persistence is handled in `runs:enqueue` (no client‑side duplication).
- Webview UI (Leptos, `tauri/src/`):
  - `app.rs`: connects to `/ws`, renders threads/messages, controls composer, and logs status.
  - `library.rs`, `jsonl.rs`, `composer.rs`: component library and rendering primitives.

Mobile (Expo)
- Uses Convex React for queries and subscriptions and uses the bridge WS for control only. See:
  - Router screens (e.g., `/session`, `/projects`, etc.) and message rendering.
  - Providers: `expo/providers/ws.tsx` (bridge status, permissions toggles) and Convex provider.
  - JSONL renderers in `expo/components/jsonl/*`.

Convex (local backend)
- Schema (selected): `convex/schema.ts`
  - `threads`: metadata (`threadId`, `title`, `resumeId`, etc.)
  - `messages`: `{ threadId, role?, kind, text?, data?, ts, createdAt, updatedAt?, itemId?, partial?, seq? }`
  - `projects`, `skills`: FS‑mirrored domain objects (see schemas below)
- Functions (selected):
  - `messages:forThread` (ordered by `(threadId, ts)`), `messages:upsertStreamed`, `messages:finalizeStreamed`, `messages:create`
  - `threads:listWithCounts` (aggregate + filter to hide zero‑message threads), `threads:create`, `threads:upsertFromStream`
  - `projects:upsertFromFs`, `projects:remove`, `skills:upsertFromFs`, `skills:removeByScope`

Sync Model & Source of Truth
- Threads/Messages: On‑disk JSONL is canonical; Convex mirrors for sync and UI queries. The bridge writes in near‑real‑time while a run is active and can backfill from history.
- Projects/Skills: Files under `~/.openagents/{projects,skills}` are canonical; the bridge mirrors them to Convex and keeps them in sync with FS watchers.

Ports & Environment
- Bridge WS: `ws://<host>:8787/ws` (bind configurable via `CODEX_BRIDGE_BIND`).
- Convex local backend:
  - Bridge‑managed default: `http://127.0.0.1:7788` (override `OPENAGENTS_CONVEX_PORT`).
  - Desktop sidecar default: also respects `OPENAGENTS_CONVEX_PORT`; binds to loopback by default.
- Sessions directory: `~/.codex/sessions` (override `CODEXD_HISTORY_DIR`).
- OpenAgents home: `~/.openagents` (override `OPENAGENTS_HOME`).

Security & Permissions
- The bridge spawns the Codex CLI with full‑access defaults for developer workflows:
  - Injected flags unless already present: `--dangerously-bypass-approvals-and-sandbox`, `-s danger-full-access`, `-m gpt-5`, `-c model_reasoning_effort=high`.
- Apps never call arbitrary HTTP endpoints on the bridge; control is strictly through WS.

Testing & Coverage
- Unit tests cover: control parser edge cases, ws payload helpers, streaming compactor, history parsing, skills/projects mapping, and Tauri mapping/hide helpers.
- See docs/test-coverage.md for current inventory and planned additions.

Related Documentation
- Streaming & Sync details: docs/sync.md
- JSONL schema notes (bridge → apps): docs/exec-jsonl-schema.md
- Resume behavior: docs/exec-resume-json.md
- Permissions model and recommended setups: docs/permissions.md
- Projects & Skills schema: docs/projects-and-skills-schema.md
- Test coverage audit: docs/test-coverage.md

Development Quickstart
- Bridge: from repo root `cargo run -p codex-bridge -- --bind 0.0.0.0:8787` (or `cargo bridge` alias if configured).
- Desktop: `cd tauri && cargo tauri dev` (starts Convex sidecar and the bridge in dev; webview connects automatically after `bridge:ready`).
- Mobile: `cd expo && bun install && bun run start` (then `bun run ios|android|web`).
