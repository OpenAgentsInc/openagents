# HUD Event Mapping for Golden Loop Phases

This document describes how MechaCoder orchestrator events map to HUD WebSocket messages during the Golden Loop v2 execution.

## Architecture Overview

```
┌─────────────────────┐      OrchestratorEvent      ┌─────────────────────┐
│   Orchestrator      │ ──────────────────────────► │    emit()           │
│   (overnight.ts)    │                             │    callback         │
└─────────────────────┘                             └──────────┬──────────┘
                                                               │
                                                               ▼
                                                    ┌─────────────────────┐
                                                    │ orchestratorEvent   │
                                                    │ ToHudMessage()      │
                                                    └──────────┬──────────┘
                                                               │
                                                     HudMessage (JSON)
                                                               │
                                                               ▼
                                                    ┌─────────────────────┐
                                                    │    HudClient        │
                                                    │    (WebSocket)      │
                                                    └──────────┬──────────┘
                                                               │
                                                     ws://localhost:4242
                                                               │
                                                               ▼
                                                    ┌─────────────────────┐
                                                    │   Electrobun HUD    │
                                                    │   (mainview)        │
                                                    └─────────────────────┘
```

## Event Mapping by Golden Loop Phase

The Golden Loop v2 runs through these phases:
1. **Orient** - Load config, check repo state, run init.sh
2. **Select Task** - Pick highest-priority ready task
3. **Decompose** - Break task into subtasks
4. **Execute** - Run subagent on each subtask
5. **Verify** - Run typecheck and tests
6. **Commit** - Create git commit
7. **Update** - Close task, write progress
8. **Log** - Write session summary

### Phase → Event → HUD Message Mapping

| Phase | OrchestratorEvent | HudMessage | Description |
|-------|-------------------|------------|-------------|
| **Start** | `session_start` | `session_start` | Session begins with ID and timestamp |
| Orient | `lock_acquired` | *not forwarded* | Internal lock management |
| Orient | `lock_stale_removed` | *not forwarded* | Cleanup of dead process lock |
| Orient | `lock_failed` | *not forwarded* | Another agent is running |
| Orient | `lock_released` | *not forwarded* | Lock cleanup on exit |
| Orient | `init_script_start` | *not forwarded* | Preflight script starting |
| Orient | `init_script_complete` | *not forwarded* | Preflight results (internal) |
| Orient | `orientation_complete` | *not forwarded* | Repo state assessment done |
| **Select** | `task_selected` | `task_selected` | Task picked for execution |
| **Decompose** | `task_decomposed` | `task_decomposed` | Subtask list created |
| **Execute** | `subtask_start` | `subtask_start` | Beginning work on subtask |
| Execute | `subtask_complete` | `subtask_complete` | Subtask finished successfully |
| Execute | `subtask_failed` | `subtask_failed` | Subtask failed with error |
| **Verify** | `verification_start` | `verification_start` | Running typecheck/tests |
| Verify | `verification_complete` | `verification_complete` | Verification result |
| **Commit** | `commit_created` | `commit_created` | Git commit with SHA |
| Commit | `push_complete` | `push_complete` | Pushed to remote branch |
| **Update** | `task_updated` | *not forwarded* | Internal task state change |
| **Log** | `progress_written` | *not forwarded* | Progress file written |
| **End** | `session_complete` | `session_complete` | Session finished |
| *Any* | `error` | `error` | Error occurred in phase |

### Streaming Events (During Execute Phase)

During subtask execution, Claude Code streams output which generates these additional messages:

| HudMessage | Source | Description |
|------------|--------|-------------|
| `text_output` | Claude Code stdout | Streaming text from agent reasoning |
| `tool_call` | Claude Code | Agent is invoking a tool |
| `tool_result` | Claude Code | Tool execution result |
| `phase_change` | Orchestrator | Phase transition notification |

## Message Type Reference

### Session Lifecycle

```typescript
// Session start
{ type: "session_start", sessionId: string, timestamp: string }

// Session end
{ type: "session_complete", success: boolean, summary: string }
```

### Task Flow

```typescript
// Task selected for work
{
  type: "task_selected",
  task: { id: string, title: string, status: string, priority: number }
}

// Task broken into subtasks
{
  type: "task_decomposed",
  subtasks: Array<{ id: string, description: string, status: SubtaskStatus }>
}

// Subtask execution
{ type: "subtask_start", subtask: { id, description, status } }
{ type: "subtask_complete", subtask: {...}, result: { success, filesModified, turns, agent?, error? } }
{ type: "subtask_failed", subtask: {...}, error: string }
```

### Verification

```typescript
// Starting verification command
{ type: "verification_start", command: string }

// Verification result
{ type: "verification_complete", command: string, passed: boolean, output?: string }
```

### Git Operations

```typescript
// Commit created
{ type: "commit_created", sha: string, message: string }

// Push completed
{ type: "push_complete", branch: string }
```

### Streaming Output

```typescript
// Text from Claude Code or orchestrator
{ type: "text_output", text: string, source?: "claude-code" | "minimal" | "orchestrator" }

// Tool invocation
{ type: "tool_call", toolName: string, arguments: string, callId?: string }

// Tool result
{ type: "tool_result", toolName: string, result: string, isError: boolean, callId?: string }
```

### Phase Changes and Errors

```typescript
// Phase transition
{ type: "phase_change", phase: OrchestratorPhase }

// Error in any phase
{ type: "error", phase: OrchestratorPhase, error: string }
```

Where `OrchestratorPhase` is one of:
- `idle` | `orienting` | `selecting_task` | `decomposing`
- `executing_subtask` | `verifying` | `committing`
- `updating_task` | `logging` | `done` | `failed`

## Example Event Sequence

A typical successful Golden Loop run produces this event sequence:

```
1. session_start { sessionId: "session-2025-12-03T10-00-00Z-abc123" }
2. task_selected { task: { id: "oa-abc123", title: "Add feature X", ... } }
3. task_decomposed { subtasks: [{ id: "oa-abc123-sub-001", ... }, ...] }
4. subtask_start { subtask: { id: "oa-abc123-sub-001", ... } }
   ... text_output, tool_call, tool_result messages ...
5. subtask_complete { subtask: {...}, result: { success: true, ... } }
6. verification_start { command: "bun run typecheck" }
7. verification_complete { command: "bun run typecheck", passed: true }
8. verification_start { command: "bun test" }
9. verification_complete { command: "bun test", passed: true }
10. commit_created { sha: "abc123", message: "oa-abc123: Add feature X" }
11. push_complete { branch: "main" }
12. session_complete { success: true, summary: "Completed task oa-abc123" }
```

## Events NOT Forwarded to HUD

These internal events are intentionally filtered out:

| Event | Reason |
|-------|--------|
| `orientation_complete` | Internal state, not user-facing |
| `init_script_start/complete` | Preflight details are logged, not displayed |
| `task_updated` | Redundant with task_selected/session_complete |
| `progress_written` | Internal file operation |
| `lock_*` | Process coordination, not user-facing |

## Using the HUD Emitter

To connect the orchestrator to the HUD:

```typescript
import { createHudEmitter, createHudOutputCallback } from "../hud/emit.js";

// Create emit callback for orchestrator events
const emit = createHudEmitter();

// Create output callback for streaming text
const onOutput = createHudOutputCallback();

// Run orchestrator with HUD integration
await runOrchestrator({ ...config, onOutput }, emit);
```

Or use the combined helper:

```typescript
import { createHudCallbacks } from "../hud/emit.js";

const { emit, onOutput, client } = createHudCallbacks();

await runOrchestrator({ ...config, onOutput }, emit);

// When done
client.close();
```

## Testing HUD Integration

See `src/hud/emit.test.ts` for test fixtures that verify:
- All forwarded events produce valid HudMessages
- Events not meant for HUD return null
- Message round-trips serialize/deserialize correctly
- Mock server receives expected messages during a sample loop

## Related Files

- `src/hud/protocol.ts` - HudMessage type definitions
- `src/hud/emit.ts` - Event conversion and emitter factories
- `src/hud/client.ts` - WebSocket client with queue/reconnect
- `src/hud/server.ts` - WebSocket server for Electrobun mainview
- `src/agent/orchestrator/types.ts` - OrchestratorEvent definitions
- `docs/mechacoder/GOLDEN-LOOP-v2.md` - Golden Loop specification
