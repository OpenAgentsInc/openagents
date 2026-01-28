# Unified Layout Engine Refactor Plan (WGPUI)

## Goal
Refactor **all WGPUI UI** in this codebase to use a single, unified layout system based on the
`LayoutEngine` + `LayoutStyle` (Taffy) pattern already used in `crates/autopilot_ui`. Manual
coordinate math and ad-hoc bounds arithmetic should be removed across all WGPUI surfaces.

This plan starts with **storybook** and then migrates the rest of the WGPUI UI stack to the
same layout engine.

## Scope (all WGPUI UI)
Primary WGPUI surfaces to refactor:
- `crates/wgpui/examples/storybook/**`
- `crates/autopilot/src/app/ui/**` (rendering, modals, overlays)
- `crates/autopilot_ui/**` (keep as reference + unify helpers)
- `crates/compute/src/ui/**`
- `crates/onyx/src/**`
- `crates/wgpui/examples/**` (component_showcase, first_light, ui_pane_demo, etc.)

Secondary (layout-aware components and containers):
- `crates/wgpui/src/components/**`
- `crates/wgpui/src/layout.rs`
- `crates/wgpui/src/styled/**`

## Current State (summary)
- `autopilot_ui` uses `LayoutEngine` + `LayoutStyle` with explicit trees and `compute_layout`.
- Storybook and most other WGPUI screens use **manual bounds math** (x/y increments, fixed sizes).
- The layout engine supports measured nodes in API, but **`request_measured` does not wire a
  measurement callback to Taffy** (so auto-sizing is not functional yet).
- Components generally **paint manually** and do not expose layout nodes or children to a layout
  engine.

## Target State (definition)
1. **All screens** (storybook, autopilot UI, compute, onyx, examples) build layout trees via
   `LayoutEngine` and consume computed bounds.
2. **No manual layout math** for positioning/stacking, except inside leaf components where the
   component is the layout unit.
3. **Measured layout** works: components can report size (e.g., text, badges) via layout
   measurement for dynamic sizing.
4. **Shared layout helpers** live in WGPUI (or a small shared crate) and are reused across
   all UI surfaces.
5. **Consistency checks**: lint/tests detect manual layout usage in UI layers.

## Workstreams Overview
1. **Layout Engine Foundation** (WGPUI core)
2. **Layout Primitives + Helpers**
3. **Storybook Migration (start here)**
4. **Autopilot UI Migration**
5. **Compute + Onyx Migration**
6. **WGPUI Examples Migration**
7. **Guardrails + Enforcement**

---

## 1) Layout Engine Foundation (WGPUI core)
### Objective
Make the layout engine fully capable of measured layout and container composition.

### Tasks
- Implement real measurement support in `LayoutEngine::request_measured` (wire measure callbacks
  to Taffy nodes). This enables intrinsic sizing (e.g., text, badge, row heights).
- Add a **LayoutNode wrapper** that pairs a `Component` with a `LayoutStyle` and optional
  measurement closure.
- Add adapters that map `Component::size_hint` into a measurement callback so components can be
  measured without manual sizing.
- Add a **layout tree builder** helper that matches the `autopilot_ui` pattern and is safe to use
  across UIs.

### Acceptance Criteria
- A component can opt into measured sizing through the layout engine.
- `LayoutEngine` supports dynamic sizing without manual height constants.

---

## 2) Layout Primitives + Helpers
### Objective
Create reusable primitives so every UI surface can use the same layout approach.

### Tasks
- Introduce layout helpers in WGPUI, modeled after `autopilot_ui`:
  - `stack_bounds` / `row_bounds` / `grid_bounds` using `LayoutEngine`
  - `layout_panel`, `layout_header_nav_content`
- Add layout container components (Flex / Stack / Grid) that consume children + styles and produce
  bounds from `LayoutEngine`.
- Ensure containers expose layout styles (via `StyleRefinement.layout`) and propagate layout
  context to children.

### Acceptance Criteria
- Storybook, compute, and autopilot UI can use the same helper functions for common layouts.

---

## 3) Storybook Migration (first and complete)
### Objective
Refactor the entire storybook to use the unified layout engine and layout primitives.

### Phases
**Phase A: Skeleton layout**
- Replace `Storybook::layout` manual math with a `LayoutEngine` tree:
  header + nav + content (flex row, fixed nav width, flexible content).

**Phase B: Section stacking**
- Replace manual `y += height + gap` stacking with layout-engine stacks.
- Convert panel heights from hardcoded values to measured or fixed styles where appropriate.

**Phase C: Panel internals**
- Refactor each `paint_*` section to use layout primitives for rows/columns/grids.
- Remove direct x/y position increments in storybook sections.

### Acceptance Criteria
- Zero manual layout math in storybook.
- All storybook sections render via layout engine bounds.
- Layout scales cleanly at different window sizes.

---

## 4) Autopilot UI Migration
### Objective
Unify all autopilot UI rendering (including overlays/modals) under the same layout engine.

### Tasks
- Audit `crates/autopilot/src/app/ui/rendering/**` for manual layout.
- Introduce layout helpers into those render paths (or reuse the new WGPUI primitives).
- Migrate modals (help, wallet, pylon, dspy, etc.) to layout engine bounds.
- Replace manual scroll region bounds with layout-computed areas.

### Acceptance Criteria
- All autopilot UI rendering paths use layout engine for bounds.
- No manual positioning except within leaf components.

---

## 5) Compute + Onyx Migration
### Objective
Convert the compute UI and onyx UI to use the unified layout engine.

### Tasks
- Convert `crates/compute/src/ui/**` panels to layout primitives.
- Convert `crates/onyx/src/app.rs` screen layout to layout engine.
- Replace fixed x/y drawing with layout engine computed bounds.

### Acceptance Criteria
- Compute + Onyx UIs do not use manual layout arithmetic.

---

## 6) WGPUI Examples Migration
### Objective
Ensure all examples use the unified layout system (to avoid regression and teach the pattern).

### Tasks
- Refactor all `crates/wgpui/examples/**` to use layout primitives.
- Update example docs/comments to point to layout helpers rather than manual math.

### Acceptance Criteria
- Examples serve as clean reference usage for layout engine patterns.

---

## 7) Guardrails + Enforcement
### Objective
Prevent regressions to manual layout across the codebase.

### Tasks
- Add a lint or CI check that flags manual layout patterns (e.g., `y +=` or repeated `Bounds::new`
  in UI layers) in targeted directories.
- Add tests for layout stability and minimum sizing.
- Add developer docs for layout usage (quick reference + examples).

### Acceptance Criteria
- New UI code must use layout engine to pass CI.

---

## Ordering / Milestones
1. **Layout Engine Foundation**
2. **Layout Primitives + Helpers**
3. **Storybook Migration (complete)**
4. **Autopilot UI Migration**
5. **Compute + Onyx Migration**
6. **Examples Migration**
7. **Guardrails + Enforcement**

---

## Risks / Dependencies
- **Measured layout**: if not implemented, dynamic sizing will still require hardcoded values.
- **Component internals**: some components will still manually layout internal glyphs and icons,
  which is acceptable as long as parent layout uses the engine.
- **Incremental migration**: mixing manual layout with layout engine in the same surface will
  create inconsistencies; refactors should fully convert each surface.

---

## Definition of Done
- Storybook and every WGPUI UI surface uses the same layout engine (no manual bounds math).
- Layout helpers are shared and used consistently across crates.
- Measured layout works for text-heavy components.
- CI blocks regressions to manual layout.

## Worklog
- 2026-01-28: Phase 1: added guidance module story, wired measured layout support in WGPUI,
  and fixed Storybook command palette bindings/shortcuts.
- 2026-01-28: Phase 2: added `layout_helpers` utilities (offset/stack/row/grid/header-nav/
  panel), introduced Flex/Stack/Grid elements with per-child layout styles, and exported the
  new APIs from WGPUI. Added unit tests for helper layouts.
- 2026-01-28: Phase 3: refactored Storybook scaffolding to use `layout_header_nav_content`
  for header/nav/content layout and `stack_bounds` for nav item layout and hit-testing.
- 2026-01-28: Phase 3: added `panel_stack` helper and converted the Atoms + Codex storybook
  sections to use layout-engine stacking for top-level panels.
- 2026-01-28: Phase 3: converted Arwes storybook sections to use layout-engine panel stacking
  across frames/backgrounds/text effects/illuminator panels.
- 2026-01-28: Phase 3: converted HUD storybook sections (widgets/light demo/system UI) to use
  layout-engine panel stacking.
- 2026-01-28: Phase 3: converted the Storybook chat threads + bitcoin wallet panels to use
  layout-engine panel stacking.
