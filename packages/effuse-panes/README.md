# @openagentsinc/effuse-panes

Browser/DOM implementation of the Autopilot Desktop "HUD pane system":

- A blank canvas background (optionally dotted) that can be panned.
- A z-ordered stack of draggable/resizable floating panes.
- A bottom-center hotbar model for slot-based commands.

This package is intentionally DOM-first:

- No React dependency.
- No Tailwind dependency.
- Works with Effuse by mounting once and letting Effuse render pane content.

## Docs

Start here:

- `docs/INDEX.md`

Useful deep dives:

- `docs/API_REFERENCE.md`
- `docs/PARITY_CHECKLIST.md`
- `docs/INTEGRATION_EFFUSE.md`
- `docs/THEMING_AND_STYLING.md`
- `docs/TROUBLESHOOTING.md`

## Parity Sources (Rust)

These are the source-of-truth implementations this port was derived from:

- `crates/autopilot_ui/src/lib.rs` (pane store + orchestration in `MinimalRoot`)
- `crates/wgpui/src/components/hud/pane.rs` (`PaneFrame`)
- `crates/wgpui/src/components/hud/resizable_pane.rs` (`ResizablePane`, `ResizeEdge`)
- `crates/wgpui/src/components/hud/hotbar.rs` (`Hotbar`, `HotbarSlot`)

When docs disagree with code: code wins.

## Quick Start (Minimal)

```ts
import {
  mountPaneSystemDom,
  calculateNewPanePosition,
  hotbarSlot,
} from "@openagentsinc/effuse-panes";

const root = document.querySelector("#panes");
if (!(root instanceof HTMLElement)) throw new Error("missing #panes");

const sys = mountPaneSystemDom(root, {
  hotbarItems: [
    hotbarSlot(1, "+", "New pane"),
    { ...hotbarSlot(2, "EV", "Events"), active: true },
  ],
  onHotbarSlotClick: (slot) => {
    if (slot === 1) {
      const screen = { width: root.clientWidth, height: root.clientHeight };
      const rect = calculateNewPanePosition(
        sys.store.lastPanePosition,
        screen,
        520,
        320,
      );

      sys.store.addPane({
        id: `pane-${Date.now()}`,
        kind: "default",
        title: "Pane",
        rect,
        dismissable: true,
      });

      sys.render();
    }
  },
});
```

Notes:

- Pane content is intentionally host-rendered. Render into `[data-pane-id="<id>"] [data-oa-pane-content]`.
- Keyboard:
  - `Esc` closes the active (dismissable) pane.
  - `Cmd/Ctrl + 0-9` triggers hotbar slots (and flashes the slot briefly).

## Running Tests

```bash
cd packages/effuse-panes
bun install
bun run typecheck
bun test
```
