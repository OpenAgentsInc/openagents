# Add Streaming Output Panel to TestGen Graph

**Goal:** Show real-time HillClimber/MAP orchestrator output in the TestGen graph view using ATIF-style thread components.

**Current State:**
- HillClimber emits HUD messages (map_turn_start, map_fm_action, map_verify, etc.)
- TestGenGraphComponent receives these but only updates node state
- User can't see what's happening (FM requests, tool calls, etc.)

**Solution:** Add a collapsible output panel that streams ThreadItem entries as messages arrive.

---

## Architecture

### Data Flow
```
MAP Orchestrator → HUD Emitter → WebSocket → SocketService.getMessages()
                                                    ↓
                              TestGenGraphComponent subscriptions
                                                    ↓
                              Convert to ThreadItem → Append to state.logItems
                                                    ↓
                              Render log panel with renderThreadContainer()
```

### Reusable Components
From `src/effuse/components/atif-thread.ts`:
- `ThreadItem` type union (progress, reflection, test, complete, error)
- `renderThreadContainer()` - renders all items chronologically
- `renderThreadItem()` - renders individual items with styling
- Category badges, confidence bars, etc.

---

## Implementation Plan

### Step 1: Extend State with Log Items

**File:** `src/effuse/components/testgen-graph/types.ts`

Add to `TestGenGraphState`:
```typescript
interface TestGenGraphState {
  // ... existing fields ...

  // Log/output panel
  logItems: LogItem[]
  logPanelCollapsed: boolean
}

// Log item types for HillClimber output
type LogItem =
  | { type: "turn"; timestamp: number; data: { turn: number; maxTurns: number; subtask: string } }
  | { type: "fm_action"; timestamp: number; data: { action: "thinking" | "tool_call" | "complete"; tool?: string; args?: string } }
  | { type: "verify"; timestamp: number; data: { status: "running" | "passed" | "failed"; passed?: number; total?: number } }
  | { type: "progress"; timestamp: number; data: { phase: string; message: string } }
  | { type: "complete"; timestamp: number; data: { passed: boolean; progress: number; duration: number } }
  | { type: "error"; timestamp: number; data: { message: string } }
```

### Step 2: Create Log Panel Renderer

**File:** `src/effuse/components/testgen-graph/log-panel.ts` (NEW)

```typescript
import { html, joinTemplates } from "../../template/html.js"
import type { TemplateResult } from "../../template/types.js"
import type { LogItem } from "./types.js"

export function renderLogPanel(
  items: LogItem[],
  collapsed: boolean
): TemplateResult {
  if (collapsed) {
    return html`
      <div data-action="toggle-log" style="...collapsed styles...">
        ▶ Show Output (${items.length} items)
      </div>
    `
  }

  return html`
    <div style="position: absolute; bottom: 10px; right: 10px; width: 400px; max-height: 50%; ...">
      <div data-action="toggle-log" style="...header...">
        ▼ Output (${items.length} items)
      </div>
      <div style="overflow-y: auto; max-height: calc(100% - 30px);">
        ${joinTemplates(items.slice(-50).map(renderLogItem))}
      </div>
    </div>
  `
}

function renderLogItem(item: LogItem): TemplateResult {
  const time = formatTime(item.timestamp)
  switch (item.type) {
    case "turn":
      return html`<div class="log-turn">⚙️ ${time} Turn ${item.data.turn}/${item.data.maxTurns}: ${item.data.subtask}</div>`
    case "fm_action":
      return html`<div class="log-fm">🤖 ${time} ${item.data.action}${item.data.tool ? `: ${item.data.tool}` : ""}</div>`
    case "verify":
      return html`<div class="log-verify">✓ ${time} Verify: ${item.data.status} ${item.data.passed ?? 0}/${item.data.total ?? 0}</div>`
    // ... other types
  }
}
```

### Step 3: Update Subscription to Populate Log Items

**File:** `src/effuse/components/testgen-graph/state-mapper.ts`

Add function to convert HUD messages to LogItems:

```typescript
export function hudMessageToLogItem(msg: HudMessage): LogItem | null {
  switch (msg.type) {
    case "map_turn_start":
      return {
        type: "turn",
        timestamp: Date.now(),
        data: { turn: msg.turn, maxTurns: msg.maxTurns, subtask: msg.subtaskId }
      }
    case "map_fm_action":
      return {
        type: "fm_action",
        timestamp: Date.now(),
        data: { action: msg.action, tool: msg.toolName, args: msg.args }
      }
    case "map_verify":
      return {
        type: "verify",
        timestamp: Date.now(),
        data: { status: msg.status, passed: msg.passed, total: msg.total }
      }
    // ... map other message types
    default:
      return null
  }
}
```

Update `mapMessageToState()` to also append log items:
```typescript
export function mapMessageToState(state: TestGenGraphState, msg: HudMessage): TestGenGraphState {
  // ... existing session/node updates ...

  // Also append to log
  const logItem = hudMessageToLogItem(msg)
  const logItems = logItem
    ? [...state.logItems.slice(-200), logItem]  // Keep last 200 items
    : state.logItems

  return { ...state, /* existing updates */, logItems }
}
```

### Step 4: Add Log Panel to Render

**File:** `src/effuse/components/testgen-graph/testgen-graph-component.ts`

In render function, add after the graph SVG:
```typescript
<!-- Log/Output Panel -->
${renderLogPanel(state.logItems, state.logPanelCollapsed)}
```

### Step 5: Add Toggle Event

Add to events type:
```typescript
| { type: "toggleLogPanel" }
```

Add mousedown handler for toggle:
```typescript
const toggleLog = target.closest("[data-action='toggle-log']")
if (toggleLog) {
  Effect.runFork(ctx.emit({ type: "toggleLogPanel" }))
  return
}
```

Add to handleEvent:
```typescript
case "toggleLogPanel":
  yield* ctx.state.update(s => ({ ...s, logPanelCollapsed: !s.logPanelCollapsed }))
  break
```

---

## Files to Modify/Create

| File | Action | Changes |
|------|--------|---------|
| `src/effuse/components/testgen-graph/types.ts` | Modify | Add `LogItem` type, `logItems`, `logPanelCollapsed` to state |
| `src/effuse/components/testgen-graph/log-panel.ts` | Create | Log panel renderer |
| `src/effuse/components/testgen-graph/state-mapper.ts` | Modify | Add `hudMessageToLogItem()`, update `mapMessageToState()` |
| `src/effuse/components/testgen-graph/testgen-graph-component.ts` | Modify | Import log panel, add to render, add toggle event |

---

## UI Layout

```
┌────────────────────────────────────────────────────────────┐
│ Sessions (0)     │                   STARTING... │ Quick │ │
│ No sessions yet  │                               │ Std   │ │
│                  │                               │ Full  │ │
├──────────────────┤                                        │
│                  │      ┌──────────────────────┐          │
│                  │      │     SVG GRAPH        │          │
│                  │      │                      │          │
│                  │      └──────────────────────┘          │
│                  │                                        │
│                  │            ┌────────────────────────┐  │
│                  │            │ ▼ Output (12 items)    │  │
│                  │            │ ⚙️ 14:23 Turn 1/3: ... │  │
│                  │            │ 🤖 14:23 thinking      │  │
│                  │            │ 🤖 14:24 tool: write   │  │
│                  │            │ ✓ 14:24 Verify: 5/10   │  │
│                  │            └────────────────────────┘  │
└────────────────────────────────────────────────────────────┘
```

---

## Testing

1. Start `bun new`
2. Click "Quick (3 turns)" button
3. Verify log panel appears and shows:
   - Turn starts
   - FM actions (thinking, tool calls)
   - Verification results
   - Completion status
4. Click panel header to collapse/expand
5. Verify auto-scroll to latest items
