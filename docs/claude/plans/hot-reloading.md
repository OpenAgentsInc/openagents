# Effuse Hot Reload Implementation Plan

## Goal
Add Effect-native hot reloading to Effuse so changes reload instantly when files change, **preserving widget state** across reloads.

## Approach: WebSocket Reload + State Preservation

File change triggers instant reload while preserving scroll position, form inputs, and widget state via a global state registry.

---

## Step 1: WebSocket Reload Infrastructure

**Signal Flow:**
```
File change in src/mainview/ or src/effuse/
    ↓
server-worker.ts detects via fs.watch()
    ↓
Server broadcasts { type: "dev_reload" } via WebSocket
    ↓
Client receives in effuse-main.ts → location.reload()
    ↓
Browser re-requests effuse-main.js (server rebuilds on-demand)
```

### Files to Modify

#### 1. `src/hud/protocol.ts`
Add reload message type:
```typescript
export interface DevReloadMessage {
  type: "dev_reload"
  changedFile?: string
}
```
Update `HudMessage` union to include `DevReloadMessage`.

#### 2. `src/desktop/server-worker.ts`
Add file watching with debounce:
```typescript
import { watch } from "node:fs"

let reloadTimeout: ReturnType<typeof setTimeout> | null = null

const triggerReload = (changedFile: string) => {
  if (reloadTimeout) clearTimeout(reloadTimeout)
  reloadTimeout = setTimeout(() => {
    log("Worker", `File changed: ${changedFile}, sending reload signal`)
    server.sendHudMessage({ type: "dev_reload", changedFile })
    reloadTimeout = null
  }, 100) // 100ms debounce
}

// Watch src/mainview/ and src/effuse/ directories (recursive)
const watchDirs = [staticDir, join(projectRoot, "effuse")]
for (const dir of watchDirs) {
  watch(dir, { recursive: true }, (eventType, filename) => {
    if (filename?.match(/\.(ts|css|html)$/)) {
      triggerReload(join(dir, filename))
    }
  })
}
```

#### 3. `src/mainview/effuse-main.ts`
Add reload handler early in `initEffuse()`:
```typescript
const socketClient = getSocketClient()
socketClient.onMessage((message) => {
  if (message.type === "dev_reload") {
    console.log("[Effuse] Dev reload:", (message as any).changedFile)
    location.reload()
  }
})
```

---

## Step 2: State Preservation Registry

### New File: `src/effuse/hmr/registry.ts`
```typescript
declare global {
  interface Window {
    __EFFUSE_HMR__?: { widgets: Map<string, unknown> }
  }
}

export const saveWidgetState = (widgetId: string, state: unknown): void => {
  window.__EFFUSE_HMR__ ??= { widgets: new Map() }
  window.__EFFUSE_HMR__.widgets.set(widgetId, structuredClone(state))
}

export const loadWidgetState = <S>(widgetId: string): S | undefined => {
  const state = window.__EFFUSE_HMR__?.widgets.get(widgetId) as S | undefined
  window.__EFFUSE_HMR__?.widgets.delete(widgetId)
  return state
}
```

### Step 3: Modify `src/effuse/widget/mount.ts`
```typescript
import { loadWidgetState, saveWidgetState } from "../hmr/registry.js"

// At mount time, check for preserved state
const preservedState = loadWidgetState<S>(widget.id)
const initialState = preservedState ?? widget.initialState()

// Add continuous state snapshot via stream (after creating StateCell)
yield* pipe(
  state.changes,
  Stream.tap((s) => Effect.sync(() => saveWidgetState(widget.id, s))),
  Stream.runDrain,
  Effect.forkScoped
)
```

---

## Architecture Summary

```
File change (src/effuse/*.ts or src/mainview/*.ts)
    ↓
server-worker.ts (fs.watch with 100ms debounce)
    ↓
WebSocket broadcast: { type: "dev_reload" }
    ↓
effuse-main.ts receives message
    ↓
StateCell.changes snapshots state to window.__EFFUSE_HMR__
    ↓
location.reload()
    ↓
Server rebuilds effuse-main.js on-demand (existing behavior)
    ↓
mountWidget() checks registry, restores preserved state
    ↓
Widget renders with previous state intact
```

---

## Files to Create/Modify

| File | Change |
|------|--------|
| `src/hud/protocol.ts` | Add `DevReloadMessage` to union |
| `src/desktop/server-worker.ts` | Add `fs.watch()` + broadcast |
| `src/mainview/effuse-main.ts` | Add reload handler |
| `src/effuse/hmr/registry.ts` | **NEW** - State registry |
| `src/effuse/widget/mount.ts` | Load/save state on mount |
| `src/effuse/index.ts` | Export hmr module |

---

## Testing

1. Start desktop app: `bun src/desktop/main.ts`
2. Edit any `.ts` file in `src/effuse/` or `src/mainview/`
3. Observe browser reload within ~200ms
4. Verify widget state is preserved (scroll position, selection, etc.)
5. Check console for `[Effuse] Dev reload: <file>` message

---

## State Compatibility Notes

Widget state must be JSON-serializable for preservation:
- Primitives, plain objects, arrays: preserved
- Functions, DOM refs, Effect Fibers: recreated fresh

Current widgets (HFTrajectoryList, HFTrajectoryDetail) use only serializable state - no changes needed.
