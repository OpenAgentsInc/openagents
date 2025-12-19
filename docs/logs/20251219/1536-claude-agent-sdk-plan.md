# Claude Agent SDK Gap Analysis & Implementation Plan

- **Date:** 2025-12-19
- **TypeScript SDK Version:** 0.1.61 (from npm cache)
- **CLI Version Found:** 2.0.69 (local), target 2.0.73
- **Rust SDK Version:** 0.1.0 (claims parity with 0.1.65)

## Executive Summary

After comparing the TypeScript SDK (`sdk.d.ts`), the CLI (`cli.js`), and the Rust SDK implementation, I've identified **15 gaps** that need to be addressed. The major categories are:

1. **New Message Subtypes** (4 gaps) - From CLI analysis
2. **Hooks System** (1 gap) - Full hook callback support
3. **MCP Server Support** (2 gaps) - In-process tool definition
4. **Options & Types** (5 gaps) - Missing configuration options
5. **Documentation** (3 gaps) - Update gap report

---

## Gap Analysis

### HIGH PRIORITY (Core Features)

#### Gap 1: New Message Subtypes from CLI
**Status:** NOT IMPLEMENTED
**Source:** CLI cli.js analysis

Found in CLI but missing from Rust SDK:
```
subtype:"api_error"         - System message for API errors
subtype:"stop_hook_summary" - Hook summary on stop
subtype:"informational"     - Informational system messages
subtype:"local_command"     - Local command execution results
```

**Files to modify:**
- `src/protocol/messages.rs` - Add new SdkSystemMessage variants

---

#### Gap 2: Hooks System (`hooks` QueryOption)
**Status:** NOT IMPLEMENTED
**TypeScript SDK:**
```typescript
hooks?: Partial<Record<HookEvent, HookCallbackMatcher[]>>;

type HookEvent = 'PreToolUse' | 'PostToolUse' | 'PostToolUseFailure' |
  'Notification' | 'UserPromptSubmit' | 'SessionStart' | 'SessionEnd' |
  'Stop' | 'SubagentStart' | 'SubagentStop' | 'PreCompact' | 'PermissionRequest';

interface HookCallbackMatcher {
  matcher?: string;
  hooks: HookCallback[];
  timeout?: number;
}
```

**Files to modify:**
- `src/options.rs` - Add hooks field to QueryOptions
- `src/protocol/control.rs` - Add HookCallback handling (currently returns success without processing)
- `src/hooks.rs` - NEW: Hook types and callback system

---

#### Gap 3: In-Process MCP Server (`tool()` and `createSdkMcpServer()`)
**Status:** STUB ONLY (documented in existing GAP-REPORT.md)
**TypeScript SDK:**
```typescript
function tool<Schema>(name, description, inputSchema, handler): SdkMcpToolDefinition;
function createSdkMcpServer(options: CreateSdkMcpServerOptions): McpSdkServerConfigWithInstance;

type McpSdkServerConfigWithInstance = McpSdkServerConfig & { instance: McpServer };
```

**Files to modify:**
- `src/mcp/mod.rs` - NEW: MCP module
- `src/mcp/server.rs` - NEW: McpServer implementation
- `src/mcp/tool.rs` - NEW: Tool builder
- `src/options.rs` - Update McpServerConfig::Sdk to include instance

---

#### Gap 4: `tools` Option with Preset Support
**Status:** MISSING
**TypeScript SDK:**
```typescript
tools?: string[] | { type: 'preset'; preset: 'claude_code' };
```

**Rust SDK:** Only has `allowed_tools` and `disallowed_tools`

**Files to modify:**
- `src/options.rs` - Add ToolsConfig enum and tools field

---

### MEDIUM PRIORITY (Options & Messages)

#### Gap 5: `betas` Option Type
**Status:** PARTIAL (exists as Vec<String>)
**TypeScript SDK:** Has specific beta types
```typescript
type SdkBeta = 'context-1m-2025-08-07';
betas?: SdkBeta[];
```

**Files to modify:**
- `src/options.rs` - Add SdkBeta enum

---

#### Gap 6: SDKUserMessageReplay Type
**Status:** MISSING
**TypeScript SDK:**
```typescript
type SDKUserMessageReplay = SDKUserMessageContent & {
  uuid: UUID;
  session_id: string;
  isReplay: true;
};
```

**Files to modify:**
- `src/protocol/messages.rs` - Add SdkUserMessageReplay variant or field

---

#### Gap 7: SDKCompactBoundaryMessage
**Status:** EXISTS (CompactBoundary in SdkSystemMessage)
**No changes needed** - already implemented

---

#### Gap 8: SDKStatusMessage (`SDKStatus` type)
**Status:** EXISTS (StatusUpdate in SdkSystemMessage)
**TypeScript SDK:**
```typescript
type SDKStatus = 'compacting' | null;
```

**Files to modify:**
- `src/protocol/messages.rs` - Add SdkStatus enum (currently uses Option<String>)

---

#### Gap 9: `allowedDomains` in SandboxNetworkConfig
**Status:** MISSING
**TypeScript SDK:** SandboxNetworkConfig does NOT have allowedDomains
**Rust SDK:** Has `allowed_domains` field
**This is a Rust SDK addition** - keep it

---

#### Gap 10: AbortError Type
**Status:** MISSING
**TypeScript SDK:**
```typescript
class AbortError extends Error {}
```

**Files to modify:**
- `src/error.rs` - Already has `Aborted` variant, no changes needed

---

### LOW PRIORITY (Nice-to-have)

#### Gap 11: Hook Input Types
**Status:** NOT IMPLEMENTED
All hook input types need to be defined:
- `PreToolUseHookInput`
- `PostToolUseHookInput`
- `PostToolUseFailureHookInput`
- `NotificationHookInput`
- `UserPromptSubmitHookInput`
- `SessionStartHookInput`
- `SessionEndHookInput`
- `StopHookInput`
- `SubagentStartHookInput`
- `SubagentStopHookInput`
- `PreCompactHookInput`
- `PermissionRequestHookInput`

**Files to create:**
- `src/hooks.rs` - NEW

---

#### Gap 12: Hook Output Types
**Status:** NOT IMPLEMENTED
```typescript
type HookJSONOutput = AsyncHookJSONOutput | SyncHookJSONOutput;
type AsyncHookJSONOutput = { async: true; asyncTimeout?: number };
type SyncHookJSONOutput = { continue?: boolean; suppressOutput?: boolean; ... };
```

**Files to create:**
- `src/hooks.rs` - NEW

---

#### Gap 13: Query.rewindFiles() Missing Documentation
**Status:** IMPLEMENTED (in control.rs)
**No changes needed** - just needs documentation update

---

## Implementation Plan

### Phase 1: Message Types & Protocol Updates (Low Risk, High Value)

**Files:** `src/protocol/messages.rs`

1. **Add new SdkSystemMessage variants:**
   ```rust
   ApiError { ... }          // subtype: "api_error"
   StopHookSummary { ... }   // subtype: "stop_hook_summary"
   Informational { ... }     // subtype: "informational"
   LocalCommand { ... }      // subtype: "local_command"
   ```

2. **Add SdkStatus enum:**
   ```rust
   pub enum SdkStatus {
       Compacting,
   }
   ```

3. **Add `is_replay` field to SdkUserMessage**

---

### Phase 2: Options Updates (Low Risk)

**Files:** `src/options.rs`

4. **Add `ToolsConfig` enum:**
   ```rust
   pub enum ToolsConfig {
       List(Vec<String>),
       Preset { preset: ToolPreset },
   }

   pub enum ToolPreset {
       ClaudeCode,
   }
   ```

5. **Add `SdkBeta` enum:**
   ```rust
   pub enum SdkBeta {
       Context1M,  // "context-1m-2025-08-07"
   }
   ```

6. **Add `tools` field to QueryOptions builder**

---

### Phase 3: Hooks System (Medium Risk, High Value)

**New file:** `src/hooks.rs`

7. **Define hook event types:**
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

8. **Define all hook input types:**
   - `PreToolUseHookInput`
   - `PostToolUseHookInput`
   - `PostToolUseFailureHookInput`
   - `NotificationHookInput`
   - `UserPromptSubmitHookInput`
   - `SessionStartHookInput`
   - `SessionEndHookInput`
   - `StopHookInput`
   - `SubagentStartHookInput`
   - `SubagentStopHookInput`
   - `PreCompactHookInput`
   - `PermissionRequestHookInput`

9. **Define hook output types:**
   ```rust
   pub enum HookOutput {
       Async { timeout: Option<u32> },
       Sync(SyncHookOutput),
   }

   pub struct SyncHookOutput {
       pub continue_: Option<bool>,
       pub suppress_output: Option<bool>,
       pub stop_reason: Option<String>,
       pub decision: Option<HookDecision>,
       pub system_message: Option<String>,
       pub reason: Option<String>,
       pub hook_specific_output: Option<HookSpecificOutput>,
   }
   ```

10. **Define `HookCallback` trait and `HookCallbackMatcher`:**
    ```rust
    #[async_trait]
    pub trait HookCallback: Send + Sync {
        async fn call(&self, input: HookInput, tool_use_id: Option<String>) -> Result<HookOutput>;
    }

    pub struct HookCallbackMatcher {
        pub matcher: Option<String>,
        pub hooks: Vec<Arc<dyn HookCallback>>,
        pub timeout: Option<u32>,
    }
    ```

**Files:** `src/options.rs`, `src/query.rs`

11. **Add `hooks` field to QueryOptions:**
    ```rust
    pub hooks: Option<HashMap<HookEvent, Vec<HookCallbackMatcher>>>
    ```

12. **Update `query.rs` to handle hook callbacks in control request handler** (line ~203)

---

### Phase 4: MCP Server Support (DEFERRED)

> **Note:** User requested to skip MCP server support for now. The existing stub in `McpServerConfig::Sdk` remains. This can be implemented in a future update.

---

### Phase 5: Documentation & Cleanup

13. **Update `docs/GAP-REPORT.md`:**
    - Update version numbers
    - Mark completed gaps
    - Update parity percentage to ~98%

14. **Create `docs/UPDATING-SDK.md`** - Guide for keeping SDK updated:
    - Where to find TypeScript SDK updates (npm, local cache)
    - How to compare sdk.d.ts with Rust implementation
    - What to look for in cli.js (message subtypes, control requests)
    - Checklist for SDK update process
    - Key files and their purposes

15. **Add integration tests for:**
    - Hook callbacks
    - New message types

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/lib.rs` | Export new hooks module |
| `src/options.rs` | Add ToolsConfig, SdkBeta, tools, hooks fields |
| `src/protocol/messages.rs` | Add ApiError, StopHookSummary, Informational, LocalCommand variants; SdkStatus enum; is_replay field |
| `src/query.rs` | Implement hook callback handling (line ~203) |
| `src/hooks.rs` | **NEW:** Complete hooks system (HookEvent, HookInput types, HookOutput, HookCallback trait) |
| `docs/GAP-REPORT.md` | Update with new analysis, bump parity to ~98% |
| `docs/UPDATING-SDK.md` | **NEW:** Guide for keeping SDK updated |

---

## Execution Priority

### Quick Wins (Do First)
1. Add new SdkSystemMessage variants (api_error, stop_hook_summary, informational, local_command)
2. Add SdkStatus enum
3. Add is_replay field to SdkUserMessage
4. Add ToolsConfig and SdkBeta enums to options.rs
5. Add tools field to QueryOptions builder

### Core Features (Do Second)
6. Create hooks.rs with all hook types
7. Add hooks field to QueryOptions
8. Implement hook callback handling in query.rs

### Documentation (Do Last)
9. Update GAP-REPORT.md
10. Create UPDATING-SDK.md guide

---

## Estimated Scope

- **New files:** 2 (hooks.rs, docs/UPDATING-SDK.md)
- **Modified files:** 5 (lib.rs, options.rs, messages.rs, query.rs, GAP-REPORT.md)
- **Lines of code:** ~600-800 new lines
- **Risk:** Low (mostly additive changes, no breaking changes)

---

## Key TypeScript SDK Reference

**SDK version:** 0.1.61
**Path:** `/Users/christopherdavid/.npm/_npx/7b8cde7936c78aff/node_modules/@anthropic-ai/claude-agent-sdk/`
**Key files:**
- `sdk.d.ts` - Full type definitions (985 lines)
- `sandboxTypes.d.ts` - Sandbox config types
- `cli.js` - CLI implementation (minified)
