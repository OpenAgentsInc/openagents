# OpenCode Integration Plan

This document proposes how to support OpenCode in the OpenAgents mobile app and Rust bridge, alongside the existing OpenAI Codex CLI integration. It surveys OpenCode’s architecture, identifies the best integration points, defines an adapter strategy in the bridge, and outlines a phased rollout plan. All events emitted to the app will conform to the unified, canonical ThreadEvent schema described in docs/integrations/README.md and implemented in crates/oa-bridge/src/events.rs.

## Goals

- Provide a seamless “use OpenCode instead of Codex” experience from the mobile app.
- Preserve the app’s WebSocket‑only contract with the bridge (no direct HTTP from the app).
- Reuse existing feed UI as much as possible by mapping OpenCode events to our established JSONL row types.
- Keep provider selection a runtime choice in Settings; maintain backward compatibility with Codex.

## OpenCode Architecture (survey)

Key observations from the OpenCode monorepo at `~/code/opencode`:

- Server: Bun/Hono HTTP server with REST + SSE endpoints
  - Entry: `packages/opencode/src/server/server.ts:1`
  - Start: `opencode serve --hostname 127.0.0.1 --port 0` prints listening URL
  - SSE stream: `GET /event` emits all Bus events (JSON) via Server‑Sent Events
  - Session lifecycle and prompt APIs:
    - `POST /session` → create session (returns Session.Info)
    - `GET /session/:id/message` → list messages
    - `POST /session/:id/message` → submit prompt; responds with created assistant message
    - `POST /session/:id/command|shell|revert|...` → additional actions
  - TUI bridge endpoints: `/tui/*` plus `/tui/control/*` long‑poll channel (for Go TUI)
- Message and event model (v2)
  - Types: `packages/opencode/src/session/message-v2.ts:1`
  - Parts: `text`, `reasoning`, `tool` (with running/completed/error states), `file`, `step-start`, `step-finish`, `snapshot`, `patch`, `agent`, `retry`
  - Bus events: session/message/part updated/removed published on a central event bus
- SDKs and clients
  - JS SDK generated via Stainless: `packages/sdk/js/src/gen/sdk.gen.ts:1`
  - VSCode extension talks HTTP to local server and uses `/tui/append-prompt`, etc.

Implications:

- OpenCode exposes a local HTTP API and an SSE event stream. It is not a line‑oriented JSONL CLI like Codex; we’ll need an adapter.
- The event granularity is rich and maps well to our UI concepts (agent text, reasoning, command/tool runs, file patches, turn summary).

## Current Bridge Behavior (baseline)

- The Rust bridge (`crates/oa-bridge/src/main.rs:1`) spawns `codex exec --json` and fans out the child’s stdout/stderr lines to all WebSocket clients.
- The app sends control messages (e.g., `{ "control": "run.submit", ... }`) over the WS; the bridge writes the prompt to the child’s stdin and closes it to signal EOF.
- Resume semantics: the bridge will auto‑append `resume --last` when supported and respawn the child per prompt.

## Integration Strategy

We will add an “OpenCode mode” to the bridge that launches an OpenCode headless server, manages an OpenCode session, subscribes to SSE events, and translates them into the canonical ThreadEvent JSONL envelope for the app.

### Bridge adapter (OpenCode → Codex‑JSONL)

- Process lifecycle
  - Spawn `opencode serve --hostname 127.0.0.1 --port 0` and parse stdout for `listening on http://HOST:PORT`.
  - Probe health via `GET /config` and `GET /path` to confirm working directory.
  - Ensure the working dir matches the repo root heuristic the bridge already uses.

- Session lifecycle
  - On first client connect or first `run.submit`, create an OpenCode session via `POST /session` and store `sessionID` in bridge state.
  - Optionally restore the last session by calling `GET /session` and selecting the most recent; configurable via app’s “Resume last session” toggle.

- Event subscription
  - Subscribe to `GET /event` SSE.
  - For each bus event, perform a best‑effort mapping into our app’s JSONL envelope. Emit one JSON object per line to WS clients.

- Prompt submission
  - On `{ control: "run.submit", text, ... }` (existing app control), call `POST /session/:id/message` with `parts: [{ type: "text", text }]` and optional system/agent/tool settings derived from app toggles.
  - For binary/attachments in future, construct `file` parts with URLs pointing to OpenCode’s served resources (if any), or embed as Markdown links.

- Error/resilience
  - If the opencode server process exits, surface `{ type: "error", message }` and auto‑restart on next command.
  - If SSE drops, auto‑reconnect with backoff; ensure no duplicate emission by tracking last seen event ids where available.

### Event mapping (OpenCode → canonical ThreadEvent)

Map OpenCode bus payloads into canonical ThreadEvent items (bridge emits one JSON line per event). The app’s `expo/lib/codex-events.ts:1` will continue to render these as it already targets the same envelope:

- MessageV2.PartUpdated with `part.type`:
  - `text` → `{ type: "item.completed", item: { type: "agent_message", id, text } }`.
  - `reasoning` → `{ type: "item.completed", item: { type: "reasoning", id, text } }`.
  - `tool` (status running/completed/error) →
    - running → `{ type: "item.started", item: { type: "command_execution", id, command: toolName, aggregated_output: "", status: "in_progress" } }`
    - completed/error → `{ type: "item.completed", item: { type: "command_execution", id, command: toolName, aggregated_output: outputOrError, exit_code: status=="completed"?0:1, status } }`.
  - `patch` → `{ type: "item.completed", item: { type: "file_change", id, changes: files.map(f=>({ path:f, kind:"update" })), status:"completed" } }`.
  - `snapshot` → suppressed (internal state).
  - `file` attachment → emit an `agent_message` with a markdown link, or ignore if redundant (phase 1);
- Session/turn
  - Part `step-start` → `{ type: "turn.started" }`.
  - Part `step-finish` → `{ type: "turn.completed", usage: { input_tokens, cached_input_tokens: cache.read, output_tokens } }` using token fields.
- Errors
  - `session.error` / assistant `error` → `{ type: "error", message }`.
- Housekeeping
  - On SSE connect → emit `{ type: "thread.started", thread_id: sessionID }`.

Notes:
- This mapping intentionally targets existing UI cards to avoid app changes in phase 1.
- As a follow‑up, we can add native renderers for OpenCode’s richer parts (tool call states, attachments, retries, snapshots).

### App behavior

- No direct HTTP calls; all interaction remains via the bridge WS.
- Settings gains a Provider selector: `Codex (CLI)` or `OpenCode (server)`.
- History continues to be powered by our AsyncStorage log store; OpenCode session IDs are included in row metadata for deep links in the future.

### Controls (WS)

We will reuse current controls where feasible and add OpenCode‑specific ones conservatively:

- Existing
  - `run.submit` — submit a prompt (bridge routes to `POST /session/:id/message`).
  - `run.abort` — future: bridge triggers abort controller in OpenCode via internal state.
- New (optional)
  - `provider.select` with `{ provider: "codex" | "opencode" }` — toggles adapter mode server‑side.
  - `opencode.session.create` — force‑create a new session; otherwise implicit on first submit.
  - `opencode.session.share` — call `POST /session/:id/share` if we want share links in the app later.

## Security & Permissions

- The app continues to run without elevated permissions; all disk/network actions occur on the desktop side.
- OpenCode manages provider API keys and permissions internally (`packages/opencode/src/server/server.ts:1`, `Auth`, `Permission`). The bridge does not proxy secrets.
- Our bridge launch arguments should remain analogous to the Codex path: run in repo root, full disk access granted by the host process, no additional sandboxing layered atop OpenCode.

## Rollout Plan

1) Phase 1 — Bridge‑only adapter
- Add an `opencode` runner to the bridge that:
  - Spawns `opencode serve`, discovers host/port, and manages one session per WS client group.
  - Subscribes to `/event` SSE and forwards mapped JSONL to WS clients.
  - Implements prompt submission via `POST /session/:id/message` on `run.submit`.
- Ship behind a Settings flag in the app; default remains Codex.

2) Phase 2 — Native event rendering (optional)
- Add `opencode-events.ts` in the app and new components for:
  - Tool lifecycle (running/completed/error with input/output diffs)
  - Step start/finish with token/cost summaries
  - Attachments (render images/files inline)
- Update history/thread views to show OpenCode session metadata.

3) Phase 3 — Projects and MCP
- Consider wiring OpenCode’s workspace/project awareness into our Projects view if/when we align schemas.
- Explore mapping OpenCode MCP/tool surfaces into our existing `mcp_call` cards.

## Testing

- Local dev
  - Start bridge in OpenCode mode, confirm SSE connects and canonical ThreadEvent lines stream into the app feed.
  - Submit prompts and verify text/reasoning/tool/file events render as expected.
  - Kill and restart the OpenCode server process to verify bridge auto‑recovers.
- Non‑goals
  - We won’t change OTA policy or add tests in this phase; adapter is server‑side.

## Risks & Open Questions

- Event fidelity: some OpenCode parts (snapshots, retries) have no 1:1 mapping; we’ll initially suppress or render as generic JSON blocks.
- Session semantics: one OpenCode session per app Session screen vs. global session per bridge — we’ll start with one per app session to avoid crosstalk.
- Abort/interrupt: wiring a user‑initiated abort into OpenCode’s processing loop will require a small addition to the adapter state.
- Attachments: require either serving local files or embedding safe URLs; we’ll start with markdown links.

## Appendix: Endpoints used

- `GET /event` — SSE bus
- `POST /session` — create
- `GET /session/:id` — fetch session info
- `POST /session/:id/message` — submit prompt
- `GET /session/:id/message` — list messages
- `GET /config` / `GET /path` — sanity checks

File references (for contributors):
- OpenCode server: `~/code/opencode/packages/opencode/src/server/server.ts:1`
- OpenCode messages: `~/code/opencode/packages/opencode/src/session/message-v2.ts:1`
- Canonical event types: `crates/oa-bridge/src/events.rs:1`
- App parser: `expo/lib/codex-events.ts:1`
- Existing JSONL contract: `docs/exec-jsonl-schema.md:1`
