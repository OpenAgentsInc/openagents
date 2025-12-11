# Claude Code Integration Strategy

> How MechaCoder should leverage the Claude Agent SDK for overnight automation.

## Executive Summary

Claude Code (via `@anthropic-ai/claude-agent-sdk`) is a production-grade coding agent with automatic context compaction, session management, and rich tooling. MechaCoder's orchestrator should delegate to Claude Code when available, using it as a "superpower subagent" while falling back to our minimal subagent when unavailable.

**Key insight:** We don't replace Claude Code - we orchestrate it. The orchestrator handles task selection, decomposition, verification, and progress tracking. Claude Code handles the actual coding with its superior context management.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                     MechaCoder Orchestrator                         │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────┐  ┌────────────┐ │
│  │   Orient    │→ │ Select Task  │→ │ Decompose  │→ │  Execute   │ │
│  └─────────────┘  └──────────────┘  └────────────┘  └─────┬──────┘ │
│                                                           │        │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────┐        │        │
│  │    Log      │← │ Update Task  │← │   Verify   │←───────┘        │
│  └─────────────┘  └──────────────┘  └────────────┘                 │
└─────────────────────────────────────────────────────────────────────┘
                                │
                    ┌───────────┴───────────┐
                    ▼                       ▼
         ┌─────────────────┐     ┌─────────────────────┐
         │  Claude Code    │     │  Minimal Subagent   │
         │  (Agent SDK)    │     │  (Built-in)         │
         ├─────────────────┤     ├─────────────────────┤
         │ • Context mgmt  │     │ • ~50 token prompt  │
         │ • Session resume│     │ • 4 tools only      │
         │ • MCP tools     │     │ • No dependencies   │
         │ • Web search    │     │ • Always available  │
         │ • Subagents     │     │                     │
         └─────────────────┘     └─────────────────────┘
```

## When to Use Each Subagent

| Use Claude Code When... | Use Minimal Subagent When... |
|------------------------|------------------------------|
| Claude Code is installed and API key available or authed with Max plan | Claude Code unavailable |
| Complex multi-file refactoring | Simple, focused edits |
| Tasks needing web search/fetch | Pure code changes |
| Long-running tasks (>20 turns) | Quick subtasks (<10 turns) |
| Need session resumption | No context bridging needed |
| Want CC's safety/permissions | Full automation control |

## Detection: Is Claude Code Available?

Create a detection module that runs at orchestrator startup:

```typescript
// src/agent/orchestrator/claude-code-detector.ts

import { Effect } from "effect";

export interface ClaudeCodeAvailability {
  available: boolean;
  version?: string;
  apiKeySource?: "env" | "config" | "none";
  reason?: string;
}

export const detectClaudeCode = (): Effect.Effect<ClaudeCodeAvailability, never, never> =>
  Effect.gen(function* () {
    // 1. Check if SDK is installed
    try {
      await import("@anthropic-ai/claude-agent-sdk");
    } catch {
      return {
        available: false,
        reason: "SDK not installed: npm install @anthropic-ai/claude-agent-sdk",
      };
    }

    // 2. Check for API key
    const apiKey = process.env.ANTHROPIC_API_KEY;
    if (!apiKey) {
      return {
        available: false,
        apiKeySource: "none",
        reason: "ANTHROPIC_API_KEY not set",
      };
    }

    // 3. Verify with a minimal query (optional, adds latency)
    // Could do a lightweight health check here

    return {
      available: true,
      apiKeySource: "env",
    };
  });
```

## Invoking Claude Code from Orchestrator

### Basic Invocation

```typescript
// src/agent/orchestrator/claude-code-subagent.ts

import { query, type SDKMessage } from "@anthropic-ai/claude-agent-sdk";
import type { Subtask, SubagentResult } from "./types.js";

export const runClaudeCodeSubagent = async (
  subtask: Subtask,
  cwd: string,
  options?: {
    maxTurns?: number;
    signal?: AbortSignal;
  }
): Promise<SubagentResult> => {
  const filesModified: string[] = [];
  let turns = 0;
  let success = false;
  let error: string | undefined;

  const abortController = new AbortController();
  if (options?.signal) {
    options.signal.addEventListener("abort", () => abortController.abort());
  }

  try {
    for await (const message of query({
      prompt: buildSubtaskPrompt(subtask),
      options: {
        cwd,
        systemPrompt: {
          type: "preset",
          preset: "claude_code",
          append: MECHACODER_APPEND_PROMPT,
        },
        settingSources: ["project"], // Load CLAUDE.md
        permissionMode: "bypassPermissions", // Automation mode
        maxTurns: options?.maxTurns ?? 30,
        abortController,
      },
    })) {
      turns++;

      // Track file modifications
      if (message.type === "assistant") {
        const toolCalls = extractToolCalls(message);
        for (const call of toolCalls) {
          if (call.name === "Edit" || call.name === "Write") {
            filesModified.push(call.input.file_path);
          }
        }
      }

      // Check for completion
      if (message.type === "result") {
        if (message.subtype === "success") {
          success = true;
        } else {
          error = `Claude Code finished with: ${message.subtype}`;
        }
      }
    }
  } catch (e: any) {
    error = e.message;
  }

  return {
    success,
    subtaskId: subtask.id,
    filesModified: [...new Set(filesModified)],
    turns,
    error,
  };
};

const MECHACODER_APPEND_PROMPT = `
You are executing a subtask for the MechaCoder orchestrator.

Guidelines:
- Focus only on this subtask - don't expand scope
- Make minimal, surgical changes
- If blocked, explain why clearly
- When complete, summarize what you changed
`;

const buildSubtaskPrompt = (subtask: Subtask): string => {
  return `## Subtask: ${subtask.id}

${subtask.description}

Complete this subtask. Focus on minimal, correct changes.`;
};
```

### With Custom MCP Tools

Provide Claude Code with MechaCoder-specific tools:

```typescript
import { query, tool, createSdkMcpServer } from "@anthropic-ai/claude-agent-sdk";
import { z } from "zod";

// Create MechaCoder-specific tools
const mechacoderServer = createSdkMcpServer({
  name: "mechacoder",
  version: "1.0.0",
  tools: [
    tool(
      "subtask_complete",
      "Signal that the current subtask is complete",
      {
        summary: z.string().describe("Brief summary of what was done"),
        filesModified: z.array(z.string()).describe("List of modified files"),
      },
      async (args) => ({
        content: [{ type: "text", text: `Subtask complete: ${args.summary}` }],
      })
    ),

    tool(
      "request_help",
      "Request orchestrator intervention when stuck",
      {
        issue: z.string().describe("What problem you're facing"),
        suggestion: z.string().optional().describe("Suggested resolution"),
      },
      async (args) => ({
        content: [{ type: "text", text: `Help requested: ${args.issue}` }],
      })
    ),

    tool(
      "read_progress",
      "Read the current session progress file",
      {},
      async () => {
        const progress = readProgress(openagentsDir);
        return {
          content: [{ type: "text", text: JSON.stringify(progress, null, 2) }],
        };
      }
    ),
  ],
});

// Use with streaming input (required for MCP)
async function* generateMessages(subtask: Subtask) {
  yield {
    type: "user" as const,
    message: {
      role: "user" as const,
      content: buildSubtaskPrompt(subtask),
    },
  };
}

for await (const message of query({
  prompt: generateMessages(subtask),
  options: {
    mcpServers: { mechacoder: mechacoderServer },
    allowedTools: [
      "Read", "Write", "Edit", "Bash", "Glob", "Grep",
      "mcp__mechacoder__subtask_complete",
      "mcp__mechacoder__request_help",
      "mcp__mechacoder__read_progress",
    ],
    // ... other options
  },
})) {
  // Handle messages
}
```

### Session Resumption for Long Tasks

For tasks that span multiple orchestrator runs:

```typescript
import { query } from "@anthropic-ai/claude-agent-sdk";

// Store session IDs in subtask metadata
interface SubtaskWithSession extends Subtask {
  claudeCodeSessionId?: string;
}

const runWithSessionResume = async (subtask: SubtaskWithSession) => {
  let sessionId: string | undefined;

  for await (const message of query({
    prompt: subtask.claudeCodeSessionId
      ? "Continue working on this subtask"
      : buildSubtaskPrompt(subtask),
    options: {
      resume: subtask.claudeCodeSessionId, // Resume if we have a session
      forkSession: false, // Continue same session
      // ... other options
    },
  })) {
    // Capture session ID from init message
    if (message.type === "system" && message.subtype === "init") {
      sessionId = message.session_id;
    }

    // Handle other messages...
  }

  // Store session ID for potential resumption
  if (sessionId && !subtask.claudeCodeSessionId) {
    subtask.claudeCodeSessionId = sessionId;
    // Persist to subtasks file
  }
};
```

## Hooks for Observability

Use hooks to integrate with MechaCoder's logging:

```typescript
import { query, type HookCallback } from "@anthropic-ai/claude-agent-sdk";

const logToolUse: HookCallback = async (input, toolUseId) => {
  if (input.hook_event_name === "PostToolUse") {
    console.log(`[CC] Tool: ${input.tool_name}`);
    // Could write to progress file here
  }
  return { continue: true };
};

const handleSessionEnd: HookCallback = async (input) => {
  if (input.hook_event_name === "SessionEnd") {
    console.log(`[CC] Session ended: ${input.reason}`);
    // Update progress file with session summary
  }
  return { continue: true };
};

for await (const message of query({
  prompt: subtaskPrompt,
  options: {
    hooks: {
      PostToolUse: [{ hooks: [logToolUse] }],
      SessionEnd: [{ hooks: [handleSessionEnd] }],
    },
    // ... other options
  },
})) {
  // Process messages
}
```

## Fallback Strategy

```typescript
// src/agent/orchestrator/subagent-router.ts

import { detectClaudeCode } from "./claude-code-detector.js";
import { runClaudeCodeSubagent } from "./claude-code-subagent.js";
import { runSubagent } from "./subagent.js"; // Minimal subagent

export const runBestAvailableSubagent = async (
  subtask: Subtask,
  cwd: string,
  options?: SubagentOptions
): Promise<SubagentResult> => {
  const ccAvailability = await detectClaudeCode();

  if (ccAvailability.available && shouldUseClaudeCode(subtask)) {
    console.log(`[Orchestrator] Using Claude Code for ${subtask.id}`);
    try {
      return await runClaudeCodeSubagent(subtask, cwd, options);
    } catch (error) {
      console.warn(`[Orchestrator] Claude Code failed, falling back: ${error}`);
      // Fall through to minimal subagent
    }
  }

  console.log(`[Orchestrator] Using minimal subagent for ${subtask.id}`);
  return runSubagent(createSubagentConfig(subtask, cwd, SUBAGENT_TOOLS, options));
};

const shouldUseClaudeCode = (subtask: Subtask): boolean => {
  // Heuristics for when Claude Code is beneficial
  const description = subtask.description.toLowerCase();

  // Use CC for complex tasks
  if (description.includes("refactor")) return true;
  if (description.includes("multi-file")) return true;
  if (description.includes("search")) return true;
  if (description.length > 500) return true;

  // Use minimal for simple tasks
  return false;
};
```

## Configuration

Add to `.openagents/project.json`:

```json
{
  "claudeCode": {
    "enabled": true,
    "preferForComplexTasks": true,
    "maxTurnsPerSubtask": 30,
    "permissionMode": "bypassPermissions",
    "fallbackToMinimal": true
  }
}
```

**claudeCode fields**
- `enabled` (default `true`): allow Claude Code at all.
- `preferForComplexTasks` (default `true`): only route complex subtasks to Claude Code; when `false`, use Claude Code for all tasks.
- `maxTurnsPerSubtask` (default `30`): maximum turns before the subagent stops.
- `permissionMode` (default `"bypassPermissions"`): Claude Code permission behavior (`default`, `acceptEdits`, `bypassPermissions`, `plan`, `dontAsk`).
- `fallbackToMinimal` (default `true`): if Claude Code fails, run the minimal subagent instead.

## Project Context via CLAUDE.md

Claude Code reads `CLAUDE.md` from the project directory when `settingSources: ['project']` is configured in query options. This provides project-specific instructions and context.

**Setup for this repo:**

1. **Symlink**: `CLAUDE.md -> AGENTS.md`
   - Single source of truth: maintain `AGENTS.md`
   - Claude Code automatically loads it via the symlink
   - No separate file to keep in sync

2. **SDK Configuration**: Pass `settingSources: ['project']` to `query()`
   ```typescript
   for await (const message of query({
     prompt: subtaskPrompt,
     options: {
       cwd: options.cwd,
       settingSources: ["project"], // Loads CLAUDE.md
       // ... other options
     },
   })) {
     // Handle messages
   }
   ```

3. **Content**: `AGENTS.md` contains:
   - Agent startup checklist (read docs, check health)
   - Task tracking with `.openagents/`
   - Git and GitHub CLI conventions
   - Work logging requirements
   - Common patterns and lessons learned

**Maintenance:**
- Update `AGENTS.md` when project patterns or conventions change
- Claude Code will automatically see updates via the symlink
- No code generation or sync scripts needed

## Implementation Tasks

Create these tasks in `.openagents/tasks.jsonl`:

1. **oa-cc01**: Implement Claude Code availability detector
   - Check SDK installation
   - Verify API key
   - Optional health check
   - Priority: P1

2. **oa-cc02**: Create Claude Code subagent wrapper
   - Wrap `query()` with our interface
   - Handle message streaming
   - Track file modifications
   - Priority: P1

3. **oa-cc03**: Add MechaCoder MCP tools for Claude Code
   - `subtask_complete` tool
   - `request_help` tool
   - `read_progress` tool
   - Priority: P1

4. **oa-cc04**: Implement subagent router with fallback
   - Detect availability
   - Route based on task complexity
   - Handle failures gracefully
   - Priority: P1

5. **oa-cc05**: Add session resumption support
   - Store session IDs in subtask metadata
   - Resume long-running subtasks
   - Fork sessions when needed
   - Priority: P2

6. **oa-cc06**: Add observability hooks
   - Log tool usage to progress file
   - Track token usage
   - Session lifecycle events
   - Priority: P2

## Security Considerations

1. **Permission Mode**: Use `bypassPermissions` for automation, but only in controlled environments
2. **API Keys**: Never log or expose `ANTHROPIC_API_KEY`
3. **Sandboxing**: Consider enabling Claude Code's sandbox for untrusted repos
4. **Tool Restrictions**: Use `allowedTools` to limit what CC can do in sensitive contexts
5. **Permission Schemas**: SDK-compatible permission types live at `src/schemas/sdk/permissions.ts` to mirror Claude Agent SDK behavior when bridging permission decisions or rule updates.

## Cost Optimization

1. **Use minimal subagent for simple tasks** - Avoids CC overhead
2. **Limit maxTurns** - Prevents runaway sessions
3. **Prompt caching** - CC handles this automatically
4. **Session resumption** - Reuses context instead of rebuilding

## Monitoring

Track these metrics:
- Claude Code vs minimal subagent usage ratio
- Average turns per subtask by subagent type
- Fallback frequency (CC failures)
- Token usage per subtask
- Session resumption frequency

## References

- [Agent SDK Overview](./agent-sdk/agent-sdk-overview.md)
- [TypeScript SDK Reference](./agent-sdk/typescript-sdk-reference.md)
- [Custom Tools Guide](./agent-sdk/guides/custom-tools.md)
- [Session Management](./agent-sdk/guides/session-management.md)
- [Subagents Guide](./agent-sdk/guides/subagents-in-the-sdk.md)
- [MechaCoder Orchestrator](../mechacoder/GOLDEN-LOOP-v2.md)
