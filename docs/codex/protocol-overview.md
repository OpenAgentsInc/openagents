# Protocol Overview

Defines the event and data contracts used between the core logic and frontends
(TUI/CLI), and for persisted rollouts.

File: `codex-rs/protocol/src/protocol.rs`

## Highlights

- Event enums for streaming: `AgentMessageDeltaEvent`, `AgentReasoningDeltaEvent`,
  `ExecCommandOutputDeltaEvent`, etc.
- `ResponseItem` variants for messages, tool calls, custom tool calls, web
  search requests, and function call outputs.
- Rollout items (`RolloutItem`, `RolloutLine`) and session metadata.
- Rate limit snapshot structure (`RateLimitSnapshotEvent`).

The protocol is intentionally stable and serde‑serializable to JSON so it can be
persisted and consumed across processes.

