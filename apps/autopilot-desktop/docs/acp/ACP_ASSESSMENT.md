# ACP Integration Assessment

## Overview

**Agent Client Protocol (ACP)** is a JSON-RPC based protocol that standardizes communication between code editors and AI coding agents. It's published as a Rust crate (`agent-client-protocol`, version 0.10.7) and provides a clean abstraction for agent-client communication.

## Current State

### Autopilot (Current Implementation)
- **Protocol**: Direct JSON-RPC communication with `codex app-server` CLI
- **Transport**: Tauri events (`app-server-event`)
- **Event Handling**: Custom event listeners parsing Codex-specific events
- **Data Flow**: 
  - Spawns `codex app-server` process
  - Listens to stdout/stderr via Tauri events
  - Parses events manually (e.g., `item/agentMessage/delta`, `item/tool/started`)

### Zed's ACP Implementation
- **Protocol**: ACP (Agent Client Protocol)
- **Transport**: stdio (stdin/stdout) via `ClientSideConnection`
- **Adapter**: Uses `codex-acp` binary (from `zed-industries/codex-acp` repo)
- **Architecture**:
  1. Downloads/installs `codex-acp` adapter from GitHub releases
  2. Spawns `codex-acp` as subprocess with stdio pipes
  3. Creates `ClientSideConnection` with a `Client` trait implementation
  4. Receives `SessionNotification` updates (standardized ACP format)
  5. Maps ACP updates to internal UI state

## Key Findings

### 1. ACP Crate Structure
- **Location**: `/Users/christopherdavid/code/agent-client-protocol/`
- **Crate Name**: `agent-client-protocol-schema` (published as `agent-client-protocol` on crates.io)
- **Core Components**:
  - `Client` trait: Methods the client must implement (file system, terminal, etc.)
  - `Agent` trait: Methods the agent must implement (initialize, prompt, etc.)
  - `ClientSideConnection`: Connection object providing `Agent` methods
  - `AgentSideConnection`: Connection object providing `Client` methods
  - `SessionNotification`: Standardized update format for streaming responses

### 2. How Zed Connects to Codex via ACP

**File**: `crates/agent_servers/src/codex.rs`

```rust
// 1. Get codex-acp command
let (command, root_dir, login) = store.update(|store, cx| {
    let agent = store.get_external_agent(&CODEX_NAME.into())?;
    agent.get_command(root_dir, extra_env, ...)
})?;

// 2. Connect via ACP
let connection = crate::acp::connect(
    name,
    command,  // codex-acp binary
    root_dir,
    default_mode,
    default_model,
    default_config_options,
    is_remote,
    cx,
).await?;
```

**File**: `crates/agent_servers/src/acp.rs`

The `connect` function:
1. Spawns the agent process (e.g., `codex-acp`) with stdio pipes
2. Creates a `ClientDelegate` implementing the `Client` trait
3. Creates `ClientSideConnection::new(client, stdin, stdout, spawner)`
4. Calls `connection.initialize()` to negotiate protocol version
5. Returns `AcpConnection` which wraps the `ClientSideConnection`

### 3. ACP Event Flow

**Client → Agent**:
- `initialize`: Negotiate protocol version and capabilities
- `new_session`: Create a new conversation session
- `prompt`: Send user message
- `set_session_model`: Change model
- File system requests (read_file, write_file, etc.)
- Terminal requests (create_terminal, terminal_output, etc.)

**Agent → Client** (via `SessionNotification`):
- `AgentMessageChunk`: Streaming agent response
- `AgentThoughtChunk`: Streaming reasoning/thinking
- `ToolCall`: Tool invocation started
- `ToolCallUpdate`: Tool execution progress/results
- `Plan`: Agent's execution plan
- `UserMessageChunk`: Echo of user message

### 4. Key Differences: Current vs ACP

| Aspect | Current (Direct JSON-RPC) | ACP |
|--------|-------------------------|-----|
| **Protocol** | Custom Codex JSON-RPC | Standardized ACP JSON-RPC |
| **Transport** | Tauri events | stdio (can be any stream) |
| **Event Format** | Codex-specific (`item/agentMessage/delta`) | Standardized (`SessionNotification`) |
| **Connection** | Manual event listening | `ClientSideConnection` abstraction |
| **File System** | Not exposed | Built-in via `Client` trait |
| **Terminal** | Not exposed | Built-in via `Client` trait |
| **Adapter** | Direct `codex app-server` | Uses `codex-acp` adapter |

## Integration Strategy

### Option 1: Use ACP with codex-acp Adapter (Recommended)
**Pros**:
- Standardized protocol
- Built-in file system and terminal support
- Better separation of concerns
- Future-proof (works with any ACP-compatible agent)
- Cleaner event handling via `SessionNotification`

**Cons**:
- Need to download/install `codex-acp` adapter
- Additional dependency
- Need to implement `Client` trait for file system/terminal

**Implementation Steps**:
1. Add `agent-client-protocol` crate to `Cargo.toml`
2. Implement `Client` trait (file system, terminal operations)
3. Download/install `codex-acp` adapter (similar to Zed's approach)
4. Spawn `codex-acp` process and create `ClientSideConnection`
5. Replace current event listeners with ACP `SessionNotification` handling
6. Map ACP updates to existing `ConversationItem` types

### Option 2: Keep Current Implementation, Add ACP Event Capture
**Pros**:
- Minimal changes
- Keep existing functionality
- Can capture ACP events separately

**Cons**:
- Dual protocol support (more complexity)
- Not using ACP benefits (file system, terminal)
- Still need to maintain custom event parsing

**Implementation Steps**:
1. Add `agent-client-protocol` crate
2. Spawn `codex-acp` in parallel with `codex app-server`
3. Create `ClientSideConnection` and capture all ACP messages
4. Display ACP events in the "Raw ACP Events" column
5. Keep existing implementation for actual functionality

### Option 3: Full Migration to ACP
**Pros**:
- Single, standardized protocol
- Access to file system and terminal capabilities
- Cleaner architecture
- Better maintainability

**Cons**:
- Significant refactoring required
- Need to reimplement file system/terminal handling
- May lose some Codex-specific features

## Recommended Approach

**Hybrid Approach**: Start with Option 2, then migrate to Option 1

1. **Phase 1** (Quick Win):
   - Add `agent-client-protocol` dependency
   - Spawn `codex-acp` alongside current `codex app-server`
   - Capture all ACP messages and display in "Raw ACP Events" column
   - Keep existing implementation working

2. **Phase 2** (Full Migration):
   - Implement `Client` trait for file system operations
   - Implement `Client` trait for terminal operations
   - Replace current event handling with ACP `SessionNotification`
   - Remove direct `codex app-server` communication
   - Use ACP as the primary protocol

## Technical Details

### Client Trait Implementation Needed

```rust
use agent_client_protocol::{Client, ClientCapabilities, ...};

struct AutopilotClient {
    // Implement file system operations
    // Implement terminal operations
    // Handle permission requests
}

impl Client for AutopilotClient {
    async fn read_text_file(&self, request: ReadTextFileRequest) -> Result<ReadTextFileResponse> {
        // Read file from workspace
    }
    
    async fn write_text_file(&self, request: WriteTextFileRequest) -> Result<WriteTextFileResponse> {
        // Write file to workspace
    }
    
    async fn create_terminal(&self, request: CreateTerminalRequest) -> Result<CreateTerminalResponse> {
        // Create terminal instance
    }
    
    // ... other Client methods
}
```

### Connection Setup

```rust
use agent_client_protocol::{ClientSideConnection, ProtocolVersion};

// 1. Spawn codex-acp process
let mut child = Command::new("codex-acp")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()?;

let stdin = child.stdin.take().unwrap();
let stdout = child.stdout.take().unwrap();

// 2. Create Client implementation
let client = AutopilotClient::new(...);

// 3. Create connection
let (connection, io_task) = ClientSideConnection::new(
    client,
    stdin,
    stdout,
    |fut| tokio::spawn(fut)
);

// 4. Initialize
let response = connection
    .initialize(InitializeRequest::new(ProtocolVersion::V1)
        .client_capabilities(ClientCapabilities::new()
            .fs(FileSystemCapability::new()
                .read_text_file(true)
                .write_text_file(true))
            .terminal(true)))
    .await?;

// 5. Create session
let session = connection
    .new_session(NewSessionRequest::new(workspace_path))
    .await?;

// 6. Listen for updates
// Updates come via SessionNotification
```

### Event Capture for Raw Feed

All ACP communication happens through the `ClientSideConnection`. To capture raw events:

1. **Option A**: Wrap the connection and log all messages
2. **Option B**: Use ACP's built-in notification system (`SessionNotification`)
3. **Option C**: Intercept at the RPC layer (modify `ClientSideConnection` internals)

**Recommended**: Capture `SessionNotification` events and also log the raw JSON-RPC messages at the transport layer.

## Dependencies Needed

```toml
[dependencies]
agent-client-protocol = "0.10.7"  # Or latest version
tokio = { version = "1", features = ["full"] }
```

## Implementation Status

### Phase 1: Raw ACP Event Capture (✅ COMPLETED)

**Date**: January 24, 2026

**What was implemented**:
- Added `agent-client-protocol` dependency (v0.9) to `Cargo.toml`
- Created `src-tauri/src/acp.rs` module that:
  - Finds `codex-acp` binary in PATH or common installation locations
  - Spawns `codex-acp` process when workspace connects
  - Captures all stdout (JSON-RPC messages) and stderr (logs) from `codex-acp`
  - Emits raw events via Tauri's `acp-event` channel
- Integrated ACP connection into workspace lifecycle:
  - Starts ACP connection alongside `codex app-server` when workspace connects
  - Stores ACP connections in `AppState.acp_connections`
  - Cleans up ACP connection when workspace disconnects
- Frontend updates:
  - Added `AcpEvent` type to `src/types.ts`
  - Added `rawAcpEvents` state in `App.tsx`
  - Added listener for `acp-event` Tauri events
  - Updated `RawDataFeed` component to accept both `AppServerEvent` and `AcpEvent`
  - Connected "Raw ACP Events" column to display captured events

**Current Architecture**:
```
Workspace Connection
├── codex app-server (existing, for actual functionality)
│   └── Events → "app-server-event" → Raw Data Feed (left column)
└── codex-acp (new, for ACP protocol capture)
    └── Events → "acp-event" → Raw ACP Events Feed (right column)
```

**Implementation Details**:
- **Direct stdout/stderr capture**: Currently reading raw output from `codex-acp` without using the ACP library. This allows us to see all JSON-RPC messages in their raw form.
- **Error handling**: ACP connection failures are non-fatal - if `codex-acp` isn't found or fails to start, the workspace connection still succeeds (only `codex app-server` is required).
- **Event format**: Each ACP event includes:
  - `workspace_id`: The workspace identifier
  - `message`: The raw JSON-RPC message or log output with metadata:
    - `type`: Event type (`acp/raw_message`, `acp/raw_output`, `acp/stderr`)
    - `direction`: For messages, indicates `incoming` or `outgoing`
    - `message` or `text`: The actual content

**Files Modified**:
- `src-tauri/Cargo.toml`: Added dependencies
- `src-tauri/src/lib.rs`: Added `acp` module
- `src-tauri/src/acp.rs`: New file with ACP connection logic
- `src-tauri/src/state.rs`: Added `acp_connections` to `AppState`
- `src-tauri/src/codex.rs`: Integrated ACP connection into workspace lifecycle
- `src/types.ts`: Added `AcpEvent` type
- `src/App.tsx`: Added ACP event listener and state
- `src/components/RawDataFeed.tsx`: Updated to accept both event types

**Known Limitations**:
- ACP library (`agent-client-protocol`) is included but not yet used - we're capturing raw output directly
- No file system or terminal operations implemented yet (Client trait not implemented)
- `codex-acp` must be installed separately (not auto-downloaded like in Zed)
- Initialization message is sent but we don't process responses yet

**Testing**:
- Code compiles successfully
- Ready for runtime testing when `codex-acp` is available

## Next Steps

1. **Immediate**: Test ACP event capture with actual `codex-acp` installation
2. **Short-term**: Implement basic `Client` trait for file system operations
3. **Medium-term**: Full migration to ACP, remove direct `codex app-server` communication
4. **Long-term**: Leverage ACP features (terminal, advanced file operations, etc.)

## Codex-ACP Adapter

- **Repository**: `zed-industries/codex-acp`
- **Purpose**: Adapter that translates between Codex CLI and ACP protocol
- **Installation**: Zed downloads it from GitHub releases automatically
- **Usage**: Spawn as subprocess, communicate via stdio using ACP

## Benefits of ACP

1. **Standardization**: Works with any ACP-compatible agent (not just Codex)
2. **Rich Features**: Built-in file system, terminal, and permission handling
3. **Type Safety**: Strongly typed Rust traits and types
4. **Future-Proof**: Protocol evolves independently of Codex
5. **Ecosystem**: Can integrate other agents (Claude Code, Gemini, etc.) easily
