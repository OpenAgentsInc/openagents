# Provider Integrations (Codex, OpenCode, Claude)

This folder documents three provider adapters and proposes a single, unified event schema and storage model compatible with our Convex backend. It also includes Rust and TypeScript type definitions for the canonical wire format and guidance on where to define the source of truth.

## Contents

- `opencode.md` — OpenCode server + SSE integration (planned)
- `claude-code.md` — Claude Code headless CLI integration (planned)

## Unifying Strategy

- Canonical contract: emit ACP-compliant updates and Tinyvex typed rows from the bridge; the app consumes generated TS types under `expo/types/bridge/*`.
- Adapter model:
  - Codex: pass‑through (already emits canonical JSONL).
  - OpenCode: subscribe to `/event` SSE, map Bus events → canonical items.
  - Claude Code: parse `--output-format stream-json`, map to canonical items.
- App/UI: uses typed ACP components (`expo/components/acp/*`) and Tinyvex provider (`expo/providers/tinyvex.tsx`).

## Canonical Event Schema

Top‑level events (one per line):
- `thread.started { thread_id }`
- `turn.started {}`
- `turn.completed { usage }`
- `turn.failed { error }`
- `item.started|item.updated|item.completed { item }`
- `error { message }`

Items (union):
- `agent_message { text }`
- `reasoning { text }`
- `command_execution { command, aggregated_output, exit_code?, status }`
- `file_change { changes: [{ path, kind }], status }`
- `mcp_tool_call { server, tool, status }`
- `web_search { query }`
- `todo_list { items: [{ text, completed }] }`

### Rust types (source of truth)

We propose defining canonical types in Rust in `crates/oa-bridge/src/events.rs` using `serde`, `schemars`, and `ts_rs` so we can derive JSON Schema and generate TypeScript automatically.

```rust
// crates/oa-bridge/src/events.rs
use serde::{Serialize, Deserialize};
use ts_rs::TS;
use schemars::JsonSchema;

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS, Clone)]
#[ts(export, export_to = "../../expo/lib/generated/")]
#[serde(tag = "type")]
pub enum ThreadEvent {
    #[serde(rename = "thread.started")] ThreadStarted { thread_id: String },
    #[serde(rename = "turn.started")]   TurnStarted {},
    #[serde(rename = "turn.completed")] TurnCompleted { usage: Usage },
    #[serde(rename = "turn.failed")]    TurnFailed { error: ThreadError },
    #[serde(rename = "item.started")]   ItemStarted { item: ThreadItem },
    #[serde(rename = "item.updated")]   ItemUpdated { item: ThreadItem },
    #[serde(rename = "item.completed")] ItemCompleted { item: ThreadItem },
    #[serde(rename = "error")]          Error { message: String },
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ThreadItem {
    AgentMessage { id: String, text: String },
    Reasoning    { id: String, text: String },
    CommandExecution { id: String, command: String, aggregated_output: String, exit_code: Option<i32>, status: CommandStatus },
    FileChange { id: String, changes: Vec<FileUpdateChange>, status: PatchApplyStatus },
    McpToolCall { id: String, server: String, tool: String, status: ToolCallStatus },
    WebSearch { id: String, query: String },
    TodoList { id: String, items: Vec<TodoItem> },
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS, Clone)]
#[serde(rename_all = "snake_case")]
pub enum CommandStatus { InProgress, Completed, Failed }

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS, Clone)]
#[serde(rename_all = "snake_case")]
pub enum PatchApplyStatus { Completed, Failed }

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus { InProgress, Completed, Failed }

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS, Clone)]
pub struct FileUpdateChange { pub path: String, pub kind: PatchChangeKind }

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS, Clone)]
#[serde(rename_all = "snake_case")]
pub enum PatchChangeKind { Add, Delete, Update }

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS, Clone)]
pub struct TodoItem { pub text: String, pub completed: bool }

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS, Clone)]
pub struct Usage { pub input_tokens: i64, pub cached_input_tokens: i64, pub output_tokens: i64 }

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS, Clone)]
pub struct ThreadError { pub message: String }
```

- Generation:
  - Rust → TS: `ts_rs` will emit `ThreadEvent.d.ts` into `expo/lib/generated/` on build.
  - Rust → JSON Schema: `schemars` can emit for Convex validation if needed.

### TypeScript types (generated consumer)

The app and Convex can consume the generated TS types. For clarity, here is the equivalent manual form used by Convex schemas and renderers:

```ts
// expo/lib/generated/ThreadEvent.d.ts (generated) or a local shim
export type ThreadEvent =
  | { type: 'thread.started'; thread_id: string }
  | { type: 'turn.started' }
  | { type: 'turn.completed'; usage: Usage }
  | { type: 'turn.failed'; error: ThreadError }
  | { type: 'item.started'; item: ThreadItem }
  | { type: 'item.updated'; item: ThreadItem }
  | { type: 'item.completed'; item: ThreadItem }
  | { type: 'error'; message: string };

export type ThreadItem =
  | { type: 'agent_message'; id: string; text: string }
  | { type: 'reasoning'; id: string; text: string }
  | { type: 'command_execution'; id: string; command: string; aggregated_output: string; exit_code?: number; status: 'in_progress' | 'completed' | 'failed' }
  | { type: 'file_change'; id: string; changes: { path: string; kind: 'add' | 'delete' | 'update' }[]; status: 'completed' | 'failed' }
  | { type: 'mcp_tool_call'; id: string; server: string; tool: string; status: 'in_progress' | 'completed' | 'failed' }
  | { type: 'web_search'; id: string; query: string }
  | { type: 'todo_list'; id: string; items: { text: string; completed: boolean }[] };

export type Usage = { input_tokens: number; cached_input_tokens: number; output_tokens: number };
export type ThreadError = { message: string };
```

## Convex Ingest Model (proposal)

Normalize provider events into these tables (TS types reference the canonical types above):
- `sessions` — `{ id: string, provider: 'codex'|'opencode'|'claude', createdAt: number }`
- `turns` — `{ id: string, sessionId: string, startedAt: number, completedAt?: number, usage?: Usage, error?: ThreadError }`
- `items` — `{ id: string, sessionId: string, turnId: string, kind: ThreadItem['type'], payload: ThreadItem }`
- `logs` (optional) — append raw lines for auditing or replay.

Mapping is straightforward:
- `thread.started` opens/associates a `sessions` row.
- `turn.started` opens a `turns` row; `turn.completed|failed` closes it with usage/error.
- `item.*` upserts into `items` by `item.id` and status transitions.

## Adapter Notes

- Codex: passthrough JSONL.
- OpenCode: SSE → canonical; emit `thread.started` on connect, item/turn mapping per `opencode.md`.
- Claude: stream‑JSON → canonical; tool_use → command_execution, tool_result → completion.

## Source of Truth Location

- Rust canonical types live in `crates/oa-bridge/src/events.rs` (or `crates/shared-events/` if we split later).
- TS types are generated into `expo/lib/generated/` for app and Convex to import.
- Optionally emit JSON Schema to `crates/oa-bridge/schemas/bridge-events.schema.json` for validation.

## Next Steps

- Add the Rust module and generation step (CI) to keep TS/Schema in sync.
- Implement OpenCode and Claude adapters that emit only canonical events.
- Wire Convex ingestion to the canonical event stream.
