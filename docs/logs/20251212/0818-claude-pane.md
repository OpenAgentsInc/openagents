# Claude Panel UI Plan

Add a unified "Claude Panel" to MechaCoder that exposes claude-agent-sdk functionality, following the Gym Panel pattern.

## Layout Decision

**Two panels, same location, one at a time:**
- `Cmd+C` opens Claude Panel (closes Gym if open)
- `Cmd+G` opens Gym Panel (closes Claude if open)
- Same 320px right sidebar slot

## Overview

Create a single `ClaudePanel` component with collapsible sections, ordered by day-to-day usefulness:
1. **Cost** - Session spending, per-model breakdown, token usage
2. **Model** - Switch models, see descriptions
3. **Session** - Resume/fork sessions, history
4. **Account** - Auth status, email, org, subscription
5. **Tools** - MCP servers, slash commands, available tools

## UI Layout

```
+----------------------------------+
| CLAUDE              [Cmd+C close]|
+----------------------------------+
| COST                        [-]  |
|  $0.42 total                     |
|  sonnet-4-5: $0.38               |
|  haiku-4-5:  $0.04               |
|  12.4K in / 3.2K out             |
+----------------------------------+
| MODEL                       [-]  |
|  [claude-sonnet-4-5          v]  |
|  Balanced speed and capability   |
+----------------------------------+
| SESSION                     [-]  |
|  abc12345            [Copy]      |
|  [Fork] [Resume Previous...]     |
|  Recent: abc12345, xyz98765...   |
+----------------------------------+
| ACCOUNT                     [+]  |
+----------------------------------+
| TOOLS                       [+]  |
+----------------------------------+
```

## Implementation Phases

### Phase 1: Cost Tracking (Day 1)
Most impactful for day-to-day use. Data already in `SdkResultMessage::Success`.

**Files to create:**
- `crates/mechacoder/src/panels/claude_panel.rs` - Main panel component

**Files to modify:**
- `crates/mechacoder/src/panels/mod.rs` - Export ClaudePanel
- `crates/mechacoder/src/screen.rs` - Add panel entity, exclusive visibility (Claude xor Gym), Cmd+C binding
- `crates/mechacoder/src/actions.rs` - Add `ToggleClaudePanel` action
- `crates/mechacoder/src/main.rs` - Add keybinding
- `crates/mechacoder/src/sdk_thread.rs` - Add cost tracking state

**Screen state change:**
```rust
// Replace:
gym_panel_visible: bool,
// With:
active_panel: Option<ActivePanel>,

enum ActivePanel {
    Gym,
    Claude,
}
```

**State additions to SdkThread:**
```rust
pub struct CostTracker {
    pub total_cost_usd: f64,
    pub model_usage: HashMap<String, ModelUsage>,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
}
```

**Pattern:** Parse `SdkResultMessage::Success` in sdk_thread.rs, accumulate costs, emit event.

### Phase 2: Model Selector (Day 2)
Quick model switching. Requires new SDK control request.

**Files to modify:**
- `crates/claude_agent_sdk/src/query.rs` - Add `supported_models()` method
- `crates/mechacoder/src/panels/claude_panel.rs` - Add model dropdown section
- `crates/mechacoder/src/sdk_thread.rs` - Store available models, current model

**SDK addition:**
```rust
// In Query
pub async fn supported_models(&mut self) -> Result<Vec<ModelInfo>> {
    self.send_control_request(SdkControlRequest::SupportedModels).await
}
```

**UI:** Dropdown following gym_panel.rs pattern with `deferred()` + `occlude()`.

### Phase 3: Session Management (Day 3)
Resume/fork workflows. Requires local session storage.

**Files to create:**
- `crates/mechacoder/src/services/session_store.rs` - SQLite-based session history

**Files to modify:**
- `crates/mechacoder/src/panels/claude_panel.rs` - Add session section
- `crates/mechacoder/src/sdk_thread.rs` - Track session_id, emit on change

**Session history entry:**
```rust
pub struct SessionHistoryEntry {
    pub session_id: String,
    pub started_at: i64,       // Unix timestamp
    pub last_active: i64,
    pub summary: String,       // First user message
    pub cost: f64,
}
```

**Storage:** Use `bun:sqlite` pattern or Rust `rusqlite`. Store in `~/.mechacoder/sessions.db`.

### Phase 4: Account Info (Day 4)
Auth visibility. Single control request.

**Files to modify:**
- `crates/claude_agent_sdk/src/query.rs` - Add `account_info()` method
- `crates/mechacoder/src/panels/claude_panel.rs` - Add account section
- `crates/mechacoder/src/sdk_thread.rs` - Store AccountInfo

**SDK addition:**
```rust
pub async fn account_info(&mut self) -> Result<AccountInfo> {
    self.send_control_request(SdkControlRequest::AccountInfo).await
}
```

**Display:** Email, org, subscription_type, token_source with status indicator.

### Phase 5: Tools/MCP Display (Day 5)
Reference info. Data available in `SystemInit` message.

**Files to modify:**
- `crates/mechacoder/src/panels/claude_panel.rs` - Add tools section
- `crates/mechacoder/src/sdk_thread.rs` - Store tools, mcp_servers, slash_commands from SystemInit

**Display:**
- MCP servers with status indicators (connected/error)
- Slash commands list (name + description)
- Tool count

## Keybindings

| Shortcut | Action |
|----------|--------|
| `Cmd+C` / `Ctrl+C` | Toggle Claude Panel |
| `Cmd+Shift+M` | Quick model switcher (future) |

## Critical Files

| File | Purpose |
|------|---------|
| `crates/mechacoder/src/panels/gym_panel.rs` | Pattern to follow |
| `crates/mechacoder/src/screen.rs` | Panel integration |
| `crates/mechacoder/src/sdk_thread.rs` | State management |
| `crates/claude_agent_sdk/src/query.rs` | Add control request methods |
| `crates/claude_agent_sdk/src/protocol/control.rs` | Control request types |

## Component Structure

```rust
pub struct ClaudePanel {
    focus_handle: FocusHandle,
    // Section visibility
    cost_expanded: bool,
    model_expanded: bool,
    session_expanded: bool,
    account_expanded: bool,
    tools_expanded: bool,
    // State
    cost_tracker: CostTracker,
    available_models: Vec<ModelInfo>,
    current_model: Option<String>,
    model_dropdown_open: bool,
    session_id: Option<String>,
    session_history: Vec<SessionHistoryEntry>,
    account_info: Option<AccountInfo>,
    tools: Vec<String>,
    mcp_servers: Vec<McpServerStatus>,
    slash_commands: Vec<SlashCommand>,
}

pub enum ClaudePanelEvent {
    ModelChanged { model: String },
    SessionFork,
    SessionResume { session_id: String },
}
```

## Event Flow

1. User clicks button/dropdown in ClaudePanel
2. ClaudePanel emits `ClaudePanelEvent`
3. MechaCoderScreen receives via subscription
4. Screen calls appropriate SDK method (set_model, create_session, etc.)
5. SDK response updates SdkThread state
6. SdkThread emits event
7. ClaudePanel subscribes and updates UI

## Notes

- Follow Gym Panel dropdown pattern exactly: `deferred()` + `occlude()` + `with_priority(1)`
- Use `theme_oa` colors: `bg::CARD`, `border::DEFAULT`, `text::PRIMARY`
- Collapsible sections use `[+]`/`[-]` indicators
- Cost section always expanded by default, Account/Tools collapsed
- Copy button for session ID uses clipboard API
