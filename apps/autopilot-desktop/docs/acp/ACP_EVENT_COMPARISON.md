# ACP vs Codex App-Server Event Comparison

**Date**: January 24, 2026  
**Analysis**: Comparison of events captured from `codex app-server` vs `codex-acp` (ACP protocol)

## Summary

After implementing dual logging for both protocols, we can now compare what events are captured by each:

### App-Server Events (Codex Direct JSON-RPC)
- **Total unique event types**: 28
- **File size**: 118KB (390 events in latest session)
- **Status**: ✅ Working - events flush on `turn/completed`

### ACP Events (via codex-acp)
- **Total unique event types**: 1 (`session/update`)
- **File size**: 2KB (3 events in older session)
- **Status**: ⚠️ Not flushing - completion detection needs fixing

## Missing Events in ACP

### High-Level Event Categories Missing:

1. **Connection/Initialization Events**
   - `codex/connected` - Workspace connection status
   - `codex/event/mcp_startup_complete` - MCP server initialization

2. **Thread/Turn Lifecycle Events**
   - `thread/started` - Thread creation with metadata (git info, model provider, etc.)
   - `turn/started` - Turn initiation
   - `turn/completed` - Turn completion (CRITICAL for completion detection)

3. **Item Lifecycle Events**
   - `item/started` - Item (message, reasoning, tool) started
   - `item/completed` - Item completion
   - `item/agentMessage/delta` - Streaming agent message chunks
   - `item/reasoning/summaryTextDelta` - Reasoning text deltas
   - `item/reasoning/summaryPartAdded` - Reasoning section breaks
   - `item/commandExecution/outputDelta` - Command execution output

4. **Codex-Specific Event Streams** (All `codex/event/*` methods)
   - `codex/event/agent_message_delta` - Agent message streaming
   - `codex/event/agent_message_content_delta` - Message content deltas
   - `codex/event/agent_reasoning` - Reasoning content
   - `codex/event/agent_reasoning_delta` - Reasoning streaming
   - `codex/event/agent_reasoning_section_break` - Reasoning section breaks
   - `codex/event/reasoning_content_delta` - Reasoning content deltas
   - `codex/event/item_started` - Item start notifications
   - `codex/event/item_completed` - Item completion notifications
   - `codex/event/task_started` - Task initiation
   - `codex/event/user_message` - User message echo
   - `codex/event/exec_command_begin` - Command execution start
   - `codex/event/exec_command_output_delta` - Command output streaming
   - `codex/event/exec_command_end` - Command execution completion
   - `codex/event/token_count` - Token usage updates

5. **Account/Usage Events**
   - `account/rateLimits/updated` - Rate limit status
   - `thread/tokenUsage/updated` - Token usage per thread

## Assessment: Should These Events Be Saved?

### ✅ **CRITICAL - Must Have for Full Functionality**

1. **Turn Lifecycle**: `turn/started`, `turn/completed`
   - **Why**: Essential for knowing when a conversation turn begins/ends
   - **ACP Equivalent**: ACP uses `stopReason: "end_turn"` in responses, but no explicit `turn/started`

2. **Item Streaming**: `item/agentMessage/delta`, `item/reasoning/summaryTextDelta`
   - **Why**: Required for real-time UI updates during streaming
   - **ACP Equivalent**: ACP has `SessionNotification` with `AgentMessageChunk` and `AgentThoughtChunk` (but we're not using the ACP library yet)

3. **Token Usage**: `thread/tokenUsage/updated`, `codex/event/token_count`
   - **Why**: Important for usage tracking and rate limit management
   - **ACP Equivalent**: Unknown - need to check ACP spec

### ⚠️ **IMPORTANT - Nice to Have**

4. **Command Execution**: `item/commandExecution/outputDelta`, `codex/event/exec_command_*`
   - **Why**: Useful for debugging and understanding agent actions
   - **ACP Equivalent**: ACP has `ToolCall` and `ToolCallUpdate` notifications (but we're not using them)

5. **Reasoning Details**: `item/reasoning/*`, `codex/event/agent_reasoning_*`
   - **Why**: Provides insight into agent thinking process
   - **ACP Equivalent**: ACP has `AgentThoughtChunk` in `SessionNotification`

6. **Connection Status**: `codex/connected`, `codex/event/mcp_startup_complete`
   - **Why**: Useful for debugging connection issues
   - **ACP Equivalent**: ACP has `initialize` response, but no explicit connection status

### ❌ **LOW PRIORITY - Can Skip**

7. **Codex-Specific Events**: Most `codex/event/*` methods
   - **Why**: These are Codex-specific and may not be needed if using ACP
   - **ACP Equivalent**: None - these are Codex extensions

## ACP Protocol Capabilities

### What ACP Provides (via `SessionNotification`):

According to the ACP specification and our assessment:

1. **Standardized Notifications**:
   - `AgentMessageChunk` - Streaming agent responses
   - `AgentThoughtChunk` - Streaming reasoning/thinking
   - `ToolCall` - Tool invocation started
   - `ToolCallUpdate` - Tool execution progress/results
   - `Plan` - Agent's execution plan
   - `UserMessageChunk` - Echo of user message

2. **Extensibility**:
   - ACP spec supports **custom methods and notifications** via `extMethod()` and `extNotification()`
   - Both agents and clients can expose custom methods
   - This allows protocol extensions without breaking compatibility

### What ACP Does NOT Provide:

1. **Codex-Specific Events**: All `codex/event/*` methods are Codex extensions
2. **Rate Limit Updates**: `account/rateLimits/updated` is Codex-specific
3. **Detailed Token Usage**: `thread/tokenUsage/updated` with breakdown is Codex-specific
4. **MCP Startup Events**: `codex/event/mcp_startup_complete` is Codex-specific

## What Events Are NOT Being Stored via ACP?

### Complete List (26 missing event types):

**Connection & Initialization**:
- `codex/connected` - Workspace connection status
- `codex/event/mcp_startup_complete` - MCP server initialization

**Thread/Turn Lifecycle**:
- `thread/started` - Thread creation with metadata
- `turn/started` - Turn initiation  
- `turn/completed` - Turn completion

**Item Lifecycle**:
- `item/started` - Item start
- `item/completed` - Item completion
- `item/agentMessage/delta` - Agent message streaming
- `item/reasoning/summaryTextDelta` - Reasoning text deltas
- `item/reasoning/summaryPartAdded` - Reasoning parts
- `item/commandExecution/outputDelta` - Command output

**Codex Event Streams** (17 types):
- All `codex/event/*` methods for detailed streaming and status updates

**Account/Usage**:
- `account/rateLimits/updated` - Rate limits
- `thread/tokenUsage/updated` - Token usage

## Should These Events Be Saved?

### ✅ **YES - Critical for Full Functionality**

1. **Turn Lifecycle** (`turn/started`, `turn/completed`)
   - **Why**: Essential for UI state management and completion detection
   - **ACP Status**: ACP uses `stopReason: "end_turn"` in responses (we're now detecting this)
   - **Action**: ✅ Fixed - now tracking request→session mapping

2. **Streaming Events** (`item/agentMessage/delta`, `item/reasoning/*`)
   - **Why**: Required for real-time UI updates
   - **ACP Status**: ACP has `AgentMessageChunk` and `AgentThoughtChunk` in `SessionNotification`
   - **Action**: ⚠️ Need to verify if `codex-acp` sends these notifications

3. **Token Usage** (`thread/tokenUsage/updated`, `codex/event/token_count`)
   - **Why**: Important for usage tracking and rate limit management
   - **ACP Status**: ❌ Not in ACP spec
   - **Action**: Use ACP extensions or keep dual protocol

### ⚠️ **MAYBE - Important for Debugging**

4. **Command Execution** (`item/commandExecution/*`, `codex/event/exec_command_*`)
   - **Why**: Useful for understanding agent actions
   - **ACP Status**: ACP has `ToolCall` and `ToolCallUpdate` notifications
   - **Action**: Verify if `codex-acp` sends these

5. **Connection Status** (`codex/connected`, `codex/event/mcp_startup_complete`)
   - **Why**: Useful for debugging
   - **ACP Status**: ❌ Not in ACP spec
   - **Action**: Use ACP extensions or keep dual protocol

### ❌ **NO - Codex-Specific, Can Skip**

6. **Codex Event Streams** (Most `codex/event/*` methods)
   - **Why**: Codex-specific implementation details
   - **ACP Status**: ❌ Not in ACP spec
   - **Action**: Only needed if staying with Codex app-server

## ACP Extensibility Options

### Option 1: Use ACP Extensions (Recommended for Missing Events)

**Use ACP's extensibility mechanism** to add custom notifications for missing events:

```rust
// Example: Send custom notification for token usage
connection.send_notification("codex/tokenUsage", json!({
    "sessionId": session_id,
    "tokenUsage": { /* ... */ }
}));
```

**Pros**:
- Stays within ACP protocol
- Other ACP clients can ignore unknown notifications
- Maintains protocol compatibility

**Cons**:
- Requires `codex-acp` to support these extensions
- May need to modify `codex-acp` adapter

### Option 2: Dual Protocol Support

**Keep both protocols active** and merge events:

- Use ACP for standardized agent communication
- Use `codex app-server` for Codex-specific events (rate limits, token usage, MCP status)

**Pros**:
- Get all events without modification
- No changes to `codex-acp` needed

**Cons**:
- More complex architecture
- Duplicate communication channels

### Option 3: Extend ACP Spec

**Propose extensions to ACP specification** for:
- Token usage tracking
- Rate limit notifications
- Connection status events

**Pros**:
- Benefits entire ACP ecosystem
- Standardized approach

**Cons**:
- Requires spec approval process
- Long-term solution

### Option 4: Use ACP Library Properly

**Migrate to using `agent-client-protocol` crate** instead of raw JSON-RPC:

- Use `SessionNotification` enum to receive standardized events
- Map ACP events to our internal event format
- Use `Client` trait for file/terminal operations

**Pros**:
- Proper ACP implementation
- Type-safe event handling
- Access to all ACP features

**Cons**:
- Requires significant refactoring
- Still need extensions for Codex-specific events

## Immediate Action Items

1. ✅ **Fix ACP completion detection** - Now tracks request→session mapping and detects `stopReason: "end_turn"`
2. ⚠️ **Verify ACP events are being captured** - Check if `SessionNotification` events (notifications without `id`) are coming through
3. ⚠️ **Compare event coverage** - Once ACP is working, compare what's actually available
4. ⚠️ **Decide on extension strategy** - Choose one of the options above

## Technical Implementation Status

### ✅ Completed:
- Request ID → Session ID tracking for proper event flushing
- `stopReason: "end_turn"` detection in responses  
- Session ID extraction from multiple locations
- File logging with buffering and completion-based flushing
- Fixed completion detection to flush all events when `stopReason: "end_turn"` is detected

### ⚠️ In Progress:
- Verifying if `SessionNotification` events are being sent by `codex-acp`
- Testing completion detection with actual message streams

### 🔜 Next Steps:
1. Test the new completion detection with a real message stream
2. Analyze what notifications `codex-acp` actually sends (check for notifications without `id`)
3. Determine if we need to use ACP library to decode `SessionNotification` properly
4. Decide on extension strategy for missing events

## Final Assessment & Recommendations

### What We Know:

1. **App-Server Events**: 27 unique event types, all working perfectly
2. **ACP Events**: Only seeing `session/update` notifications currently
3. **Missing Events**: 26 event types from app-server are not appearing in ACP logs

### Critical Questions:

1. **Are `SessionNotification` events being sent?**
   - ACP spec says agents send `SessionNotification` (notifications without `id`)
   - We should see `AgentMessageChunk`, `AgentThoughtChunk`, `ToolCall`, etc.
   - **Action**: Check ACP logs for notifications (messages without `id` field)

2. **Does `codex-acp` send these notifications?**
   - `codex-acp` is an adapter - it should translate Codex events to ACP notifications
   - If it's not sending them, we need to either:
     - Modify `codex-acp` to send them
     - Use dual protocol (keep app-server for events)
     - Use ACP extensions

3. **Can we use ACP extensions for missing events?**
   - ✅ YES - ACP spec explicitly supports `extNotification()` for custom notifications
   - We can define custom notifications like `codex/tokenUsage`, `codex/rateLimits`, etc.
   - **Requirement**: `codex-acp` needs to send these extensions

### Recommended Path Forward:

**Short-term (Immediate)**:
1. ✅ Fix completion detection (DONE - now tracks request→session and detects `stopReason`)
2. Test completion detection with real message stream
3. Analyze ACP logs for any notifications we're missing

**Medium-term (Next Phase)**:
1. **Option A - Dual Protocol** (Easiest):
   - Keep both `codex app-server` and `codex-acp` running
   - Use app-server for Codex-specific events (rate limits, token usage, MCP status)
   - Use ACP for standardized agent communication
   - Merge events in UI

2. **Option B - ACP Extensions** (Best long-term):
   - Work with `codex-acp` maintainers to add extensions for:
     - `codex/tokenUsage` - Token usage updates
     - `codex/rateLimits` - Rate limit status  
     - `codex/connectionStatus` - Connection events
   - Use ACP as primary protocol
   - Gradually migrate away from app-server

**Long-term (Future)**:
1. Propose ACP spec extensions for token usage and rate limits (benefits entire ecosystem)
2. Full migration to ACP with proper `SessionNotification` handling
3. Remove dependency on `codex app-server` entirely

### Conclusion:

**Current State**: ACP is capturing raw JSON-RPC but we're only seeing `session/update`. The missing events fall into two categories:

1. **ACP-Equivalent Events** (Should be available via `SessionNotification`):
   - Streaming messages, reasoning, tool calls
   - **Action**: Verify if `codex-acp` sends these, or use ACP library to decode them

2. **Codex-Specific Events** (Not in ACP spec):
   - Rate limits, detailed token usage, MCP status, Codex event streams
   - **Action**: Use ACP extensions (`extNotification`) or keep dual protocol

**Recommendation**: Start with **Dual Protocol** (Option A) to get all events immediately, then gradually migrate to **ACP Extensions** (Option B) as `codex-acp` adds support.

## Current Status

- ✅ App-server events: Working perfectly (390 events logged, 27 unique event types)
- ⚠️ ACP events: Only capturing `session/update` (completion detection fixed - now tracks request→session mapping)
- 🔧 Next: Test with new completion detection, then reassess event coverage

## Critical Discovery: ACP Event Model is Different

**Key Finding**: ACP doesn't send the same granular events as Codex app-server. Instead:

1. **ACP uses `SessionNotification` notifications** - These are JSON-RPC notifications (no `id` field) sent by the agent
2. **Responses include `stopReason`** - Completion is indicated in the response to `session/prompt`, not a separate event
3. **We're only seeing `session/update`** - This suggests `codex-acp` may not be sending all `SessionNotification` types, OR we're not capturing them properly

**The Real Question**: Are `SessionNotification` events (like `AgentMessageChunk`, `AgentThoughtChunk`) being sent by `codex-acp` but we're not capturing them? Or does `codex-acp` only send `session/update`?

**What We Need to Check**:
- Look for JSON-RPC notifications (no `id` field) in ACP logs
- Check if `codex-acp` sends `SessionNotification` events at all
- Verify if we need to use the ACP library to properly decode notifications

## Event Coverage Analysis

### App-Server Events (27 unique types):
1. `account/rateLimits/updated` - Rate limit status
2. `codex/connected` - Connection status
3. `codex/event/agent_message_content_delta` - Message content streaming
4. `codex/event/agent_message_delta` - Message streaming
5. `codex/event/agent_reasoning` - Reasoning content
6. `codex/event/agent_reasoning_delta` - Reasoning streaming
7. `codex/event/agent_reasoning_section_break` - Reasoning sections
8. `codex/event/exec_command_begin` - Command start
9. `codex/event/exec_command_end` - Command end
10. `codex/event/exec_command_output_delta` - Command output streaming
11. `codex/event/item_completed` - Item completion
12. `codex/event/item_started` - Item start
13. `codex/event/mcp_startup_complete` - MCP initialization
14. `codex/event/reasoning_content_delta` - Reasoning content deltas
15. `codex/event/task_started` - Task initiation
16. `codex/event/token_count` - Token usage
17. `codex/event/user_message` - User message echo
18. `item/agentMessage/delta` - Agent message streaming
19. `item/commandExecution/outputDelta` - Command output
20. `item/completed` - Item completion
21. `item/reasoning/summaryPartAdded` - Reasoning parts
22. `item/reasoning/summaryTextDelta` - Reasoning text
23. `item/started` - Item start
24. `thread/started` - Thread creation
25. `thread/tokenUsage/updated` - Token usage per thread
26. `turn/completed` - Turn completion
27. `turn/started` - Turn start

### ACP Events (1 unique type):
1. `session/update` - Session status updates (available commands, etc.)

### Missing in ACP (26 event types):
All of the above except `session/update` are missing from ACP logs.

## Critical Finding: ACP Uses Different Event Model

**Key Insight**: ACP doesn't use the same event-based notification system as Codex app-server. Instead:

1. **ACP uses `SessionNotification` enum** - Standardized notification types
2. **Responses include `stopReason`** - Completion is indicated in responses, not separate events
3. **Streaming via notifications** - `AgentMessageChunk`, `AgentThoughtChunk`, etc.

**The Problem**: We're currently capturing raw JSON-RPC from `codex-acp`, but we're not:
- Parsing `SessionNotification` types
- Using the ACP library to decode events
- Mapping ACP notifications to equivalent Codex events

**What We're Missing**: All the `SessionNotification` events that `codex-acp` is sending, because we're only seeing the raw JSON-RPC layer, not the decoded ACP protocol layer.
