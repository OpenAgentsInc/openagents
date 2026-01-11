# Zed ACP Component Mapping: How Zed Renders Codex Conversations

This document provides a detailed analysis of how Zed renders Codex conversations through the Agent Client Protocol (ACP), mapping UI components to protocol types for OpenAgents integration planning.

## Executive Summary

Zed's Codex integration uses ACP (Agent Client Protocol) as the standardized communication layer between the IDE and AI agents. The UI is built around three core concepts:
1. **AcpThread** - Represents a conversation session with an agent
2. **AgentThreadEntry** - Individual entries in a conversation (user messages, assistant messages, tool calls)
3. **AcpThreadView** - The GPUI component that renders the conversation

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           Zed IDE                                       │
├─────────────────────────────────────────────────────────────────────────┤
│  agent_ui/src/acp/                                                       │
│  ├── thread_view.rs       → Main conversation view (AcpThreadView)      │
│  ├── entry_view_state.rs  → Per-entry view state and caching            │
│  ├── message_editor.rs    → User input component                        │
│  ├── mode_selector.rs     → Agent mode selection (plan/code/etc)        │
│  └── model_selector.rs    → Model selection dropdown                    │
├─────────────────────────────────────────────────────────────────────────┤
│  acp_thread/src/                                                         │
│  ├── acp_thread.rs        → Thread state, entries, event handling       │
│  ├── terminal.rs          → Terminal output wrapping                    │
│  ├── diff.rs              → File diff rendering                         │
│  └── connection.rs        → Agent connection abstraction                │
├─────────────────────────────────────────────────────────────────────────┤
│  agent_servers/src/                                                      │
│  ├── acp.rs               → AcpConnection (JSON-RPC stdio transport)    │
│  ├── codex.rs            → CodexCode AgentServer wrapper              │
│  ├── codex.rs             → Codex AgentServer wrapper                   │
│  └── gemini.rs            → Gemini AgentServer wrapper                  │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ JSON-RPC 2.0 over stdio
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                     Agent Subprocess (Codex)                       │
│  - Receives initialize, session/new, session/prompt requests            │
│  - Sends SessionNotification with SessionUpdate events                   │
└─────────────────────────────────────────────────────────────────────────┘
```

## ACP Protocol Types → UI Component Mapping

### 1. Session Updates (`acp::SessionUpdate`)

The core of ACP communication is `SessionNotification` containing `SessionUpdate` variants:

| ACP SessionUpdate Variant | Zed UI Component | Rendering Location |
|---------------------------|------------------|-------------------|
| `AgentMessageChunk` | Markdown text block | `AssistantMessage.chunks` → `render_markdown()` |
| `AgentThoughtChunk` | Collapsible thinking block | `render_thinking_block()` |
| `UserMessageChunk` | User message editor | `MessageEditor` in `entry_view_state.rs` |
| `ToolCall` | Tool card with icon/label | `render_tool_call()` or `render_terminal_tool_call()` |
| `ToolCallUpdate` | Updates existing tool card | Modifies `ToolCall.status`, `content`, etc. |
| `Plan` | Todo list display | `render_thread_controls()` → plan entries |
| `CurrentModeUpdate` | Mode selector state | Updates `ModeSelector` dropdown |
| `AvailableCommandsUpdate` | Slash command picker | Populates `available_commands` |

### 2. Thread Entries (`AgentThreadEntry`)

Zed's `acp_thread` crate models conversations as a list of entries:

```rust
pub enum AgentThreadEntry {
    UserMessage(UserMessage),      // User input with optional checkpoint
    AssistantMessage(AssistantMessage), // Agent response chunks
    ToolCall(ToolCall),            // Tool execution with content
}
```

#### UserMessage Rendering
**File**: `agent_ui/src/acp/thread_view.rs:1940-2098`

- Rendered as an editable `MessageEditor` component
- Has optional checkpoint restore button
- Border styling indicates edit state (focused, hovering, read-only)
- Edit actions: regenerate from this point, cancel editing

#### AssistantMessage Rendering
**File**: `agent_ui/src/acp/thread_view.rs:2099-2138`

```rust
AssistantMessageChunk::Message { block } => {
    // Rendered as Markdown using render_markdown()
    block.markdown().map(|md| self.render_markdown(md, style))
}
AssistantMessageChunk::Thought { block } => {
    // Rendered as collapsible "Thinking" block
    self.render_thinking_block(entry_ix, chunk_ix, md, window, cx)
}
```

#### ToolCall Rendering
**File**: `agent_ui/src/acp/thread_view.rs:2139-2527`

Tool calls are rendered differently based on their kind:

| `acp::ToolKind` | Icon | Card Layout | Special Handling |
|-----------------|------|-------------|------------------|
| `Read` | `ToolSearch` | No | Markdown output |
| `Edit` | `ToolPencil` or file icon | Yes | Inline diff editor |
| `Delete` | `ToolDeleteFile` | Yes | Confirmation buttons |
| `Execute` | `ToolTerminal` | Yes | Terminal emulator |
| `Search` | `ToolSearch` | No | File list |
| `Think` | `ToolThink` | No | Collapsible |
| `Fetch` | `ToolWeb` | No | URL preview |
| `SwitchMode` | `ArrowRightLeft` | Yes | Mode change dialog |
| `Other` | `ToolHammer` | No | Generic |

### 3. Tool Call Content Types

```rust
pub enum ToolCallContent {
    ContentBlock(ContentBlock),  // Text/Markdown/ResourceLink
    Diff(Entity<Diff>),          // File diff viewer
    Terminal(Entity<Terminal>),  // Terminal output
}
```

#### Diff Rendering (`diff.rs`)
**File**: `acp_thread/src/diff.rs`

- Uses `BufferDiff` from Zed's buffer_diff crate
- Shows inline additions/deletions with syntax highlighting
- Supports streaming diffs (reveals ranges as agent writes)
- Two states: `PendingDiff` (streaming) and `FinalizedDiff` (complete)

The diff is displayed in a `MultiBuffer` editor with:
- Hunks extracted and displayed as excerpts
- Context lines around changes
- Green/red diff highlighting
- No gutter, no scrollbar, read-only

#### Terminal Rendering (`terminal.rs`)
**File**: `acp_thread/src/terminal.rs`

Wraps Zed's `terminal::Terminal` with:
- Command label (displayed as markdown code block)
- Working directory
- Output capture with byte limit
- Exit status tracking
- Truncation for large outputs

The terminal view (`TerminalView`) is embedded in the conversation with `set_embedded_mode(Some(1000))`.

### 4. Permission System

When Codex needs authorization for a tool:

```rust
ToolCallStatus::WaitingForConfirmation {
    options: Vec<acp::PermissionOption>,
    respond_tx: oneshot::Sender<acp::PermissionOptionId>,
}
```

**File**: `thread_view.rs:2777-2799`

Permission buttons rendered based on `acp::PermissionOptionKind`:
- `AllowOnce` → "Allow" button
- `AllowAlways` → "Always Allow" button
- `RejectOnce` → "Reject" button
- `RejectAlways` → "Always Reject" button

Keybindings:
- `y` or `Enter` → AllowOnce
- `Y` → AllowAlways
- `n` → RejectOnce
- `N` → RejectAlways

### 5. Connection & Transport

**File**: `agent_servers/src/acp.rs`

`AcpConnection` wraps:
- `smol::process::Child` - The agent subprocess
- `acp::ClientSideConnection` - JSON-RPC handler
- `sessions: HashMap<SessionId, AcpSession>` - Active conversations

The connection implements `AgentConnection` trait:
```rust
trait AgentConnection {
    fn new_thread(...) -> Task<Result<Entity<AcpThread>>>;
    fn prompt(...) -> Task<Result<acp::PromptResponse>>;
    fn cancel(...);
    fn session_modes(...) -> Option<Rc<dyn AgentSessionModes>>;
    fn model_selector(...) -> Option<Rc<dyn AgentModelSelector>>;
}
```

### 6. Client Delegate (IDE → Agent Requests)

**File**: `agent_servers/src/acp.rs:680-976`

The `ClientDelegate` implements `acp::Client` to handle agent requests:

| Method | Purpose | UI Integration |
|--------|---------|----------------|
| `request_permission` | Tool authorization | Shows confirmation dialog |
| `read_text_file` | File access | Opens buffer, returns content |
| `write_text_file` | File modification | Writes to buffer |
| `session_notification` | Status updates | Updates thread state |
| `create_terminal` | Spawn shell | Creates `TerminalView` |
| `terminal_output` | Get output | Returns captured text |
| `wait_for_terminal_exit` | Block until done | Returns exit status |

### 7. Special UI Features

#### Mode Selector
**File**: `agent_ui/src/acp/mode_selector.rs`

Modes are session-scoped configurations (e.g., "plan", "code", "ask"):
- Populated from `acp::SessionModeState.available_modes`
- Current mode tracked in `current_mode_id`
- Changes sent via `session/set_mode` request

#### Model Selector
**File**: `agent_ui/src/acp/model_selector.rs`

For agents supporting multiple models:
- Lists from `acp::SessionModelState.available_models`
- Selection via `session/set_model` request
- UI shows model name and supports search

#### Checkpoint Restoration
Users can restore files to a previous state:
- Captured in `UserMessage.checkpoint`
- Uses Git to track project state
- "Restore Checkpoint" button reverts changes

#### Thread Feedback
After completion, users can rate the conversation:
- Thumbs up/down buttons
- Optional comment field
- Telemetry reporting

## Key Rendering Functions

### `render_entry()` - Main dispatcher
```rust
fn render_entry(&self, entry_ix: usize, total_entries: usize,
                entry: &AgentThreadEntry, ...) -> AnyElement
```
Routes to specific renderers based on entry type.

### `render_tool_call()` - Tool card
```rust
fn render_tool_call(&self, entry_ix: usize, tool_call: &ToolCall,
                    window: &Window, cx: &Context<Self>) -> Div
```
Builds the tool call card with:
- Header with icon and label
- Collapsible content area
- Permission buttons if waiting
- Status indicators

### `render_diff_editor()` - File changes
```rust
fn render_diff_editor(&self, entry_ix: usize, diff: &Entity<Diff>,
                      tool_call: &ToolCall, cx: &Context<Self>) -> AnyElement
```
Embeds a read-only Editor showing file changes.

### `render_terminal_tool_call()` - Command output
```rust
fn render_terminal_tool_call(&self, entry_ix: usize, terminal: &Entity<Terminal>,
                             tool_call: &ToolCall, ...) -> AnyElement
```
Embeds a TerminalView with command output.

### `render_markdown()` - Text content
```rust
fn render_markdown(&self, markdown: Entity<Markdown>,
                   style: MarkdownStyle) -> MarkdownElement
```
Converts Markdown to styled GPUI elements.

## Event Flow

1. **User types message** → `MessageEditor` captures input
2. **User presses Enter** → `send()` called
3. **Prompt sent** → `connection.prompt(PromptRequest)`
4. **Agent responds** → `SessionNotification` with updates
5. **Client receives** → `session_notification()` dispatches
6. **Thread updated** → `AcpThread.handle_session_update()`
7. **UI notified** → `AcpThreadEvent` emitted
8. **View refreshes** → `handle_thread_event()` syncs state
9. **Render cycle** → `render_entry()` draws updated content

## Implications for OpenAgents

### What We Need to Implement

1. **Session Management**
   - Create sessions with `session/new`
   - Handle session modes if agent supports them
   - Track session state for UI

2. **Entry Rendering**
   - User message display with edit support
   - Assistant message with Markdown
   - Tool calls with appropriate icons
   - Diffs with syntax highlighting
   - Terminal output embedding

3. **Permission Flow**
   - Display confirmation dialogs
   - Send responses back to agent
   - Support "always allow" persistence

4. **Real-time Updates**
   - Stream content as it arrives
   - Show progress indicators
   - Update tool status dynamically

### Component Reuse Opportunities

Since OpenAgents uses Maud+HTMX (server-rendered) instead of GPUI:

| Zed Component | OpenAgents Equivalent |
|---------------|----------------------|
| `MarkdownElement` | Server-side Markdown render |
| `Editor` (diff) | Monaco/CodeMirror with diff plugin |
| `TerminalView` | xterm.js WebSocket terminal |
| `MessageEditor` | Textarea with @ mention support |
| List scrolling | HTMX infinite scroll or virtual scroll |

### ACP Adapter Usage

Our `acp-adapter` crate should:
1. Spawn Codex with ACP protocol
2. Receive `SessionNotification` events
3. Convert to rlog format for replay
4. Expose REST API for GUI consumption
5. Handle permission requests via WebSocket

## References

- **Zed ACP Implementation**: `~/code/zed/crates/agent_servers/src/acp.rs`
- **Thread Data Model**: `~/code/zed/crates/acp_thread/src/acp_thread.rs`
- **UI Rendering**: `~/code/zed/crates/agent_ui/src/acp/thread_view.rs`
- **Protocol Schema**: `agent-client-protocol-schema` crate
- **OpenAgents Directive**: `.openagents/directives/d-017.md`
