# Codex Exec JSONL Schema

When you run `codex exec --json`, Codex streams one JSON object per line (JSONL). This document defines that external JSON shape, where the types live in the codebase, and how internal protocol events map to the JSONL stream.

## Where Definitions Live

- CLI JSON types (the schema Codex prints): `codex-rs/exec/src/exec_events.rs:1`
- Mapper (protocol → CLI JSONL): `codex-rs/exec/src/event_processor_with_jsonl_output.rs:1`
- Upstream internal protocol (source events consumed by the mapper):
  - Envelope: `codex-rs/protocol/src/protocol.rs:409`
  - Variants: `codex-rs/protocol/src/protocol.rs:422`

The CLI JSON types derive `serde::{Serialize, Deserialize}` and `ts_rs::TS`, enabling both Rust and TypeScript bindings that match the JSON shape.

## Top-Level Event Envelope

Each line is a `ThreadEvent` tagged by a `type` string. The full set of top‑level events:

- `"thread.started"` — first event; identifies the thread.
- `"turn.started"` — a user turn began.
- `"turn.completed"` — turn finished successfully; includes token usage.
- `"turn.failed"` — turn finished with an error; includes error object.
- `"item.started"` — a new item began (e.g., command, MCP call, plan).
- `"item.updated"` — an item was updated (e.g., plan progress).
- `"item.completed"` — an item reached a terminal state (success/failure).
- `"error"` — unrecoverable error emitted directly by the stream.

Rust: `codex-rs/exec/src/exec_events.rs:1` (`enum ThreadEvent` with `#[serde(tag = "type")]`).

## Event Payloads (by `type`)

- thread.started
  - Rust: `ThreadStartedEvent`
  - JSON: `{ "type": "thread.started", "thread_id": string }`

- turn.started
  - Rust: `TurnStartedEvent` (empty)
  - JSON: `{ "type": "turn.started" }`

- turn.completed
  - Rust: `TurnCompletedEvent { usage: Usage }`
  - JSON: `{ "type": "turn.completed", "usage": { "input_tokens": number, "cached_input_tokens": number, "output_tokens": number } }`

- turn.failed
  - Rust: `TurnFailedEvent { error: ThreadErrorEvent }`
  - JSON: `{ "type": "turn.failed", "error": { "message": string } }`

- error
  - Rust: `ThreadErrorEvent`
  - JSON: `{ "type": "error", "message": string }`

- item.started | item.updated | item.completed
  - Rust: `ItemStartedEvent | ItemUpdatedEvent | ItemCompletedEvent` each with `item: ThreadItem`
  - JSON: `{ "type": "item.started" | "item.updated" | "item.completed", "item": ThreadItem }`

### ThreadItem

- Rust: `ThreadItem { id: String, details: ThreadItemDetails }`
- JSON: `{ "id": string, ...details... }` (the `details` enum is flattened)

`id` is assigned by the CLI and is unique within the stream (monotonically increasing like `item_0`, `item_1`, ...).

### ThreadItemDetails Variants

`ThreadItemDetails` uses `#[serde(tag = "type", rename_all = "snake_case")]` so the `item` object has its own `type` field.

- agent_message
  - Rust: `AgentMessageItem { text: String }`
  - JSON: `{ "type": "agent_message", "text": string }`

- reasoning
  - Rust: `ReasoningItem { text: String }`
  - JSON: `{ "type": "reasoning", "text": string }`

- command_execution
  - Rust: `CommandExecutionItem { command: String, aggregated_output: String, exit_code: Option<i32>, status: CommandExecutionStatus }`
  - JSON (in progress):
    ```json
    {"type":"command_execution","command":"echo hello","aggregated_output":"","status":"in_progress"}
    ```
  - JSON (completed):
    ```json
    {"type":"command_execution","command":"echo hello","aggregated_output":"hello\n","exit_code":0,"status":"completed"}
    ```
  - Status: `CommandExecutionStatus` ∈ `"in_progress" | "completed" | "failed"`

- file_change
  - Rust: `FileChangeItem { changes: Vec<FileUpdateChange>, status: PatchApplyStatus }`
  - Rust: `FileUpdateChange { path: String, kind: PatchChangeKind }`
  - JSON:
    ```json
    {"type":"file_change","changes":[{"path":"src/main.rs","kind":"update"}],"status":"completed"}
    ```
  - Status: `PatchApplyStatus` ∈ `"completed" | "failed"`
  - Change kind: `PatchChangeKind` ∈ `"add" | "delete" | "update"`

- mcp_tool_call
  - Rust: `McpToolCallItem { server: String, tool: String, status: McpToolCallStatus }`
  - JSON:
    ```json
    {"type":"mcp_tool_call","server":"my-server","tool":"do_thing","status":"in_progress"}
    ```
  - Status: `McpToolCallStatus` ∈ `"in_progress" | "completed" | "failed"`

- web_search
  - Rust: `WebSearchItem { query: String }`
  - JSON: `{ "type": "web_search", "query": string }`

- todo_list
  - Rust: `TodoListItem { items: Vec<TodoItem> }` with `TodoItem { text: String, completed: bool }`
  - JSON:
    ```json
    {"type":"todo_list","items":[{"text":"step one","completed":false}]}
    ```

- error
  - Rust: `ErrorItem { message: String }`
  - JSON: `{ "type": "error", "message": string }`

### Usage

- Rust: `Usage { input_tokens: i64, cached_input_tokens: i64, output_tokens: i64 }`
- Emitted in `turn.completed`.

## Mapping From Internal Protocol Events

Implemented in `codex-rs/exec/src/event_processor_with_jsonl_output.rs:1`. This file translates `codex-rs/protocol/src/protocol.rs:422` `EventMsg` variants into the JSONL events described above.

- `SessionConfigured` → emit `thread.started`
  - Output: `ThreadStartedEvent { thread_id: session_id }`

- `TaskStarted` → emit `turn.started`
  - Output: `TurnStartedEvent {}`

- `AgentMessage` → emit `item.completed` with `AgentMessageItem`

- `AgentReasoning` → emit `item.completed` with `ReasoningItem`

- `ExecCommandBegin` → emit `item.started` with `CommandExecutionItem` (`status = in_progress`)
  - Mapper tracks by `call_id` to join begin/end.

- `ExecCommandEnd` → emit `item.completed` with `CommandExecutionItem`
  - `status = completed` if `exit_code == 0`, else `failed`.
  - If a matching `begin` was not seen, the mapper logs a warning and skips emission.

- `McpToolCallBegin` → emit `item.started` with `McpToolCallItem` (`status = in_progress`)

- `McpToolCallEnd` → emit `item.completed` with `McpToolCallItem`
  - `status = completed` if `result` is not an error; otherwise `failed`.
  - If a matching `begin` was not seen, the mapper synthesizes a new item id and emits completion.

- `PatchApplyBegin` → tracked only (no immediate output).

- `PatchApplyEnd` → emit `item.completed` with `FileChangeItem`
  - `status = completed | failed` based on `success`.
  - `changes` are mapped from the recorded `begin` payload (`Add/Delete/Update`).

- `WebSearchEnd` → emit `item.completed` with `WebSearchItem { query }`

- `PlanUpdate(UpdatePlanArgs)` →
  - First occurrence: `item.started` with `TodoListItem`.
  - Subsequent occurrences in the same turn: `item.updated` with updated list.
  - On turn end, any running todo_list emits `item.completed`.

- `TokenCount` → not emitted directly; captured to populate `turn.completed.usage`.

- `Error` → emit `error` immediately and record as last critical error.

- `StreamError` → emit `error` (does not flip the turn into failed by itself).

- `TaskComplete` → end of turn sentinel
  - If a critical `Error` was recorded during the turn: emit `turn.failed` with that error.
  - Otherwise: emit `turn.completed` with `usage` (or zeroed `Usage` if unknown).

Ignored: `ExecCommandOutputDelta`, `WebSearchBegin`, approval requests, review‑mode and some list/response meta events (not part of the CLI JSONL contract).

## Example Transcript

```json
{"type":"thread.started","thread_id":"67e55044-10b1-426f-9247-bb680e5fe0c8"}
{"type":"turn.started"}
{"type":"item.started","item":{"id":"item_0","type":"command_execution","command":"echo hello","aggregated_output":"","status":"in_progress"}}
{"type":"item.completed","item":{"id":"item_0","type":"command_execution","command":"echo hello","aggregated_output":"hello\n","exit_code":0,"status":"completed"}}
{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"Done."}}
{"type":"turn.completed","usage":{"input_tokens":123,"cached_input_tokens":0,"output_tokens":45}}
```

## Notes and Guarantees

- Numbers are signed (`i64` for token counts, `i32` for `exit_code`) and serialized as JSON numbers.
- `exit_code` is omitted while a command is in progress.
- `ThreadItem.id` is generated client‑side and is unique within the stream.
- The wire format is stable; if you change `exec_events.rs`, please update this document.

## Quick Pointers for Implementers

- Types: `codex-rs/exec/src/exec_events.rs:1`
- Mapper: `codex-rs/exec/src/event_processor_with_jsonl_output.rs:1`
- Protocol (source): `codex-rs/protocol/src/protocol.rs:409` and `codex-rs/protocol/src/protocol.rs:422`
