# Implementation Details

## Current Implementation

This document describes the current implementation of the unified agent architecture in Autopilot.

## Backend Implementation

### Agent Module Structure

The agent module (`apps/autopilot-desktop/src-tauri/src/agent/`) provides a unified interface for all AI agents:

```
agent/
├── mod.rs              # Module exports and public API
├── unified.rs          # Unified types (AgentId, UnifiedEvent, UnifiedConversationItem)
├── trait_def.rs        # Agent trait - unified interface
├── acp_agent.rs        # ACP agent implementation (wraps codex-acp)
├── codex_agent.rs      # Legacy Codex agent (deprecated)
├── gemini_agent.rs     # Gemini CLI ACP agent
├── adjutant/           # DSPy-native Adjutant agent
├── manager.rs          # AgentManager - manages multiple agents
└── commands.rs         # Tauri commands for frontend
```

### Adjutant Plan Mode Pipeline (`agent/adjutant/`)

Plan mode is implemented as a DSPy signature pipeline with an attached optimization loop:

- `planning.rs`: `PlanModePipeline` orchestrates complexity classification → topic decomposition → parallel exploration → synthesis/deep planning → validation.
- `plan_mode_training.rs`: persists plan-mode examples to `~/.openagents/autopilot-desktop/training/plan_mode.json`.
- `plan_mode_optimizer.rs`: runs MIPROv2/COPRO/GEPA over collected examples and writes manifests to `~/.openagents/autopilot-desktop/manifests/plan_mode/`.
- Optimization logs + state live under `~/.openagents/autopilot-desktop/optimization/`.

### Unified Types (`unified.rs`)

#### AgentId
```rust
pub enum AgentId {
    Codex,
    ClaudeCode,
    Cursor,
    Gemini,
    Adjutant,
}
```

#### UnifiedEvent
Normalized event types that all agents emit:
- `MessageChunk`: Streaming message content
- `ThoughtChunk`: Streaming reasoning content
- `ToolCall`: Tool execution started
- `ToolCallUpdate`: Tool execution progress
- `SessionStarted`: Session lifecycle event
- `SessionCompleted`: Session completion event
- `TokenUsage`: Token usage statistics
- `RateLimitUpdate`: Rate limit information

#### UnifiedConversationItem
Normalized conversation items for UI:
- `Message`: User or assistant message
- `Reasoning`: Agent reasoning/thinking
- `Tool`: Tool execution
- `Diff`: Code diff

### Agent Trait (`trait_def.rs`)

The `Agent` trait provides a unified interface:

```rust
#[async_trait]
pub trait Agent: Send + Sync {
    fn agent_id(&self) -> AgentId;
    async fn connect(&self, workspace_path: &Path) -> Result<String, String>;
    async fn disconnect(&self, session_id: &str) -> Result<(), String>;
    async fn start_session(&self, session_id: &str, cwd: &Path) -> Result<(), String>;
    async fn send_message(&self, session_id: &str, text: String) -> Result<(), String>;
    fn events_receiver(&self) -> mpsc::Receiver<UnifiedEvent>;
    async fn get_conversation_items(&self, session_id: &str) -> Result<Vec<UnifiedConversationItem>, String>;
}
```

### AcpAgent Implementation (`acp_agent.rs`)

`AcpAgent` wraps an ACP connection (e.g., `codex-acp`) and implements the `Agent` trait.

#### Key Features

1. **Connection Management**:
   - Spawns `codex-acp` process via `AcpConnection`
   - Manages process lifecycle
   - Handles connection errors

2. **Event Mapping**:
   - Maps ACP `SessionNotification` → `UnifiedEvent`
   - Handles custom extensions (e.g., `codex/tokenUsage`)
   - Extracts session IDs from events

3. **Session Management**:
   - Stores actual ACP session ID from `session/new` response
   - Tracks session state
   - Emits `SessionStarted` with actual session ID

4. **Event Forwarding**:
   - Uses broadcast channel for multiple receivers
   - Forwards events to `AgentManager`
   - Emits Tauri events for frontend

#### Event Mapping Logic

The `map_acp_event_static` function maps ACP events to unified events:

1. **Session Notifications** (`session/notification`):
   - `AgentMessageChunk` → `MessageChunk`
   - `AgentThoughtChunk` → `ThoughtChunk`
   - `ToolCall` → `ToolCall`
   - `ToolCallUpdate` → `ToolCallUpdate`

2. **Session Updates** (`session/update`):
   - `availableCommands` → `SessionStarted`
   - `agent_message_chunk` → `MessageChunk`
   - `agent_thought_chunk` → `ThoughtChunk`

3. **Responses**:
   - `result.stopReason: "end_turn"` → `SessionCompleted`

4. **Custom Extensions**:
   - `codex/tokenUsage` → `TokenUsage`
   - `codex/rateLimits` → `RateLimitUpdate`

### AgentManager (`manager.rs`)

`AgentManager` manages multiple agent instances:

#### Key Features

1. **Agent Registry**:
   - Stores active agents by `AgentId`
   - Tracks active sessions (session_id → agent_id mapping)

2. **Command Routing**:
   - Routes commands to appropriate agent
   - Handles agent lookup by session ID

3. **Event Merging**:
   - Merges event streams from all agents
   - Provides unified event stream

4. **Session Management**:
   - Tracks which agent owns which session
   - Updates session mappings

### Tauri Commands (`commands.rs`)

Commands exposed to frontend:

#### `connect_unified_agent`
Connects an agent to a workspace.

**Parameters** (camelCase for Tauri):
- `agentIdStr`: String - "codex", "claude_code", or "cursor"
- `workspacePath`: String - Path to workspace directory
- `workspaceId`: String - Unique workspace identifier

**Returns**:
```json
{
  "success": true,
  "sessionId": "workspace-123",  // Temporary, actual comes from SessionStarted event
  "agentId": "codex",
  "workspaceId": "workspace-123"
}
```

#### `start_unified_session`
Starts a new ACP session.

**Parameters**:
- `sessionId`: String - Session ID from connect (temporary workspace ID)
- `workspacePath`: String - Path to workspace directory

**Returns**:
```json
{
  "success": true,
  "sessionId": "actual-acp-session-id"
}
```

**Note**: The actual ACP session ID comes from the `session/new` response and is emitted via `SessionStarted` event.

#### `send_unified_message`
Sends a message to an agent session.

**Parameters**:
- `sessionId`: String - Actual ACP session ID (from `SessionStarted` event)
- `text`: String - Message text

**Returns**:
```json
{
  "success": true,
  "sessionId": "session-id"
}
```

#### `disconnect_unified_agent`
Disconnects an agent session.

**Parameters**:
- `sessionId`: String - Session ID to disconnect

**Returns**:
```json
{
  "success": true,
  "sessionId": "session-id"
}
```

### ACP Connection (`acp.rs`)

Low-level ACP connection management:

#### Key Features

1. **Process Management**:
   - Finds `codex-acp` binary (PATH or auto-download)
   - Spawns process with stdio pipes
   - Manages process lifecycle

2. **Event Capture**:
   - Captures stdout (JSON-RPC messages)
   - Captures stderr (logs)
   - Emits raw events for debugging

3. **Event Callback**:
   - Sets callback on `AcpConnection` for event forwarding
   - Extracts session ID from `session/new` responses
   - Forwards events to `AcpAgent`

## Frontend Implementation

### AutopilotCanvasComponent

Main Effuse component that renders the signature-driven canvas.

#### Startup

On mount, the component:
1. Initializes a `DataModel` for workspace/task/session/status state
2. Seeds a `UITree` via `createSetupTree`
3. Prefills the workspace path via `get_current_directory`

#### Action Wiring

Effuse EZ actions handle UI events:

- `ui.set` updates the data model (inputs + text areas)
- `ui.start` connects the unified agent, starts the session, and sends the prompt

#### UI Event Handling

Listens for `ui-event` Tauri events:

1. **UiTreeReset**: Validate and replace the current UITree
2. **UiPatch**: Apply incremental patches
3. **UiDataUpdate**: Update the data model at a path

### Effuse UITree Runtime

Effuse provides a lightweight UI runtime for signature-driven layouts:
- UITree + UIElement model
- Dynamic values resolved from the data model
- Catalog validation (component whitelist + schemas)
- Patch application for streamed UI changes

### Unified Event Stream (Chat)

Unified events (`unified-event`) are still emitted by the backend but are not yet rendered in the canvas UI. The next step is to map these events into UITree components alongside the signature panels.

## Event Flow

### UI Patch Flow Example

1. **User clicks "Start Autopilot"**:
   ```
   Frontend: ui.start → connect_unified_agent + start_unified_session
   ```

2. **Adjutant emits UI tree**:
   ```
   Backend: UiTreeReset + UiPatch → Tauri emits ui-event
   Frontend: apply patch → render updated UITree
   ```

## Session ID Management

### The Two-Phase Session ID

1. **Temporary Session ID** (from `connect_unified_agent`):
   - Format: `workspace-{timestamp}`
   - Used for initial session setup
   - Not the actual ACP session ID

2. **Actual Session ID** (from `SessionStarted` event):
   - Format: ACP UUID (e.g., `019bf22b-f5dc-7261-b4c3-1d084484dc25`)
   - Comes from `session/new` response
   - Used for all subsequent operations

### Why Two IDs?

- `connect_unified_agent` returns immediately with a temporary ID
- Actual ACP session ID comes asynchronously from `session/new` response
- Frontend must wait for `SessionStarted` event to get actual ID
- All message sends use actual ACP session ID

## Known Issues

### Current Issues

1. **Unified events not rendered in canvas**:
   - `unified-event` stream is emitted but not mapped into UITree components yet
   - Planned to add a conversation panel driven by unified events

2. **Event Deduplication**: Some events may be duplicated
   - Multiple event sources (raw ACP + mapped unified)
   - Need to deduplicate based on event ID or timestamp

3. **Error Handling**: Limited error handling in some paths
   - Connection failures handled
   - Session failures need better handling
   - Message send failures need user feedback

### Future Improvements

1. **Conversation Persistence**: Save conversations to disk
2. **Agent Selection**: UI to choose which agent to use
3. **Multi-Agent**: Support multiple agents simultaneously
4. **File System**: Implement ACP Client trait for file operations
5. **Terminal**: Implement ACP Client trait for terminal operations

## Testing

### Manual Testing

1. **Start app**: Should auto-connect to Codex
2. **Send message**: Should see message in UI
3. **Receive response**: Should see streaming response
4. **Check events**: Debug feed should show all events

### Debugging

- Check browser console for event logs
- Check Rust logs for backend events
- Use RawDataFeed to see all unified events
- Check Tauri devtools for event emissions

## Dependencies

### Backend (Rust)

- `agent-client-protocol`: ACP protocol types (v0.9)
- `tokio`: Async runtime
- `serde`: Serialization
- `tauri`: Desktop app framework

### Frontend (TypeScript)

- `react`: UI framework
- `@tauri-apps/api`: Tauri API bindings
- `typescript`: Type safety
