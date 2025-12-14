# OpenCode Port - Session Log

**Date:** 2024-12-14
**Session Start:** 09:17
**Objective:** Continue porting opencode project (~80% of AI/chat logic) into Coder

## Context

This session is a continuation of previous work porting the opencode TypeScript project into Rust for the Coder application. The goal is to port all AI/chat logic while using the existing HUD for UI (not opencode's TUI).

### Previous Sessions Completed
- Created `crates/llm/` with Provider trait and core types
- Implemented Anthropic provider with SSE streaming
- Created `crates/coder/storage/` with SQLite backend
- Extended `crates/coder/domain/` with session/permission events
- Created `crates/tool_registry/` with Tool trait
- Created `crates/coder/permission/` with async ask pattern
- Created `crates/coder/session/` with processor and prompt builder

### This Session's Tasks
1. Integrate llm provider registry with mechacoder
2. Create crates/coder/agent/ with built-in agents

---

## Task 1: Integrate LLM Provider Registry with Mechacoder

### Files Modified

#### `crates/mechacoder/Cargo.toml`
Added dependencies for llm provider integration:
```toml
tracing = { workspace = true, optional = true }
llm = { path = "../llm", optional = true }

[features]
server = ["dep:claude-agent-sdk", "dep:tokio", "dep:futures", "dep:llm", "dep:tracing"]
```

#### `crates/mechacoder/src/lib.rs`
Added provider module export:
```rust
#[cfg(feature = "server")]
pub mod provider;

#[cfg(feature = "server")]
pub use provider::{run_conversation, run_provider_session};
```

#### `crates/mechacoder/src/router.rs`
Added Anthropic backend support:

1. **New Backend Variant:**
```rust
pub enum Backend {
    ClaudeCode,
    Anthropic,  // NEW
    OpenAI,
    Ollama,
    Pi,
    OpenAgentsCloud,
}
```

2. **New Helper Methods:**
```rust
impl Backend {
    pub fn default_model(&self) -> Option<&'static str> {
        match self {
            Backend::Anthropic => Some("claude-sonnet-4-20250514"),
            Backend::OpenAI => Some("gpt-4o"),
            Backend::Ollama => Some("llama3.2"),
            _ => None,
        }
    }

    pub fn provider_id(&self) -> Option<&'static str> {
        match self {
            Backend::Anthropic => Some("anthropic"),
            Backend::OpenAI => Some("openai"),
            Backend::Ollama => Some("ollama"),
            _ => None,
        }
    }
}
```

3. **Updated Routing Priority:**
   - ClaudeCode (most capable, uses claude CLI)
   - Anthropic (API direct, requires ANTHROPIC_API_KEY)
   - Ollama (local, no API key)
   - Pi (always available)

4. **Backend Detection:**
   - Added `ANTHROPIC_API_KEY` environment variable check

### Files Created

#### `crates/mechacoder/src/provider.rs`
New module providing LLM provider-based session handling:

**Key Functions:**

1. `run_provider_session()` - Single message streaming:
```rust
pub async fn run_provider_session(
    provider_id: &str,
    model: &str,
    message: String,
    system_prompt: Option<String>,
    tools: Vec<Tool>,
    tx: mpsc::UnboundedSender<ServerMessage>,
    registry: Arc<ProviderRegistry>,
)
```

2. `run_conversation()` - Multi-turn with tool execution:
```rust
pub async fn run_conversation(
    provider_id: &str,
    model: &str,
    messages: Vec<Message>,
    system_prompt: Option<String>,
    tools: Vec<Tool>,
    tx: mpsc::UnboundedSender<ServerMessage>,
    registry: Arc<ProviderRegistry>,
    max_turns: usize,
)
```

3. `convert_stream_event()` - Maps `llm::StreamEvent` to `ServerMessage`:
   - `StreamEvent::TextDelta` → `ServerMessage::TextDelta`
   - `StreamEvent::ToolInputStart` → `ServerMessage::ToolStart`
   - `StreamEvent::ToolInputDelta` → `ServerMessage::ToolInput`
   - `StreamEvent::ToolResult` → `ServerMessage::ToolResult`
   - `StreamEvent::Error` → `ServerMessage::Done { error }`

### Tests
Added 3 new tests to router.rs:
- `test_backend_provider_id`
- `test_backend_default_model`
- `test_router_anthropic_priority`

**Total: 6 tests pass**

---

## Task 2: Create Agent Definitions Crate

### Reference: opencode Agent Pattern

Studied `~/code/opencode/packages/opencode/src/agent/agent.ts`:
- Agent definitions with name, description, mode, tools, permissions
- Built-in agents: general, explore, build, plan
- Permission configurations per agent
- Custom prompts per agent type

### Files Created

#### `crates/coder/agent/Cargo.toml`
```toml
[package]
name = "coder_agent"
description = "Agent definitions and configuration for Coder"

[dependencies]
coder_domain = { path = "../domain" }
coder_permission = { path = "../permission" }
tool_registry = { path = "../../tool_registry" }
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
indexmap.workspace = true
```

#### `crates/coder/agent/src/lib.rs`
Module exports:
```rust
mod definition;
mod permission;
mod registry;

pub use definition::*;
pub use permission::*;
pub use registry::*;
```

#### `crates/coder/agent/src/definition.rs`
Core types:

1. **AgentMode:**
```rust
pub enum AgentMode {
    Subagent,  // Spawned by primary agents
    Primary,   // User-selectable
    All,       // Any context
}
```

2. **AgentDefinition** with builder pattern:
```rust
pub struct AgentDefinition {
    pub name: String,
    pub description: Option<String>,
    pub mode: AgentMode,
    pub built_in: bool,
    pub model: Option<AgentModelConfig>,
    pub prompt: Option<String>,
    pub tools: IndexMap<String, bool>,
    pub permission: AgentPermission,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_steps: Option<u32>,
    pub color: Option<String>,
    pub options: IndexMap<String, serde_json::Value>,
}
```

3. **AgentPermission:**
```rust
pub struct AgentPermission {
    pub edit: Permission,
    pub bash: IndexMap<String, Permission>,
    pub webfetch: Permission,
    pub doom_loop: Permission,
    pub external_directory: Permission,
}
```

4. **Permission presets:**
   - `AgentPermission::permissive()` - Allow everything
   - `AgentPermission::read_only()` - No writes, limited bash
   - `AgentPermission::plan_mode()` - Read + safe commands

5. **Glob-based bash permission matching:**
```rust
fn glob_match(pattern: &str, input: &str) -> bool
fn check_bash(&self, command: &str) -> Permission
```

#### `crates/coder/agent/src/permission.rs`
Permission checker utility:
```rust
pub struct PermissionChecker<'a> {
    agent: &'a AgentDefinition,
}

impl PermissionChecker {
    pub fn check_edit(&self) -> Permission
    pub fn check_bash(&self, command: &str) -> Permission
    pub fn check_webfetch(&self) -> Permission
    pub fn check_doom_loop(&self) -> Permission
    pub fn check_external_directory(&self) -> Permission
}

pub fn merge_permissions(base, override_) -> AgentPermission
```

#### `crates/coder/agent/src/registry.rs`
Agent registry with built-in agents:

1. **AgentRegistry:**
```rust
impl AgentRegistry {
    pub fn with_builtin_agents() -> Self
    pub fn register(&mut self, agent: AgentDefinition)
    pub fn get(&self, name: &str) -> Option<Arc<AgentDefinition>>
    pub fn list(&self) -> Vec<Arc<AgentDefinition>>
    pub fn list_primary(&self) -> Vec<Arc<AgentDefinition>>
    pub fn list_subagents(&self) -> Vec<Arc<AgentDefinition>>
    pub fn remove(&mut self, name: &str) -> Result<(), AgentError>
}
```

2. **Built-in Agents:**

**general** - General-purpose subagent:
- Mode: Subagent
- Tools: All except todoread/todowrite
- Permission: Permissive

**explore** - File search specialist:
- Mode: Subagent
- Tools: Read-only (no edit/write)
- Permission: Read-only
- Custom prompt for file searching

**plan** - Planning/architecture mode:
- Mode: Primary
- Permission: Plan mode (read + safe commands)

**build** - Full capability agent:
- Mode: Primary
- Permission: Permissive
- All tools enabled

### Tests
9 tests across 3 modules:
- `test_agent_definition_builder`
- `test_glob_match`
- `test_permission_check_bash`
- `test_permission_checker`
- `test_merge_permissions`
- `test_registry_with_builtins`
- `test_list_by_mode`
- `test_cannot_remove_builtin`
- `test_custom_agent`

**All 9 tests pass**

---

## Workspace Updates

### `Cargo.toml` (workspace root)
Added to members:
```toml
"crates/coder/agent",
```

Added workspace dependency:
```toml
coder_agent = { path = "crates/coder/agent" }
```

---

## Summary

### Crates Created/Modified

| Crate | Status | Tests |
|-------|--------|-------|
| `mechacoder` | Modified | 6 pass |
| `coder_agent` | Created | 9 pass |

### Architecture Achieved

```
┌─────────────────────────────────────────────────────────────┐
│                      coder_app                               │
│                  (application entry)                         │
└─────────────────────────────────────────────────────────────┘
                              │
         ┌────────────────────┼────────────────────┐
         │                    │                    │
         ▼                    ▼                    ▼
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│  coder_session  │  │  coder_agent    │  │   mechacoder    │
│   (processor)   │  │  (definitions)  │  │    (router)     │
└─────────────────┘  └─────────────────┘  └─────────────────┘
         │                    │                    │
         │           ┌────────┴────────┐           │
         │           │                 │           │
         ▼           ▼                 ▼           ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│ coder_permission│ │  tool_registry  │ │       llm       │
│   (ask/respond) │ │    (tools)      │ │   (providers)   │
└─────────────────┘ └─────────────────┘ └─────────────────┘
                              │                    │
                              ▼                    ▼
                    ┌─────────────────┐  ┌─────────────────┐
                    │     tools       │  │    Anthropic    │
                    │  (bash,read,..) │  │  OpenAI, Ollama │
                    └─────────────────┘  └─────────────────┘
```

### Key Integration Points

1. **mechacoder** can now route to:
   - Claude Code (via claude CLI)
   - Anthropic API direct (via llm crate)
   - Ollama (local)
   - Pi (built-in)

2. **Agents** are defined with:
   - Tool availability configuration
   - Permission policies (edit, bash patterns, webfetch)
   - Model overrides
   - Custom prompts

3. **Provider streaming** converts `llm::StreamEvent` to `ServerMessage` for WebSocket delivery

### Next Steps

The opencode port structure is now complete. Remaining work:
1. Connect coder_app to the new session/agent system
2. Implement additional providers (OpenAI, Ollama)
3. Wire up tool execution with permission UI
4. Add conversation persistence via coder_storage
