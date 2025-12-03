# Plan: Harmonize Effect Schemas with Claude Agent SDK Types

## Summary

Adopt Claude Agent SDK naming conventions and type patterns throughout OpenAgents, creating a comprehensive SDK-compatible type library using Effect Schema. Enable config-controlled routing between Claude Code and minimal subagent.

## Key Decisions (from user input)

1. **Naming**: Adopt SDK snake_case conventions (`file_path`, `old_string`, `new_string`)
2. **Location**: `src/schemas/sdk/` - dedicated directory for SDK-compatible types
3. **Scope**: Full feature set (tools, messages, hooks, permissions, subagent definitions)
4. **Routing**: Config-controlled via `project.json` settings

---

## Architecture

```
src/schemas/sdk/                    # NEW: SDK-compatible Effect schemas
├── index.ts                        # Re-exports all types
├── tool-inputs.ts                  # FileReadInput, FileEditInput, BashInput, GrepInput...
├── tool-outputs.ts                 # ReadOutput, EditOutput, BashOutput, GrepOutput...
├── messages.ts                     # SDKMessage, ContentBlock unions
├── hooks.ts                        # HookEvent, PreToolUseHookInput, PostToolUseHookInput...
├── permissions.ts                  # PermissionMode, PermissionResult, CanUseTool
├── agents.ts                       # AgentDefinition, SubagentConfig, SubagentResult
├── usage.ts                        # TokenUsage, CostBreakdown, NonNullableUsage
└── type-guards.ts                  # isTextBlock, isToolUseBlock, etc.

src/schemas/sdk/adapters/           # Conversion utilities
├── effect-to-zod.ts                # Effect Schema -> Zod (for MCP tools)
├── tool-adapter.ts                 # Effect Tool -> MCP tool wrapper
├── message-adapter.ts              # ChatMessage <-> SDKMessage
└── result-adapter.ts               # ToolResult <-> SDK result types

src/interop/                        # Dual-mode infrastructure
├── router/
│   ├── subagent-router.ts          # runBestAvailableSubagent()
│   └── config.ts                   # ClaudeCodeConfig schema
├── mcp-bridge.ts                   # MechaCoder MCP tools for Claude Code
└── index.ts
```

---

## Phase 1: SDK Tool Schemas (Priority 1)

### 1.1 Create `src/schemas/sdk/tool-inputs.ts`

Define all tool input schemas matching Claude SDK conventions:

```typescript
import * as S from "effect/Schema";

export const FileReadInput = S.Struct({
  file_path: S.String.pipe(S.minLength(1), S.annotations({ description: "Absolute path to file" })),
  offset: S.optional(S.Number.pipe(S.int(), S.greaterThanOrEqualTo(1))),
  limit: S.optional(S.Number.pipe(S.int(), S.greaterThanOrEqualTo(1))),
});

export const FileEditInput = S.Struct({
  file_path: S.String.pipe(S.minLength(1)),
  old_string: S.String.pipe(S.minLength(1)),
  new_string: S.String,
  replace_all: S.optional(S.Boolean),
});

export const FileWriteInput = S.Struct({
  file_path: S.String.pipe(S.minLength(1)),
  content: S.String,
});

export const BashInput = S.Struct({
  command: S.String.pipe(S.minLength(1)),
  timeout: S.optional(S.Number.pipe(S.greaterThan(0), S.lessThanOrEqualTo(600000))),
  description: S.optional(S.String),
  run_in_background: S.optional(S.Boolean),
});

export const GrepInput = S.Struct({
  pattern: S.String.pipe(S.minLength(1)),
  path: S.optional(S.String),
  glob: S.optional(S.String),
  type: S.optional(S.String),
  output_mode: S.optional(S.Literal("content", "files_with_matches", "count")),
  "-i": S.optional(S.Boolean),
  "-n": S.optional(S.Boolean),
  "-B": S.optional(S.Number),
  "-A": S.optional(S.Number),
  "-C": S.optional(S.Number),
  head_limit: S.optional(S.Number),
  multiline: S.optional(S.Boolean),
});

export const GlobInput = S.Struct({
  pattern: S.String.pipe(S.minLength(1)),
  path: S.optional(S.String),
});

// ... WebFetchInput, WebSearchInput, TodoWriteInput, etc.
```

### 1.2 Create `src/schemas/sdk/tool-outputs.ts`

```typescript
export const TextContent = S.Struct({ type: S.Literal("text"), text: S.String });
export const ImageContent = S.Struct({ type: S.Literal("image"), data: S.String, mimeType: S.String });
export const ToolContent = S.Union(TextContent, ImageContent);

export const ReadOutput = S.Struct({
  content: S.Array(ToolContent),
  total_lines: S.optional(S.Number),
  lines_returned: S.optional(S.Number),
});

export const EditOutput = S.Struct({
  message: S.String,
  replacements: S.Number,
  file_path: S.String,
});

export const BashOutput = S.Struct({
  output: S.String,
  exitCode: S.Number,
  killed: S.optional(S.Boolean),
  shellId: S.optional(S.String),
});

// ... GrepOutput variants, GlobOutput, etc.
```

### 1.3 Update existing tools to use SDK naming

Migrate each tool in `src/tools/`:
- `read.ts`: `path` → `file_path`, `offset`, `limit`
- `edit.ts`: `path` → `file_path`, `oldText` → `old_string`, `newText` → `new_string`, add `replace_all`
- `write.ts`: `path` → `file_path`
- `bash.ts`: add `description`, `run_in_background`
- `grep.ts`: align parameter names with SDK
- `find.ts`, `ls.ts`: align with SDK patterns

---

## Phase 2: SDK Message Types (Priority 1)

### 2.1 Create `src/schemas/sdk/messages.ts`

```typescript
export const TextBlock = S.Struct({ type: S.Literal("text"), text: S.String });
export const ThinkingBlock = S.Struct({ type: S.Literal("thinking"), thinking: S.String, signature: S.optional(S.String) });
export const ToolUseBlock = S.Struct({ type: S.Literal("tool_use"), id: S.String, name: S.String, input: S.Unknown });
export const ToolResultBlock = S.Struct({ type: S.Literal("tool_result"), tool_use_id: S.String, content: S.Array(ToolContent), is_error: S.optional(S.Boolean) });

export const ContentBlock = S.Union(TextBlock, ThinkingBlock, ToolUseBlock, ToolResultBlock);

export const SDKUserMessage = S.Struct({
  type: S.Literal("user"),
  uuid: S.optional(S.String),
  session_id: S.String,
  message: S.Struct({ role: S.Literal("user"), content: S.Union(S.String, S.Array(ContentBlock)) }),
  parent_tool_use_id: S.NullOr(S.String),
});

export const SDKAssistantMessage = S.Struct({
  type: S.Literal("assistant"),
  uuid: S.String,
  session_id: S.String,
  message: S.Unknown, // APIAssistantMessage from Anthropic SDK
  parent_tool_use_id: S.NullOr(S.String),
});

export const SDKResultMessage = S.Struct({
  type: S.Literal("result"),
  subtype: S.Literal("success", "error_max_turns", "error_during_execution"),
  uuid: S.String,
  session_id: S.String,
  duration_ms: S.Number,
  duration_api_ms: S.Number,
  is_error: S.Boolean,
  num_turns: S.Number,
  result: S.optional(S.String),
  total_cost_usd: S.Number,
  usage: S.Unknown, // NonNullableUsage
  permission_denials: S.Array(S.Unknown),
});

export const SDKMessage = S.Union(SDKUserMessage, SDKAssistantMessage, SDKResultMessage, ...);
```

---

## Phase 3: Hooks & Permissions (Priority 2)

### 3.1 Create `src/schemas/sdk/hooks.ts`

```typescript
export const HookEvent = S.Literal(
  "PreToolUse", "PostToolUse", "Notification", "UserPromptSubmit",
  "SessionStart", "SessionEnd", "Stop", "SubagentStop", "PreCompact"
);

export const BaseHookInput = S.Struct({
  session_id: S.String,
  transcript_path: S.String,
  cwd: S.String,
  permission_mode: S.optional(S.String),
});

export const PreToolUseHookInput = S.extend(BaseHookInput, S.Struct({
  hook_event_name: S.Literal("PreToolUse"),
  tool_name: S.String,
  tool_input: S.Unknown,
}));

export const PostToolUseHookInput = S.extend(BaseHookInput, S.Struct({
  hook_event_name: S.Literal("PostToolUse"),
  tool_name: S.String,
  tool_input: S.Unknown,
  tool_response: S.Unknown,
}));

export const HookJSONOutput = S.Struct({
  continue: S.optional(S.Boolean),
  decision: S.optional(S.Literal("approve", "block")),
  systemMessage: S.optional(S.String),
  reason: S.optional(S.String),
  // ... hookSpecificOutput
});
```

### 3.2 Create `src/schemas/sdk/permissions.ts`

```typescript
export const PermissionMode = S.Literal("default", "acceptEdits", "bypassPermissions", "plan");

export const PermissionResult = S.Union(
  S.Struct({
    behavior: S.Literal("allow"),
    updatedInput: S.Unknown,
    updatedPermissions: S.optional(S.Array(S.Unknown)),
  }),
  S.Struct({
    behavior: S.Literal("deny"),
    message: S.String,
    interrupt: S.optional(S.Boolean),
  })
);

export const PermissionBehavior = S.Literal("allow", "deny", "ask");

export const PermissionRuleValue = S.Struct({
  toolName: S.String,
  ruleContent: S.optional(S.String),
});
```

---

## Phase 4: Subagent Definitions (Priority 2)

### 4.1 Create `src/schemas/sdk/agents.ts`

```typescript
export const AgentDefinition = S.Struct({
  description: S.String,
  tools: S.optional(S.Array(S.String)),
  prompt: S.String,
  model: S.optional(S.Literal("sonnet", "opus", "haiku", "inherit")),
});

export const SubagentConfig = S.Struct({
  subtask: S.Unknown, // Subtask schema
  cwd: S.String,
  tools: S.optional(S.Array(S.Unknown)),
  model: S.optional(S.String),
  maxTurns: S.optional(S.Number),
  signal: S.optional(S.Unknown),
  // Claude Code specific
  permissionMode: S.optional(PermissionMode),
  resume: S.optional(S.String),
  forkSession: S.optional(S.Boolean),
});

export const SubagentResult = S.Struct({
  success: S.Boolean,
  subtaskId: S.String,
  filesModified: S.Array(S.String),
  turns: S.Number,
  error: S.optional(S.String),
  tokenUsage: S.optional(S.Struct({
    input_tokens: S.Number,
    output_tokens: S.Number,
    cache_creation_input_tokens: S.optional(S.Number),
    cache_read_input_tokens: S.optional(S.Number),
  })),
  total_cost_usd: S.optional(S.Number),
  mode: S.optional(S.Literal("claude-code", "minimal")),
  claudeCodeSessionId: S.optional(S.String),
});
```

---

## Phase 5: Adapters & Converters (Priority 1)

### 5.1 Create `src/schemas/sdk/adapters/effect-to-zod.ts`

Convert Effect Schema to Zod for MCP tool compatibility:

```typescript
import * as S from "effect/Schema";
import { z } from "zod";

export const effectSchemaToZod = <A, I>(schema: S.Schema<A, I>): z.ZodTypeAny => {
  // Walk Effect Schema AST and generate equivalent Zod schema
  // Handle: String, Number, Boolean, Literal, Union, Struct, Array, Optional
};
```

### 5.2 Create `src/schemas/sdk/adapters/tool-adapter.ts`

Wrap Effect tools for Claude SDK MCP servers:

```typescript
export const effectToolToMcpTool = <Params, Details, R, E>(
  effectTool: Tool<Params, Details, R, E>,
  runtime: Runtime<R>
): SdkMcpToolDefinition => {
  const zodSchema = effectSchemaToZod(effectTool.schema);
  return tool(
    effectTool.name,
    effectTool.description,
    zodSchema,
    async (args) => {
      const result = await Effect.runPromise(
        effectTool.execute(args as Params).pipe(Effect.provide(runtime))
      );
      return toolResultToCallToolResult(result);
    }
  );
};
```

### 5.3 Create `src/schemas/sdk/adapters/message-adapter.ts`

Bidirectional message conversion:

```typescript
export const chatMessageToSdkMessage = (msg: ChatMessage, sessionId: string): SDKMessage => { ... };
export const sdkMessageToChatMessage = (msg: SDKMessage): ChatMessage | null => { ... };
```

---

## Phase 6: Interop Infrastructure (Priority 1)

### 6.1 Create `src/interop/router/config.ts`

Add Claude Code config to project.json:

```typescript
export const ClaudeCodeConfig = S.Struct({
  enabled: S.optionalWith(S.Boolean, { default: () => true }),
  preferForComplexTasks: S.optionalWith(S.Boolean, { default: () => true }),
  maxTurnsPerSubtask: S.optionalWith(S.Number, { default: () => 30 }),
  permissionMode: S.optionalWith(PermissionMode, { default: () => "acceptEdits" as const }),
  fallbackToMinimal: S.optionalWith(S.Boolean, { default: () => true }),
  complexityThreshold: S.optionalWith(S.Number, { default: () => 0.6 }),
  forceMinimalLabels: S.optional(S.Array(S.String)), // e.g., ["simple", "quick"]
  forceClaudeCodeLabels: S.optional(S.Array(S.String)), // e.g., ["complex", "refactor"]
});
```

### 6.2 Create `src/interop/router/subagent-router.ts`

```typescript
export const runBestAvailableSubagent = (
  config: SubagentConfig,
  projectConfig: ProjectConfig
): Effect.Effect<SubagentResult, Error> =>
  Effect.gen(function* () {
    const ccConfig = projectConfig.claudeCode ?? defaultClaudeCodeConfig;
    if (!ccConfig.enabled) return yield* runMinimalSubagent(config);

    const availability = yield* detectClaudeCode();
    if (!availability.available) return yield* runMinimalSubagent(config);

    const useCC = shouldUseClaudeCode(config.subtask, ccConfig);
    if (useCC) {
      try {
        return yield* runClaudeCodeSubagent(config);
      } catch (e) {
        if (ccConfig.fallbackToMinimal) return yield* runMinimalSubagent(config);
        throw e;
      }
    }
    return yield* runMinimalSubagent(config);
  });

const shouldUseClaudeCode = (subtask: Subtask, config: ClaudeCodeConfig): boolean => {
  // Check label-based overrides
  if (config.forceMinimalLabels?.some(l => subtask.labels?.includes(l))) return false;
  if (config.forceClaudeCodeLabels?.some(l => subtask.labels?.includes(l))) return true;

  // Complexity heuristics
  if (!config.preferForComplexTasks) return true;
  return estimateComplexity(subtask.description) > (config.complexityThreshold ?? 0.6);
};
```

### 6.3 Create `src/interop/mcp-bridge.ts`

MechaCoder coordination tools for Claude Code:

```typescript
export const createMechaCoderMcpServer = (callbacks: MechaCoderCallbacks) =>
  createSdkMcpServer({
    name: "mechacoder",
    tools: [
      tool("subtask_complete", "Signal subtask completion", {
        summary: z.string(),
        filesModified: z.array(z.string()),
      }, async (args) => { ... }),
      tool("request_help", "Request orchestrator help", {
        issue: z.string(),
        suggestion: z.string().optional(),
      }, async (args) => { ... }),
      tool("read_progress", "Read progress file", {}, async () => { ... }),
    ],
  });
```

---

## Phase 7: Migrate Existing Tools (Priority 2)

Update each tool in `src/tools/` to use SDK-compatible schemas:

| Tool | Changes Required |
|------|------------------|
| `read.ts` | `path` → `file_path` |
| `edit.ts` | `path` → `file_path`, `oldText` → `old_string`, `newText` → `new_string`, add `replace_all` |
| `write.ts` | `path` → `file_path` |
| `bash.ts` | Add `description`, `run_in_background` |
| `grep.ts` | Align with SDK `GrepInput` (output_mode, head_limit, etc.) |
| `find.ts` | Rename to match SDK patterns |
| `ls.ts` | Rename to match SDK patterns |

---

## Critical Files to Modify

**New Files:**
- `src/schemas/sdk/index.ts`
- `src/schemas/sdk/tool-inputs.ts`
- `src/schemas/sdk/tool-outputs.ts`
- `src/schemas/sdk/messages.ts`
- `src/schemas/sdk/hooks.ts`
- `src/schemas/sdk/permissions.ts`
- `src/schemas/sdk/agents.ts`
- `src/schemas/sdk/usage.ts`
- `src/schemas/sdk/type-guards.ts`
- `src/schemas/sdk/adapters/effect-to-zod.ts`
- `src/schemas/sdk/adapters/tool-adapter.ts`
- `src/schemas/sdk/adapters/message-adapter.ts`
- `src/schemas/sdk/adapters/result-adapter.ts`
- `src/interop/router/config.ts`
- `src/interop/router/subagent-router.ts`
- `src/interop/mcp-bridge.ts`
- `src/interop/index.ts`

**Files to Modify:**
- `src/tools/schema.ts` - Add SDK-compatible exports
- `src/tools/read.ts` - Rename `path` → `file_path`
- `src/tools/edit.ts` - Rename params, add `replace_all`
- `src/tools/write.ts` - Rename `path` → `file_path`
- `src/tools/bash.ts` - Add `description`, `run_in_background`
- `src/tools/grep.ts` - Align with SDK naming
- `src/tools/find.ts` - Align with SDK naming
- `src/tools/ls.ts` - Align with SDK naming
- `src/tasks/schema.ts` - Add `ClaudeCodeConfig` to `ProjectConfig`
- `src/agent/orchestrator/orchestrator.ts` - Use subagent router
- `src/agent/orchestrator/claude-code-subagent.ts` - Use SDK adapters

---

## Task List for `.openagents/tasks.jsonl`

```
oa-sdk-01: Create SDK tool input schemas (src/schemas/sdk/tool-inputs.ts)
oa-sdk-02: Create SDK tool output schemas (src/schemas/sdk/tool-outputs.ts)
oa-sdk-03: Create SDK message schemas (src/schemas/sdk/messages.ts)
oa-sdk-04: Create SDK hooks schemas (src/schemas/sdk/hooks.ts)
oa-sdk-05: Create SDK permissions schemas (src/schemas/sdk/permissions.ts)
oa-sdk-06: Create SDK agent definition schemas (src/schemas/sdk/agents.ts)
oa-sdk-07: Create SDK usage/cost schemas (src/schemas/sdk/usage.ts)
oa-sdk-08: Create type guards (src/schemas/sdk/type-guards.ts)
oa-sdk-09: Create Effect Schema → Zod converter (src/schemas/sdk/adapters/effect-to-zod.ts)
oa-sdk-10: Create tool adapter for MCP (src/schemas/sdk/adapters/tool-adapter.ts)
oa-sdk-11: Create message adapter (src/schemas/sdk/adapters/message-adapter.ts)
oa-sdk-12: Create result adapter (src/schemas/sdk/adapters/result-adapter.ts)
oa-sdk-13: Add ClaudeCodeConfig to project.json schema (src/interop/router/config.ts)
oa-sdk-14: Create subagent router (src/interop/router/subagent-router.ts)
oa-sdk-15: Create MechaCoder MCP bridge (src/interop/mcp-bridge.ts)
oa-sdk-16: Migrate read tool to SDK naming
oa-sdk-17: Migrate edit tool to SDK naming + add replace_all
oa-sdk-18: Migrate write tool to SDK naming
oa-sdk-19: Migrate bash tool + add new params
oa-sdk-20: Migrate grep tool to SDK naming
oa-sdk-21: Migrate find tool to SDK naming
oa-sdk-22: Migrate ls tool to SDK naming
oa-sdk-23: Wire subagent router into orchestrator
oa-sdk-24: Add tests for SDK schemas
oa-sdk-25: Add tests for adapters
oa-sdk-26: Add tests for subagent router
oa-sdk-epic: Epic - Harmonize Effect schemas with Claude Agent SDK
```

---

## Estimated Effort

| Phase | Tasks | Priority | Dependencies |
|-------|-------|----------|--------------|
| 1. Tool Schemas | oa-sdk-01, 02 | P1 | None |
| 2. Message Schemas | oa-sdk-03 | P1 | None |
| 3. Hooks & Permissions | oa-sdk-04, 05 | P2 | None |
| 4. Agent Definitions | oa-sdk-06, 07, 08 | P2 | None |
| 5. Adapters | oa-sdk-09, 10, 11, 12 | P1 | Phase 1, 2 |
| 6. Interop Router | oa-sdk-13, 14, 15 | P1 | Phase 5 |
| 7. Tool Migration | oa-sdk-16 through 22 | P2 | Phase 1 |
| 8. Integration | oa-sdk-23 | P1 | Phase 6 |
| 9. Testing | oa-sdk-24, 25, 26 | P1 | All above |
