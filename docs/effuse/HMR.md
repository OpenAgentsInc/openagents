# Effuse Hot Module Replacement (HMR)

Effuse supports hot reloading with state preservation for rapid UI development. When you edit widget code, the browser reloads instantly while preserving widget state (selections, scroll position, form inputs, etc.).

## Quick Start

1. Start the desktop app:
   ```bash
   bun src/desktop/main.ts
   ```

2. Edit any `.ts`, `.css`, or `.html` file in `src/effuse/` or `src/mainview/`

3. The browser reloads within ~200ms with state preserved

## How It Works

```
File change (src/effuse/*.ts or src/mainview/*.ts)
    ↓
server-worker.ts (fs.watch with 100ms debounce)
    ↓
WebSocket broadcast: { type: "dev_reload" }
    ↓
effuse-main.ts receives message
    ↓
location.reload()
    ↓
Server rebuilds effuse-main.js on-demand
    ↓
mountWidget() checks window.__EFFUSE_HMR__ for preserved state
    ↓
Widget renders with previous state intact
```

### State Preservation

Widget state is continuously saved to `window.__EFFUSE_HMR__` via a Stream that taps into `StateCell.changes`:

```typescript
// In mount.ts - runs for every widget
yield* pipe(
  state.changes,
  Stream.tap((s) => Effect.sync(() => saveWidgetState(widget.id, s))),
  Stream.runDrain,
  Effect.forkScoped
)
```

On reload, `mountWidget()` checks for preserved state before using `initialState`:

```typescript
const preservedState = loadWidgetState<S>(widget.id)
const initialState = preservedState ?? widget.initialState()
```

## Architecture

### Files Involved

| File | Role |
|------|------|
| `src/desktop/server-worker.ts` | File watching + WebSocket broadcast |
| `src/mainview/effuse-main.ts` | Reload handler |
| `src/effuse/hmr/registry.ts` | State registry (`window.__EFFUSE_HMR__`) |
| `src/effuse/widget/mount.ts` | State save/restore on mount |
| `src/hud/protocol.ts` | `DevReloadMessage` type |

### State Registry API

```typescript
import {
  saveWidgetState,
  loadWidgetState,
  hasWidgetState,
  clearAllState,
  getHMRVersion,
  bumpHMRVersion
} from "../effuse/index.js"

// Save state (called automatically via Stream)
saveWidgetState("my-widget", { count: 5 })

// Load and consume state (one-time restore)
const state = loadWidgetState<MyState>("my-widget")

// Check if state exists (doesn't consume)
if (hasWidgetState("my-widget")) { ... }

// Clear all preserved state
clearAllState()

// Get/bump version for debugging
console.log("HMR version:", getHMRVersion())
```

## State Compatibility

For state to be preserved across reloads, it must be JSON-serializable:

| Type | Preserved | Notes |
|------|-----------|-------|
| Primitives | Yes | strings, numbers, booleans, null |
| Plain objects | Yes | `{ foo: "bar" }` |
| Arrays | Yes | `[1, 2, 3]` |
| Functions | No | Recreated from widget definition |
| DOM references | No | Re-queried after mount |
| Effect Fibers/Queues | No | Recreated on mount |

Current widgets use only serializable state, so no changes are needed.

## Watched Directories

The server watches these directories recursively:

- `src/mainview/` - Main view entry point and socket client
- `src/effuse/` - Widget framework and all widgets

File types that trigger reload: `.ts`, `.css`, `.html`

## Debouncing

File changes are debounced by 100ms to prevent multiple rapid reloads when editors save + format + lint in quick succession.

## Test Environment

The HMR registry is browser-only. In test environments (where `window` is undefined), all registry functions are no-ops:

- `saveWidgetState()` - silently skips
- `loadWidgetState()` - returns `undefined`
- `hasWidgetState()` - returns `false`

This ensures tests run without errors while widgets still use fresh `initialState()`.

## Debugging

Check the console for HMR messages:

```
[Effuse] HMR handler registered
[Effuse] HMR: Reload triggered by /path/to/changed/file.ts
[Effuse HMR] Restored state for "hf-trajectory-list"
```

Server-side logs:

```
[Worker] HMR: Watching /path/to/src/mainview for changes
[Worker] HMR: Watching /path/to/src/effuse for changes
[Worker] File changed: /path/to/widget.ts, sending reload signal
```

## Limitations

1. **Full page reload** - Not true HMR (module-level replacement). The entire page reloads, but state is preserved.

2. **State shape changes** - If you change the widget's state interface, preserved state may not match. Clear state with `clearAllState()` or reload twice.

3. **Non-serializable state** - Functions, DOM refs, and Effect primitives cannot be preserved.

4. **WebSocket dependency** - HMR requires the WebSocket connection to be established. If disconnected, changes won't trigger reload.
