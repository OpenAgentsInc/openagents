# MechaCoder: Claude Code Harness with Zed Parity

## Overview

Create a new `crates/mechacoder/` crate providing a focused Claude Code harness with full Zed feature parity: chat, diffs, terminal, permissions, modes, and models.

**Strategy:** Hybrid minimal port - copy Zed's protocol layer, build custom UI on OpenAgents components.

## Key Decision: Why Not Direct Port

Zed's `agent_ui` crate has 80+ dependencies including `editor` (50k+ lines), `workspace`, `multi_buffer`. Direct port would require porting ~15-20 additional crates. Instead:
- Use `agent-client-protocol` v0.9.0 from crates.io
- Copy/adapt ACP connection layer from Zed
- Build UI with OpenAgents' `ui` crate (33 components)
- Use `similar` for diffs, `alacritty_terminal` for terminal

---

## New Crates to Create

### 1. `crates/acp/` - ACP Protocol Layer
Core ACP connection handling, session management, terminal support.

### 2. `crates/mechacoder/` - Main Application
GPUI application with chat interface, binary entrypoint.

---

## Phase 1: ACP Protocol Layer (Week 1)

### Create `crates/acp/Cargo.toml`
```toml
[package]
name = "acp"
version = "0.1.0"
edition = "2024"

[dependencies]
agent-client-protocol = { version = "0.9.0", features = ["unstable"] }
gpui = { path = "../gpui" }
collections = { path = "../collections" }
anyhow = "1"
async-trait = "0.1"
futures = "0.3"
smol = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
uuid = { version = "1", features = ["v4"] }
log = "0.4"
portable-pty = "0.9"
```

### Files to Create
```
crates/acp/src/
  lib.rs              # Re-exports
  connection.rs       # AcpConnection (from Zed acp.rs, ~400 lines adapted)
  session.rs          # AcpSession, session state
  claude_code.rs      # ClaudeCode struct, binary discovery
  terminal.rs         # Terminal wrapper using portable-pty
  types.rs            # Local type definitions
  error.rs            # Error types
```

### Zed Files to Adapt
| Source | Target | Changes |
|--------|--------|---------|
| `zed/crates/agent_servers/src/acp.rs` (977 lines) | `acp/src/connection.rs` | Strip Zed deps, keep stdio + RPC |
| `zed/crates/agent_servers/src/claude.rs` (128 lines) | `acp/src/claude_code.rs` | Simplify binary discovery |
| `zed/crates/acp_thread/src/connection.rs` | `acp/src/types.rs` | Port `AgentConnection` trait |
| `zed/crates/acp_thread/src/terminal.rs` | `acp/src/terminal.rs` | Replace with portable-pty |

### Key Stubs Needed
```rust
// Minimal project stub
pub struct Project {
    pub root_path: PathBuf,
    pub is_local: bool,
}

// Minimal settings
pub struct AgentSettings {
    pub default_model: Option<String>,
    pub default_mode: Option<String>,
}
```

---

## Phase 2: MechaCoder Application (Week 2)

### Create `crates/mechacoder/Cargo.toml`
```toml
[package]
name = "mechacoder"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "MechaCoder"
path = "src/main.rs"

[dependencies]
acp = { path = "../acp" }
gpui = { path = "../gpui" }
ui = { path = "../ui" }
theme = { path = "../theme" }
agent-client-protocol = { version = "0.9.0", features = ["unstable"] }
similar = "2"
pulldown-cmark = "0.12"
anyhow = "1"
serde = { version = "1", features = ["derive"] }
```

### Files to Create
```
crates/mechacoder/src/
  lib.rs              # Library exports
  main.rs             # Binary entrypoint (like Commander)
  actions.rs          # GPUI actions
  app_menus.rs        # Application menus
  screen.rs           # MechaCoderScreen main view
  ui/
    mod.rs
    thread_view.rs    # Chat message list
    message_input.rs  # Message editor (wraps ui::TextInput)
    tool_call_view.rs # Tool call display
    diff_view.rs      # File diff with accept/reject
    terminal_view.rs  # Terminal output
    permission_prompt.rs # Tool permission dialog
    mode_selector.rs  # Mode picker
    model_selector.rs # Model picker
```

### Main.rs Pattern (from Commander)
```rust
fn main() {
    Application::new().run(|cx: &mut App| {
        cx.text_system().add_fonts(vec![...]).unwrap();
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-enter", SendMessage, None),
            KeyBinding::new("escape", CancelGeneration, None),
        ]);
        cx.set_menus(app_menus());
        cx.open_window(
            WindowOptions { ... },
            |window, cx| cx.new(|cx| MechaCoderScreen::new(cx))
        );
    });
}
```

---

## Phase 3: Thread View UI (Week 2-3)

### ThreadView Component
```rust
pub struct ThreadView {
    thread: Option<Entity<AcpThread>>,
    message_input: Entity<MessageInput>,
    entries: Vec<ThreadEntry>,
    list_state: ListState,
    pending_permission: Option<PermissionPrompt>,
}

pub enum ThreadEntry {
    UserMessage(UserMessage),
    AssistantMessage(AssistantMessage),
    ToolCall(ToolCallView),
}
```

### UI Components to Build
1. **MessageInput** - Wraps `ui::TextInput` with multi-line support
2. **ToolCallView** - Shows tool title, status, expand/collapse content
3. **DiffView** - Uses `similar` crate, shows hunks with accept/reject buttons
4. **TerminalView** - Plain text output display
5. **PermissionPrompt** - Uses `ui::Dialog` for tool approval

---

## Phase 4: Diff and Terminal Support (Week 3)

### Diff Implementation
```rust
// Using similar crate
pub struct DiffView {
    path: PathBuf,
    old_content: String,
    new_content: String,
    hunks: Vec<DiffHunk>,
    expanded: bool,
}

impl DiffView {
    fn compute_diff(&mut self) {
        let diff = similar::TextDiff::from_lines(&self.old_content, &self.new_content);
        self.hunks = diff.grouped_ops(3).iter().map(|group| {
            // Convert to DiffHunk with context
        }).collect();
    }
}
```

### Terminal Implementation
Using `portable-pty` directly:
```rust
pub struct TerminalOutput {
    terminal_id: acp::TerminalId,
    command_label: String,
    output: String,
    exit_status: Option<acp::TerminalExitStatus>,
}
```

---

## Phase 5: Polish and Integration (Week 4)

### Add to Workspace Cargo.toml
```toml
[workspace]
members = [
    # ... existing
    "crates/acp",
    "crates/mechacoder",
]

[workspace.dependencies]
agent-client-protocol = { version = "0.9.0", features = ["unstable"] }
similar = "2"
pulldown-cmark = "0.12"
portable-pty = "0.9"
```

### Key Bindings
```rust
actions!(
    mechacoder,
    [
        Quit, SendMessage, CancelGeneration,
        AcceptDiff, RejectDiff, AcceptAllDiffs, RejectAllDiffs,
        AllowToolOnce, AllowToolAlways, RejectTool,
        ToggleTerminalPanel, SelectMode, SelectModel,
    ]
);
```

---

## Critical Zed Files Reference

| File | Purpose | Lines |
|------|---------|-------|
| `zed/crates/agent_servers/src/acp.rs` | Core ACP stdio connection | 977 |
| `zed/crates/agent_servers/src/claude.rs` | ClaudeCode struct | 128 |
| `zed/crates/acp_thread/src/connection.rs` | AgentConnection trait | 471 |
| `zed/crates/acp_thread/src/acp_thread.rs` | Thread state pattern | 1200+ |
| `zed/crates/acp_thread/src/terminal.rs` | Terminal wrapper | 225 |
| `zed/crates/agent_ui/src/acp/thread_view.rs` | UI pattern reference | 300+ |

---

## Dependencies Summary

### External (add to workspace)
- `agent-client-protocol = "0.9.0"` - ACP protocol types
- `similar = "2"` - Diff computation
- `pulldown-cmark = "0.12"` - Markdown parsing
- `portable-pty = "0.9"` - Terminal spawning

### Internal (existing)
- `gpui` - Local fork for UI
- `ui` - Button, Dialog, TextInput, Card, etc.
- `theme` - Colors (bg::, text::, border::, status::)
- `collections` - HashSet, HashMap wrappers

---

## Risks and Mitigations

| Risk | Level | Mitigation |
|------|-------|------------|
| ACP protocol version mismatch | Medium | Track Zed's version closely |
| Complex diff rendering | Medium | Start with plain text, add syntax later |
| Terminal rendering | Medium | Use simple text output initially |
| Permission UX complexity | Low | Use existing Dialog component |

---

## Estimated Timeline

- **Week 1:** Phase 1 - ACP crate, connection, Claude Code spawn
- **Week 2:** Phase 2-3 - Main app, basic thread view
- **Week 3:** Phase 4 - Diff view, terminal output
- **Week 4:** Phase 5 - Polish, mode/model selection, permissions

---

## Success Criteria

1. `cargo mechacoder` launches GPUI window
2. Can authenticate with Claude Code
3. Send messages and receive streaming responses
4. View and accept/reject file diffs
5. View terminal output from tool calls
6. Handle permission prompts for tool execution
7. Switch modes and models
