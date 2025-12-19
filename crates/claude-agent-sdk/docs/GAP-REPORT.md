# Claude Agent SDK: Rust vs Node.js Parity Report

**Date:** 2025-12-19
**Rust SDK Version:** 0.1.0
**Node.js SDK Version:** 0.1.61 (comparing against latest CLI 2.0.73)
**Current Parity:** ~99%

## Executive Summary

Our Rust Claude Agent SDK (`crates/claude_agent_sdk/`) provides near-complete feature parity with the official Node.js SDK (`@anthropic-ai/claude-agent-sdk`). All core functionality required for building agents is implemented, including the full hooks callback system. The remaining ~1% gap consists of convenience features for in-process MCP servers that require implementing the full MCP protocol.

---

## Recent Updates (2025-12-19)

### Newly Implemented Features

1. **Hooks System (100%)** - Full hook callback support:
   - All 12 hook events: `PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `Notification`, `UserPromptSubmit`, `SessionStart`, `SessionEnd`, `Stop`, `SubagentStart`, `SubagentStop`, `PreCompact`, `PermissionRequest`
   - `HookCallbackMatcher` for pattern-based matching
   - `HookCallback` trait for custom async callbacks
   - `FnHookCallback` helper for closure-based hooks
   - All hook input types with full field support
   - All hook output types including hook-specific outputs

2. **New Message Types** - From CLI 2.0.73:
   - `SdkSystemMessage::ApiError` - API error details
   - `SdkSystemMessage::StopHookSummary` - Hook execution summary on stop
   - `SdkSystemMessage::Informational` - Informational system messages
   - `SdkSystemMessage::LocalCommand` - Local command execution results
   - `SdkStatus` enum - Typed status values (`Compacting`)

3. **New Options Types**:
   - `ToolsConfig` - List or preset (`claude_code`) tool configuration
   - `SdkBeta` enum - Typed beta features (`context-1m-2025-08-07`)
   - `tools` field in `QueryOptions`
   - `hooks` field in `QueryOptions`

---

## Parity Status by Category

### Fully Implemented (100%)

| Category | Features |
|----------|----------|
| **Core API** | `query()`, `query_with_permissions()`, `query_no_permissions()` |
| **Session API (V2)** | `unstable_v2_create_session()`, `unstable_v2_resume_session()`, `unstable_v2_prompt()`, `Session` type |
| **Query Control** | `interrupt()`, `set_permission_mode()`, `set_model()`, `set_max_thinking_tokens()`, `mcp_server_status()`, `rewind_files()`, `supported_commands()`, `supported_models()`, `account_info()`, `stream_input()` |
| **Message Types** | All 7 types: User, Assistant, Result, System, StreamEvent, ToolProgress, AuthStatus |
| **System Subtypes** | Init, CompactBoundary, Status, HookResponse, ApiError, StopHookSummary, Informational, LocalCommand |
| **Permission System** | 5 modes, PermissionHandler trait, PermissionResult, PermissionUpdate, 4 built-in handlers |
| **Hooks System** | 12 event types, callback trait, matcher, all input/output types |
| **MCP Server Types** | Stdio, SSE, HTTP (Sdk is stub) |
| **Configuration** | 40+ options including sandbox, plugins, agents, betas, tools, hooks |
| **Transport** | Process spawning, JSONL streaming, auto-detection |
| **Error Handling** | 12 error variants with thiserror |

### Partially Implemented (~80%)

| Feature | Status | Gap |
|---------|--------|-----|
| `McpServerConfig::Sdk` | Stub only | No in-process MCP server support |

### Not Implemented (0%)

| Feature | Node.js SDK | Rust SDK |
|---------|-------------|----------|
| `tool()` helper | Creates MCP tool definitions with Zod schema | Not implemented |
| `createSdkMcpServer()` | Creates in-process MCP server | Not implemented |

---

## Detailed Gap Analysis

### Gap 1: In-Process MCP Server (`McpServerConfig::Sdk`)

**Node.js SDK Capability:**
```typescript
import { createSdkMcpServer, tool } from '@anthropic-ai/claude-agent-sdk';
import { z } from 'zod';

const server = createSdkMcpServer({
  name: 'my-tools',
  version: '1.0.0',
  tools: [
    tool('get_weather', 'Get weather for a city', {
      city: z.string().describe('City name'),
    }, async ({ city }) => {
      return { content: [{ type: 'text', text: `Weather in ${city}: Sunny` }] };
    }),
  ],
});

const query = await sdk.query('What is the weather?', {
  mcpServers: {
    'my-tools': server,
  },
});
```

**Current Rust SDK Status:**
```rust
pub enum McpServerConfig {
    Stdio { command, args, env },
    Sse { url, headers },
    Http { url, headers },
    Sdk { name },  // Stub - no instance field, no handler
}
```

**What's Missing:**

1. **MCP Protocol Implementation**
   - JSON-RPC 2.0 message handling
   - Tool discovery (`tools/list`)
   - Tool invocation (`tools/call`)
   - Resource handling (`resources/list`, `resources/read`)
   - Prompt templates (`prompts/list`, `prompts/get`)

2. **Tool Definition API**
   - Schema definition (equivalent to Zod)
   - Handler registration
   - Result formatting

3. **Server Instance Management**
   - In-process message routing
   - Lifecycle management
   - Error handling

**Implementation Requirements:**

```rust
// Required new types
pub struct McpServer {
    name: String,
    version: String,
    tools: Vec<McpToolDefinition>,
    resources: Vec<McpResourceDefinition>,
}

pub struct McpToolDefinition {
    name: String,
    description: String,
    input_schema: serde_json::Value,  // JSON Schema
    handler: Box<dyn Fn(Value) -> BoxFuture<'static, McpToolResult> + Send + Sync>,
}

pub struct McpToolResult {
    content: Vec<McpContent>,
    is_error: bool,
}

pub enum McpContent {
    Text { text: String },
    Image { data: String, mime_type: String },
    Resource { uri: String, text: Option<String> },
}

// Required new function
pub fn create_sdk_mcp_server(options: McpServerOptions) -> McpServer {
    // Build server with tools
}

// Required trait for tool definition
pub trait McpTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    async fn call(&self, input: Value) -> McpToolResult;
}
```

**Files to Create/Modify:**
- `crates/claude_agent_sdk/src/mcp/mod.rs` - NEW: MCP module
- `crates/claude_agent_sdk/src/mcp/server.rs` - NEW: Server implementation
- `crates/claude_agent_sdk/src/mcp/tools.rs` - NEW: Tool definitions
- `crates/claude_agent_sdk/src/mcp/protocol.rs` - NEW: JSON-RPC messages
- `crates/claude_agent_sdk/src/options.rs` - Modify McpServerConfig::Sdk
- `crates/claude_agent_sdk/src/query.rs` - Add MCP message routing

---

### Gap 2: `tool()` Helper Function

**Node.js SDK Capability:**
```typescript
import { tool } from '@anthropic-ai/claude-agent-sdk';
import { z } from 'zod';

const weatherTool = tool(
  'get_weather',
  'Get current weather for a location',
  {
    city: z.string().describe('City name'),
    units: z.enum(['celsius', 'fahrenheit']).default('celsius'),
  },
  async ({ city, units }) => {
    const weather = await fetchWeather(city, units);
    return {
      content: [{ type: 'text', text: JSON.stringify(weather) }],
    };
  }
);
```

**Alternative Approach:**
Use `schemars` crate for automatic JSON Schema derivation:
```rust
use schemars::JsonSchema;

#[derive(JsonSchema, Deserialize)]
struct GetWeatherInput {
    /// City name
    city: String,
    /// Temperature units
    #[serde(default)]
    units: Units,
}
```

---

## Implementation Priority

| Priority | Feature | Effort | Value | Status |
|----------|---------|--------|-------|--------|
| ~~P1~~ | ~~Hooks System~~ | ~~3-4 days~~ | ~~High~~ | ✅ Done |
| P2 | MCP Protocol | 3-5 days | High | Not started |
| P3 | Tool Builder | 2-3 days | Medium | Not started |
| P4 | createSdkMcpServer | 1 day | Medium | Not started |

---

## Workarounds for Missing Features

### For In-Process MCP Tools

Use Stdio MCP servers instead:

```rust
// Create a separate MCP server binary
// mcp-server/main.rs
fn main() {
    // Implement MCP protocol over stdio
}

// In your SDK code
let options = QueryOptions::new()
    .mcp_server("my-tools", McpServerConfig::Stdio {
        command: "./mcp-server".to_string(),
        args: None,
        env: None,
    });
```

---

## Complete Feature Matrix

| Node.js SDK Feature | Rust SDK | Status |
|---------------------|----------|--------|
| `query()` | `query()` | ✅ |
| `query()` with permissions | `query_with_permissions()` | ✅ |
| `unstable_v2_createSession()` | `unstable_v2_create_session()` | ✅ |
| `unstable_v2_resumeSession()` | `unstable_v2_resume_session()` | ✅ |
| `unstable_v2_prompt()` | `unstable_v2_prompt()` | ✅ |
| `Query.interrupt()` | `Query::interrupt()` | ✅ |
| `Query.setPermissionMode()` | `Query::set_permission_mode()` | ✅ |
| `Query.setModel()` | `Query::set_model()` | ✅ |
| `Query.setMaxThinkingTokens()` | `Query::set_max_thinking_tokens()` | ✅ |
| `Query.mcpServerStatus()` | `Query::mcp_server_status()` | ✅ |
| `Query.rewindFiles()` | `Query::rewind_files()` | ✅ |
| `Query.supportedCommands()` | `Query::supported_commands()` | ✅ |
| `Query.supportedModels()` | `Query::supported_models()` | ✅ |
| `Query.accountInfo()` | `Query::account_info()` | ✅ |
| `Query.streamInput()` | `Query::stream_input()` | ✅ |
| `Session.sessionId` | `Session::session_id()` | ✅ |
| `Session.send()` | `Session::send()` | ✅ |
| `Session.receive()` | `Session::receive()` | ✅ |
| `Session.close()` | `Session::close()` | ✅ |
| `SDKUserMessage` | `SdkUserMessage` | ✅ |
| `SDKAssistantMessage` | `SdkAssistantMessage` | ✅ |
| `SDKResultMessage` | `SdkResultMessage` | ✅ |
| `SDKSystemMessage` | `SdkSystemMessage` | ✅ |
| `SDKSystemMessage.api_error` | `SdkSystemMessage::ApiError` | ✅ |
| `SDKSystemMessage.stop_hook_summary` | `SdkSystemMessage::StopHookSummary` | ✅ |
| `SDKSystemMessage.informational` | `SdkSystemMessage::Informational` | ✅ |
| `SDKSystemMessage.local_command` | `SdkSystemMessage::LocalCommand` | ✅ |
| `SDKStatus` | `SdkStatus` | ✅ |
| `SDKPartialAssistantMessage` | `SdkStreamEvent` | ✅ |
| `SDKToolProgressMessage` | `SdkToolProgressMessage` | ✅ |
| `SDKAuthStatusMessage` | `SdkAuthStatusMessage` | ✅ |
| `McpServerConfig.stdio` | `McpServerConfig::Stdio` | ✅ |
| `McpServerConfig.sse` | `McpServerConfig::Sse` | ✅ |
| `McpServerConfig.http` | `McpServerConfig::Http` | ✅ |
| `McpServerConfig.sdk` | `McpServerConfig::Sdk` | ⚠️ Stub |
| `tool()` | - | ❌ |
| `createSdkMcpServer()` | - | ❌ |
| `options.hooks` | `QueryOptions::hooks` | ✅ |
| `options.tools` | `QueryOptions::tools` | ✅ |
| `options.betas` | `QueryOptions::betas` | ✅ |
| `PermissionMode.default` | `PermissionMode::Default` | ✅ |
| `PermissionMode.acceptEdits` | `PermissionMode::AcceptEdits` | ✅ |
| `PermissionMode.bypassPermissions` | `PermissionMode::BypassPermissions` | ✅ |
| `PermissionMode.plan` | `PermissionMode::Plan` | ✅ |
| `PermissionMode.dontAsk` | `PermissionMode::DontAsk` | ✅ |
| `options.model` | `QueryOptions::model()` | ✅ |
| `options.cwd` | `QueryOptions::cwd()` | ✅ |
| `options.maxTurns` | `QueryOptions::max_turns()` | ✅ |
| `options.maxBudgetUsd` | `QueryOptions::max_budget_usd()` | ✅ |
| `options.maxThinkingTokens` | `QueryOptions::max_thinking_tokens()` | ✅ |
| `options.systemPrompt` | `QueryOptions::system_prompt` | ✅ |
| `options.outputFormat` | `QueryOptions::output_format` | ✅ |
| `options.mcpServers` | `QueryOptions::mcp_server()` | ✅ |
| `options.agents` | `QueryOptions::agent()` | ✅ |
| `options.sandbox` | `QueryOptions::sandbox` | ✅ |
| `options.plugins` | `QueryOptions::plugins` | ✅ |
| `options.strictMcpConfig` | `QueryOptions::strict_mcp_config()` | ✅ |
| `options.permissionPromptToolName` | `QueryOptions::permission_prompt_tool_name()` | ✅ |
| `HookEvent.PreToolUse` | `HookEvent::PreToolUse` | ✅ |
| `HookEvent.PostToolUse` | `HookEvent::PostToolUse` | ✅ |
| `HookEvent.PostToolUseFailure` | `HookEvent::PostToolUseFailure` | ✅ |
| `HookEvent.Notification` | `HookEvent::Notification` | ✅ |
| `HookEvent.UserPromptSubmit` | `HookEvent::UserPromptSubmit` | ✅ |
| `HookEvent.SessionStart` | `HookEvent::SessionStart` | ✅ |
| `HookEvent.SessionEnd` | `HookEvent::SessionEnd` | ✅ |
| `HookEvent.Stop` | `HookEvent::Stop` | ✅ |
| `HookEvent.SubagentStart` | `HookEvent::SubagentStart` | ✅ |
| `HookEvent.SubagentStop` | `HookEvent::SubagentStop` | ✅ |
| `HookEvent.PreCompact` | `HookEvent::PreCompact` | ✅ |
| `HookEvent.PermissionRequest` | `HookEvent::PermissionRequest` | ✅ |
| `HookCallback` | `HookCallback` trait | ✅ |
| `HookCallbackMatcher` | `HookCallbackMatcher` | ✅ |
| All hook input types | All hook input types | ✅ |
| All hook output types | All hook output types | ✅ |

---

## Appendix A: Node.js SDK File Reference

Key files from `@anthropic-ai/claude-agent-sdk@0.1.61`:

| File | Purpose |
|------|---------|
| `sdk.mjs` | Main SDK implementation |
| `sdk.d.ts` | Main type definitions (985 lines) |
| `sdk-tools.d.ts` | Tool input schemas |
| `sandboxTypes.d.ts` | Sandbox configuration |
| `cli.js` | Claude Code CLI executable (minified) |

**Location:** `~/.npm/_npx/<hash>/node_modules/@anthropic-ai/claude-agent-sdk/`

---

## Appendix B: Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        Rust Application                          │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                   claude_agent_sdk                        │   │
│  ├─────────────────────────────────────────────────────────┤   │
│  │  query.rs          │  session.rs      │  permissions.rs  │   │
│  │  - Query struct    │  - Session       │  - Handler trait │   │
│  │  - Control methods │  - V2 API        │  - Built-in impls│   │
│  │  - Hook execution  │                  │                  │   │
│  ├─────────────────────────────────────────────────────────┤   │
│  │  options.rs        │  protocol/       │  transport/      │   │
│  │  - QueryOptions    │  - Messages      │  - Process spawn │   │
│  │  - MCP configs     │  - Control       │  - JSONL stream  │   │
│  │  - Hooks config    │                  │                  │   │
│  ├─────────────────────────────────────────────────────────┤   │
│  │  hooks.rs          │  mcp/ (NOT IMPLEMENTED)             │   │
│  │  - HookEvent       │  - Server        │  - Protocol      │   │
│  │  - HookCallback    │  - Tools         │                  │   │
│  │  - HookInput types │                  │                  │   │
│  │  - HookOutput types│                  │                  │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                   │
│                              │ JSONL over stdin/stdout           │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                   Claude Code CLI                         │   │
│  │  - Tool execution                                         │   │
│  │  - MCP server management                                  │   │
│  │  - Permission handling                                    │   │
│  │  - Session management                                     │   │
│  │  - Hook invocation                                        │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                   │
│                              │ API calls                         │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                   Claude API                              │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

---

## Conclusion

Our Rust Claude Agent SDK provides **99% feature parity** with the official Node.js SDK. All core functionality for building agents is fully implemented, including:

- Complete query and session APIs
- Full permission system with custom handlers
- All message types including new CLI 2.0.73 subtypes
- Complete hooks callback system with all 12 event types
- MCP server configuration (stdio, SSE, HTTP)

The remaining 1% consists of convenience features for in-process MCP servers (`tool()`, `createSdkMcpServer()`) that can be worked around using Stdio MCP servers.

**Recommended Next Steps:**
1. Implement MCP Protocol module - Enables custom in-process tools
2. Add tool builder helpers - Improves developer experience
