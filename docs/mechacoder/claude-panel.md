# Claude Panel - SDK Feature Exposure

## Overview

The Claude Panel is a dedicated sidebar component in MechaCoder that exposes all major claude-agent-sdk functionality. It's toggled with **Cmd+C** (or Ctrl+C on Linux) and occupies the same 320px sidebar slot as the Gym Panel, ensuring only one panel is visible at a time.

## Architecture

### Components

```
Screen (main_ui.rs)
├── ThreadView (thread_view.rs) - displays conversation
├── GymPanel (gym_panel.rs) - Terminal-Bench training interface
└── ClaudePanel (claude_panel.rs) - SDK feature exposure
    ├── Cost Tracking Section
    ├── Model Selection Section
    ├── Session Management Section
    ├── Account Information Section
    └── Tools & MCP Display Section
```

### Event Flow

1. **User Action**: User clicks button in ClaudePanel
2. **Panel Event**: Panel emits `ClaudePanelEvent` (ModelChanged, SessionFork, etc.)
3. **Screen Handler**: `screen.rs` catches event and processes it
4. **SDK Thread**: Updates propagate through `SdkThread` state
5. **Events**: `SdkThreadEvent` emitted (CostUpdated, ToolsUpdated, etc.)
6. **UI Update**: `ClaudePanel` subscribes and renders new state

### State Management

- **SdkThread** (server-side state):
  - cost_tracker: CostTracker
  - available_models: Vec<ModelInfo>
  - account_info: Option<AccountInfo>
  - tools: Vec<String>
  - mcp_servers: Vec<(String, String)>
  - session_id: Option<String>

- **ClaudePanel** (UI state):
  - Same fields as SdkThread + UI-specific state
  - model_dropdown_open: bool
  - Section expansion states (cost_expanded, model_expanded, etc.)

## Features

### Phase 1: Cost Tracking
- **Real-time monitoring** of API costs during conversation
- **Per-model breakdown** showing cost for each model used
- **Token counting** displays input/output token totals
- **Always expanded** by default since this is critical data

**Implementation Details**:
- Subscribes to `SdkThreadEvent::CostUpdated`
- Extracts cost data from `SdkResultMessage::Success`
- Calculates per-model costs: input_tokens * 0.00001 + output_tokens * 0.00003

### Phase 2: Model Selection
- **Dropdown UI** showing available models from SDK
- **Auto-selection** of first model when available
- **Display info** includes model name, value, and description
- **Status-aware** emits `ModelChanged` event when user selects

**Implementation Details**:
- Calls `query("", Default::default()).await.supported_models()` on connection
- Uses GPUI deferred rendering pattern for dropdown
- Deferred div prevents rendering conflicts with parent element destruction

### Phase 3: Session Management
- **Session ID display** in monospace font for easy copying
- **[Copy] button** (currently logs action, ready for clipboard)
- **[Fork] button** to create new session branch
- **[History...] button** placeholder for future session modal
- **"No active session" state** handling

**Implementation Details**:
- Extracts session_id from `SdkResultMessage::Init`
- Emits `SdkThreadEvent::SessionUpdated` on change
- EventEmitter pattern allows parent to respond to fork requests

### Phase 4: Account Information
- **Email display** from account_info() SDK call
- **Organization** name when available
- **Subscription type** (e.g., "pro", "enterprise")
- **Token source** (e.g., "api_key", "oauth")
- **Conditional rendering** for optional fields

**Implementation Details**:
- Calls `query("", Default::default()).await.account_info()` on connection
- Uses `Option<String>` fields to handle missing data gracefully
- Only renders sections if fields are present (`.when(field.is_some(), ...)`)

### Phase 5: Tools & MCP Display
- **Tools list** with bullet points showing available tool names
- **MCP servers** with status indicators
- **Color-coded status**:
  - Green for "ready"
  - Red for "error"
  - Yellow for other states
- **Empty state** shows "No tools or MCP servers"

**Implementation Details**:
- Extracts from `SdkSystemMessage::Init` message
- Converts McpServerStatus objects to (name, status) tuples
- Emits both `SessionUpdated` AND `ToolsUpdated` on connection
- Status colors use theme tokens (status::SUCCESS, status::ERROR, etc.)

## UI Patterns

### Section Headers
```rust
div()
    .flex()
    .flex_row()
    .items_center()
    .justify_between()
    .cursor_pointer()
    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
        this.toggle_session(cx);
    }))
    .child(div().child("SESSION"))
    .child(div().child(if is_open { "[-]" } else { "[+]" }))
```

### Collapsible Content
```rust
.when(is_open, |el| {
    el.mt(px(8.0))
        .child(content)
})
```

### Dropdown Pattern (Deferred)
```rust
gpui::deferred(
    div()
        .absolute()
        .top(px(36.0))
        .occlude()
        .children(items)
).with_priority(1)
```

## Background Tasks

### Model Fetching
```rust
cx.spawn(async move |_this, cx| {
    match query("", Default::default()).await {
        Ok(stream) => {
            match stream.supported_models().await {
                Ok(models) => {
                    let _ = thread_clone.update(cx, |thread, cx| {
                        thread.set_available_models(models, cx);
                    });
                }
                Err(e) => log::error!("Failed to fetch models: {}", e),
            }
        }
        Err(e) => log::error!("Failed to create query: {}", e),
    }
}).detach();
```

### Pattern Notes
- Use `cx.spawn()` for async operations
- No blocking on main UI thread
- Detach tasks that don't need to be tracked
- Update entities through `thread_clone.update(cx, ...)`
- Always log errors for debugging

## Future Enhancements

### Short-term (Implementable)
- [ ] Copy to clipboard for session ID (infrastructure ready)
- [ ] SetModel control request when dropdown changes
- [ ] Session history modal
- [ ] MCP server status real-time polling
- [ ] Slash command list display

### Medium-term (Requires SDK additions)
- [ ] Permission mode selector
- [ ] Max thinking tokens slider
- [ ] Budget tracker with warnings
- [ ] Model cost comparisons
- [ ] Rewind files interface

### Long-term (Architecture changes)
- [ ] Plugin management UI
- [ ] Custom agent creation in UI
- [ ] Session branching visualization
- [ ] Multi-session comparison

## Testing

### Manual Testing Checklist
- [ ] Panel opens/closes with Cmd+C
- [ ] Cost updates in real-time during messages
- [ ] Model dropdown shows all available models
- [ ] Selecting model updates display
- [ ] Session ID shows when connection established
- [ ] Account info displays all available fields
- [ ] Tools and MCP servers list correctly
- [ ] All sections collapse/expand independently
- [ ] No warnings during compilation
- [ ] No visual glitches during re-renders

### Common Issues
- **Panel not showing**: Check `active_panel` state in screen.rs
- **No models loading**: Verify `supported_models()` returns data
- **Cost not updating**: Check `SdkUpdate::Cost` handling in sdk_thread.rs
- **Dropdown stays open**: Verify `model_dropdown_open` is toggled correctly

## Files Modified

### Created
- `crates/mechacoder/src/panels/claude_panel.rs` (850+ lines)

### Modified
- `crates/mechacoder/src/panels/mod.rs` - Added ClaudePanel export
- `crates/mechacoder/src/actions.rs` - Added ToggleClaudePanel action
- `crates/mechacoder/src/main.rs` - Added Cmd+C keybinding
- `crates/mechacoder/src/screen.rs` - Panel management and subscriptions
- `crates/mechacoder/src/sdk_thread.rs` - State and event management
- `crates/mechacoder/src/ui/thread_view.rs` - Event handlers
- `crates/claude_agent_sdk/src/options.rs` - Fixed CLI flag name

## Performance Considerations

- **Deferred rendering**: Dropdown rendered separately to avoid conflicts
- **Subscription pattern**: Only updates when events occur (not on every render)
- **Background tasks**: All async work done off main thread
- **Lazy initialization**: Models/account info fetched only on connection
- **Cache management**: MessageView cache in ThreadView prevents re-renders

## Theme Integration

Uses centralized theme tokens from `theme_oa` crate:
- `bg::CARD`, `bg::SURFACE`, `bg::ELEVATED` - backgrounds
- `border::DEFAULT`, `border::FOCUS`, `border::SUBTLE` - borders
- `text::PRIMARY`, `text::SECONDARY`, `text::MUTED` - text colors
- `status::SUCCESS`, `status::ERROR`, `status::WARNING` - status indicators
- `FONT_FAMILY` - monospace font for IDs and technical info

## Related Documentation

- `docs/mechacoder/README.md` - MechaCoder overview
- `docs/SYNTHESIS.md` - Product vision and architecture
- `crates/claude_agent_sdk/src/lib.rs` - SDK documentation
