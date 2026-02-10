# Mobile Brainstorming (Behavior + UX)

This doc is deliberately opinionated brainstorming for how the HUD pane system should behave on mobile.
It includes:

- the constraints that make "desktop-style floating panes" awkward on phones
- a few viable UX modes
- concrete API/code changes we should add to `@openagentsinc/effuse-panes`

The goal is not to perfectly replicate the desktop. The goal is to keep the *core* pane/hotbar model
useful while making the mobile experience predictable and not-janky.

## Constraints (Phones Are Not Tiny Desktops)

### 1) Screen real estate

Floating windows work when you can see multiple panes at once.
On a phone:

- two panes == unreadable
- a hotbar consumes precious vertical space
- the "canvas" mental model adds complexity without benefit

### 2) Coarse pointer input

- Edges/corners are hard to hit reliably.
- Dragging is fine, but accidental drags are common.
- Resizing is a “power feature” that most mobile users won’t want.

### 3) Scroll is the primary gesture

Most panes contain scrollable content (threads, logs, tool output).
Any gesture model that interferes with scroll will feel broken.

### 4) Keyboard shortcuts don’t exist (in practice)

`Esc`, `Cmd+1..9` are desktop affordances.
On mobile, they are irrelevant unless using an external keyboard.

### 5) Safe areas + OS UI overlays

- Hotbar must respect `safe-area-inset-bottom` (iOS home indicator).
- Soft keyboard will cover bottom UI; hotbar might need to hide or move.

### 6) Performance

Re-layout + DOM reordering during pointermove can be OK on desktop and janky on phones.
We should avoid "render 120 times/sec" behavior.

## Current State (What The Package Does Today)

`mountPaneSystemDom()` currently implements desktop HUD semantics:

- Drag background to pan all panes (canvas pan)
- Drag a pane by its title bar
- Resize from edges/corners
- Hotbar UI (clickable)
- Keyboard shortcuts:
  - `Esc` closes active pane
  - `Cmd/Ctrl + 0..9` triggers hotbar slots and flashes the slot

Recent code changes that help mobile (already implemented):

- Hotbar bottom uses `env(safe-area-inset-bottom, 0px)` to avoid iOS home indicator overlap.
- `mountPaneSystemDom` supports feature gating via config:
  - `enableCanvasPan`, `enablePaneDrag`, `enablePaneResize`, `enableHotbar`, `enableKeyboardShortcuts`, `enableDotsBackground`
- Pointermove-driven rendering is throttled to `requestAnimationFrame` via `scheduleRender()` to reduce jank.

## What Should We Do On Mobile?

There are three viable modes.

### Mode A: "Floating, But Reduced" (minimal changes)

Keep floating panes, but reduce the feature surface:

- disable resize (too fiddly)
- disable canvas pan (low value)
- keep only 1-2 panes open at once
- keep hotbar as the only navigation (tap to switch)

Pros:
- fastest to ship
- still feels like the desktop HUD

Cons:
- still cognitively "window manager" on a phone
- still risks gesture conflicts if drag is too easy to trigger

Recommended code changes:
- add a `mobilePresetConfig()` helper (see "Proposed API Changes" below)
- increase title bar height and hotbar item size on mobile (touch target >= 44px)
- optionally require "long-press" to start dragging panes (avoid accidental drags)

### Mode B: "Docked Single-Pane" (recommended for phones)

Treat panes as *views*, not windows:

- Only the active pane is visible.
- Pane rect is forced to fullscreen (minus hotbar/nav).
- The store still tracks open panes (so "toggle" semantics work).
- Drag/resize are disabled.

Navigation:
- hotbar becomes bottom tabs (Chat / Threads / Events / Identity / etc)
- close becomes "back" or just switching tabs

Pros:
- predictable: it behaves like a normal mobile app
- no gesture conflicts
- the pane model still applies (open/close, active focus, restore)

Cons:
- not "desktop parity" in visuals
- can’t see multiple panes at once (but you can’t on a phone anyway)

Recommended code changes:
- implement a `layoutMode: "floating" | "docked"` option in the DOM adapter:
  - `floating`: current behavior
  - `docked`: only render active pane; force rect to viewport; hide canvas pan/resize
- hotbar becomes optional (host may want standard route nav instead)

### Mode C: "Chat + Sheets/Drawers" (product-first)

Make chat the primary screen, and treat the other panes as secondary overlays:

- Chat is always visible (base layer).
- Opening Threads/Events/Identity shows a drawer/sheet (top/bottom/side).
- Close sheet returns to chat.

Pros:
- best product UX for autopilot
- matches common mobile mental models

Cons:
- it is not really a pane system anymore (it’s routing + overlays)
- requires additional components (sheet/drawer primitives) and state management

Recommended code changes:
- keep `PaneStore` core but implement a separate "sheet renderer" adapter
- or do not use pane system on mobile at all; use normal routes

## Recommendation For Autopilot (Concrete)

For `apps/web`:

1. Phone breakpoint (`<= 768px`):
   - use Mode B (Docked Single-Pane) or Mode C (Chat + Sheets) depending on desired UX.
   - do not expose floating multi-window behavior.
2. Tablet breakpoint:
   - consider Mode A (floating reduced) if it feels good.
3. Desktop:
   - use full floating pane mode.

## Proposed API Changes (Next Iteration)

These are changes I think we should implement to make mobile support clean without hacks in the host app.

### 1) `layoutMode` and viewport layout

Add to `PaneSystemConfig`:

```ts
layoutMode?: "floating" | "docked";
```

Behavior:
- `floating`: current behavior.
- `docked`: adapter:
  - forces active pane bounds to fill the viewport (minus hotbar height / safe area)
  - hides non-active panes without deleting them from the store
  - disables drag/resize/canvas pan regardless of other flags

### 2) A mobile preset helper

Add an exported helper:

```ts
export const mobilePaneSystemPreset = (): Partial<PaneSystemConfig> => ({
  enableDotsBackground: false,
  enableCanvasPan: false,
  enablePaneDrag: false,
  enablePaneResize: false,
  enableKeyboardShortcuts: false,
  enableHotbar: true,
  hotbarHeight: 62,
  hotbarItemSize: 44,
  paneTitleHeight: 44,
});
```

This is not magic, but it gives apps a one-liner "reasonable mobile config" without copying numbers.

### 3) Touch-action tuning

If we observe drag/resize not working reliably on iOS:

- set `touch-action: none` on `[data-oa-pane-title]` to ensure pointermove is delivered during drags
- set `touch-action: pan-y` on `[data-oa-pane-content]` so vertical scroll remains native

This should be gated (config flag) because it can surprise hosts.

### 4) Soft keyboard / hotbar collision

If we embed this inside pages where the soft keyboard can open:

- auto-hide hotbar when `visualViewport.height` shrinks (keyboard visible)
- or move hotbar above keyboard using `visualViewport.offsetTop/height`

This should likely be host-controlled at first (it’s app-specific).

## Proposed Host Changes (apps/web)

Even if the pane system supports mobile, we should still prefer a standard app UX.

Concrete integration ideas:

- `apps/web/src/effuse-pages/autopilot`:
  - on mobile, render standard nav (top bar + bottom tabs)
  - use "read-only" pages for debug surfaces (DSE trace, receipts) as separate routes
- only use `effuse-panes` on mobile for "debug mode" or "operator mode"

## Open Questions (Need Product Decisions)

1. Should "multi-pane" be available at all on phones?
2. Is the hotbar a product concept on web, or only a debug/operator affordance?
3. Do we want persistence of pane layout on mobile? If so, where (LocalStorage vs Convex)?

## Next Steps

If we want to converge quickly:

1. Implement `layoutMode: "docked"` in the DOM adapter (fast, safe).
2. Add a `mobilePaneSystemPreset()` helper.
3. Add an `apps/web` storybook story showing:
   - floating desktop mode
   - docked mobile mode
4. Decide whether Autopilot mobile uses pane system at all, or uses standard routing.

