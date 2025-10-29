OpenAgents Architecture

Overview
- Goal: a mobile and desktop command center for coding agents. The system streams Codex (or compatible) JSONL output and persists to a minimal in‑process SQLite backend (Tinyvex) for realtime sync across clients, while keeping the on‑disk JSONL as the canonical resume.
- Layers:
  - Desktop (Tauri): local app with a Convex sidecar and a Rust backend that exposes commands and subscriptions to the webview.
  - Mobile (Expo): React Native app using Expo Router. Talks to Convex for data; uses the bridge only for control (e.g., run.submit).
  - Bridge (Rust): Axum WebSocket server that spawns the Codex CLI, forwards stdout/stderr lines to clients, and mirrors agent state into Tinyvex for fast local sync. No long‑running external database process.
  - Tinyvex: minimal in‑process SQLite + WebSocket changefeed; the source of truth for synced threads/messages in the apps (mirrors JSONL state). See docs/tinyvex/tinyvex.md.

High‑Level Data Flow
- Run submission (user → agent):
  1) Client sends a control message over the bridge WS: `{ "control": "run.submit", threadDocId, text, projectId?, resumeId? }`.
  2) Bridge spawns a short‑lived Codex child process (optionally CD to the project’s workingDir and optionally resume) and writes a small JSON preface + the user text to stdin, then closes stdin.
  3) As Codex emits JSONL, the bridge parses key events and writes normalized rows into Tinyvex (assistant messages, reasoning). Tool rows may be summarized in logs.
  4) Apps subscribe to Tinyvex snapshots/updates over the bridge WS and render updates in realtime.

- Desktop (Tauri) send path specifics:
  - The Convex function `runs:enqueue` persists the user message; the Tauri backend does not insert a duplicate.
  - The bridge defaults `resumeId` to `last` when not provided, ensuring Codex continues the latest session in that thread.

- Streaming (agent → apps):
  - The bridge writes partial assistant/reason text via Tinyvex streaming upserts and finalizes at turn completion. Desktop and mobile subscribe and show live text growth.

- Projects/Skills sync
  - File formats remain canonical on disk. Tinyvex MVP focuses on Threads/Messages; Projects/Skills may be added later as needed.

- External sessions tailer (JSONL → Tinyvex):
  - TODO: optional tailer for historical backfill into Tinyvex; current MVP writes live during runs.

Components & Responsibilities

Bridge (Rust, crates/oa-bridge)
- Entry: `crates/oa-bridge/src/main.rs` (thin). Most logic is in modules:
  - `ws.rs`: Axum WebSocket route (`/ws`), socket loop, control dispatch, and the stdout/stderr forwarder mapping JSONL into Tinyvex writes.
  - `codex_runner.rs`: spawn/respawn of the Codex CLI with full‑access defaults and JSON output mode; wrapper for stdin/stdout/stderr handles.
  - `tinyvex_write.rs`: streaming write helpers (upsert/finalize) and log compaction for large deltas.
  - `controls.rs`: tolerant control parser for WS payloads.
  - `state.rs`: global AppState (broadcast channel, child I/O, trackers).
  - `util.rs`: now_ms, path expansion, repo root detection, misc helpers.

- Control plane (WebSocket only; no REST): see `controls.rs` for supported verbs:
  - `bridge.status`, `projects`, `skills`, `project.save`, `project.delete`, `tvx.subscribe`, `tvx.query`, `tvx.mutate`, `interrupt`, `run.submit`.

- JSONL → Tinyvex mapping (selected examples):
  - `agent_message.delta` and `reasoning.delta` → streaming upserts with partial=true and a stable itemId per kind.
  - Final agent message or reasoning item → finalizeStreamed; if no partial existed, insert a snapshot row.
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

Tinyvex (local in‑process backend)
- Schema: see docs/tinyvex/tinyvex.md (SQLite DDL for `threads`, `messages`)
- WS protocol over the bridge:
  - Subscribe: `tvx.subscribe { stream: "threads" | "messages", threadId? }` → `tinyvex.snapshot` + `tinyvex.update`
  - Query: `tvx.query { name: "threads.list" | "messages.list", args }` → `tinyvex.query_result`

Sync Model & Source of Truth
- Threads/Messages: On‑disk JSONL is canonical; Convex mirrors for sync and UI queries. The bridge writes in near‑real‑time while a run is active and can backfill from history.
- Projects/Skills: Files under `~/.openagents/{projects,skills}` are canonical; the bridge mirrors them to Convex and keeps them in sync with FS watchers.

Ports & Environment
- Bridge WS: `ws://<host>:8787/ws` (bind configurable via `CODEX_BRIDGE_BIND`).
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
- Bridge: from repo root `cargo run -p oa-bridge -- --bind 0.0.0.0:8787` (or `cargo bridge` alias if configured).
- Mobile: `cd expo && bun install && bun run start` (then `bun run ios|android|web`).
