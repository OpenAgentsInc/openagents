# HillClimber/TestGen UI Sync Architecture

> Real-time visualization of HillClimber/TestGen execution in the Effuse UI

## Overview

This document describes the architecture for synchronizing HillClimber/TestGen CLI execution with the Effuse-based TestGenGraphComponent. The system enables:

- **Real-time updates**: Socket-based streaming of execution events to UI
- **Dual start modes**: Launch runs from CLI or UI
- **Multi-session tracking**: Monitor multiple concurrent HillClimber runs
- **Live graph visualization**: SVG-based workflow graph updates as execution progresses

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     Desktop Server (port 8080)                          │
│                     Broadcasts HudMessage to all UI clients             │
└──────────────────────────────────┬──────────────────────────────────────┘
                                   │
         ┌─────────────────────────┼─────────────────────────┐
         │                         │                         │
         ▼                         ▼                         ▼
┌─────────────────┐      ┌─────────────────┐      ┌────────────────────────┐
│    CLI Run      │      │    UI Start     │      │  TestGenGraphComponent │
│                 │      │                 │      │                        │
│ bun scripts/    │      │ Click button    │      │  subscriptions()       │
│ test-progress-  │      │ → spawn process │      │  filters HillClimber   │
│ fix.ts          │      │                 │      │  messages by sessionId │
│                 │      │                 │      │                        │
│ HudClient       │      │ HudClient       │      │  Multi-session:        │
│ .send()         │      │ .send()         │      │  Map<sessionId,        │
└────────┬────────┘      └────────┬────────┘      │    SessionRunState>    │
         │                        │               └───────────┬────────────┘
         │                        │                           │
         └────────────────────────┴───────────────────────────┘
                              HudMessage stream
```

## Key Components

### 1. HillClimberHudEmitter (`src/hillclimber/hud-emitter.ts`)

The backend event emitter that wraps HudClient with typed methods for each event type.

```typescript
export interface HillClimberHudEmitter {
  // TestGen phase events
  onTestGenStart(taskId: string, description: string): void;
  onTestGenCategory(category: string, testCount: number): void;
  onTestGenComplete(totalTests: number, score: number): void;

  // MAP Orchestrator events
  onTurnStart(turn: number, maxTurns: number, subtask: string): void;
  onFMAction(action: "thinking" | "tool_call" | "complete", toolName?: string): void;
  onVerifyStart(): void;
  onVerifyComplete(passed: number, total: number, progress: number): void;
  onSubtaskChange(subtask: string, status: "active" | "completed" | "failed"): void;
  onHeartbeat(turn: number, maxTurns: number, progress: number, bestProgress: number, elapsedMs: number): void;
  onRunComplete(success: boolean, finalProgress: number): void;

  close(): void;
}
```

**Usage in MAP Orchestrator:**
```typescript
// In map-orchestrator.ts
export interface MAPOrchestratorOptions {
  // ... existing options
  hudEmitter?: HillClimberHudEmitter;
}

// Emit at key points
options.hudEmitter?.onTurnStart(turn, maxTurns, currentSubtask);
options.hudEmitter?.onFMAction("tool_call", toolName);
options.hudEmitter?.onVerifyComplete(passed, total, progress);
```

### 2. HUD Protocol Messages (`src/hud/protocol.ts`)

Message types for MAP orchestrator events:

| Message Type | Purpose | Key Fields |
|-------------|---------|------------|
| `map_turn_start` | New turn beginning | `turn`, `maxTurns`, `subtask` |
| `map_fm_action` | FM action update | `action`, `toolName?` |
| `map_verify` | Verification status | `status`, `passed?`, `total?`, `progress?` |
| `map_subtask_change` | Subtask transition | `subtask`, `status` |
| `map_heartbeat` | Periodic status | `turn`, `maxTurns`, `progress`, `bestProgress` |
| `map_run_complete` | Run finished | `success`, `finalProgress` |

All messages include a `sessionId` field for multi-session tracking.

### 3. Desktop Protocol & Handlers

**Request/Response Types (`src/desktop/protocol.ts`):**
```typescript
export interface StartHillClimberRequest extends BaseRequest {
  type: "request:startHillClimber";
  task: string;           // e.g., "regex-log"
  mode: "quick" | "standard" | "full";
  suitePath?: string;
}

export interface StartHillClimberResponse extends BaseResponse {
  type: "response:startHillClimber";
  data?: { sessionId: string };
}
```

**Handler (`src/desktop/handlers.ts`):**
```typescript
export async function startHillClimber(
  task: string,
  mode: "quick" | "standard" | "full",
  suitePath?: string
): Promise<{ sessionId: string }> {
  const sessionId = `hc-${Date.now()}-${randomUUID().slice(0, 8)}`;

  // Spawn process with session ID in environment
  spawn({
    cmd: [process.execPath, "scripts/test-progress-fix.ts", `--${mode}`],
    env: { ...process.env, HC_SESSION_ID: sessionId, HC_TASK: task },
  });

  return { sessionId };
}
```

### 4. Socket Service (`src/effuse/services/socket.ts`)

Effect-based socket service interface:

```typescript
export interface SocketService {
  // ... existing methods

  /** Start a HillClimber run (TestGen + MAP orchestrator) */
  readonly startHillClimber: (
    task: string,
    mode: "quick" | "standard" | "full",
    suitePath?: string
  ) => Effect.Effect<{ sessionId: string }, SocketError>
}
```

### 5. TestGenGraphComponent (`src/effuse/components/testgen-graph/`)

The Effuse UI component with multi-session state management.

**State Structure (`types.ts`):**
```typescript
export interface SessionRunState {
  sessionId: string;
  status: "waiting" | "testgen" | "running" | "completed" | "failed";

  // TestGen phase
  testGenProgress: { category: string; count: number }[];
  totalTests: number;

  // MAP phase
  currentTurn: number;
  maxTurns: number;
  currentSubtask: string;
  fmAction: string;
  testsPassed: number;
  testsTotal: number;
  progress: number;
  bestProgress: number;

  // Timestamps
  startedAt: number;
  lastUpdateAt: number;
}

export interface TestGenGraphState {
  sessions: Map<string, SessionRunState>;
  activeSessionId: string | null;
  nodes: TestGenNode[];
  connections: TestGenConnection[];
  // ... canvas state
}
```

**Socket Subscriptions:**
```typescript
subscriptions: (ctx) => [
  pipe(
    Stream.unwrap(Effect.map(SocketServiceTag, (socket) => socket.getMessages())),
    Stream.filter((msg): msg is HudMessage => isHillClimberMessage(msg)),
    Stream.map((msg) => Effect.gen(function* () {
      yield* ctx.state.update((state) => mapMessageToState(state, msg))
    }))
  ),
]
```

### 6. State Mapper (`state-mapper.ts`)

Converts HUD messages to component state updates:

```typescript
export function mapMessageToState(
  state: TestGenGraphState,
  msg: HudMessage
): TestGenGraphState {
  const sessionId = (msg as { sessionId?: string }).sessionId;
  if (!sessionId) return state;

  // Get or create session
  const sessions = new Map(state.sessions);
  let session = sessions.get(sessionId) ?? createNewSession(sessionId);

  switch (msg.type) {
    case "testgen_start":
      session = { ...session, status: "testgen", startedAt: Date.now() };
      break;
    case "map_turn_start":
      session = { ...session, status: "running", currentTurn: msg.turn, ... };
      break;
    case "map_heartbeat":
      session = { ...session, progress: msg.progress, bestProgress: msg.bestProgress };
      break;
    // ... other cases
  }

  sessions.set(sessionId, session);
  return { ...state, sessions, nodes: updateNodesFromSession(state.nodes, session) };
}
```

## Data Flow

### Starting a Run from UI

```
1. User clicks "Quick (3 turns)" button
   ↓
2. Component emits { type: "startRun", mode: "quick" }
   ↓
3. handleEvent calls socket.startHillClimber("regex-log", "quick")
   ↓
4. SocketClient sends request:startHillClimber to server
   ↓
5. handlers.ts spawns process with HC_SESSION_ID env var
   ↓
6. Response with sessionId returned to UI
   ↓
7. Component sets activeSessionId = sessionId
```

### Receiving Updates

```
1. HillClimber process calls hudEmitter.onHeartbeat(...)
   ↓
2. HudClient sends map_heartbeat message to Desktop Server
   ↓
3. Server broadcasts message to all WebSocket clients
   ↓
4. SocketClient receives message, emits to handlers
   ↓
5. Component's subscription filters by isHillClimberMessage
   ↓
6. mapMessageToState updates SessionRunState
   ↓
7. updateNodesFromSession updates graph node statuses
   ↓
8. Component re-renders with new state
```

## UI Features

### Control Panel
Located top-right, contains:
- Status display: `running | Turn 5/10 | Progress: 67.5%`
- Quick button (green): 3 turns, 5 min timeout
- Standard button (yellow): 10 turns, 15 min timeout
- Full button (orange): 25 turns, 45 min timeout

### Session Sidebar
Located left side, shows:
- List of all tracked sessions (sorted by last update)
- Session ID (truncated)
- Status with color coding
- Progress percentage and best progress
- Turn counter
- Click to switch active session

### Graph Visualization
SVG-based workflow graph showing:
- Task node (input)
- TestGen node with test count
- Category nodes (boundary, existence, etc.)
- Decomposer with subtask count
- Subtask nodes with active state
- FM node with current action
- Solution node
- Verifier with pass/total count
- Progress node with percentages
- Animated connections (dashed feedback loops)

Node colors update based on session state:
- Waiting: gray
- Running: yellow
- Completed: green
- Failed: red
- Partial: orange

## File Reference

| File | Purpose |
|------|---------|
| `src/hillclimber/hud-emitter.ts` | Backend event emitter |
| `src/hillclimber/map-orchestrator.ts` | MAP orchestrator (modified) |
| `src/hillclimber/testgen-integration.ts` | TestGen integration (modified) |
| `src/hud/protocol.ts` | HUD message types |
| `src/desktop/protocol.ts` | Socket request/response types |
| `src/desktop/handlers.ts` | Server-side handlers |
| `src/mainview/socket-client.ts` | Browser WebSocket client |
| `src/effuse/services/socket.ts` | Effect socket service interface |
| `src/effuse/services/socket-live.ts` | Socket service implementation |
| `src/effuse/components/testgen-graph/types.ts` | Component types |
| `src/effuse/components/testgen-graph/state-mapper.ts` | Message → State mapper |
| `src/effuse/components/testgen-graph/testgen-graph-component.ts` | UI component |
| `src/effuse/components/testgen-graph/render.ts` | Graph SVG rendering |
| `scripts/test-progress-fix.ts` | CLI entry point (modified) |

## Design Decisions

### Why Socket-Based Updates?
- **Real-time**: Updates appear instantly as they occur
- **Efficient**: No polling overhead
- **Scalable**: Multiple UI clients can subscribe
- **Unified**: Same infrastructure as TB runs and other HUD messages

### Why Multi-Session?
- **Parallel runs**: Support running multiple HillClimber instances
- **History**: Keep previous sessions visible for comparison
- **Debugging**: Compare different run modes side-by-side

### Why Both CLI and UI Start?
- **Flexibility**: Developers can use preferred workflow
- **Automation**: CLI for scripts, CI/CD
- **Convenience**: UI for interactive exploration
- **Same events**: Both paths emit identical HUD messages

## Testing

1. **CLI test**: `bun scripts/test-progress-fix.ts --quick`
2. **UI test**: Start desktop server, open TestGenGraph, click Quick button
3. **Multi-session**: Start multiple runs, verify sidebar updates
4. **Session switching**: Click different sessions, verify graph updates

## Future Enhancements

- Task selector dropdown (currently hardcoded to "regex-log")
- Stop/cancel button for active runs
- Session persistence (survive page reload)
- Detailed session history view
- Export session logs
- Graph layout auto-adjustment based on active subtasks
