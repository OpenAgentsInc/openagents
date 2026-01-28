# HUD + Pane System

## Overview
Autopilot now uses a Commander-style HUD and pane system. The main window is a blank black canvas, and all features (chat, identity, raw events) live inside panes that can be opened and closed via the hotbar or keyboard shortcuts.

This mirrors the Commander architecture:
- a pane store (ordered list = z-index),
- a pane renderer (maps pane type -> content),
- a hotbar for slot-based toggles.

Dragging/resizing is intentionally deferred to the next phase; see `docs/panes/PANE_DRAGGING.md`.

## Layout Layers
1. **HUD background**: full-screen black canvas.
2. **Pane stack**: absolute-positioned panes rendered in store order.
3. **Hotbar**: fixed bottom-center row of slots.

## Pane Model (Rust)
`crates/autopilot_ui/src/lib.rs` defines:
- `PaneKind`: `Chat | Events | Identity`
- `PaneRect`: `x/y/width/height`
- `Pane`: `{ id, kind, title, rect, dismissable }`
- `PaneStore`: `{ panes, active_pane_id, last_pane_position, closed_positions }`

### Core Behaviors
- **add**: creates a pane and marks it active (or brings an existing pane to front).
- **toggle**: if active -> close; if inactive -> bring to front; if missing -> create.
- **bring to front**: moves pane to end of `panes` for top z-order.
- **close**: removes from list and stores rect in `closed_positions`.

Pane positions are calculated using `PANE_OFFSET` and clamped by `ensure_pane_visible` to keep panes on-screen.

## Hotbar (Commander Style)
`crates/wgpui/src/components/hud/hotbar.rs` renders the Commander-style bar:
- square slots with icons + numeric overlays
- ghost slots maintain layout symmetry
- slot actions trigger pane open/close

### Current Slot Mapping
1. **Chat** (toggle chat pane)
2. **Events** (toggle raw event feed)
3. **Identity** (toggle Nostr/Spark keys)
4–9. Reserved (ghosts)

### Keybindings
`Cmd+1..Cmd+9` trigger the corresponding hotbar slots.

## Pane Types
### Chat Pane
- Title bar: pane chrome with close button.
- Header: **New chat** + **Full Auto** toggle.
- Thread info: “Initialized thread …” + model line.
- Body: streaming markdown, reasoning cards, tool call cards.
- Footer: text input + Stop + Send.

The **New chat** button opens a fresh chat pane and triggers a new Codex thread.
Currently the UI keeps a single chat state; opening a new chat replaces the existing chat pane.

### Events Pane
- Title: “CODEX EVENTS”
- Copy button to clipboard.
- Scrollable raw event log.

### Identity Pane
- “Generate keys” button (NIP-06 + Spark derived).
- Displays:
  - Nostr public key
  - Nostr secret key
  - Spark public key
  - Seed phrase

## Future: Drag/Resize
Drag + resize are intentionally deferred. The storage model already includes rects and a `last_pane_position` anchor. The plan is documented in:
`docs/panes/PANE_DRAGGING.md`.

## Work Log
- Read Commander pane management docs and mapped the architecture to WGPUI + Autopilot.
- Added WGPUI HUD components: `Hotbar` + `PaneFrame`.
- Implemented pane store + pane manager logic in `autopilot_ui`.
- Moved chat, events, and identity UIs into panes.
- Replaced old sidebars/hotbar with Commander-style hotbar slots.
- Updated keybindings to `Cmd+1..Cmd+9`.
- Ensured all builds run clean with no warnings.
