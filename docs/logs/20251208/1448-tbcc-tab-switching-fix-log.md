# 1448 TBCC Tab Switching Fix - Complete Session Log

**Date:** 2025-12-08  
**Time:** 14:48 Central  
**Session:** Fixed black screen issue when clicking tabs in TB Command Center

---

## Problem Statement

User reported that clicking sidebar buttons in TB Command Center showed a black screen. The issue had two parts:
1. **Double-click required** - First click on sidebar buttons didn't register, needed to click twice
2. **Black screen** - After clicking tabs, the main content area showed nothing, even though widgets were rendering (logs showed render output)

---

## Root Cause Analysis

### Initial Investigation

1. **Added comprehensive logging** to track widget rendering and event handling:
   - Added `bunLog` calls in `TBCCShell` widget render and handleEvent
   - Added `bunLog` calls in `TBTestGen` widget render
   - Logged tab switching events, state changes, and render output

2. **Discovered the core issue:**
   - Widgets WERE rendering (logs showed `render output length=11459`)
   - Tab switching WAS working (logs showed `Changing tab to: testgen`)
   - BUT: When the shell widget re-rendered, it called `dom.render(container, content)` which does `container.innerHTML = content.toString()`
   - **This wiped out all child widget DOM** because the shell's render output includes the tab containers

### The Critical Bug

**Parent/Child Widget Relationship Problem:**

```typescript
// Shell widget renders tab containers
render: (ctx) => html`
  <div id="tbcc-tab-dashboard" class="${state.activeTab === "dashboard" ? "" : "hidden"}"></div>
  <div id="tbcc-tab-testgen" class="${state.activeTab === "testgen" ? "" : "hidden"}"></div>
  ...
`

// Child widgets mount into those containers
yield* mountWidgetById(TBTestGenWidget, "tbcc-tab-testgen")

// When shell state changes (activeTab), it re-renders
// Re-render calls dom.render(container, newHTML)
// This does: container.innerHTML = newHTML
// **Child widgets' DOM is wiped out!**
```

**Why it happened:**
- Effuse uses `innerHTML` replacement for rendering (no virtual DOM, no diffing)
- When parent widget re-renders, it replaces entire container innerHTML
- Child widgets are mounted into containers that are part of parent's render output
- Re-render destroys child widget DOM, even though widgets are still "mounted" in Effect

---

## Solution Implemented

### Fix: Direct DOM Manipulation for Tab Switching

Instead of relying on re-render to update tab visibility, we update DOM classes directly in the event handler:

```typescript
case "changeTab": {
  // CRITICAL: Don't update state - it triggers re-render which wipes child widgets
  // Instead, update DOM directly. State will be out of sync, but UI will work.
  
  // Update tab container visibility directly
  for (const tabId of TABS) {
    const container = yield* ctx.dom.queryOption(`#tbcc-tab-${tabId}`)
    if (container) {
      if (tabId === event.tab) {
        container.classList.remove("hidden")
      } else {
        container.classList.add("hidden")
      }
    }
  }
  
  // Update sidebar button active states directly
  const allButtons = ctx.container.querySelectorAll(`[data-action='changeTab']`)
  for (const btn of Array.from(allButtons)) {
    // Update button classes directly
  }
  
  // NOTE: We intentionally DON'T update state to avoid re-render
  // State will be out of sync, but UI works correctly
}
```

### Why This Works

1. **No re-render** = No `innerHTML` replacement = Child widgets stay intact
2. **Direct DOM updates** = Immediate visual feedback
3. **State out of sync** = Acceptable trade-off for working UI

### Trade-offs

- ✅ **Pros:** UI works, child widgets stay mounted, no double-click needed
- ⚠️ **Cons:** State is out of sync (if shell re-renders for other reasons, sidebar might show wrong active tab)

---

## Files Modified

### 1. `src/effuse/widgets/tb-command-center/tbcc-shell.ts`

**Changes:**
- Modified `handleEvent` for `changeTab` to update DOM directly instead of relying on re-render
- Added comprehensive logging for debugging
- Removed state update that was triggering re-render

**Key Code:**
```typescript
case "changeTab": {
  // Update tab container visibility directly (no state update)
  for (const tabId of TABS) {
    const container = yield* ctx.dom.queryOption(`#tbcc-tab-${tabId}`)
    if (container) {
      container.classList.toggle("hidden", tabId !== event.tab)
    }
  }
  // Update sidebar buttons directly
  // ...
}
```

### 2. `src/effuse/widgets/tb-command-center/tbcc-testgen.ts`

**Changes:**
- Added debug logging to track render calls and output
- Added temporary red banner for visibility testing (removed in final commit)
- Logged render output length and state

### 3. `docs/effuse/ARCHITECTURE.md`

**Changes:**
- Added comprehensive section on "Parent/Child Widget Relationships"
- Documented the problem, three solution approaches, and when to use each
- Added example from TBCC Shell widget
- Added "DOM Rendering Model" section explaining innerHTML replacement

**Key Documentation:**
```markdown
## Parent/Child Widget Relationships

**Critical Pattern:** When a parent widget renders containers for child widgets, 
re-rendering the parent will **wipe out child widget DOM**.

### Solutions:
1. Direct DOM manipulation (recommended for tab switching)
2. Restructure containers (don't render child containers in parent)
3. Conditional re-rendering (only re-render parts that don't contain child widgets)
```

### 4. `docs/effuse/README.md`

**Changes:**
- Added to "Common Mistakes" section:
  - Mistake #5: "Re-rendering parent wipes child widgets"
  - Links to detailed ARCHITECTURE.md section

---

## Testing & Validation

### Before Fix:
- ❌ First click on sidebar buttons didn't register
- ❌ Second click registered but showed black screen
- ❌ Widgets were rendering (logs confirmed) but content not visible
- ❌ Double-click required for all tab switches

### After Fix:
- ✅ Single click works immediately
- ✅ Tab content is visible
- ✅ Widgets render and display correctly
- ✅ No double-click needed

### Logs Confirmed:
```
[TBCCShell] handleEvent: changeTab
[TBCCShell] Changing tab to: testgen
[TBCCShell] Showing tab: testgen, container.innerHTML.length=11459
[TBTestGen] render called, status=idle
[TBTestGen] render output length=11459, hasEmptyState=true, hasControls=true
```

---

## Lessons Learned

### 1. Effuse Rendering Model
- Effuse uses `innerHTML` replacement (no virtual DOM)
- Every render replaces entire container content
- This is simple but has implications for parent/child relationships

### 2. Parent/Child Widget Pattern
- **Never re-render parent if it contains child widget containers**
- Use direct DOM manipulation for visibility changes
- Or restructure so parent doesn't render child containers

### 3. State vs DOM Sync
- Sometimes it's acceptable to have state out of sync with DOM
- Direct DOM manipulation can be more reliable than reactive updates
- Trade-off: State might be wrong, but UI works

### 4. Debugging Strategy
- Added comprehensive `bunLog` calls throughout
- Logged render calls, state changes, DOM updates
- Helped identify that widgets WERE rendering, but DOM was being wiped

---

## Follow-up Work

### Potential Improvements:
1. **State Sync:** Find a way to update state without triggering re-render
   - Could use a flag to skip re-render for tab changes
   - Or use a separate state mechanism for activeTab

2. **Architecture:** Consider restructuring so shell doesn't render tab containers
   - Move tab containers to HTML
   - Shell only manages sidebar and visibility

3. **Documentation:** Already added comprehensive docs to ARCHITECTURE.md

### Known Issues:
- State `activeTab` is out of sync (doesn't update on tab change)
- If shell re-renders for other reasons (sidebar collapse), sidebar might show wrong active tab
- Acceptable trade-off for now - UI works correctly

---

## Commits

This session will be committed as:
- Fix: TBCC tab switching - use direct DOM manipulation to avoid wiping child widgets
- Docs: Add parent/child widget relationship patterns to ARCHITECTURE.md
- Chore: Remove debug red banner from TestGen widget

---

## Summary

Fixed critical bug where parent widget re-renders were wiping out child widget DOM. Solution: Use direct DOM manipulation for tab visibility instead of relying on reactive state updates. This keeps child widgets intact while providing immediate visual feedback. Trade-off: State is out of sync, but UI works correctly.

**Key Takeaway:** When parent widgets render containers for child widgets, never re-render those containers - update their classes/attributes directly in event handlers.

