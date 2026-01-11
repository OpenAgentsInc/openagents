# Autopilot UI: Claude Agent SDK 100% Implementation Roadmap

> A comprehensive roadmap to achieve full parity with Claude Code CLI

## Executive Summary

This document outlines the path to 100% implementation of the Claude Agent SDK in the Autopilot desktop application. The goal is complete feature parity with Claude Code CLI, leveraging the existing wgpui component library.

**Current State**: ~15% complete (basic chat, model selection, streaming)
**Target State**: 100% SDK coverage + full CLI parity
**Estimated Scope**: ~10,000 additional lines of code

---

## Table of Contents

1. [Current State Assessment](#1-current-state-assessment)
2. [Phase 1: Core Infrastructure](#phase-1-core-infrastructure)
3. [Phase 2: Slash Commands](#phase-2-slash-commands)
4. [Phase 3: Permission System](#phase-3-permission-system)
5. [Phase 4: Session Management](#phase-4-session-management)
6. [Phase 5: Tool Visualization](#phase-5-tool-visualization)
7. [Phase 6: MCP & Custom Tools](#phase-6-mcp--custom-tools)
8. [Phase 7: Agents & Skills](#phase-7-agents--skills)
9. [Phase 8: Hooks System](#phase-8-hooks-system)
10. [Phase 9: Settings & Configuration](#phase-9-settings--configuration)
11. [Phase 10: Polish & Parity](#phase-10-polish--parity)
12. [SDK Method Coverage Checklist](#sdk-method-coverage-checklist)
13. [Component Mapping](#component-mapping)
14. [Success Criteria](#success-criteria)

---

## 1. Current State Assessment

### Implemented

| Feature | Status | Notes |
|---------|--------|-------|
| Query streaming | Done | Full async streaming via SDK |
| Model selection | Done | `/model` command with modal |
| Markdown rendering | Done | StreamingMarkdown + MarkdownRenderer |
| Tool call display | Done | Inline `[ToolName]` with formatted input |
| Status bar | Done | Permission mode, model, tools, session, backend |
| Auto-scroll | Done | Scroll to bottom on new content |
| Config persistence | Done | Model saved to ~/.openagents/autopilot/ |
| Multi-backend support | Done | Claude + Codex via `/backend` command |
| Wallet status pane | Done | OANIX identity + Pylon wallet config |
| DVM providers pane | Done | NIP-89 discovery for compute providers |
| Gateway health pane | Done | Cerebras gateway status + model list |
| LM router pane | Done | Backend auto-detect + health summary |
| Nexus stats pane | Done | Relay stats + event/job counters |
| Spark wallet pane | Done | Spark balance, addresses, and payments |
| OANIX manifest pane | Done | Hardware, compute, network, workspace overview |
| Workspace issues pane | Done | Issues.json list with status and priority |
| Workspace directives pane | Done | Directive list with status and progress |
| DSPy status pane | Done | Sessions + performance + auto-optimizer |
| NIP-28 chat pane | Done | Public Nostr chat with relay + channel |
| NIP-90 jobs pane | Done | Requests, results, and feedback monitor |
| Issue tracker pane | Done | Autopilot issues from .openagents/autopilot.db |
| RLM runs pane | Done | Recent RLM runs from ~/.openagents/pylon/rlm.db |
| RLM trace pane | Done | Trace events and run metadata for latest run |
| Pylon earnings pane | Done | Provider earnings from ~/.openagents/pylon/pylon.db |
| Pylon jobs pane | Done | Provider jobs from ~/.openagents/pylon/pylon.db |
| Agent backends pane | Done | CLI availability, model lists, and selection |

### Not Implemented

| Feature | Priority | SDK Support |
|---------|----------|-------------|
| All other slash commands | P0 | Yes |
| Permission prompts | P0 | Yes |
| Session history/resume | P0 | Yes |
| Tool results display | P0 | Yes |
| Interrupt/cancel | P0 | Yes |
| File browser | P1 | Via Glob tool |
| Diff viewer | P1 | Via Edit tool |
| Agent management | P1 | Yes |
| Skill management | P1 | Yes |
| MCP server config | P1 | Yes |
| Hook configuration | P2 | Yes |
| Budget/cost tracking | P2 | Yes |
| Thinking toggle | P2 | Yes |
| Keyboard shortcuts | P2 | N/A |
| Theme switching | P3 | N/A |

---

## Phase 1: Core Infrastructure

**Goal**: Foundation for all other features

### 1.1 Command System Architecture

```rust
// New file: src/commands.rs
pub enum Command {
    // Built-in commands
    Help,
    Clear,
    Compact,
    Model,
    Undo,
    Cancel,
    // Session commands
    SessionList,
    SessionResume(String),
    SessionFork,
    // Permission commands
    PermissionMode(PermissionMode),
    PermissionRules,
    // Tool commands
    ToolsEnable(Vec<String>),
    ToolsDisable(Vec<String>),
    // Config commands
    Config,
    OutputStyle(String),
    // Custom commands
    Custom(String, Vec<String>),
}

pub fn parse_command(input: &str) -> Option<Command>;
pub async fn execute_command(cmd: Command, state: &mut AppState) -> Result<()>;
```

### 1.2 State Management Refactor

```rust
// Expand RenderState into modular AppState
pub struct AppState {
    // Core
    pub session: SessionState,
    pub query: Option<ActiveQuery>,

    // UI state
    pub modal: ModalState,
    pub panels: PanelLayout,
    pub notifications: Vec<Notification>,

    // Config
    pub settings: UserSettings,
    pub permissions: PermissionState,

    // History
    pub command_history: Vec<String>,
    pub message_history: Vec<ChatMessage>,
}
```

### 1.3 Panel System

```rust
// Support for split views (chat, file browser, diff, terminal)
pub enum PanelLayout {
    Single,
    SplitVertical { left: Panel, right: Panel },
    SplitHorizontal { top: Panel, bottom: Panel },
    TriPane { left: Panel, top_right: Panel, bottom_right: Panel },
}

pub enum Panel {
    Chat,
    FileBrowser,
    DiffViewer,
    Terminal,
    Inspector,
    Settings,
}
```

### 1.4 Keyboard Shortcut System

```rust
pub struct Keybinding {
    pub key: Key,
    pub modifiers: Modifiers,
    pub action: Action,
}

pub enum Action {
    Submit,
    Cancel,
    Interrupt,
    OpenCommandPalette,
    TogglePanel(Panel),
    CycleModel,
    // ... more actions
}
```

### Tasks
- [x] Create `src/commands.rs` with command parsing
- [x] Refactor `RenderState` -> `AppState`
- [x] Implement `PanelLayout` system
- [x] Add keyboard shortcut registry
- [x] Wire up `Ctrl+C` for interrupt
- [x] Wire up `Ctrl+K` for command palette (or `/`)

---

## Phase 2: Slash Commands

**Goal**: Implement all Claude Code CLI slash commands

### 2.1 Built-in Commands

| Command | Description | Implementation |
|---------|-------------|----------------|
| `/help` | Show available commands | Modal with command list |
| `/clear` | Clear conversation | Reset messages, new session |
| `/compact` | Compress context | Call SDK compact, show boundary |
| `/model` | Select model | Already implemented |
| `/undo` | Undo last message | Remove last user+assistant pair |
| `/cancel` | Cancel current operation | Call `query.interrupt()` |
| `/bug` | Report a bug | Open GitHub issues URL |
| `/wallet` | Open wallet status | Modal with identity + config |
| `/wallet refresh` | Refresh wallet status | Reload config + OANIX |

### 2.2 Session Commands

| Command | Description | Implementation |
|---------|-------------|----------------|
| `/session list` | List recent sessions | Modal with SessionCard list |
| `/session resume <id>` | Resume session | QueryOptions::resume() |
| `/session fork` | Fork current session | QueryOptions::fork_session() |
| `/session export` | Export to markdown | Generate markdown file |

### 2.3 Permission Commands

| Command | Description | Implementation |
|---------|-------------|----------------|
| `/permission mode <m>` | Set permission mode | set_permission_mode() |
| `/permission rules` | Show rules | Modal with PermissionRuleRow |
| `/permission allow <tool>` | Allow a tool | Add to allow list |
| `/permission deny <tool>` | Deny a tool | Add to deny list |

### 2.4 Tool Commands

| Command | Description | Implementation |
|---------|-------------|----------------|
| `/tools` | List available tools | Modal with tool list |
| `/tools enable <t>` | Enable specific tools | Update QueryOptions |
| `/tools disable <t>` | Disable specific tools | Update disallowed_tools |

### 2.5 Config Commands

| Command | Description | Implementation |
|---------|-------------|----------------|
| `/config` | Open settings | Settings panel |
| `/output-style <s>` | Set output style | Load from .claude/output-styles/ |
| `/theme` | Toggle theme | Dark/light mode |

### 2.6 Custom Commands

| Feature | Description | Implementation |
|---------|-------------|----------------|
| Project commands | `.claude/commands/*.md` | Load and execute |
| User commands | `~/.claude/commands/*.md` | Load and execute |
| File references | `@filename` syntax | Inject file contents |
| Bash execution | `!command` syntax | Run and inject output |

### Tasks
- [x] Implement `/help` with command list modal
- [x] Implement `/clear` (reset session)
- [x] Implement `/compact` with SDK call
- [x] Implement `/undo` (remove last exchange)
- [x] Implement `/cancel` (interrupt query)
- [x] Implement `/bug` (open URL)
- [x] Implement `/wallet` + `/wallet refresh` commands
- [x] Implement `/session list` with SessionCard
- [x] Implement `/session resume <id>`
- [x] Implement `/session fork`
- [x] Implement `/session export`
- [x] Implement `/permission mode`
- [x] Implement `/permission rules`
- [x] Implement `/tools` listing
- [x] Implement `/config` panel
- [x] Implement `/output-style`
- [x] Load custom commands from `.claude/commands/`
- [x] Support `@filename` file references
- [x] Support `!command` bash execution

---

## Phase 3: Permission System

**Goal**: Full permission management UI

### 3.1 Permission Modes

| Mode | Behavior | UI |
|------|----------|-----|
| `default` | Ask for dangerous ops | Prompt dialog |
| `plan` | Read-only tools | Badge indicator |
| `acceptEdits` | Auto-accept edits | Badge indicator |
| `bypassPermissions` | Allow all | Warning badge |
| `dontAsk` | Deny if not pre-approved | Badge indicator |

### 3.2 Permission Prompt UI

**Use wgpui `PermissionDialog` organism**

```rust
pub struct PermissionPrompt {
    pub tool_name: String,
    pub tool_input: Value,
    pub risk_level: RiskLevel,
    pub options: Vec<PermissionOption>,
}

pub enum PermissionOption {
    Allow,
    AllowOnce,
    AllowForSession,
    Deny,
    DenyForSession,
    AddRule,
}
```

### 3.3 Permission Rules Management

**Use wgpui `PermissionRuleRow` molecule**

```rust
pub struct PermissionRule {
    pub tool_name: String,
    pub pattern: Option<String>,  // Regex for Bash commands
    pub behavior: PermissionBehavior,
    pub scope: RuleScope,
}

pub enum RuleScope {
    Session,
    Project,
    Global,
}
```

### 3.4 SDK Integration

```rust
// Replace dangerously_skip_permissions with proper handler
let handler = CallbackPermissionHandler::new(|request| {
    // Show PermissionDialog
    // Wait for user decision
    // Return PermissionResult
});

query_with_permissions(prompt, options, handler).await;
```

### Tasks
- [x] Remove `dangerously_skip_permissions(true)` default
- [x] Integrate `PermissionDialog` from wgpui
- [x] Implement permission mode switcher in status bar
- [x] Create permission rules modal
- [x] Implement `CallbackPermissionHandler` with UI
- [x] Support rule patterns for Bash commands
- [x] Persist permission rules to config
- [x] Show permission history

---

## Phase 4: Session Management

**Goal**: Full session lifecycle management

### 4.1 Session List UI

**Use wgpui `SessionCard` molecule**

```rust
pub struct SessionInfo {
    pub id: String,
    pub created_at: DateTime,
    pub last_message: String,
    pub message_count: u32,
    pub model: String,
    pub cost_usd: Option<f64>,
}
```

### 4.2 Session Operations

| Operation | SDK Method | UI |
|-----------|------------|-----|
| Create | `query()` | Auto on first message |
| Continue | `continue_session()` | Resume most recent |
| Resume | `resume(session_id)` | From session list |
| Fork | `fork_session()` | From session list or command |
| Export | N/A | Generate markdown |
| Delete | N/A | File system cleanup |

### 4.3 Session Storage

```
~/.openagents/autopilot/sessions/
├── index.json           # Session metadata index
├── <session_id>/
│   ├── messages.jsonl   # Full message history
│   ├── checkpoints/     # File checkpoints for rewind
│   └── metadata.json    # Session metadata
```

### 4.4 Checkpoint & Rewind

**Use wgpui `CheckpointRestore` molecule**

```rust
// SDK method
query.rewind_files(message_id).await?;
```

### Tasks
- [x] Create session list modal with SessionCard
- [x] Implement session metadata storage
- [x] Wire up `continue_session()` for resumption
- [x] Wire up `resume(session_id)` from list
- [x] Wire up `fork_session()` for forking
- [x] Implement session export to markdown
- [x] Integrate CheckpointRestore component
- [x] Wire up `rewind_files()` for checkpoint restore
- [x] Add session indicator to status bar

---

## Phase 5: Tool Visualization

**Goal**: Rich tool call display with inputs, outputs, progress

### 5.1 Tool Call Display Enhancements

**Use wgpui `ToolCallCard` organism**

| Tool | Display | wgpui Component |
|------|---------|-----------------|
| Glob | File list with icons | `SearchToolCall` |
| Grep | Matches with context | `SearchToolCall` |
| Read | File content preview | `CodePane` |
| Edit | Diff view | `DiffToolCall` |
| Write | File preview | `CodePane` |
| Bash | Terminal output | `TerminalToolCall` |
| Task | Agent status | `AgentStatusBadge` |

### 5.2 Tool Progress

**Use wgpui `ToolStatusBadge` atom**

```rust
// Already receiving from SDK
SdkMessage::ToolProgress(tp) => {
    // Show progress bar
    // Update elapsed time
    // Show tool name
}
```

### 5.3 Tool Results

```rust
pub struct ToolResult {
    pub tool_name: String,
    pub input: Value,
    pub output: Value,
    pub duration_ms: u64,
    pub success: bool,
    pub error: Option<String>,
}
```

### 5.4 Tool Cancellation

```rust
// Add cancel button during tool execution
if is_tool_running {
    show_cancel_button();
}

// On cancel click
query.abort();
```

### Tasks
- [x] Integrate `ToolCallCard` for rich display
- [x] Integrate `DiffToolCall` for Edit results
- [x] Integrate `TerminalToolCall` for Bash results
- [x] Integrate `SearchToolCall` for Glob/Grep
- [x] Show tool progress bar during execution
- [x] Display tool results (not just inputs)
- [x] Add cancel button during tool execution
- [x] Create tool history panel

---

## Phase 6: MCP & Custom Tools

**Goal**: MCP server configuration and custom tool support

### 6.1 MCP Server Configuration UI

```rust
pub enum McpServerConfig {
    Stdio { command: String, args: Vec<String>, env: HashMap<String, String> },
    Sse { url: String, headers: HashMap<String, String> },
    Http { url: String, headers: HashMap<String, String> },
}
```

### 6.2 MCP Status Display

```rust
// SDK method
let status = query.mcp_server_status().await?;

// Display in UI
for server in status.servers {
    show_mcp_status_badge(server.name, server.status);
}
```

### 6.3 MCP Configuration Modal

| Field | Type | Description |
|-------|------|-------------|
| Name | String | Server identifier |
| Type | Enum | stdio/sse/http |
| Command/URL | String | Server endpoint |
| Args | Vec<String> | Command arguments |
| Env | HashMap | Environment variables |

### 6.4 .mcp.json Integration

```json
{
  "servers": {
    "my-server": {
      "command": "my-mcp-server",
      "args": ["--port", "3000"],
      "env": { "API_KEY": "${MCP_API_KEY}" }
    }
  }
}
```

### Tasks
- [x] Create MCP configuration modal
- [x] Load `.mcp.json` from project root
- [x] Show MCP server status in status bar
- [x] Wire up `mcp_server_status()` SDK method
- [x] Support environment variable expansion
- [x] Add/remove MCP servers at runtime

---

## Phase 7: Agents & Skills

**Goal**: Custom agent and skill management

### 7.1 Agent Definition UI

**Use wgpui `AgentProfileCard` molecule**

```rust
pub struct AgentDefinition {
    pub name: String,
    pub description: String,
    pub prompt: String,
    pub tools: Option<Vec<String>>,
    pub disallowed_tools: Option<Vec<String>>,
    pub model: Option<AgentModel>,
}
```

### 7.2 Agent Sources

| Source | Location | Priority |
|--------|----------|----------|
| Programmatic | QueryOptions::agent() | Highest |
| Project | `.claude/agents/*.md` | Medium |
| User | `~/.claude/agents/*.md` | Lowest |

### 7.3 Skill Definition UI

**Use wgpui `SkillCard` molecule**

```rust
pub struct SkillDefinition {
    pub name: String,
    pub description: String,
    pub files: Vec<String>,  // Supporting files
    pub tools: Option<Vec<String>>,
}
```

### 7.4 Skill Sources

| Source | Location |
|--------|----------|
| Project | `.claude/skills/*/SKILL.md` |
| User | `~/.claude/skills/*/SKILL.md` |

### Tasks
- [x] Create agent management modal
- [x] Load agents from `.claude/agents/`
- [x] Integrate `AgentProfileCard` component
- [x] Create skill management modal
- [x] Load skills from `.claude/skills/`
- [x] Integrate `SkillCard` component
- [x] Show active agent in status bar
- [x] Support agent switching mid-session

---

## Phase 8: Hooks System

**Goal**: Hook configuration and monitoring

### 8.1 Hook Events

| Event | Description | Configurable |
|-------|-------------|--------------|
| PreToolUse | Before tool execution | Yes |
| PostToolUse | After tool success | Yes |
| PostToolUseFailure | After tool failure | Yes |
| UserPromptSubmit | User sends message | Yes |
| SessionStart | Session begins | Yes |
| SessionEnd | Session ends | Yes |
| Stop | Execution stops | Yes |
| SubagentStart | Subagent launched | Yes |
| SubagentStop | Subagent finished | Yes |
| PreCompact | Before compaction | Yes |
| Notification | System notification | Yes |
| PermissionRequest | Permission needed | Yes |

### 8.2 Hook Configuration UI

```rust
pub struct HookConfig {
    pub event: HookEvent,
    pub enabled: bool,
    pub pattern: Option<String>,  // Tool name pattern
    pub timeout: Duration,
    pub callback: HookCallback,
}
```

### 8.3 Built-in Hooks

| Hook | Purpose | Default |
|------|---------|---------|
| ToolBlocker | Block dangerous commands | Enabled |
| ToolLogger | Log tool executions | Disabled |
| OutputTruncator | Prevent context overflow | Enabled |
| ContextInjection | Inject CLAUDE.md | Enabled |
| TodoEnforcer | Enforce task completion | Disabled |

### Tasks
- [x] Create hook configuration panel
- [x] Implement built-in hook toggles
- [x] Show hook execution in event log
- [x] Integrate `EventInspector` component
- [x] Support custom hook scripts
- [x] Wire up SDK HookCallback system

---

## Phase 9: Settings & Configuration

**Goal**: Full settings management

### 9.1 Settings Categories

| Category | Settings |
|----------|----------|
| General | Theme, font size, auto-scroll |
| Model | Default model, thinking tokens |
| Permissions | Default mode, saved rules |
| Sessions | Auto-save, history limit |
| MCP | Server configurations |
| Hooks | Hook enable/disable |
| Keyboard | Shortcut customization |

### 9.2 Settings Storage

```
~/.openagents/autopilot/
├── config.toml           # Main configuration
├── permissions.json      # Permission rules
├── keybindings.json      # Custom keybindings
└── sessions/             # Session storage
```

### 9.3 Settings UI

**Tabbed settings panel**

```rust
pub enum SettingsTab {
    General,
    Model,
    Permissions,
    Sessions,
    MCP,
    Hooks,
    Keyboard,
}
```

### Tasks
- [x] Create settings panel with tabs
- [x] Implement general settings (theme, font)
- [x] Implement model settings (default, thinking)
- [x] Implement permission settings
- [x] Implement session settings
- [x] Implement MCP settings
- [x] Implement hook settings
- [x] Implement keyboard shortcut editor
- [x] Add settings search

---

## Phase 10: Polish & Parity

**Goal**: Final polish for 100% parity

### 10.1 UI Polish

| Feature | Component | Priority |
|---------|-----------|----------|
| Notifications | `Notifications` HUD | P1 |
| Error display | Styled error messages | P1 |
| Loading states | Progress indicators | P1 |
| Empty states | Placeholder content | P2 |
| Animations | Transitions | P3 |

### 10.2 Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Enter` | Submit message |
| `Ctrl+C` | Interrupt/cancel |
| `Ctrl+K` | Command palette |
| `Ctrl+L` | Clear |
| `Ctrl+M` | Model picker |
| `Ctrl+P` | Permission mode |
| `Ctrl+S` | Session list |
| `Ctrl+,` | Settings |
| `Escape` | Close modal |
| `Up/Down` | Command history |

### 10.3 Accessibility

- Focus management
- Keyboard navigation
- Screen reader support
- High contrast mode

### 10.4 Performance

- Virtual scrolling for long chats
- Lazy loading for session list
- Incremental rendering

### Tasks
- [ ] Integrate `Notifications` component
- [ ] Add styled error display
- [ ] Add loading indicators
- [ ] Add empty state placeholders
- [ ] Implement all keyboard shortcuts
- [ ] Add command history (Up/Down)
- [ ] Add focus management
- [ ] Optimize virtual scrolling
- [ ] Add performance monitoring

---

## SDK Method Coverage Checklist

### QueryOptions Builder Methods

- [ ] `cwd()` - Working directory
- [x] `model()` - Model selection
- [ ] `permission_mode()` - Permission mode
- [x] `dangerously_skip_permissions()` - Bypass (to remove)
- [ ] `max_turns()` - Turn limit
- [ ] `max_budget_usd()` - Cost limit
- [ ] `mcp_server()` - MCP configuration
- [x] `agent()` - Custom agents
- [x] `include_partial_messages()` - Streaming
- [ ] `continue_session()` - Continue session
- [ ] `resume()` - Resume by ID
- [ ] `fork_session()` - Fork session
- [x] `setting_sources()` - Load settings
- [ ] `tools()` - Tool configuration
- [ ] `disallowed_tools()` - Tool filtering
- [ ] `beta()` - Beta features
- [x] `hooks()` - Hook callbacks

### Query Control Methods

- [ ] `interrupt()` - Graceful stop
- [ ] `abort()` - Hard kill
- [ ] `set_permission_mode()` - Change mode
- [ ] `set_model()` - Change model
- [ ] `set_max_thinking_tokens()` - Thinking limit
- [ ] `mcp_server_status()` - MCP status
- [ ] `rewind_files()` - Checkpoint restore
- [ ] `supported_commands()` - Slash commands
- [ ] `supported_models()` - Model list
- [ ] `account_info()` - User info
- [x] `session_id()` - Get session ID
- [ ] `is_completed()` - Check completion

### Message Types

- [x] `SdkMessage::Assistant` - Responses
- [x] `SdkMessage::StreamEvent` - Streaming
- [x] `SdkMessage::System` - System messages
- [x] `SdkMessage::ToolProgress` - Progress (partial)
- [x] `SdkMessage::Result` - Completion
- [x] `SdkMessage::AuthStatus` - Auth status
- [x] `SdkMessage::User` - User echo

### Permission System

- [ ] `AllowAllPermissions` - Allow all
- [ ] `DenyAllPermissions` - Deny all
- [ ] `RulesPermissionHandler` - Rule-based
- [ ] `CallbackPermissionHandler` - Callback-based
- [ ] `PermissionRules` builder

### Hooks

- [ ] `PreToolUse` - Before tool
- [ ] `PostToolUse` - After tool
- [ ] `PostToolUseFailure` - After failure
- [ ] `UserPromptSubmit` - User message
- [ ] `SessionStart` - Session start
- [ ] `SessionEnd` - Session end
- [ ] `Stop` - Stop event
- [ ] `SubagentStart` - Subagent start
- [ ] `SubagentStop` - Subagent stop
- [ ] `PreCompact` - Before compact
- [ ] `Notification` - Notifications
- [ ] `PermissionRequest` - Permission request

---

## Component Mapping

### wgpui -> Feature Mapping

| wgpui Component | Feature | Priority |
|-----------------|---------|----------|
| `PermissionDialog` | Permission prompts | P0 |
| `SessionCard` | Session list | P0 |
| `ToolCallCard` | Tool display | P0 |
| `DiffToolCall` | Edit visualization | P1 |
| `TerminalToolCall` | Bash output | P1 |
| `SearchToolCall` | Glob/Grep results | P1 |
| `CommandPalette` | Command search | P1 |
| `ThinkingToggle` | Extended thinking | P1 |
| `AgentProfileCard` | Agent management | P1 |
| `SkillCard` | Skill management | P1 |
| `CheckpointRestore` | Session rewind | P2 |
| `EventInspector` | Hook monitoring | P2 |
| `AgentStateInspector` | Agent debugging | P2 |
| `Notifications` | Toast messages | P2 |
| `TerminalPane` | Terminal panel | P2 |
| `CodePane` | Code panel | P2 |

---

## Success Criteria

### 100% SDK Coverage
- [ ] All QueryOptions methods accessible via UI
- [ ] All Query control methods wired up
- [ ] All message types properly handled
- [ ] Full permission system implementation
- [ ] Complete hook system support

### 100% CLI Parity
- [ ] All slash commands implemented
- [ ] Session management (list, resume, fork, export)
- [ ] Custom command support (`.claude/commands/`)
- [ ] Agent support (`.claude/agents/`)
- [ ] Skill support (`.claude/skills/`)
- [ ] MCP configuration (`.mcp.json`)
- [ ] Output styles (`.claude/output-styles/`)
- [ ] CLAUDE.md integration

### UI Completeness
- [ ] All tool types have rich visualization
- [ ] Permission prompts work correctly
- [ ] Settings panel with all categories
- [ ] Keyboard shortcuts functional
- [ ] Notifications and error handling
- [ ] Responsive and performant

### Quality Metrics
- [ ] All existing wgpui tests pass (377+)
- [ ] New integration tests for each phase
- [ ] No regressions in existing functionality
- [ ] Performance: <16ms frame time
- [ ] Memory: <200MB baseline

---

*Last updated: 2026-01-11*

## Execution Log
- 2026-01-09 07:58 UTC - Added command, panel, and keybinding modules; refactored Autopilot UI state to AppState with new core fields.
- 2026-01-09 08:07 UTC - Wired command parsing/execution, command palette modal, keybindings for Ctrl+C/Ctrl+K, and interrupt plumbing; ran `cargo check -p autopilot`.
- 2026-01-09 08:20 UTC - Added session tracking scaffolding, session index/message persistence helpers, and captured tools/output-style metadata from SystemInit.
- 2026-01-09 08:34 UTC - Implemented Phase 2 command handlers, session/tool/permission/config modals, output-style wiring, resume/fork/export logic, and permission/tool state plumbing.
- 2026-01-09 08:38 UTC - Added custom command loading, @file and !command prompt expansion, and updated prompt submission to inject file/command contents; ran `cargo check -p autopilot`.
- 2026-01-09 09:06 UTC - Added permission config persistence, Bash rule patterns, permission queue/history state, and permission handler plumbing for prompts.
- 2026-01-09 09:08 UTC - Wired PermissionDialog UI/input flow, status bar permission mode cycling, permission history display, and re-exported PermissionType; ran `cargo check -p autopilot`.
- 2026-01-09 09:31 UTC - Implemented SessionCard modal layout, session metadata/checkpoint persistence, checkpoint restore UI with rewind_files control, session indicator updates, and user message id capture; ran `cargo check -p autopilot`.
- 2026-01-09 10:08 UTC - Added tool tracking state, tool result parsing/output helpers, and detail builders for Bash/Search/Edit tool visuals.
- 2026-01-09 10:12 UTC - Implemented tool history panel with ToolCallCard rendering, progress bar/cancel handling, tool input events, and viewport adjustments; ran `cargo check -p autopilot`.
- 2026-01-09 10:28 UTC - Implemented MCP config loading with env expansion, runtime add/remove commands, status polling, status bar summary, and MCP configuration modal; ran `cargo check -p autopilot`.
- 2026-01-09 11:08 UTC - Added agent/skill loading with frontmatter parsing, agent/skill modals + commands, active agent status/prompt wiring, and skills setting_sources; ran `cargo check -p autopilot`.
- 2026-01-09 12:02 UTC - Implemented hooks runtime (built-ins + scripts), wired hook logging/inspector, connected hooks to query options, and ran `cargo check -p autopilot`.
- 2026-01-09 13:10 UTC - Added settings persistence (config.toml + keybindings.json), wired theme/font/auto-scroll/session limits into runtime and query options, and ran `cargo check -p autopilot`.
- 2026-01-09 13:24 UTC - Built tabbed settings modal with search, permissions/MCP/hooks/keyboard controls, keybinding capture, and theme-aware UI updates.
- 2026-01-09 - Implemented true Autopilot loop (`autopilot_loop.rs`): autonomous execution that loops Adjutant until task succeeds with verification (cargo check + cargo test), definitive failure, max iterations (10), or user interrupt (Escape).
- 2026-01-10 10:47 UTC - Refactored Autopilot UI rendering: split the pipeline into `app/ui/rendering/` submodules (base/layout/modals/etc), extracted layout helpers/constants, and delegated `AutopilotApp::render` to `render_app`.
- 2026-01-10 11:22 UTC - Split `app.rs` into focused app_entry submodules (application, commands, hooks, settings, state_actions), keeping the entrypoint slim while preserving behavior.
- 2026-01-10 16:56 UTC - Added DSPy status pane with auto-optimizer controls, keybinding, and command palette entry.
- 2026-01-10 17:13 UTC - Added NIP-28 chat pane with relay/channel controls, keybinding, and chat input.
- 2026-01-10 17:34 UTC - Hardened NIP-28 pane with channel id detection, post-auth setup retry, and unit tests.
- 2026-01-10 18:05 UTC - Added OANIX manifest pane with keybinding, commands, and refresh handling.
- 2026-01-10 18:16 UTC - Added gateway health pane with keybinding, commands, and model list view.
- 2026-01-10 18:37 UTC - Added LM router pane with backend auto-detect, health status, and model list.
- 2026-01-10 18:47 UTC - Added Nexus relay stats pane with health/status, counters, and kind summaries.
- 2026-01-10 19:00 UTC - Added Spark wallet pane with balances, addresses, and payment history.
- 2026-01-10 19:02 UTC - Added NIP-90 jobs pane with relay subscription, keybinding, and command palette entry.
- 2026-01-10 19:46 UTC - Added DVM providers pane with NIP-89 discovery, keybinding, and commands.
- 2026-01-10 20:18 UTC - Added workspace issues pane with issue list, status summaries, keybinding, and commands.
- 2026-01-10 20:36 UTC - Added workspace directives pane with status summary, progress, keybinding, and commands.
- 2026-01-10 21:02 UTC - Added issue tracker pane for autopilot.db with summaries, keybinding, and commands.
- 2026-01-10 21:24 UTC - Added RLM runs pane backed by rlm.db with summary metrics, keybinding, and commands.
- 2026-01-10 21:48 UTC - Added RLM trace pane with run metadata, trace event list, keybinding, and commands.
- 2026-01-10 22:12 UTC - Added Pylon earnings pane with totals, source breakdown, keybinding, and commands.
- 2026-01-10 22:34 UTC - Added Pylon jobs pane with status counts, pricing totals, keybinding, and commands.
- 2026-01-10 - Implemented multi-backend support (Claude/Codex): agent abstraction layer with AgentBackend/AgentSession traits, `/backend` toggle command, and full Codex integration via codex-agent-sdk with event mapping to ResponseEvent.
- 2026-01-10 23:02 UTC - Added agent backends pane with CLI availability, model selection, keybinding, and commands.
- 2026-01-11 06:01 UTC - Wired Autopilot CLI to use ACP streaming + DSPy loop for tasks/issues, added Codex executor support, and backend selection flags/env wiring.
- 2026-01-11 06:10 UTC - Added ACP tool-result updates for Claude streaming, aligned CLI command name with autopilot alias, and captured CLI renderer tool completion output.
- 2026-01-11 06:25 UTC - Fixed dsrs sync LM init to avoid nested tokio runtime panics and validated `autopilot-cli run` with ACP/DSPy loop.
- 2026-01-11 06:44 UTC - Auto-selected Ollama model for DSPy/Pylon local runs, fixed Claude ACP XML filtering, and validated CLI runs across local-tools/Codex/Claude backends.
- 2026-01-11 06:52 UTC - Filtered tool XML tags from DSPy streaming tokens and validated `cargo autopilot run` with Codex backend.
