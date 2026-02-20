# Parity Checklist (WGPUI -> Effuse Panes)

This document is the blunt checklist for "100% parity with the wgpui version".

Source-of-truth references:

- Desktop orchestration: `crates/autopilot_ui/src/lib.rs` (look for `MinimalRoot`)
- HUD components:
  - `crates/wgpui/src/components/hud/pane.rs`
  - `crates/wgpui/src/components/hud/resizable_pane.rs`
  - `crates/wgpui/src/components/hud/hotbar.rs`

## What Matches Today (Behavioral Parity)

### PaneStore semantics

- Z-order == array order (last is top-most): YES
- `activePaneId` tracked separately and used to bring active last: YES
- `addPane(existing)` brings-to-front and activates without overwriting state: YES
- `removePane(storePosition=true)` stores snapshot and updates active to last: YES
- `togglePane()` close-if-active, focus-if-inactive, restore-if-missing: YES
- `offsetAll(dx,dy)` pans all panes and last position: YES

### Drag/resize/pan orchestration

- Drag starts only from title bar (excluding close button): YES
- Drag updates rect continuously and persists last position on release: YES
- Resize from edges/corners (hit test + pure bounds math): YES
- Resize enforces min size (200x100) + persists last position on release: YES
- Canvas pan by dragging background (not panes): YES

### Keyboard behaviors

- `Escape` closes active pane if dismissable: YES
- `Cmd/Ctrl + 0..9` triggers hotbar slots (and flashes): YES

## What Is Not Yet 100% Parity (Known Gaps)

These gaps are mainly in UI polish and Autopilot-specific behaviors.

### Hotbar visuals

WGPUI hotbar renders:

- icons
- slot numbers overlay
- hover/pressed states
- ghost slots to maintain layout symmetry

DOM adapter currently:

- renders `HotbarSlot.icon` as plain text
- does not draw slot number overlays
- renders `ghost=true` as reduced opacity only

Status: PARTIAL (behavior parity, not visual parity).

### Pane chrome visuals

WGPUI `PaneFrame` includes:

- active border highlighting
- title bar styling
- close button hover behavior

DOM adapter includes minimal equivalents, but not pixel-identical styling.

Status: PARTIAL (behavior parity, not visual parity).

### Autopilot-specific pane inventory

`MinimalRoot` defines many pane kinds and actions:

- Events, Threads, File Editor, Identity, Pylon, Wallet, Sell Compute, etc.
- Multiple chat panes with slot assignments, model dropdowns, etc.

This package intentionally does not provide those app-specific actions.
The host app must implement the mapping:

- hotbar slot -> open/toggle pane id -> create pane with kind/title/rect

Status: INTENTIONALLY OUT OF SCOPE for this package.

### Persistence beyond memory

WGPUI currently stores "closed pane positions" in memory only (no long-term storage).

If the web wants to persist layouts between sessions, we need a host-level mechanism
(LocalStorage, Khala, etc).

Status: NOT PROVIDED (by design).

## Steps To Reach "100% parity" (If Required)

If the goal is "a user can't tell the difference":

1. Hotbar visuals:
   - add slot number overlays
   - implement hovered/pressed states similar to Rust
   - implement ghost slot layout symmetry
2. Pane visuals:
   - match colors/alpha/borders more precisely
   - close button sizing/placement to match `PaneFrame`
3. Cursor feedback:
   - resize cursor per edge/corner
   - drag cursor on title bar
4. DOM adapter performance:
   - reduce work during pointer move (avoid full replaceChildren where possible)
5. Autopilot canary integration:
   - add a demo page in `apps/web` that hosts the pane system and provides a couple panes
     (events + trace viewer) so parity is visible.

