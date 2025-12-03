# Plan: WebSocket Streaming for MechaCoder HUD UI

## Goal
Connect MechaCoder agent flow to the HUD UI via WebSocket for real-time updates of:
- OrchestratorEvents (task_selected, subtask_start, verification_complete, etc.)
- Claude Code text output (streaming)
- Tool call JSON

## Architecture

```
+------------------------------------------------------------------+
|                     ELECTROBUN DESKTOP APP                        |
|  +------------------------------------------------------------+  |
|  |                   mainview (UI Process)                     |  |
|  |                                                             |  |
|  |  +-----------------------+   +-------------------------+    |  |
|  |  |   Bun.serve()         |   |    SVG Flow HUD         |    |  |
|  |  |   WebSocket SERVER    |-->|    Real-time updates    |    |  |
|  |  |   ws://localhost:4242 |   +-------------------------+    |  |
|  |  +-----------------------+                                  |  |
|  +------------------------------------------------------------+  |
+------------------------------------------------------------------+
               ^ WebSocket Connection(s)
               |
+--------------|----------------------------------------------------+
|   AGENT PROCESS (overnight.ts / do-one-task.ts)                   |
|              |                                                    |
|  +---------------------------+                                    |
|  |  HudClient (WebSocket)    |                                    |
|  |  - Auto-reconnect         |                                    |
|  |  - Queue when disconnected|                                    |
|  +------------|--------------+                                    |
|               ^                                                   |
|               |                                                   |
|  +------------+---------------+    +---------------------------+  |
|  |   Orchestrator             |--->|   Claude Code Subagent    |  |
|  |   emit(event) --> HudClient|    |   onOutput --> HudClient  |  |
|  +----------------------------+    +---------------------------+  |
+-------------------------------------------------------------------+
```

## Message Protocol

```typescript
// src/hud/protocol.ts

export type HudMessage =
  // Orchestrator lifecycle events
  | { type: "session_start"; sessionId: string; ts: string }
  | { type: "session_complete"; success: boolean; summary: string; ts: string }
  | { type: "task_selected"; taskId: string; title: string; ts: string }
  | { type: "subtask_start"; subtaskId: string; description: string; ts: string }
  | { type: "subtask_complete"; subtaskId: string; success: boolean; agent: string; ts: string }
  | { type: "subtask_failed"; subtaskId: string; error: string; ts: string }
  | { type: "verification_start"; command: string; ts: string }
  | { type: "verification_complete"; command: string; passed: boolean; ts: string }
  | { type: "commit_created"; sha: string; message: string; ts: string }
  | { type: "push_complete"; branch: string; ts: string }
  | { type: "error"; phase: string; error: string; ts: string }
  // Streaming text from Claude Code
  | { type: "text"; text: string; ts: string }
  // Tool calls
  | { type: "tool_call"; tool: string; input: unknown; ts: string }
  // Control messages
  | { type: "agent_connect"; agentId: string; cwd: string; ts: string }
  | { type: "agent_disconnect"; agentId: string; ts: string };
```

## Files to Create

| File | Purpose |
|------|---------|
| `src/hud/protocol.ts` | HudMessage type definitions |
| `src/hud/client.ts` | WebSocket client for agents (auto-reconnect, queue) |
| `src/hud/server.ts` | Bun.serve() WebSocket server for Electrobun |
| `src/hud/index.ts` | Public exports |

## Files to Modify

| File | Changes |
|------|---------|
| `src/agent/overnight.ts` | Create HudClient, wire emit and onOutput |
| `src/agent/do-one-task.ts` | Same pattern as overnight.ts |
| `src/mainview/index.ts` | Start HUD server, subscribe to messages, update UI |

## Implementation Steps

### Phase 1: Protocol and Client (Agent Side)

1. **Create `src/hud/protocol.ts`**
   - Define HudMessage union type
   - Add `mapOrchestratorEvent(event: OrchestratorEvent): HudMessage` helper

2. **Create `src/hud/client.ts`**
   ```typescript
   export class HudClient {
     private ws: WebSocket | null = null;
     private queue: HudMessage[] = [];

     connect(url: string = "ws://localhost:4242"): void
     send(message: HudMessage): void  // Queues if disconnected
     createEmit(): (event: OrchestratorEvent) => void
     createOnOutput(): (text: string) => void
     close(): void
   }
   ```
   - Auto-reconnect with backoff (max 5 attempts)
   - Silent fail if HUD not running (agent continues)
   - Queue messages during reconnect

3. **Create `src/hud/index.ts`** - Export HudClient and types

### Phase 2: Server (Electrobun Side)

4. **Create `src/hud/server.ts`**
   ```typescript
   export function startHudServer(
     port: number,
     onMessage: (msg: HudMessage) => void
   ): void
   ```
   - Use Bun.serve() with websocket handlers
   - Track connected agents
   - Forward messages to callback

5. **Modify `src/mainview/index.ts`**
   - Call `startHudServer()` on app start
   - Replace 5-second polling with message-driven updates
   - Update UI state on each HudMessage

### Phase 3: Integration

6. **Modify `src/agent/overnight.ts`**
   ```typescript
   const hudClient = new HudClient();
   hudClient.connect();

   const emit = (event: OrchestratorEvent) => {
     logOrchestratorEvent(event);  // Existing logging
     hudClient.createEmit()(event);  // Send to HUD
   };

   const onOutput = (text: string) => {
     process.stdout.write(text);  // Existing console output
     hudClient.createOnOutput()(text);  // Send to HUD
   };
   ```

7. **Modify `src/agent/do-one-task.ts`** - Same pattern

### Phase 4: UI Updates

8. **Update mainview UI** to display:
   - Real-time event log
   - Streaming text output
   - Tool call visualization
   - Task/subtask progress

## Design Decisions

| Decision | Rationale |
|----------|-----------|
| **No Effect PubSub** | Agent only has one subscriber (WS). PubSub adds complexity without benefit. |
| **Native WebSocket client** | Simpler than @effect/platform Socket. HudClient handles lifecycle. |
| **Bun.serve() for server** | Native to Electrobun, no extra deps. |
| **Silent fail on no HUD** | Agent should work even if HUD isn't running. |
| **Message queue during reconnect** | Don't lose events during brief disconnects. |
| **Port 4242** | Arbitrary but memorable. Could make configurable. |

## Error Handling

- **HUD not running**: Agent logs debug message, continues working
- **HUD restarts**: Client auto-reconnects, flushes queue
- **Agent crashes**: Server detects disconnect, notifies UI
- **Invalid messages**: Server logs and ignores

## Testing

- Unit tests for protocol mapping
- Integration test with mock WebSocket
- Manual E2E test running overnight.ts with HUD open

## Critical Files to Read Before Implementation

1. `src/agent/orchestrator/types.ts` - OrchestratorEvent definitions
2. `src/agent/overnight.ts` - Current emit/onOutput wiring
3. `src/mainview/index.ts` - Current UI state management
4. `src/flow/mechacoder-state.ts` - State loading patterns
