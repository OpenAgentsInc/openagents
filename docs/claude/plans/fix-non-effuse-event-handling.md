# Fix TestGenGraphComponent Event Handling

**Problem:** Button clicks don't work in TestGenGraphComponent
**Root Cause:** Component uses raw `addEventListener()` instead of Effuse's `ctx.dom.delegate()` pattern
**Solution:** Refactor to use the proper Effuse event delegation pattern

---

## Analysis Summary

### The Architecture Is Sound
Effuse's event system is well-designed and works correctly in other components:
- `ctx.dom.delegate()` attaches listeners to **container** (survives re-renders)
- Uses event bubbling + `closest()` to find targets
- Other components (tb-controls.ts, apm-widget.ts, tbcc-shell.ts) all work correctly

### The Bug
TestGenGraphComponent's `setupEvents()` uses raw JavaScript:
```typescript
// WRONG - bypasses Effuse's event system
const handleClick = (e: Event) => { ... }
container.addEventListener("click", handleClick)
```

Instead of the proper Effuse pattern:
```typescript
// CORRECT - uses Effuse's delegate system
yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (e, target) => {
  // handler
})
```

---

## Implementation Plan

### Step 1: Refactor setupEvents() to use ctx.dom.delegate()

**File:** `src/effuse/components/testgen-graph/testgen-graph-component.ts`

Replace the raw addEventListener approach with Effuse's delegate pattern:

```typescript
setupEvents: (ctx) =>
  Effect.gen(function* () {
    const container = ctx.container
    const getSvg = () => container.querySelector<SVGSVGElement>("#testgen-graph-svg")

    // Animation loop (keep as-is)
    let _animationId: number
    const animate = () => { ... }
    _animationId = requestAnimationFrame(animate)

    // Canvas interaction state
    let isDraggingNode = false
    let isPanningCanvas = false
    // ... other state vars

    // Mouse handlers for drag/pan (keep on container - these work)
    container.addEventListener("mousedown", handleMouseDown as EventListener)
    document.addEventListener("mousemove", handleMouseMove as EventListener)
    document.addEventListener("mouseup", handleMouseUp as EventListener)
    container.addEventListener("wheel", handleWheel as EventListener, { passive: false })

    // START BUTTON CLICKS - Use Effuse delegate pattern
    yield* ctx.dom.delegate(ctx.container, "[data-action^='start-']", "click", (e, target) => {
      e.stopPropagation()
      const action = (target as HTMLElement).dataset.action
      if (action?.startsWith("start-")) {
        const mode = action.replace("start-", "") as "quick" | "standard" | "full"
        console.log("[TestGen Graph] Start button clicked:", mode)
        Effect.runFork(ctx.emit({ type: "startRun", mode }))
      }
    })

    // SESSION SELECTION - Use Effuse delegate pattern
    yield* ctx.dom.delegate(ctx.container, "[data-action='select-session']", "click", (e, target) => {
      e.stopPropagation()
      const sessionId = (target as HTMLElement).dataset.sessionId
      if (sessionId) {
        console.log("[TestGen Graph] Session selected:", sessionId)
        Effect.runFork(ctx.emit({ type: "selectSession", sessionId }))
      }
    })

    // Cleanup
    return Effect.sync(() => {
      cancelAnimationFrame(_animationId)
      container.removeEventListener("mousedown", handleMouseDown)
      document.removeEventListener("mousemove", handleMouseMove)
      document.removeEventListener("mouseup", handleMouseUp)
      container.removeEventListener("wheel", handleWheel)
      // Note: delegate() cleanup is handled automatically by Effect scope
    })
  })
```

### Step 2: Verify data attributes in render()

Ensure the buttons have proper `data-action` attributes:

```typescript
<button
  data-action="start-quick"
  style="..."
>
  Quick (3 turns)
</button>
```

And session cards have:
```typescript
<div
  data-action="select-session"
  data-session-id="${session.sessionId}"
  style="..."
>
```

### Step 3: Clean up handleEvent

The handleEvent function should already be correct - it handles `startRun` and `selectSession` events.

---

## Why This Will Work

1. **ctx.dom.delegate() attaches to container** - Container element never changes during re-renders
2. **Uses event bubbling** - Clicks on buttons bubble up to container where delegate handler catches them
3. **closest() finds targets** - Even after innerHTML replacement, new buttons with `data-action` are found
4. **Other components prove it works** - tb-controls.ts, tbcc-shell.ts use identical pattern successfully

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/effuse/components/testgen-graph/testgen-graph-component.ts` | Refactor setupEvents() to use ctx.dom.delegate() |

---

## Testing

1. Start desktop server: `bun new`
2. Navigate to TestGen graph view
3. Click "Quick (3 turns)" button
4. Verify console shows: `[TestGen Graph] Start button clicked: quick`
5. Verify HillClimber run starts (or at least the request is made)

---

## Not Required

- No framework-level changes needed
- No HMR changes needed
- No virtual DOM or reconciliation needed
- Effuse's architecture is correct; just need to use it properly
