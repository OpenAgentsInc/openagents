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
Bun.build() rebuilds effuse-main.js (~50-100ms)
    ↓
WebSocket broadcast: { type: "dev_reload" }
    ↓
effuse-main.ts receives message
    ↓
location.reload()
    ↓
Browser loads freshly rebuilt effuse-main.js
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

## State Migration

When you change a widget's state interface (e.g., rename fields, change structure), preserved HMR state may not match the new shape. Effuse provides a migration system to handle this automatically.

### Automatic Migration

The mount system (`src/effuse/widget/mount.ts`) includes migration logic for widgets that change their state structure. Currently, this is implemented for the `tbcc-testgen` widget as an example.

**Example: tbcc-testgen Migration**

When the TestGen widget was refactored from separate `tests` and `reflections` arrays to a unified `threadItems` array, migration logic was added to convert old state:

```typescript
// In src/effuse/widget/mount.ts
if (widget.id === "tbcc-testgen" && preservedState) {
  const oldState = preservedState as any
  // Check if it has old format (tests/reflections arrays) but not threadItems
  if (!oldState.threadItems && 
      ((oldState.tests && Array.isArray(oldState.tests)) || 
       (oldState.reflections && Array.isArray(oldState.reflections)))) {
    // Convert old arrays to new threadItems format
    const threadItems = []
    // ... migration logic ...
    preservedState = { ...oldState, threadItems }
  }
}
```

### Adding Migration for Your Widget

To add migration logic for your widget:

1. **Identify the state change** - What fields changed? What's the old format vs new format?

2. **Add migration in mount.ts** - Add a check in `mountWidget()` function:
   ```typescript
   if (widget.id === "your-widget-id" && preservedState) {
     const oldState = preservedState as any
     if (/* old format detected */) {
       // Convert to new format
       preservedState = migrateState(oldState)
     }
   }
   ```

3. **Test migration** - Load widget with old state, make change, verify migration works

4. **Remove migration later** - After a few releases, you can remove migration logic if all users have upgraded

### Migration Best Practices

1. **Detect old format explicitly** - Check for absence of new fields AND presence of old fields
2. **Preserve all other state** - Use spread operator to keep unchanged fields
3. **Add approximate data** - If new format needs data not in old format, use reasonable defaults
4. **Log migration** - Use `console.log()` to help debug migration issues
5. **Handle edge cases** - What if old state is partially corrupted? What if migration fails?

### When to Use Migration vs Clear State

**Use Migration When:**
- State contains user work that's hard to recreate (form inputs, selections, generated content)
- The change is a structural refactor (rename, reorganize) not a semantic change
- You want seamless upgrade experience

**Clear State When:**
- The change is semantic (old state doesn't make sense with new code)
- Migration would be too complex or error-prone
- The widget is in early development (users expect resets)

**Clear state manually:**
```typescript
// In browser console
clearAllState()
location.reload()
```

## Limitations

1. **Full page reload** - Not true HMR (module-level replacement). The entire page reloads, but state is preserved.

2. **State shape changes** - If you change the widget's state interface, preserved state may not match. Use migration logic (see above) or clear state with `clearAllState()`.

3. **Non-serializable state** - Functions, DOM refs, and Effect primitives cannot be preserved.

4. **WebSocket dependency** - HMR requires the WebSocket connection to be established. If disconnected, changes won't trigger reload.

5. **Migration complexity** - Complex state migrations may be error-prone. Consider clearing state for major breaking changes.
