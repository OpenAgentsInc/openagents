# System Architecture

This document gives a high‑level map of how Codex is organized, how a user’s
input flows through the system, where tools and sandboxes fit, and which crates
own which responsibilities.

## Workspace layout (Rust)

- `codex-rs/core` — business logic: prompt assembly, streaming, tool calls,
  approvals, sandbox selection, conversation history, rollouts, compaction.
- `codex-rs/cli` — CLI frontends, flags, and glue to `codex-core`.
- `codex-rs/tui` — terminal UI. Renders streams and handles approvals.
- `codex-rs/linux-sandbox` — Landlock/seccomp helper for Linux.
- `codex-rs/execpolicy` — Platform policies for what tools can do.
- `codex-rs/file-search` — Fast text search & candidate ranking for files.
- `codex-rs/apply-patch` — The safe patch format and CLI used by the agent.
- `codex-rs/protocol` — Serializable events and data structures (shared).
- `codex-rs/mcp-*` — Model Context Protocol client/server/types.

## End‑to‑end request lifecycle

1. CLI/TUI builds `Config` and starts a session (core `Config` in
   `core/src/config.rs`).
2. A submission creates a `Prompt` (context, tools, instructions) in
   `core/src/client_common.rs`.
3. `ModelClient` (`core/src/client.rs`) selects wire API (Responses vs Chat),
   builds payload, streams SSE/JSON, and adapts events to internal
   `ResponseEvent`s.
4. `codex.rs` orchestrates: consumes `ResponseEvent`s, dispatches tool calls,
   streams output, and records rollouts (`core/src/codex.rs`).
5. Tool calls go through `core/src/exec.rs` (and `exec_command.rs`), which
   picks a sandbox (Seatbelt on macOS, Landlock on Linux) and executes the
   command with policies from `execpolicy`.
6. File edits use `apply_patch` (freeform grammar or function tool), emitting
   diffs via `TurnDiffTracker`.
7. History/rollouts persist to `~/.codex/sessions` via `RolloutRecorder`.

## Data contracts

- Protocol events live in `codex-rs/protocol/src/protocol.rs` and define the
  cross‑process stream between UI and core logic.
- Tool shape definitions live in `core/src/openai_tools.rs`. A single list of
  tools is adapted into Responses API or Chat Completions format.

## Streaming model support

- Responses API: incremental `response.output_*` SSE events mapped to
  `ResponseEvent::{OutputItemDone, OutputTextDelta, …}`.
- Chat Completions: aggregated per‑turn stream, wrapped to look like Responses
  API for the rest of the pipeline.

## Safety layers

- Approval policy: when to ask vs run automatically.
- Platform sandbox: Seatbelt (macOS) / Landlock+seccomp (Linux).
- Exec output truncation and delta stream limits.

## Persistence

- Rollouts (.jsonl) with session meta and items (`core/src/rollout/`).
- Optional `history.jsonl` (see `docs/config.md` for `history` settings).

---
See also:
- `docs/systems/sandbox.md` for sandboxes and bypass options.
- `docs/systems/prompts.md` for prompt composition and caching.
- `docs/systems/protocol-overview.md` for event/data contracts.

