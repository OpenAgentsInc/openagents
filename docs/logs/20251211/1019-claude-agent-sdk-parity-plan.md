# Plan: Achieve 100% Parity with Official Claude Agent SDK

## Overview
Compare our Rust SDK (`crates/claude_agent_sdk/`) with the official Node.js SDK (`@anthropic-ai/claude-agent-sdk@0.1.65`) and implement all missing features to achieve complete parity.

## Gap Analysis Summary

### ✅ COMPLETE (Already at Parity)
- Core `query()` API with permission variants
- Query control: `interrupt()`, `set_permission_mode()`, `set_model()`, `set_max_thinking_tokens()`, `mcp_server_status()`, `rewind_files()`
- All 7 message types (User, Assistant, Result, System, StreamEvent, ToolProgress, AuthStatus)
- Permission system (modes, handler trait, rules, results, updates)
- MCP support (Stdio, SSE, HTTP)
- Configuration options (30+ options via builder)
- Transport layer (process spawn, JSONL streaming)
- Error handling (12 error variants)
- Agent definitions
- Hooks support
- Sandbox configuration
- File checkpointing
- Structured output
- Plugin system

### ⚠️ MISSING FEATURES (Must Add)

#### HIGH PRIORITY - API Surface

1. **Session API (V2)** - Unstable but in official SDK
   - `unstable_v2_create_session(options) -> Session`
   - `unstable_v2_resume_session(session_id, options) -> Session`
   - `unstable_v2_prompt(message, options) -> ResultMessage`
   - `Session` type with `send()`, `receive()`, `close()`, `session_id()`

2. **Query Control Methods**
   - `supported_commands() -> Vec<SlashCommand>` - List available slash commands
   - `supported_models() -> Vec<ModelInfo>` - List available models
   - `account_info() -> AccountInfo` - Get authenticated account details
   - `stream_input(stream: impl Stream<Item = SdkUserMessage>)` - Stream user messages

3. **MCP SDK Server** - In-process MCP servers
   - `McpServerConfig::Sdk { name, instance }` variant
   - Helper to create in-process MCP servers (if needed)

4. **Custom Tool Helpers** (Lower priority - convenience APIs)
   - Helper functions to create tool definitions
   - Schema validation utilities

#### MEDIUM PRIORITY - Configuration & Types

5. **Configuration Options** - Verify all 40+ options present
   - `strict_mcp_config` - Strict MCP validation
   - `permission_prompt_tool_name` - Route prompts to MCP tool
   - Any other missing options from Node.js SDK

6. **Supporting Types**
   - `SlashCommand { name, description, argument_hint }`
   - `ModelInfo { value, display_name, description }`
   - `AccountInfo { email, organization, subscription_type, token_source, api_key_source }`

7. **Hook System** - Verify all 12 events supported
   - PreToolUse, PostToolUse, PostToolUseFailure, Notification
   - UserPromptSubmit, SessionStart, SessionEnd, Stop
   - SubagentStart, SubagentStop, PreCompact, PermissionRequest

8. **Sandbox Configuration** - Verify completeness
   - `allow_unsandboxed_commands`
   - `ignore_violations`
   - `enable_weaker_nested_sandbox`
   - `excluded_commands`
   - `ripgrep` config

---

## Implementation Plan

### Phase 1: Query Control Methods (Easiest Wins)

Add missing control methods to `Query` struct in `src/query.rs`:

#### 1.1 `supported_commands() -> Result<Vec<SlashCommand>>`

**Add to protocol/control.rs:**
```rust
// In SdkControlRequest enum
SupportedCommands,

// In SdkControlResponse, add response type
pub struct SlashCommand {
    pub name: String,
    pub description: String,
    pub argument_hint: String,
}
```

**Add to src/query.rs:**
```rust
pub async fn supported_commands(&self) -> Result<Vec<SlashCommand>> {
    let response = self.send_control_request(
        SdkControlRequest::SupportedCommands
    ).await?;

    // Parse response.commands
    Ok(parsed_commands)
}
```

#### 1.2 `supported_models() -> Result<Vec<ModelInfo>>`

**Add to protocol/control.rs:**
```rust
// In SdkControlRequest enum
SupportedModels,

// Response type
pub struct ModelInfo {
    pub value: String,              // API identifier
    pub display_name: String,
    pub description: String,
}
```

**Add to src/query.rs:**
```rust
pub async fn supported_models(&self) -> Result<Vec<ModelInfo>> {
    let response = self.send_control_request(
        SdkControlRequest::SupportedModels
    ).await?;

    Ok(parsed_models)
}
```

#### 1.3 `account_info() -> Result<AccountInfo>`

**Add to protocol/control.rs:**
```rust
// In SdkControlRequest enum
AccountInfo,

// Response type
pub struct AccountInfo {
    pub email: Option<String>,
    pub organization: Option<String>,
    pub subscription_type: Option<String>,
    pub token_source: Option<String>,
    pub api_key_source: Option<String>,
}
```

**Add to src/query.rs:**
```rust
pub async fn account_info(&self) -> Result<AccountInfo> {
    let response = self.send_control_request(
        SdkControlRequest::AccountInfo
    ).await?;

    Ok(parsed_account_info)
}
```

#### 1.4 `stream_input()` - Stream user messages

**Add to src/query.rs:**
```rust
pub async fn stream_input<S>(&self, stream: S) -> Result<()>
where
    S: Stream<Item = SdkUserMessage> + Send + 'static
{
    // Spawn task to forward stream items to stdin
    let stdin_tx = self.stdin_tx.clone();
    tokio::spawn(async move {
        pin_mut!(stream);
        while let Some(msg) = stream.next().await {
            let stdin_msg = StdinMessage::UserMessage(msg);
            if stdin_tx.send(stdin_msg).await.is_err() {
                break;
            }
        }
    });

    Ok(())
}
```

### Phase 2: Session API (V2)

Create new `src/session.rs` file for unstable V2 API:

#### 2.1 Session Type

```rust
pub struct Session {
    session_id: String,
    query: Query,
    stdin_tx: mpsc::UnboundedSender<StdinMessage>,
    message_rx: mpsc::UnboundedReceiver<Result<SdkMessage>>,
}

impl Session {
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub async fn send(&self, message: impl Into<SdkUserMessage>) -> Result<()> {
        // Send user message via stdin_tx
    }

    pub fn receive(&mut self) -> impl Stream<Item = Result<SdkMessage>> + '_ {
        // Return stream from message_rx
    }

    pub async fn close(self) -> Result<()> {
        // Clean shutdown
    }
}

// Async disposal support
impl AsyncDrop for Session {
    async fn async_drop(&mut self) {
        let _ = self.close().await;
    }
}
```

#### 2.2 Session Creation Functions

**Add to src/lib.rs:**
```rust
pub async fn unstable_v2_create_session(options: QueryOptions) -> Result<Session> {
    let query = query_with_permissions("", options, AllowAllPermissions)?;

    // Wait for SystemInit message to get session_id
    // Return Session wrapping query
}

pub async fn unstable_v2_resume_session(
    session_id: String,
    options: QueryOptions
) -> Result<Session> {
    let mut opts = options;
    opts.resume = Some(session_id);
    unstable_v2_create_session(opts).await
}

pub async fn unstable_v2_prompt(
    message: String,
    options: QueryOptions
) -> Result<SdkResultMessage> {
    let mut session = unstable_v2_create_session(options).await?;
    session.send(message).await?;

    // Consume stream until SdkResultMessage
    pin_mut!(receive_stream);
    while let Some(msg) = receive_stream.next().await {
        match msg? {
            SdkMessage::Result(result) => return Ok(result),
            _ => continue,
        }
    }

    Err(Error::InvalidMessage("No result".into()))
}
```

### Phase 3: MCP SDK Server Support

#### 3.1 Add SDK Variant to McpServerConfig

**In src/options.rs:**
```rust
pub enum McpServerConfig {
    Stdio { command: String, args: Option<Vec<String>>, env: Option<HashMap<String, String>> },
    Sse { url: String, headers: Option<HashMap<String, String>> },
    Http { url: String, headers: Option<HashMap<String, String>> },

    // NEW: In-process MCP server
    Sdk {
        name: String,
        // Could include Arc<dyn McpServer> if we implement MCP server trait
        // For now, just mark as unsupported or document that users should
        // use stdio with a local process
    },
}
```

**Note:** Full in-process MCP server requires implementing MCP protocol. For MVP, we can:
- Add the enum variant for API compatibility
- Return error if used: "In-process MCP servers not yet supported in Rust SDK"
- OR: Document that users should spawn local MCP servers via Stdio

### Phase 4: Configuration Completeness

#### 4.1 Verify All Options in QueryOptions

Compare with Node.js `Options` type and add any missing:

**Check and add to src/options.rs:**
```rust
impl QueryOptions {
    // Already have most, verify these exist:
    // - strict_mcp_config
    // - permission_prompt_tool_name
    // - (any others from Node.js SDK)

    pub fn strict_mcp_config(mut self, strict: bool) -> Self {
        self.strict_mcp_config = Some(strict);
        self
    }

    pub fn permission_prompt_tool_name(mut self, tool_name: String) -> Self {
        self.permission_prompt_tool_name = Some(tool_name);
        self
    }
}
```

#### 4.2 Complete Sandbox Settings

**In src/options.rs, verify SandboxSettings has:**
```rust
pub struct SandboxSettings {
    pub enabled: Option<bool>,
    pub auto_allow_bash_if_sandboxed: Option<bool>,
    pub allow_unsandboxed_commands: Option<bool>,  // Add if missing
    pub network: Option<SandboxNetworkConfig>,
    pub ignore_violations: Option<HashMap<String, Vec<String>>>,  // Add if missing
    pub enable_weaker_nested_sandbox: Option<bool>,  // Add if missing
    pub excluded_commands: Option<Vec<String>>,  // Add if missing
    pub ripgrep: Option<RipgrepConfig>,  // Add if missing
}

pub struct RipgrepConfig {
    pub command: String,
    pub args: Option<Vec<String>>,
}
```

### Phase 5: Hooks Completeness

#### 5.1 Verify Hook System

Check `src/options.rs` or protocol for hooks support. Ensure all 12 hook events are supported:

```rust
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    Notification,
    UserPromptSubmit,
    SessionStart,
    SessionEnd,
    Stop,
    SubagentStart,
    SubagentStop,
    PreCompact,
    PermissionRequest,
}
```

If hooks aren't exposed in our API yet, add:
```rust
impl QueryOptions {
    pub fn hook(mut self, event: HookEvent, callback: HookCallback) -> Self {
        self.hooks.entry(event).or_default().push(callback);
        self
    }
}
```

---

## Implementation Order

### Sprint 1: Quick Wins (Query Control Methods)
1. Add `SlashCommand`, `ModelInfo`, `AccountInfo` types to `protocol/control.rs`
2. Add control request variants: `SupportedCommands`, `SupportedModels`, `AccountInfo`
3. Implement methods in `src/query.rs`: `supported_commands()`, `supported_models()`, `account_info()`
4. Add `stream_input()` method

**Files to modify:**
- `crates/claude_agent_sdk/src/protocol/control.rs` - Add types and request variants
- `crates/claude_agent_sdk/src/query.rs` - Add methods

### Sprint 2: Session API
1. Create `src/session.rs` with `Session` struct
2. Implement `unstable_v2_create_session()`, `unstable_v2_resume_session()`, `unstable_v2_prompt()`
3. Export from `src/lib.rs`

**Files to create/modify:**
- `crates/claude_agent_sdk/src/session.rs` - NEW FILE
- `crates/claude_agent_sdk/src/lib.rs` - Add exports

### Sprint 3: Configuration Audit
1. Compare all Node.js `Options` fields with our `QueryOptions`
2. Add any missing configuration options
3. Complete `SandboxSettings` with all fields
4. Verify hooks support is complete

**Files to modify:**
- `crates/claude_agent_sdk/src/options.rs` - Add missing options

### Sprint 4: MCP SDK Support (Optional/Future)
1. Add `Sdk` variant to `McpServerConfig`
2. Either:
   - Return clear error message for now
   - OR implement full in-process MCP server support (larger effort)

**Files to modify:**
- `crates/claude_agent_sdk/src/options.rs` - Add enum variant

---

## Testing Plan

After each sprint:
1. Add unit tests for new types
2. Integration test with actual Claude CLI for control methods
3. Session API tests with create/send/receive flow
4. Configuration validation tests

---

## Notes

- Session API is marked "unstable" in Node.js SDK - maintain same warning in Rust
- MCP SDK server support may require significant work if we want in-process servers
- Focus on API surface parity first, implementation details can follow
- Document any features we skip with clear rationale

---

## Current Status

**Estimated Parity:** ~85%

**Missing critical features:** 4 (Session API, Query control methods, MCP SDK, Config audit)

**Estimated effort:** 2-3 days for 100% parity
