# Effuse Panes Docs Index

`@openagentsinc/effuse-panes` is a browser/DOM port of the Autopilot Desktop HUD pane system.

If you have used the Rust/WGPUI "Commander-style" UI (blank canvas + floating panes + hotbar),
this package provides the same *state model* and *interaction semantics* in TypeScript for the web.

## What You Get

The package is deliberately split into:

- A small, testable **state core**:
  - `PaneStore` (z-order, active focus, close/restore snapshots)
  - `ResizablePane` (resize hit-testing + pure resize math)
  - `HotbarModel` (slot flashing + click collection)
- A thin **DOM adapter**:
  - `mountPaneSystemDom()` which wires pointer + keyboard events and paints the minimal chrome.

For Effect-based hosts, you should still keep lifecycle scoped (mount/release) even though the
adapter itself is framework-agnostic. In `apps/web`, this is done via:

- `apps/web/src/effect/paneSystem.ts`

The DOM adapter intentionally does **not** own your pane content. You render content yourself
(Effuse, React, vanilla DOM), and the pane system handles geometry + chrome + interactions.

## Where This Fits In OpenAgents

Source-of-truth Rust implementations (parity targets):

- `crates/autopilot_ui/src/lib.rs` (orchestration in `MinimalRoot`)
- `crates/wgpui/src/components/hud/resizable_pane.rs`
- `crates/wgpui/src/components/hud/hotbar.rs`
- `crates/wgpui/src/components/hud/pane.rs`

The Effuse/DOM port exists to enable:

- Web-based operator surfaces that feel like Autopilot Desktop.
- Re-using the pane/hotbar UX patterns for debugging cards, trace viewers, inspectors.
- Deterministic state transitions (unit-testable) and straightforward replay.

## Reading Order

1. `OVERVIEW.md`:
   - The mental model: layers, z-order, active pane, close/restore.
2. `ARCHITECTURE.md`:
   - Module boundaries and why the DOM adapter is intentionally thin.
3. `API_REFERENCE.md`:
   - A concrete list of exports and how to call them.
4. `INTEGRATION_EFFUSE.md`:
   - How to mount once in `apps/web`, render pane contents with Effuse, and handle scoped cleanup.
5. `PARITY_CHECKLIST.md`:
   - What matches WGPUI today, and what is still missing for 100% parity.
6. `THEMING_AND_STYLING.md`:
   - How the DOM adapter injects CSS and how to override theme tokens.
7. `MOBILE.md`:
   - Brainstorming for how the pane system should degrade or change on phones/tablets.
8. `TESTING.md`:
   - How to run and extend unit tests for the core logic.
9. `TROUBLESHOOTING.md`:
   - Pointer capture, focus, embedding gotchas.

## Quick Start (Minimal)

```ts
import { mountPaneSystemDom, calculateNewPanePosition } from "@openagentsinc/effuse-panes";

const root = document.querySelector("#panes");
if (!(root instanceof HTMLElement)) throw new Error("missing root");

const sys = mountPaneSystemDom(root);

const screen = { width: root.clientWidth, height: root.clientHeight };
const rect = calculateNewPanePosition(sys.store.lastPanePosition, screen, 520, 320);
sys.store.addPane({
  id: "events",
  kind: "events",
  title: "Events",
  rect,
  dismissable: true,
});
sys.render();
```

From here, you can render your pane content into:

`[data-pane-id="events"] [data-oa-pane-content]`

On unmount, always call:

```ts
sys.destroy();
```
