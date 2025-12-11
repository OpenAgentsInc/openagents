# Plan Mode Research: Zed (ACP) vs Claude Code SDK

**Date:** 2024-12-11

## Summary

Investigated how Zed implements plan mode via ACP (Agent Client Protocol) and how Claude Code SDK handles task planning via the TodoWrite tool.

## Key Findings

### Zed Uses ACP with Dedicated Plan Messages

Zed uses the `agent-client-protocol` crate (v0.9.0) which provides:

1. **Dedicated SessionUpdate::Plan message type**
2. **PlanEntry structure:**
   ```rust
   pub struct PlanEntry {
       pub content: String,        // Task description
       pub priority: PlanEntryPriority, // high/medium/low
       pub status: PlanEntryStatus,     // pending/in_progress/completed
   }
   ```

3. **UI in `acp_thread.rs`:**
   - `Plan` struct with `Vec<PlanEntry>`
   - `PlanStats` for progress tracking
   - `update_plan()` method handles `SessionUpdate::Plan`

4. **UI in `thread_view.rs`:**
   - Icons: TodoProgress (spinning), TodoComplete (checkmark), TodoPending
   - Strikethrough styling for completed items
   - Progress display (X pending, Y completed)

### Claude Code Uses TodoWrite Tool

Per official SDK docs at platform.claude.com:

1. **TodoWrite is a tool_use block** in assistant messages
2. **Todo structure:**
   ```typescript
   {
     content: string,      // Imperative: "Run tests"
     activeForm: string,   // Progressive: "Running tests"
     status: "pending" | "in_progress" | "completed"
   }
   ```
3. **Complete replacement** - each call sends ALL todos
4. **No priority field** (unlike ACP)

### Our SDK Status

From `crates/claude_agent_sdk/src/protocol/control.rs`:
- `PermissionMode::Plan` exists for permission control
- No dedicated plan/todo message types
- Todo updates come as tool_use blocks in assistant messages

## Implementation Plan for MechaCoder

### Phase 1: Extract TodoWrite (Minimal)
- Detect `tool_use` blocks with `name: "TodoWrite"` in stream
- Add `SdkUpdate::TodoUpdate { todos: Vec<Todo> }` variant
- Store todos in `SdkThread` state

### Phase 2: UI Integration
- Display todo list in MechaCoder panel
- Progress bar (completed/total)
- Status icons matching Zed's approach

### Phase 3: Interactive Plan Mode (Future)
- User approval before execution
- Leverage existing `PermissionMode::Plan`

## Comparison Table

| Feature | Zed (ACP) | Claude Code SDK |
|---------|-----------|-----------------|
| Protocol | `SessionUpdate::Plan` | Tool use (`TodoWrite`) |
| Updates | Streaming session updates | Assistant message blocks |
| Priority | `high/medium/low` | Not supported |
| Active form | N/A | `activeForm` field |

## Sources

- https://agentclientprotocol.com/protocol/agent-plan
- https://platform.claude.com/docs/en/agent-sdk/todo-tracking
- https://crates.io/crates/agent-client-protocol
- Zed source: `crates/acp_thread/src/acp_thread.rs`
- Zed source: `crates/agent_ui/src/acp/thread_view.rs`
