# Codex App Server (codex-rs/app-server)

This document explains how the Codex app server works, where the pieces live, and how to embed the
full Codex agent in a client via JSON-RPC. It complements the API reference in
`codex-rs/app-server/README.md`.

## What it is

- A JSON-RPC 2.0-style server over stdio (JSONL, one object per line).
- Embeds the full Codex agent: threads, tool calls, approvals, auth, skills, config, MCP, etc.
- Same core engine as the CLI/TUI; clients talk to a real Codex thread/turn pipeline.
- The protocol omits the `"jsonrpc": "2.0"` field (see `codex-rs/app-server-protocol/src/jsonrpc_lite.rs`).

## Minimal client flow

1. Spawn the server (`codex app-server` or `codex-app-server`).
2. Send `initialize` (request) with `clientInfo`, wait for response.
3. Send `initialized` (notification).
4. Start/resume a thread, then start turns and handle streamed notifications plus approval requests.

Example JSONL (client -> server):

```json
{"id":0,"method":"initialize","params":{"clientInfo":{"name":"my_app","title":"My App","version":"0.1.0"}}}
{"method":"initialized"}
{"id":1,"method":"thread/start","params":{"cwd":"/path/to/repo"}}
{"id":2,"method":"turn/start","params":{"threadId":"thr_123","input":[{"type":"text","text":"Hello"}]}}
```

The server will stream notifications like `thread/started`, `turn/started`, `item/started`,
`item/*/delta`, and `turn/completed` on stdout.

## Entrypoints and runtime wiring

- `codex-rs/cli/src/main.rs` - `codex app-server` subcommand and schema generation wiring.
- `codex-rs/app-server/Cargo.toml` - binary `codex-app-server` and lib `codex_app_server`.
- `codex-rs/app-server/src/main.rs` - CLI entry, `arg0` dispatch, debug managed config override.
- `codex-rs/arg0/src/lib.rs` - arg0 dispatch, PATH alias setup, helper binaries.
- `codex-rs/app-server/src/lib.rs` - `run_main`: stdin reader, processor task, stdout writer,
  config and telemetry setup.
- `codex-rs/app-server/src/message_processor.rs` - initialize handshake gate + config API.
- `codex-rs/app-server/src/codex_message_processor.rs` - main API handlers, core integration.
- `codex-rs/app-server/src/outgoing_message.rs` - JSON-RPC responses/notifications and server
  request tracking.

## Protocol and schema

- `codex-rs/app-server-protocol/src/jsonrpc_lite.rs` - JSON-RPC envelope types (no `"jsonrpc"`).
- `codex-rs/app-server-protocol/src/protocol/common.rs` - `ClientRequest`, `ServerRequest`,
  `ClientNotification`, `ServerNotification`, and method names.
- `codex-rs/app-server-protocol/src/protocol/v2.rs` - thread/turn v2 types and item schemas.
- `codex-rs/app-server-protocol/src/protocol/v1.rs` - legacy conversation APIs (v1).
- `codex-rs/app-server-protocol/src/protocol/thread_history.rs` - rebuild turn history from rollouts.
- `codex-rs/app-server-protocol/src/protocol/mappers.rs` - v1/v2 mapping helpers.
- `codex-rs/app-server-protocol/src/export.rs` - TypeScript + JSON Schema generation.
- `codex-rs/app-server-protocol/src/bin/export.rs` - standalone schema export binary.

## Thread/turn lifecycle

- Core thread manager: `codex-rs/core/src/thread_manager.rs`.
- Per-thread conduit: `codex-rs/core/src/codex_thread.rs`.
- Submission ops (`Op`): `codex-rs/protocol/src/protocol.rs`.
- app-server v2 handlers (`thread/*`, `turn/*`, `review/*`): `codex-rs/app-server/src/codex_message_processor.rs`.
- Default session source for app-server sessions: `SessionSource::VSCode` in
  `codex-rs/app-server/src/message_processor.rs`.
- Rollout summaries + resume helpers: `read_summary_from_rollout` and `read_event_msgs_from_rollout`
  in `codex-rs/app-server/src/codex_message_processor.rs`.

## Persistence and history

- Rollouts are JSONL files stored under `CODEX_HOME/sessions` and archived to
  `CODEX_HOME/archived_sessions`.
- Constants and paths: `codex-rs/core/src/rollout/mod.rs`.
- Rollout writing: `codex-rs/core/src/rollout/recorder.rs`.
- Rollout listing and lookup: `codex-rs/core/src/rollout/list.rs`.
- History to turns for resume: `codex-rs/app-server-protocol/src/protocol/thread_history.rs`.

## Event stream and notifications

- Event loop: `attach_conversation_listener` in `codex-rs/app-server/src/codex_message_processor.rs`
  consumes `CodexThread::next_event`.
- Raw event passthrough: `codex/event/*` notifications in the same file.
- Structured v2 notifications: `codex-rs/app-server/src/bespoke_event_handling.rs`.
  It maps core `EventMsg` values to `turn/*`, `item/*`, `item/*/delta`, `thread/tokenUsage/updated`,
  `turn/diff/updated`, `turn/plan/updated`, `contextCompacted`, and error notifications.
- Notification types and item schemas: `codex-rs/app-server-protocol/src/protocol/v2.rs`.

## Approvals and tool calls

- Core approval events: `ExecApprovalRequestEvent` and `ApplyPatchApprovalRequestEvent` in
  `codex-rs/protocol/src/approvals.rs` and `codex-rs/protocol/src/protocol.rs`.
- Server-initiated requests + response routing: `codex-rs/app-server/src/outgoing_message.rs`.
- Approval handling and item lifecycle: `codex-rs/app-server/src/bespoke_event_handling.rs`.
- Tool orchestration and runtimes: `codex-rs/core/src/tools`.

## Sandbox and command exec

- `command/exec` endpoint handler: `codex-rs/app-server/src/codex_message_processor.rs`.
- Execution engine: `codex-rs/core/src/exec.rs`.
- Sandbox policy and enforcement: `codex-rs/core/src/sandboxing/mod.rs`.
- Platform sandboxes:
  - `codex-rs/linux-sandbox`
  - `codex-rs/core/src/seatbelt.rs`
  - `codex-rs/core/src/windows_sandbox.rs`
  - `codex-rs/windows-sandbox-rs`
- Exec policy constraints: `codex-rs/core/src/exec_policy.rs`.

## Auth and account

- Auth manager: `codex-rs/core/src/auth.rs`.
- Auth storage and `auth.json`: `codex-rs/core/src/auth/storage.rs`.
- ChatGPT OAuth login server: `codex-rs/login/src/server.rs`.
- app-server auth endpoints (`account/*`): `codex-rs/app-server/src/codex_message_processor.rs`.
- User agent + originator handling: `codex-rs/core/src/default_client.rs`.

## Skills

- Skill manager and caching: `codex-rs/core/src/skills/manager.rs`.
- Skill loading/parsing: `codex-rs/core/src/skills/loader.rs`.
- System skill install location (`CODEX_HOME/skills/.system`): `codex-rs/core/src/skills/system.rs`.
- Skill injection into prompts: `codex-rs/core/src/skills/injection.rs`.
- Client input type + `skills/list` types: `codex-rs/app-server-protocol/src/protocol/v2.rs`.

## MCP integration

- MCP snapshot + tool grouping: `codex-rs/core/src/mcp/mod.rs`.
- MCP connection manager: `codex-rs/core/src/mcp_connection_manager.rs`.
- MCP OAuth login helper: `codex-rs/rmcp-client/src/perform_oauth_login.rs`.
- app-server endpoints (`mcpServerStatus/list`, `mcpServer/oauth/login`):
  `codex-rs/app-server/src/codex_message_processor.rs`.
- MCP server config schema: `codex-rs/core/src/config/types.rs`.

## Config APIs

- app-server config adapter: `codex-rs/app-server/src/config_api.rs`.
- Config read/write/edit: `codex-rs/core/src/config/service.rs` and
  `codex-rs/core/src/config/edit.rs`.
- Config schema + defaults: `codex-rs/core/src/config/mod.rs` and
  `codex-rs/core/src/config/types.rs`.
- Config layering + requirements: `codex-rs/core/src/config_loader/mod.rs` and
  `codex-rs/core/src/config_loader/config_requirements.rs`.
- Files on disk:
  - `CODEX_HOME/config.toml` (user config)
  - `/etc/codex/requirements.toml` (policy constraints on Unix)

## Models, git, file search, feedback

- `model/list`: `codex-rs/app-server/src/models.rs` and
  `codex-rs/core/src/models_manager/manager.rs`.
- Git info/diff helpers: `codex-rs/core/src/git_info.rs` and
  `codex-rs/app-server/src/codex_message_processor.rs`.
- Fuzzy file search: `codex-rs/app-server/src/fuzzy_file_search.rs` and `codex-rs/file-search`.
- Feedback upload: `codex-rs/app-server/src/codex_message_processor.rs` and `codex-rs/feedback`.

## Example clients and tests

- Minimal test client: `codex-rs/app-server-test-client/src/main.rs`.
- Interactive debug client: `codex-rs/debug-client/src/main.rs`.
- Integration tests: `codex-rs/app-server/tests`.

## Canonical wire-level reference

- `codex-rs/app-server/README.md` - full API guide and examples.
- `codex-rs/docs/codex_mcp_interface.md` - MCP interface notes (links to app-server auth docs).
