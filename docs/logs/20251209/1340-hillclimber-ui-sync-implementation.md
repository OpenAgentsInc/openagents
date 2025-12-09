# 1340 HillClimber UI Sync Implementation Log

**Time:** 13:40 CT
**Date:** 2025-12-09
**Task:** Connect HillClimber/TestGen CLI execution to Effuse UI with real-time updates

---

## Summary

Implemented full HillClimber/TestGen CLI ↔ Effuse UI synchronization, enabling real-time visualization of HillClimber execution in the TestGenGraphComponent. This builds upon the static component created earlier (see `1302-testgen-graph-component-implementation.md`).

**Key Features Implemented:**
- Real-time socket-based updates from CLI to UI
- Start runs from UI or CLI (both emit same events)
- Multi-session tracking with session sidebar
- Live graph node updates based on execution state

---

## Implementation Phases

### Phase 1: Backend Event Emission

**Objective:** Enable HillClimber/TestGen processes to emit HUD events

**Files Modified:**
- `src/hillclimber/map-orchestrator.ts` - Added `hudEmitter` option
- `src/hillclimber/testgen-integration.ts` - Added `hudEmitter` option
- `scripts/test-progress-fix.ts` - Creates emitter with session ID

**Key Changes:**
```typescript
// MAPOrchestratorOptions now includes:
export interface MAPOrchestratorOptions {
  hudEmitter?: import("./hud-emitter.js").HillClimberHudEmitter;
}

// CLI script creates emitter:
const sessionId = `hc-${Date.now()}-${randomUUID().slice(0, 8)}`;
const hudEmitter = createHillClimberHudEmitter(sessionId);
```

### Phase 2: HUD Protocol Messages

**Objective:** Add MAP orchestrator message types to protocol

**Files Modified:**
- `src/hud/protocol.ts` - Added ~60 lines

**Message Types Added:**
| Type | Purpose |
|------|---------|
| `map_turn_start` | Turn beginning |
| `map_fm_action` | FM action (thinking/tool_call/complete) |
| `map_verify` | Verification status |
| `map_subtask_change` | Subtask transitions |
| `map_heartbeat` | Periodic status update |
| `map_run_complete` | Run finished |

### Phase 3: UI Multi-Session State

**Objective:** Enable tracking multiple concurrent sessions in component

**Files Created:**
- `src/effuse/components/testgen-graph/state-mapper.ts` (NEW ~320 lines)

**Files Modified:**
- `src/effuse/components/testgen-graph/types.ts` - Added SessionRunState
- `src/effuse/components/testgen-graph/testgen-graph-component.ts` - Added subscriptions

**State Structure:**
```typescript
interface SessionRunState {
  sessionId: string;
  status: "waiting" | "testgen" | "running" | "completed" | "failed";
  testGenProgress: { category: string; count: number }[];
  totalTests: number;
  currentTurn: number;
  maxTurns: number;
  currentSubtask: string;
  fmAction: string;
  testsPassed: number;
  testsTotal: number;
  progress: number;
  bestProgress: number;
  startedAt: number;
  lastUpdateAt: number;
}

interface TestGenGraphState {
  sessions: Map<string, SessionRunState>;
  activeSessionId: string | null;
  nodes: TestGenNode[];
  connections: TestGenConnection[];
  // ... existing fields
}
```

### Phase 4: StartHillClimber Protocol & Handler

**Objective:** Enable UI to start HillClimber runs

**Files Created/Modified:**
- `src/desktop/protocol.ts` - Added StartHillClimberRequest/Response (~40 lines)
- `src/desktop/handlers.ts` - Added startHillClimber handler (~100 lines)
- `src/mainview/socket-client.ts` - Added startHillClimber method (~15 lines)
- `src/effuse/services/socket.ts` - Added interface method (~15 lines)
- `src/effuse/services/socket-live.ts` - Added implementation (~5 lines)

**Protocol Types:**
```typescript
interface StartHillClimberRequest extends BaseRequest {
  type: "request:startHillClimber";
  task: string;           // e.g., "regex-log"
  mode: "quick" | "standard" | "full";
  suitePath?: string;
}

interface StartHillClimberResponse extends BaseResponse {
  type: "response:startHillClimber";
  data?: { sessionId: string };
}
```

**Test File Fixes:**
- `hf-trajectory-browser.e2e.test.ts` - Added mock startHillClimber
- `tbcc-testgen.e2e.test.ts` - Added mock startHillClimber
- `tbcc.e2e.test.ts` - Added mock startHillClimber

### Phase 5: UI Controls & Session Sidebar

**Objective:** Add UI for starting runs and switching sessions

**Files Modified:**
- `src/effuse/components/testgen-graph/testgen-graph-component.ts` (~150 lines)

**Features Added:**

1. **Control Panel** (top-right):
   - Status display: `running | Turn 5/10 | Progress: 67.5%`
   - Quick button (green): 3 turns, 5 min timeout
   - Standard button (yellow): 10 turns, 15 min timeout
   - Full button (orange): 25 turns, 45 min timeout

2. **Session Sidebar** (left):
   - Lists all tracked sessions (sorted by last update)
   - Shows: session ID (truncated), status (color-coded), progress %, turn info
   - Click to switch active session

3. **Event Handlers:**
   - Start button clicks → `startRun` events
   - Session card clicks → `selectSession` events
   - handleEvent calls socket.startHillClimber for startRun

**Component Type Update:**
```typescript
// Added SocketServiceTag as third type parameter
export const TestGenGraphComponent: Component<
  TestGenGraphState,
  TestGenGraphEvent,
  SocketServiceTag
> = { ... }
```

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                  Desktop Server (port 8080)                  │
│                  Broadcasts HudMessage to all UI clients     │
└──────────────────────────────┬──────────────────────────────┘
                               │
      ┌────────────────────────┼────────────────────────┐
      │                        │                        │
      ▼                        ▼                        ▼
┌──────────────┐      ┌──────────────┐      ┌──────────────────┐
│   CLI Run    │      │   UI Start   │      │  TestGenGraph    │
│              │      │              │      │  Component       │
│ bun scripts/ │      │ Click button │      │                  │
│ test-prog... │      │ → spawn proc │      │ subscriptions()  │
│              │      │              │      │ filters by       │
│ HudClient    │      │ HudClient    │      │ sessionId        │
│ .send()      │      │ .send()      │      │                  │
└──────┬───────┘      └──────┬───────┘      │ Multi-session:   │
       │                     │              │ Map<sessionId,   │
       └─────────────────────┘              │   RunState>      │
                 │                          └────────┬─────────┘
                 │                                   │
                 └───────────────────────────────────┘
                           HudMessage stream
```

---

## Data Flow

### Starting a Run from UI
```
1. User clicks "Quick (3 turns)" button
2. Component emits { type: "startRun", mode: "quick" }
3. handleEvent calls socket.startHillClimber("regex-log", "quick")
4. SocketClient sends request:startHillClimber to server
5. handlers.ts spawns process with HC_SESSION_ID env var
6. Response with sessionId returned to UI
7. Component sets activeSessionId = sessionId
```

### Receiving Updates
```
1. HillClimber process calls hudEmitter.onHeartbeat(...)
2. HudClient sends map_heartbeat message to Desktop Server
3. Server broadcasts message to all WebSocket clients
4. SocketClient receives message, emits to handlers
5. Component's subscription filters by isHillClimberMessage
6. mapMessageToState updates SessionRunState
7. updateNodesFromSession updates graph node statuses
8. Component re-renders with new state
```

---

## TypeScript Notes

**Pre-existing Errors (not fixed):**
- addEventListener type issues in testgen-graph-component.ts (lines 368, 371, 405, 408)
- Same issue exists in agent-graph-component.ts
- Caused by `container` being typed as `Element` instead of `HTMLElement`

**New Errors Fixed:**
- Missing `startHillClimber` in mock SocketService objects in 3 test files
- Component type needing `SocketServiceTag` as service requirement (R parameter)

---

## Files Changed Summary

| File | Type | Changes |
|------|------|---------|
| `src/hud/protocol.ts` | Modified | +60 lines (MAP messages) |
| `src/hillclimber/map-orchestrator.ts` | Modified | +30 lines (hudEmitter option) |
| `src/hillclimber/testgen-integration.ts` | Modified | +20 lines (hudEmitter option) |
| `scripts/test-progress-fix.ts` | Modified | +15 lines (emitter creation) |
| `src/desktop/protocol.ts` | Modified | +40 lines (StartHillClimber types) |
| `src/desktop/handlers.ts` | Modified | +100 lines (startHillClimber handler) |
| `src/mainview/socket-client.ts` | Modified | +15 lines (startHillClimber method) |
| `src/effuse/services/socket.ts` | Modified | +15 lines (interface method) |
| `src/effuse/services/socket-live.ts` | Modified | +5 lines (implementation) |
| `src/effuse/components/testgen-graph/types.ts` | Modified | +50 lines (SessionRunState) |
| `src/effuse/components/testgen-graph/state-mapper.ts` | NEW | ~320 lines |
| `src/effuse/components/testgen-graph/testgen-graph-component.ts` | Modified | +150 lines |
| Test files (3) | Modified | +10 lines total |

---

## Documentation Created

- `docs/hillclimber/UI-SYNC-ARCHITECTURE.md` - Comprehensive architecture doc (~400 lines)

---

## Testing Recommendations

1. **CLI Test:**
   ```bash
   bun scripts/test-progress-fix.ts --quick
   ```
   Watch console for HUD events being emitted

2. **UI Test:**
   - Start desktop server: `bun src/desktop/server.ts`
   - Open mainview in browser
   - Navigate to TestGenGraph view
   - Click "Quick (3 turns)" button
   - Observe session appearing in sidebar
   - Watch graph nodes update as run progresses

3. **Multi-Session Test:**
   - Start multiple runs (click buttons multiple times)
   - Verify each appears in sidebar
   - Click different sessions to switch active view
   - Verify graph updates to show selected session's state

---

## Future Enhancements

- Task selector dropdown (currently hardcoded to "regex-log")
- Stop/cancel button for active runs
- Session persistence (survive page reload)
- Detailed session history view
- Export session logs
- Graph layout auto-adjustment based on active subtasks

---

**Status:** ✅ Complete - Full CLI ↔ UI sync implemented
