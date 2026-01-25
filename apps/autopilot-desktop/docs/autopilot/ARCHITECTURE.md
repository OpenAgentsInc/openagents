# Autopilot Architecture

## Overview

Autopilot is a Tauri-based desktop application that provides a unified interface for multiple AI coding agents (Codex, Gemini, Adjutant) using the Agent Client Protocol (ACP) as the base protocol.

## Architecture Layers

```
┌─────────────────────────────────────────┐
│       UI Layer (Effuse/TypeScript)       │
│  - Unified stream components              │
│  - Effuse state + templates               │
│  - Unified event listeners                │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│    Tauri Bridge (Commands & Events)     │
│  - connect_unified_agent                 │
│  - start_unified_session                 │
│  - send_unified_message                  │
│  - unified-event emission                │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│    Unified Agent Abstraction (Rust)      │
│  - Agent trait (unified interface)      │
│  - UnifiedEvent (normalized events)     │
│  - AgentManager (multi-agent support)   │
│  - AcpAgent (ACP implementation)        │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│      ACP Protocol Layer (Rust)          │
│  - AcpConnection (codex-acp wrapper)   │
│  - SessionNotification parsing          │
│  - Custom extension handling            │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│      Agent Adapters (External)          │
│  - codex-acp (Codex adapter)            │
│  - gemini (Gemini CLI ACP mode)         │
│  - claude-code-acp (future)             │
│  - cursor-acp (future)                  │
└─────────────────────────────────────────┘
```

## Core Concepts

### Unified Agent Architecture

The unified agent architecture provides a single, consistent interface for all AI agents, regardless of their underlying protocol. This allows:

1. **Single Protocol**: All agents use ACP (standardized)
2. **Unified Types**: One set of types for all agents
3. **Easy to Add Agents**: Just implement `Agent` trait
4. **Consistent API**: Same interface for all agents
5. **Future-Proof**: Works with any ACP-compatible agent

### Agent Client Protocol (ACP)

ACP is a JSON-RPC based protocol that standardizes communication between code editors and AI coding agents. Key features:

- **Standardized Protocol**: All ACP agents use the same `SessionNotification` types
- **Extensible**: Custom notifications via `extNotification` field
- **Type-Safe**: Strongly typed Rust traits and types
- **Rich Features**: Built-in file system, terminal, and permission handling

### Event Flow

1. **User sends message** → Frontend calls `send_unified_message`
2. **Tauri command** → `AgentManager` routes to appropriate `AcpAgent`
3. **ACP connection** → Sends `session/prompt` request to `codex-acp`
4. **Agent responds** → `codex-acp` sends ACP events
5. **Event mapping** → `AcpAgent` maps ACP events to `UnifiedEvent`
6. **Event emission** → Tauri emits `unified-event` to frontend
7. **UI update** → Frontend maps `UnifiedEvent` to `ConversationItem`

## Key Components

### Backend (Rust)

#### `agent/unified.rs`
Defines unified types:
- `AgentId`: Enum for agent identifiers (Codex, ClaudeCode, Cursor)
- `UnifiedEvent`: Normalized event types from all agents
- `UnifiedConversationItem`: Normalized conversation items

#### `agent/trait_def.rs`
Defines the `Agent` trait - unified interface for all agents:
- `connect()`: Connect to agent for a workspace
- `start_session()`: Start new session/thread
- `send_message()`: Send user message
- `events_receiver()`: Get stream of unified events
- `get_conversation_items()`: Get conversation history

#### `agent/acp_agent.rs`
Implements `Agent` trait for ACP-based agents:
- Wraps `AcpConnection` (codex-acp process)
- Maps ACP `SessionNotification` → `UnifiedEvent`
- Handles custom extensions (codex/tokenUsage, etc.)
- Manages session lifecycle

#### `agent/manager.rs`
Manages multiple agent instances:
- Tracks active agents and sessions
- Routes commands to appropriate agents
- Merges event streams from all agents
- Provides unified event stream to frontend

#### `agent/commands.rs`
Tauri commands for unified agent interface:
- `connect_unified_agent`: Connect agent to workspace
- `start_unified_session`: Start new session
- `send_unified_message`: Send message to agent
- `disconnect_unified_agent`: Disconnect agent
- `get_unified_conversation_items`: Get conversation history

#### `acp.rs`
Low-level ACP connection management:
- Spawns `codex-acp` process
- Captures stdout/stderr (JSON-RPC messages)
- Emits raw ACP events for debugging
- Manages process lifecycle

### Frontend (Effuse/TypeScript)

#### `src/components/unified-stream/`
Effuse-based unified conversation UI:
- Normalizes `UnifiedEvent` into stream items
- Handles streaming updates and tool output
- Provides the main chat rendering pipeline

#### `src/effuse/`
Effuse runtime and primitives:
- State cells, DOM adapters, and template utilities
- Central EZ action registry

#### `src/effuse-storybook/`
In-app component storybook (toggle with F12)

#### `src/agent/`
TypeScript agent adapters:
- Codex, Gemini, and Adjutant client shims
- Frontend agent registry

#### `src/contracts/tauri.ts`
Effect Schema decoders for IPC contracts

## Current Implementation Status

### ✅ Completed

- Unified agent types (`AgentId`, `UnifiedEvent`, `UnifiedConversationItem`)
- `Agent` trait definition
- `AcpAgent` implementation for Codex
- `AgentManager` for multi-agent support
- Tauri commands for unified interface
- Event mapping from ACP to unified events
- Frontend integration with unified events
- Auto-setup on app start
- Session ID management (ACP session IDs)

### ⏳ In Progress

- Frontend UI rendering (state updates working, UI not reflecting changes)
- Event deduplication (some events may be duplicated)
- Error handling improvements

### 🔜 Planned

- Claude Code agent support
- Cursor agent support
- Agent selection UI
- Conversation persistence
- File system operations via ACP
- Terminal operations via ACP

## File Structure

```
crates/autopilot-desktop-backend/src/
├── agent/
│   ├── mod.rs              # Module exports
│   ├── unified.rs          # Unified types (AgentId, UnifiedEvent, etc.)
│   ├── trait_def.rs        # Agent trait definition
│   ├── acp_agent.rs        # ACP agent implementation
│   ├── codex_agent.rs      # Codex-specific agent (deprecated)
│   ├── manager.rs          # Multi-agent manager
│   └── commands.rs         # Tauri commands
├── acp.rs                  # ACP connection management
├── ai_server/              # Local AI server config + lifecycle
├── backend/                # Codex app-server bridge
└── lib.rs                  # Backend entry point

apps/autopilot-desktop/src/
├── agent/                  # Frontend agent adapters
├── components/
│   └── unified-stream/     # Effuse unified conversation UI
├── contracts/              # Effect schema decoders
├── effuse/                 # Effuse runtime + templates
├── effuse-storybook/       # In-app storybook
├── gen/                    # Generated IPC contracts
├── index.css               # Global styles
└── main.ts                 # App entry point
```

## Event Types

### UnifiedEvent Types

- `MessageChunk`: Streaming agent message content
- `ThoughtChunk`: Streaming reasoning/thinking content
- `ToolCall`: Tool execution started
- `ToolCallUpdate`: Tool execution progress/results
- `SessionStarted`: Session lifecycle - started
- `SessionCompleted`: Session lifecycle - completed
- `TokenUsage`: Token usage statistics
- `RateLimitUpdate`: Rate limit information

### ACP to UnifiedEvent Mapping

- `session/notification` with `AgentMessageChunk` → `MessageChunk`
- `session/notification` with `AgentThoughtChunk` → `ThoughtChunk`
- `session/notification` with `ToolCall` → `ToolCall`
- `session/notification` with `ToolCallUpdate` → `ToolCallUpdate`
- `session/update` with `availableCommands` → `SessionStarted`
- Response with `result.stopReason: "end_turn"` → `SessionCompleted`
- Custom extensions (e.g., `codex/tokenUsage`) → `TokenUsage` / `RateLimitUpdate`

## Session Management

### Session ID Flow

1. **Connect**: `connect_unified_agent` returns temporary `workspaceId`
2. **Start Session**: `start_unified_session` sends `session/new` to ACP
3. **Session Started**: ACP responds with actual `sessionId` in `result.sessionId`
4. **Store Session ID**: `AcpAgent` stores actual session ID from response
5. **Emit Event**: `SessionStarted` event emitted with actual session ID
6. **Frontend Update**: Frontend receives `SessionStarted` and updates `unifiedSessionId`
7. **Send Messages**: All subsequent messages use actual ACP session ID

### Session Lifecycle

- **Connect**: Agent connects to workspace, returns temporary session ID
- **Start**: Agent starts new ACP session, receives actual session ID
- **Active**: Agent processes messages, emits events
- **Complete**: Agent completes turn, emits `SessionCompleted`
- **Disconnect**: Agent disconnects, cleans up resources

## Error Handling

### Connection Errors
- If `codex-acp` not found: Attempts auto-download from GitHub releases
- If download fails: Shows error in UI, prevents connection
- If connection fails: Shows error in status bar

### Session Errors
- If session creation fails: Emits error event
- If message send fails: Logs error, shows in console
- If event mapping fails: Logs warning, continues processing

## Future Enhancements

1. **Multi-Agent Support**: Add Claude Code and Cursor agents
2. **Agent Selection UI**: Allow users to choose which agent to use
3. **Conversation Persistence**: Save conversations to disk
4. **File System Operations**: Implement ACP `Client` trait for file operations
5. **Terminal Operations**: Implement ACP `Client` trait for terminal operations
6. **Advanced Features**: Leverage ACP capabilities (permissions, etc.)
