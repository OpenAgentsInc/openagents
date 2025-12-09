# Plan: HillClimber/TestGen CLI ↔ Effuse UI Sync

**Date:** 2025-12-09
**Status:** Planning
**Goal:** Run HillClimber/TestGen via UI and see live progress in TestGenGraphComponent

---

## Executive Summary

We have:
- **CLI side**: HillClimber/TestGen running via `bun scripts/test-progress-fix.ts`
- **UI side**: TestGenGraphComponent with hardcoded static data
- **Socket infrastructure**: Desktop server (port 8080), HudClient, SocketService

We need to connect them so:
1. CLI emits structured events via HudClient
2. UI subscribes to socket events and updates component state
3. Graph visualization shows live progress as HillClimber runs

---

## Current Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Desktop Server (port 8080)                   │
│                     src/desktop/server.ts                           │
│  - Serves static files + WebSocket                                  │
│  - Broadcasts HudMessage to all connected UI clients                │
│  - Maintains message history (100 messages)                         │
└──────────────────────────────────┬──────────────────────────────────┘
                                   │
         ┌─────────────────────────┼─────────────────────────┐
         │                         │                         │
         ▼                         ▼                         ▼
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────────┐
│  Effuse UI      │    │  HillClimber    │    │  MechaCoder/TB      │
│  (browser)      │    │  (CLI process)  │    │  (other agents)     │
│                 │    │                 │    │                     │
│ SocketClient    │    │ HudClient ❌    │    │ HudClient ✓         │
│ subscribes to   │    │ NOT CONNECTED   │    │ emits events        │
│ messages        │    │                 │    │                     │
└─────────────────┘    └─────────────────┘    └─────────────────────┘
```

**Gap**: HillClimber CLI doesn't emit socket events. TestGenGraphComponent has static data.

---

## Target Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Desktop Server (port 8080)                   │
└──────────────────────────────────┬──────────────────────────────────┘
                                   │
         ┌─────────────────────────┼─────────────────────────┐
         │                         │                         │
         ▼                         ▼                         ▼
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────────┐
│  Effuse UI      │    │  HillClimber    │    │  TestGen Process    │
│                 │    │                 │    │                     │
│ TestGenGraph    │◀───│ HudClient ✓     │◀───│ HudClient ✓         │
│ Component       │    │                 │    │                     │
│ - subscribes    │    │ Emits:          │    │ Emits:              │
│ - updates state │    │ - turn_start    │    │ - testgen_start     │
│ - re-renders    │    │ - progress      │    │ - testgen_progress  │
│                 │    │ - subtask_change│    │ - testgen_test      │
│                 │    │ - heartbeat     │    │ - testgen_complete  │
└─────────────────┘    └─────────────────┘    └─────────────────────┘
```

---

## Implementation Plan

### Phase 1: Backend Event Emission (2-3 hours)

#### 1.1 Create HillClimber HUD Emitter

**File:** `src/hillclimber/hud-emitter.ts` (NEW)

```typescript
import { HudClient } from "../hud/client.js";

export interface HillClimberHudEmitter {
  // TestGen phase
  onTestGenStart: (taskId: string, description: string) => void;
  onTestGenProgress: (phase: string, category: string, roundNumber: number) => void;
  onTestGenTest: (test: { id: string; category: string; input: string }) => void;
  onTestGenComplete: (totalTests: number, score: number) => void;

  // MAP Orchestrator phase
  onTurnStart: (turn: number, maxTurns: number, subtask: string) => void;
  onSubtaskChange: (subtask: string, status: "active" | "completed" | "failed") => void;
  onFMAction: (action: "thinking" | "tool_call" | "complete", toolName?: string) => void;
  onVerifyStart: () => void;
  onVerifyComplete: (passed: number, total: number, progress: number) => void;
  onProgressUpdate: (current: number, best: number) => void;
  onHeartbeat: (turn: number, maxTurns: number, subtask: string, progress: number, elapsed: number) => void;

  // Lifecycle
  close: () => void;
}

export function createHillClimberHudEmitter(sessionId: string): HillClimberHudEmitter {
  const client = new HudClient();

  return {
    onTestGenStart: (taskId, description) => {
      client.send({
        type: "testgen_start",
        sessionId,
        taskId,
        taskDescription: description,
        environment: {},
      });
    },

    onTestGenProgress: (phase, category, roundNumber) => {
      client.send({
        type: "testgen_progress",
        sessionId,
        phase,
        currentCategory: category,
        roundNumber,
        status: "running",
      });
    },

    // ... more methods

    close: () => client.close(),
  };
}
```

#### 1.2 Integrate Emitter into MAP Orchestrator

**File:** `src/hillclimber/map-orchestrator.ts` (MODIFY)

Add to `MAPOrchestratorOptions`:
```typescript
export interface MAPOrchestratorOptions {
  // ... existing options
  hudEmitter?: HillClimberHudEmitter;  // NEW
}
```

Add emissions at key points:
- Line ~664: `hudEmitter?.onTurnStart(turn, maxTurns, subtask)`
- Line ~822: `hudEmitter?.onVerifyComplete(passed, total, progress)`
- Line ~844: `hudEmitter?.onSubtaskChange(subtask, "completed")`
- Line ~650: `hudEmitter?.onHeartbeat(...)`

#### 1.3 Integrate Emitter into TestGen

**File:** `src/hillclimber/testgen-integration.ts` (MODIFY)

The iterative generator already has `IterativeTestGenEmitter`. Wire it to HudClient:

```typescript
export async function runTestGenWithHud(
  task: TB2Task,
  workspace: string,
  hudEmitter: HillClimberHudEmitter
): Promise<GeneratedTestSuite> {
  return generateTestsIteratively(
    task.id,
    task.description,
    { workspace },
    {
      onStart: () => hudEmitter.onTestGenStart(task.id, task.description),
      onProgress: (msg) => hudEmitter.onTestGenProgress(msg.phase, msg.currentCategory, msg.roundNumber),
      onTest: (msg) => hudEmitter.onTestGenTest(msg.test),
      onComplete: (msg) => hudEmitter.onTestGenComplete(msg.totalTests, msg.comprehensivenessScore),
      onReflection: () => {},
      onError: () => {},
    }
  );
}
```

#### 1.4 Update CLI Scripts

**File:** `scripts/test-progress-fix.ts` (MODIFY)

```typescript
import { createHillClimberHudEmitter } from "../src/hillclimber/hud-emitter.js";

// Create emitter at start
const sessionId = crypto.randomUUID();
const hudEmitter = createHillClimberHudEmitter(sessionId);

// Pass to orchestrator
const result = await runMAPOrchestrator(task, {
  workspace,
  maxTurns: 10,
  hudEmitter,  // NEW
});

// Clean up
hudEmitter.close();
```

---

### Phase 2: Frontend Socket Subscription (2-3 hours)

#### 2.1 Define HillClimber Message Types

**File:** `src/hud/protocol.ts` (MODIFY)

Add new message types for MAP orchestrator (TestGen types already exist):

```typescript
// MAP Orchestrator Messages
export interface MAPTurnStartMessage {
  type: "map_turn_start";
  sessionId: string;
  turn: number;
  maxTurns: number;
  subtask: string;
}

export interface MAPSubtaskChangeMessage {
  type: "map_subtask_change";
  sessionId: string;
  subtask: string;
  status: "active" | "completed" | "failed";
}

export interface MAPFMActionMessage {
  type: "map_fm_action";
  sessionId: string;
  action: "thinking" | "tool_call" | "complete";
  toolName?: string;
  content?: string;
}

export interface MAPVerifyMessage {
  type: "map_verify";
  sessionId: string;
  status: "running" | "complete";
  passed?: number;
  total?: number;
  progress?: number;
}

export interface MAPHeartbeatMessage {
  type: "map_heartbeat";
  sessionId: string;
  turn: number;
  maxTurns: number;
  subtask: string;
  progress: number;
  bestProgress: number;
  elapsedMs: number;
}

// Add to HudMessage union
export type HudMessage =
  | ... existing types ...
  | MAPTurnStartMessage
  | MAPSubtaskChangeMessage
  | MAPFMActionMessage
  | MAPVerifyMessage
  | MAPHeartbeatMessage;
```

#### 2.2 Add Subscriptions to TestGenGraphComponent

**File:** `src/effuse/components/testgen-graph/testgen-graph-component.ts` (MODIFY)

Add `subscriptions` method following APM widget pattern:

```typescript
import { SocketServiceTag } from "../../services/socket.js";
import { Stream, pipe } from "effect";
import type { HudMessage } from "../../../hud/protocol.js";

// Type guards
const isTestGenMessage = (msg: HudMessage): msg is TestGenHudMessage =>
  msg.type.startsWith("testgen_") || msg.type.startsWith("map_");

// Add to component
subscriptions: (ctx) => {
  const socket = Effect.map(SocketServiceTag, (s) => s);

  return [
    pipe(
      Stream.unwrap(Effect.map(socket, (s) => s.getMessages())),
      Stream.filter(isTestGenMessage),
      Stream.map((msg) => Effect.gen(function* () {
        yield* ctx.state.update((state) => mapMessageToState(state, msg));
      }))
    ),
  ];
},
```

#### 2.3 Create State Mapping Function

**File:** `src/effuse/components/testgen-graph/state-mapper.ts` (NEW)

```typescript
import type { TestGenGraphState, TestGenNode } from "./types.js";
import type { HudMessage } from "../../../hud/protocol.js";

export function mapMessageToState(
  state: TestGenGraphState,
  msg: HudMessage
): TestGenGraphState {
  switch (msg.type) {
    case "testgen_start":
      return updateNode(state, "testgen", { status: "running", label: "TestGen\nStarting..." });

    case "testgen_progress":
      return updateNode(state, "testgen", {
        status: "running",
        label: `TestGen\n${msg.currentCategory}`
      });

    case "testgen_test":
      // Increment test count, update category node
      const categoryNode = state.nodes.find(n => n.id === `cat-${msg.test.category}`);
      if (categoryNode) {
        const newCount = (categoryNode.data?.testCount || 0) + 1;
        return updateNode(state, categoryNode.id, {
          label: `${msg.test.category}\n${newCount} tests`,
          status: "partial",
        });
      }
      return state;

    case "testgen_complete":
      return updateNode(state, "testgen", {
        status: "completed",
        label: `TestGen\n${msg.totalTests} tests`,
      });

    case "map_turn_start":
      return {
        ...state,
        nodes: state.nodes.map(n => {
          if (n.id === "fm") return { ...n, status: "running", label: `FM\nTurn ${msg.turn}/${msg.maxTurns}` };
          if (n.id === `subtask-${msg.subtask}`) return { ...n, status: "running" };
          return n;
        }),
      };

    case "map_fm_action":
      return updateNode(state, "fm", {
        status: "running",
        label: msg.action === "tool_call"
          ? `FM\n${msg.toolName}`
          : `FM\n${msg.action}`,
      });

    case "map_verify":
      if (msg.status === "running") {
        return updateNode(state, "verifier", { status: "running", label: "Verifier\nRunning..." });
      }
      return {
        ...updateNode(state, "verifier", {
          status: msg.passed === msg.total ? "completed" : "partial",
          label: `Verifier\n${msg.passed}/${msg.total}`,
        }),
        ...updateNode(state, "progress", {
          status: msg.progress >= 1 ? "completed" : "partial",
          label: `Progress\n${(msg.progress * 100).toFixed(1)}%`,
        }),
      };

    case "map_heartbeat":
      return updateNode(state, "progress", {
        label: `Progress\n${(msg.progress * 100).toFixed(1)}%\nBest: ${(msg.bestProgress * 100).toFixed(1)}%`,
      });

    default:
      return state;
  }
}

function updateNode(
  state: TestGenGraphState,
  nodeId: string,
  updates: Partial<TestGenNode>
): TestGenGraphState {
  return {
    ...state,
    nodes: state.nodes.map(n =>
      n.id === nodeId ? { ...n, ...updates } : n
    ),
  };
}
```

---

### Phase 3: UI Controls for Starting Runs (1-2 hours)

#### 3.1 Add "Start Run" Request to Protocol

**File:** `src/desktop/protocol.ts` (MODIFY)

```typescript
export interface StartHillClimberRequest {
  type: "request:startHillClimber";
  task: string;  // e.g., "regex-log"
  mode: "quick" | "standard" | "full";
  correlationId: string;
}

export interface StartHillClimberResponse {
  type: "response:startHillClimber";
  correlationId: string;
  success: boolean;
  sessionId?: string;
  error?: string;
}
```

#### 3.2 Add Handler to Desktop Server

**File:** `src/desktop/server.ts` (MODIFY)

```typescript
case "request:startHillClimber": {
  const sessionId = crypto.randomUUID();

  // Spawn HillClimber process
  const proc = Bun.spawn([
    "bun", "scripts/test-progress-fix.ts",
    "--mode", request.mode,
    "--session", sessionId,
  ], {
    cwd: process.cwd(),
    stdout: "inherit",
    stderr: "inherit",
  });

  ws.send(JSON.stringify({
    type: "response:startHillClimber",
    correlationId: request.correlationId,
    success: true,
    sessionId,
  }));
  break;
}
```

#### 3.3 Add Start Button to UI

**File:** `src/effuse/components/testgen-graph/testgen-graph-component.ts` (MODIFY)

Add a "Start Run" button in render:

```typescript
render: (ctx) => Effect.gen(function* () {
  const state = yield* ctx.state.get;

  return html`
    <div class="testgen-controls">
      <button data-action="start-quick">Quick Run</button>
      <button data-action="start-standard">Standard Run</button>
      <button data-action="start-full">Full Run</button>
    </div>
    <svg ...>
      <!-- existing graph -->
    </svg>
  `;
}),

setupEvents: (ctx) => Effect.gen(function* () {
  // ... existing event setup

  const socket = yield* SocketServiceTag;

  yield* ctx.dom.delegate(document.body, "[data-action^='start-']", "click", (_e, target) => {
    const mode = (target as HTMLElement).dataset.action?.replace("start-", "") as "quick" | "standard" | "full";
    Effect.runFork(
      socket.startHillClimber({ task: "regex-log", mode })
        .pipe(Effect.tap((res) => {
          if (res.success) {
            ctx.state.update(s => ({ ...s, sessionId: res.sessionId, running: true }));
          }
        }))
    );
  });
}),
```

---

### Phase 4: Testing & Validation (1 hour)

#### 4.1 Manual Test Procedure

1. Start desktop server: `bun src/desktop/server.ts`
2. Open UI: `http://localhost:8080/new.html`
3. Open browser console to watch for errors
4. Click "Quick Run" button
5. Verify:
   - TestGen node pulses blue, shows progress
   - Category nodes appear as tests generate
   - FM node shows turn counter
   - Verifier shows test results
   - Progress node updates percentage

#### 4.2 Unit Tests

**File:** `src/hillclimber/hud-emitter.test.ts` (NEW)

```typescript
import { describe, test, expect } from "bun:test";
import { createHillClimberHudEmitter } from "./hud-emitter.js";

describe("HillClimberHudEmitter", () => {
  test("emits testgen_start message", () => {
    const messages: any[] = [];
    // Mock HudClient...
  });
});
```

---

## File Summary

### New Files
| File | Purpose | Lines |
|------|---------|-------|
| `src/hillclimber/hud-emitter.ts` | Backend event emitter | ~150 |
| `src/effuse/components/testgen-graph/state-mapper.ts` | Message → State mapping | ~100 |
| `src/hillclimber/hud-emitter.test.ts` | Unit tests | ~50 |

### Modified Files
| File | Changes |
|------|---------|
| `src/hud/protocol.ts` | Add MAP message types (~50 lines) |
| `src/hillclimber/map-orchestrator.ts` | Add hudEmitter option, emit events (~20 lines) |
| `src/hillclimber/testgen-integration.ts` | Wire emitter to TestGen (~15 lines) |
| `scripts/test-progress-fix.ts` | Create emitter, pass to orchestrator (~10 lines) |
| `src/desktop/protocol.ts` | Add startHillClimber request/response (~20 lines) |
| `src/desktop/server.ts` | Handle startHillClimber, spawn process (~30 lines) |
| `src/effuse/components/testgen-graph/testgen-graph-component.ts` | Add subscriptions, controls (~80 lines) |
| `src/effuse/services/socket.ts` | Add startHillClimber method (~10 lines) |

---

## Timeline Estimate

| Phase | Task | Time |
|-------|------|------|
| Phase 1 | Backend event emission | 2-3 hours |
| Phase 2 | Frontend socket subscription | 2-3 hours |
| Phase 3 | UI controls | 1-2 hours |
| Phase 4 | Testing & validation | 1 hour |
| **Total** | | **6-9 hours** |

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Socket not connected when CLI starts | HudClient queues messages, auto-reconnects |
| Too many messages overwhelm UI | Batch state updates, throttle re-renders |
| Process crashes lose state | Persist session to SQLite, allow resume |
| Port 8080 already in use | Make port configurable via env var |

---

## Success Criteria

- [ ] Start HillClimber run from UI button
- [ ] TestGen phase shows live test generation
- [ ] FM node shows current turn/action
- [ ] Verifier node shows pytest results
- [ ] Progress node updates in real-time
- [ ] Graph reflects actual state (not hardcoded)
- [ ] Multiple runs don't interfere with each other

---

## Alternative: Simpler MVP

If full socket integration is too much for initial pass:

**MVP Option**: File-based state sync
1. HillClimber writes state to `logs/live-state.json` every heartbeat
2. UI polls file every 1 second
3. No socket changes needed
4. Lower complexity but higher latency

This could be a stepping stone before full socket integration.

---

## Decision Points

Before implementation, confirm:

1. **Socket vs File polling for MVP?**
   - Socket = real-time, more work
   - File = simpler, 1s latency

2. **Start runs from UI or keep CLI-only?**
   - UI start = Phase 3 required
   - CLI-only = skip Phase 3

3. **Single session or multi-session support?**
   - Single = simpler state
   - Multi = filter by sessionId

4. **Persist graph state across page reload?**
   - Yes = store in localStorage
   - No = reset on reload

---

## Next Steps

1. Get user confirmation on approach
2. Start with Phase 1 (backend emission)
3. Test with console.log before socket
4. Move to Phase 2 (frontend subscription)
5. Add Phase 3 controls if needed
