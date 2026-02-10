# @openagentsinc/effuse-panes

Browser/DOM implementation of the Autopilot Desktop “HUD pane system”:

- Draggable, resizable floating panes (window manager style)
- Canvas panning (drag background to move all panes)
- Active pane z-order (bring-to-front)
- Close + restore last position (toggle-style behavior)
- Hotbar model (slot flashing + click collection)

Parity source-of-truth:

- `crates/wgpui/src/components/hud/pane.rs` (`PaneFrame`)
- `crates/wgpui/src/components/hud/resizable_pane.rs` (`ResizablePane`, `ResizeEdge`)
- `crates/wgpui/src/components/hud/hotbar.rs` (`Hotbar`, `HotbarSlot`)
- `crates/autopilot_ui/src/lib.rs` (`PaneStore`, drag/resize/pan orchestration)

This package is deliberately self-contained and does not assume Tailwind.

## Minimal Usage

```ts
import { mountPaneSystemDom, calculateNewPanePosition, hotbarSlot } from "@openagentsinc/effuse-panes";

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
      const rect = calculateNewPanePosition(sys.store.lastPanePosition, screen, 520, 320);
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
- Pane content is intentionally host-rendered. Fill it by selecting `[data-pane-id="<id>"] [data-oa-pane-content]`.
- Keyboard: `Esc` closes the active (dismissable) pane; `Cmd/Ctrl + 0-9` triggers hotbar slots (flashes briefly).
