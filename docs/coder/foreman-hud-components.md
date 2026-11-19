# Foreman HUD — GPU-Native Unit/Arbiter Component System

Date: 2026-11-19
Status: Active implementation spec
Scope: `components/kit/foreman_hud.coax`, `apps/coder-desktop/page.coax`

## 1. Purpose & Lineage

Coder Desktop's Foreman (`docs/coder/foreman-spec.md`) is a persistent orchestrator chat that supervises managed worker chats across permission tiers (`Observe`, `Managed`, `Auto`). Previously, opening Foreman rendered the same flat conversation column as a regular chat.

This spec defines a **GPU-native 2D dataflow HUD component system** for Foreman in Coder Desktop, distilling the strongest ideas from:

- **Samuel Timbo's Unit framework** and OpenAgents' prior **`arbiter-effect`** 2D dataflow graph renderer (`openagents.arbiter.graph_spec.v0`, commits `dc71c1bb74`, `ba3299933b`, `5c64b20b3b`): typed nodes, input/output ports, orthogonal signal wires, explicit node lifecycle status, and direct click-to-inspect interaction.
- **Coder Desktop's GPU component architecture** (`components/gpu-components.md`): pure Coax, zero heap allocation, caller-owned `Sheet` / `UiSurface` / `KeyedStore` buffers, shared Metal vector primitives (`prim_box`, `prim_line`, `prim_ring`, `prim_arc`), skin-driven tokens (`UiSkin`), reduced-motion support, and first-class semantic/hit-test rows (`uiframe_semantic`, `sheet_hit`).

## 2. Design Principles

1. **Shared GPU primitives only** — Every bracket, node plate, port socket, orthogonal conduit, pulse bead, and progress ring is emitted onto `Sheet.under` / `Sheet.over` using the standard `Prim` set. No bespoke shader or AppKit overlay is required.
2. **Zero allocation, bounded frame budget** — All graph inputs and scratch buffers are fixed-size value arrays. `uiframe_end` refuses any frame with `sheet.lost > 0`.
3. **Skin-first monochrome + semantic accents** — Surfaces, borders, and typography follow `cp_skin()` (monochrome Arcade palette). Status energy is expressed through `UiSkin` semantic roles:
   - `FHUD_IDLE` -> `s.faint` / `s.muted`
   - `FHUD_RUNNING` -> `s.accent` (with animated arc / wire pulse)
   - `FHUD_DONE` -> `s.success`
   - `FHUD_WARN` -> `s.warning`
   - `FHUD_FAILED` -> `s.danger`
4. **Interactive by construction** — Every node and toggle registers a semantic `EL_BUTTON` row with a stable ID and `sheet_hit` region. Clicking a managed worker node dispatches `CP_FOREMAN_HUD_OPEN` with its `foreman.node.chat.<id>` row ID (kept distinct from sidebar `chat.<id>` rows), jumping directly into that worker chat.
5. **Reduced-motion respectful** — When `f[0].reduced_motion` is true, rotating arcs and traveling wire signal pulses freeze cleanly into crisp static indicators.

## 3. Component Primitives (`components/kit/foreman_hud.coax`)

### 3.1 `fhud_port`
Renders a circular Unit-style input or output port socket centered at `(cx, cy)`:
- Outer socket ring (`prim_ring`) colored by status tone when active or `s.hairline` when idle.
- Inner terminal core (`sheet_fill` rounded disc) lit with the node's status color when connected/active.

### 3.2 `fhud_wire`
Renders a 3-segment orthogonal (Manhattan) dataflow conduit from `(x0, y0)` to `(x1, y1)` via a shared routing bus `mid_x`:
- Horizontal launch `(x0, y0) -> (mid_x, y0)`
- Vertical bus `(mid_x, y0) -> (mid_x, y1)`
- Horizontal entry `(mid_x, y1) -> (x1, y1)`
- When `active` is true, draws a brighter conduit stroke plus a traveling GPU signal pulse bead along the path parameterized by `f[0].clock` (frozen at midpoint when `reduced_motion`).

### 3.3 `fhud_node`
Renders an interactive 2D dataflow node card (`w x h`) with:
- Left input port and/or right output port (`has_in`, `has_out`).
- Left status rail + status beacon (`activity` dual-arc spinner when `FHUD_RUNNING`, solid status dot otherwise).
- Primary node title (`s.strong`) and monospace/small status or role subtitle (`s.small`).
- Hover/press surface elevation (`s.hover` / `s.raised`) and semantic `EL_BUTTON` row (`uiframe_semantic`) + `sheet_hit` when `action != 0`.

### 3.4 `fhud_metric`
Renders a compact telemetry HUD tile (`w x h`) showing:
- Uppercase/small muted label (`s.small`),
- Crisp value (`s.strong`),
- Optional `progress_ring` gauge on the right when `progress >= 0.0` (e.g., budget utilization or active worker ratio).

### 3.5 `fhud_hud`
Top-level Foreman HUD panel composed from the primitives above:
- **Tactical Frame**: Corner-bracketed GPU frame (`animated_frame`-style corner ticks + subtle surface fill + hairline border).
- **Header Bar**: `FOREMAN HUD` title, live beacon, status pill (`LIVE` / `STANDBY`), and a collapse/expand button (`foreman.hud.toggle` -> `CP_FOREMAN_HUD_TOGGLE`).
- **Telemetry Strip** (when expanded): 4 metric tiles when wide (2 when narrow) — `TIER`, `MANAGED` (with active-ratio `progress_ring`), `SUB-AGENTS`, and `BUDGET` (with budget-ratio `progress_ring`).
- **2D Dataflow Topology**:
  - Left column: **Foreman Orchestrator Node** (`out` port connected to the central routing bus).
  - Center: **Signal Wires** fanning out from the Foreman output port to each visible managed worker node (up to `FHUD_MAX_NODES = 6`, with an overflow count badge if more exist).
  - Right column: **Managed Worker Nodes** (each clickable to open that managed chat), or a single standby target node (`No managed chats yet`) when the fleet is empty.

## 4. Coder Desktop Integration (`apps/coder-desktop/page/layout/conversation.coax`)

- `CoderPage` adds `foreman_hud_collapsed: bool` (default `false`) and action `CP_FOREMAN_HUD_TOGGLE :: 23`.
- In `cp_conversation` (`cp_foreman_hud`), whenever `cur == "chat.foreman"`:
  1. Collects managed chats (`s[0].chats[q].managed`), counts busy workers and active sub-agents, and formats current tier and spend/budget.
  2. Renders `fhud_hud(...)` at the top of the conversation column so it scrolls naturally with the Foreman workspace and stays visually aligned with the column width `cw`.
  3. Uses Foreman-specific empty-state copy when `s[0].nturns == 0`.
- Clicking any managed worker node in the HUD reuses `CP_CHAT` with the worker's `chat.<id>` semantic ID, seamlessly opening that managed chat via existing host dispatch (`CP_ASK_OPEN`).

## 5. Verification

- Unit checks in `components/kit/foreman_hud.coax` (`coax/bin/coax test components/kit/foreman_hud.coax`): port/wire/node/metric primitives, active pulse motion vs. reduced motion, hit testing, collapse mode, and zero `sheet.lost`.
- Integration checks in `apps/coder-desktop/page/checks/layout-checks.coax` (`coax/bin/coax test apps/coder-desktop/page.coax --library`): Foreman HUD presence when `chat.foreman` is active, clicking a managed node dispatches `CP_ASK_OPEN`, and clicking the toggle collapses/expands the HUD.
- Full macOS build check via `CODER_BUILD=/tmp/coder-foreman-hud apps/coder-desktop/build.sh`.
