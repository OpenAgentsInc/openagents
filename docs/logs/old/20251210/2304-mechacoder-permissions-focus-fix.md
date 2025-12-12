# MechaCoder: Permissions and Focus Fix

**Date:** 2024-12-10
**Status:** Complete

## Problem

MechaCoder had several UX issues:
1. Permission prompts were blocking tool execution
2. Text input wasn't focused when connected
3. Manual "Connect" button was unnecessary

## Solution

### 1. Default Mode: bypassPermissions

Changed from CLI flags to ACP API approach (matching Zed):

**Before:** Used `--dangerously-skip-permissions` CLI flag
**After:** Set `default_mode: "bypassPermissions"` via ACP `set_session_mode` API

```rust
// crates/acp/src/claude_code.rs
impl ClaudeCode {
    pub fn new() -> Self {
        Self {
            default_mode: Some("bypassPermissions".to_string()),
            default_model: None,
        }
    }
}
```

The mode is then applied via ACP protocol when creating a session:
```rust
// crates/acp/src/connection.rs (new_thread)
if let Some(default_mode) = default_mode {
    conn.set_session_mode(acp::SetSessionModeRequest::new(
        session_id,
        default_mode,
    )).await;
}
```

### 2. Auto-Connect on Launch

Removed manual "Connect" button - MechaCoder now auto-connects immediately:

```rust
// crates/mechacoder/src/screen.rs
pub fn new(cx: &mut Context<Self>) -> Self {
    // ... setup ...
    screen.connect(cx);  // Auto-connect immediately
    screen
}
```

### 3. Focus Input When Connected

Added `needs_focus` flag to focus the message input on first render after connection:

```rust
// crates/mechacoder/src/screen.rs
pub struct MechaCoderScreen {
    // ...
    needs_focus: bool,
}

impl Render for MechaCoderScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Focus the message input when we just connected
        if self.needs_focus {
            self.needs_focus = false;
            if let Some(thread_view) = &self.thread_view {
                let focus_handle = thread_view.read(cx).message_input_focus_handle(cx);
                focus_handle.focus(window);
            }
        }
        // ... render ...
    }
}
```

Added public method to ThreadView to expose focus handle:
```rust
// crates/mechacoder/src/ui/thread_view.rs
pub fn message_input_focus_handle(&self, cx: &App) -> FocusHandle {
    self.message_input.read(cx).focus_handle(cx)
}
```

### 4. Loading State UI

Shows "Connecting to Claude Code..." with disabled input while loading:
- `render_connecting()` - Loading state with disabled input
- `render_error()` - Error state with retry button
- `render_connected()` - Full thread view with active input

## Files Changed

- `crates/acp/src/claude_code.rs` - Default mode, removed CLI flags
- `crates/acp/src/connection.rs` - Mode set via ACP API (unchanged, just uses default_mode)
- `crates/mechacoder/src/screen.rs` - Auto-connect, needs_focus flag, loading UI
- `crates/mechacoder/src/ui/thread_view.rs` - Exposed message_input_focus_handle()

## Key Insight

Zed doesn't use `--dangerously-skip-permissions` as a CLI flag. Instead, it uses the ACP protocol's `set_session_mode` API with `bypassPermissions` mode. This is the correct approach as it goes through the proper ACP negotiation.

## Testing

```bash
cargo test -p acp          # All 4 tests pass
cargo build --release -p mechacoder  # Builds successfully
```

Verified manually: MechaCoder launches, auto-connects, focuses input, and tool calls execute without permission prompts.
