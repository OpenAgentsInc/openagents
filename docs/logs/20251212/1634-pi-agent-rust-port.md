# Pi Coding Agent - Rust Port

**Date:** 2024-12-12
**Time:** 16:34

## Summary

Ported the Pi coding agent from `~/code/pi-mono` (TypeScript) to Rust, creating `crates/pi/` with full feature parity for the core agent runtime.

## Changes Made

### New Crate: `crates/pi/`

Created a complete Pi agent implementation in Rust:

| File | Description |
|------|-------------|
| `Cargo.toml` | Dependencies: llm, sessions, tokio, async-stream, similar |
| `src/lib.rs` | Public API exports |
| `src/agent.rs` | Core agentic loop with streaming responses |
| `src/config.rs` | PiConfig, RetryConfig, OverflowStrategy |
| `src/context.rs` | Context management, token budgeting, overflow handling |
| `src/cost.rs` | Per-message cost tracking by model (Claude, GPT, Grok) |
| `src/error.rs` | PiError types with `is_retryable()` checks |
| `src/events.rs` | AgentEvent enum for streaming UI updates |
| `src/hooks.rs` | PiHook trait + LoggingHook, FileTrackingHook, CostLimitHook |
| `src/prompt.rs` | System prompt builder with CLAUDE.md/AGENTS.md loading |
| `src/state.rs` | AgentState machine (Idle, Thinking, Streaming, etc.) |
| `src/tool_executor.rs` | ToolRegistry with BashTool, ReadTool, WriteTool, EditTool |

### Sessions Crate Updates

Added file-based persistence to `crates/sessions/`:

- `src/file_store.rs` - `FileSessionStore` with JSONL persistence
- Session compaction support for context overflow
- Async file I/O with tokio

### MechaCoder Integration

Added Pi as an alternative agent in MechaCoder:

- `src/pi_thread.rs` - `PiThread` mirrors `SdkThread` interface
- Events: `PiThreadEvent` for GPUI integration
- Shared types with `SdkThread` (ThreadEntry, ThreadStatus, TodoItem)

## Key Features Ported from pi-mono

1. **Streaming agent runtime** - `PiAgent::run()` returns `Stream<Item = AgentEvent>`
2. **Tool execution with abort** - CancellationToken support via `tokio::select!`
3. **Tail truncation** - 50KB/2000 lines limit like pi-mono
4. **Cost tracking** - Per-turn and cumulative cost by model
5. **Session persistence** - JSONL files with compaction
6. **Hooks system** - Extensible via `PiHook` trait
7. **Context management** - Token budgeting, truncate/summarize strategies
8. **System prompt builder** - Git status, file tree, CLAUDE.md loading

## Tools Implemented

| Tool | Features |
|------|----------|
| `BashTool` | Command execution, timeout, streaming output, abort support |
| `ReadTool` | File reading with line range, tail truncation |
| `WriteTool` | File creation/overwrite |
| `EditTool` | Search/replace with diff generation |

## Tests

17 tests pass covering:
- Context management and token estimation
- Cost calculation and pricing
- Hook registry and built-in hooks
- Prompt building

## Files Modified

- `Cargo.toml` - Added `crates/pi` to workspace members
- `crates/sessions/Cargo.toml` - Added tokio, thiserror, async-trait
- `crates/sessions/src/lib.rs` - Export file_store module
- `crates/mechacoder/Cargo.toml` - Added pi dependency
- `crates/mechacoder/src/lib.rs` - Export pi_thread module

## Architecture

```
User Prompt
     │
     ▼
┌─────────┐
│ PiAgent │◄─────────────────────────────┐
│ Loop    │                              │
└────┬────┘                              │
     │                                   │
     ▼                                   │
┌─────────┐    ┌─────────┐    ┌────────┐│
│ Stream  │───▶│ Process │───▶│Execute ││
│ LLM     │    │ Response│    │ Tools  ││
└─────────┘    └─────────┘    └───┬────┘│
                                  │     │
                                  │     │
                                  ▼     │
                             Has more   │
                             tool calls?│
                                  │     │
                             Yes──┘     │
                             No───▶ Done│
```

## Next Steps

- Add UI for selecting Pi vs Claude Code in MechaCoder
- Add more tools (grep, find, glob)
- Implement full summarization for context overflow
- Add session resume functionality
