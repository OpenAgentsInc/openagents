# 1423 Webview Click Event Bug - Root Cause Analysis & Fix

**Time:** 14:23 CT
**Date:** 2025-12-09
**Issue:** Button clicks not working in TestGenGraphComponent

---

## Summary

Discovered a critical bug in `webview-bun`: the `click` event does NOT fire for any elements. Only `mousedown` and `mouseup` events work. This broke all button click handlers that used the standard `click` event or Effuse's `ctx.dom.delegate()` pattern (which uses `click`).

---

## Root Cause

**The webview-bun runtime does not generate `click` events.**

Standard browser behavior:
1. `mousedown` fires when button pressed
2. `mouseup` fires when button released
3. `click` fires after mouseup (synthetic event)

Webview-bun behavior:
1. `mousedown` fires when button pressed
2. `mouseup` fires when button released
3. `click` NEVER fires

---

## Debugging Timeline

1. **Initial symptom**: Button clicks did nothing, no console output
2. **First hypothesis**: Event delegation pattern wrong → refactored to use `ctx.dom.delegate()` (didn't help)
3. **Second hypothesis**: z-index/CSS issue → verified buttons were on top (wasn't the issue)
4. **Key discovery**: Node clicks worked but used `mousedown`/`mouseup`, not `click`
5. **Verification test**: Added `document.addEventListener("click", ...)` → never fired
6. **Verification test**: Added `document.addEventListener("mousedown", ...)` → fired correctly for buttons
7. **Root cause confirmed**: `click` events don't fire in webview-bun

---

## The Fix

Instead of using `click` events, use `mousedown` for button detection:

```typescript
// WEBVIEW BUG: 'click' events don't fire in webview-bun, but mousedown/mouseup do.
// Use mousedown for button detection instead of click.
ctx.container.addEventListener("mousedown", (e) => {
  const target = e.target as HTMLElement

  // Check for start buttons
  const startButton = target.closest("[data-action^='start-']") as HTMLElement | null
  if (startButton && ctx.container.contains(startButton)) {
    e.stopPropagation()
    const action = startButton.dataset.action
    if (action?.startsWith("start-")) {
      const mode = action.replace("start-", "") as "quick" | "standard" | "full"
      Effect.runFork(ctx.emit({ type: "startRun", mode }))
    }
    return
  }
})
```

---

## Impact on Effuse

The standard Effuse pattern `ctx.dom.delegate(container, selector, "click", handler)` will NOT work in webview-bun because it relies on the `click` event.

**Workarounds:**

1. **Use mousedown directly** (as done in this fix) - triggers immediately on press
2. **Use mouseup** - triggers on release, more click-like behavior
3. **Implement click simulation** - track mousedown target, verify same target on mouseup

---

## Files Modified

| File | Changes |
|------|---------|
| `src/effuse/components/testgen-graph/testgen-graph-component.ts` | Replaced `ctx.dom.delegate(..., "click", ...)` with `mousedown` listener |

---

## Recommendations

1. **Document this limitation** in Effuse README and AGENTS.md
2. **Consider adding mousedown/mouseup delegate variants** to DomService for webview compatibility
3. **Test all Effuse components** that use click handlers in webview context
4. **File issue with webview-bun** if this is unintended behavior

---

## Affected Components (Potentially)

Any component using `ctx.dom.delegate(..., "click", ...)`:
- `tb-controls.ts`
- `tbcc-shell.ts`
- `tbcc-dashboard.ts`
- `apm-widget.ts`
- Others

These may need similar fixes if used in webview context.

---

**Status:** Fixed - Button clicks now work using `mousedown` event
