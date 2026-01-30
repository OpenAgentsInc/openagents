# Zed Phase 0 Work Log (Layout Engine + Framework Alignment)

Date: 2026-01-30

## Refactor docs found
- `docs/unified-layout-engine-plan.md` (explicit “Unified Layout Engine Refactor Plan” + worklog)
- `apps/autopilot-desktop/docs/migration/EFFUSE_TO_WGPUI_PLAN.md` (layout engine adoption plan)
- `docs/autopilot-migration-plan.md` (layout engine mandate for new UI)
- `docs/zed-gap-analysis.md` (notes manual layout in current UI surface)
- `docs/zed/zed-parity-roadmap.md` (Phase 0 exit criteria)

## Layout Engine Audit (current)

### Uses layout engine (or layout helpers) today
- **WGPUI core:** `crates/wgpui/src/layout.rs`, `crates/wgpui/src/layout_helpers.rs`, `crates/wgpui/src/element/*`, `crates/wgpui/src/styled/*` provide the layout engine + helpers.
- **Storybook top-level layout:** `crates/wgpui/examples/storybook/state.rs` uses `layout_header_nav_content` and `stack_bounds`.
- **Storybook panel stacking helpers:** `crates/wgpui/examples/storybook/helpers.rs` uses `stack_bounds` (via `panel_stack`).
- **Autopilot UI (partial):** `crates/autopilot_ui/src/lib.rs` uses `LayoutEngine` for the main panel shell and helper stacks (`stack_bounds`).

### Mixed/partial (layout engine exists but manual bounds math is still dominant)
- **Autopilot UI surface:** `crates/autopilot_ui/src/lib.rs` has a layout-engine shell, but most sub-layouts still use fixed sizes, `Bounds::new`, and manual x/y math (e.g., row lists, tool cards, command bar internals, panels).
- **Storybook sections:** Most section files in `crates/wgpui/examples/storybook/sections/*.rs` still place content with manual bounds math (numerous `Bounds::new` and `origin.x/y` arithmetic).
- **Autopilot Desktop app:** `apps/autopilot-desktop/src/main.rs` composes `autopilot_ui::MinimalRoot`, so it inherits the partial layout-engine coverage above.

### Manual layout (no layout engine usage detected)
- **Autopilot app UI rendering pipeline:** `crates/autopilot/src/app/ui/rendering/*` uses custom layout structs + manual bounds (`sidebar_layout`, `modal_y_in_content`, `row_bounds`, etc.).
- **Compute UI:** `crates/compute/src/ui/*` paints panels and rows with manual bounds math (`Bounds`, `Point`, fixed sizes).
- **Onyx UI:** `crates/onyx/src/app.rs` uses manual bounds for sidebar, editor, and file list layout.
- **WGPUI examples (non-storybook):** `crates/wgpui/examples/*.rs` (e.g., `component_showcase.rs`, `first_light.rs`, `ui_pane_demo.rs`) are manual layout.

## Proposed Plan to Finish Phase 0 (Layout + Framework Alignment)

### 1) Decide the canonical desktop UI surface
- Confirm whether `crates/autopilot_ui` (used by `apps/autopilot-desktop`) is the canonical UI, and whether `crates/autopilot/src/app/ui/rendering` is legacy or still required.
- If legacy, plan to either migrate it to layout engine or route it through the same WGPUI element/layout path as `autopilot_ui`.

### 2) Eliminate manual layout in Autopilot Desktop UI
- Convert `crates/autopilot_ui/src/lib.rs` sub-layouts (lists, headers, command bar, panels, tool cards) to layout engine nodes or WGPUI element-tree primitives.
- Replace explicit `Bounds::new` positioning with layout helpers (`layout_helpers`, `h_flex`/`v_flex`, or new shared containers).
- Reduce fixed pixel sizes where possible; use measured nodes for text-sized elements and flex sizing for lists.

### 3) Align on GPUI-style layout API surface
- Create a small, shared layout prelude (in WGPUI or `autopilot_ui`) to standardize `flex_*`, `min_w_0`, `overflow_*`, `gap_*`, etc.
- Ensure layout is expressed declaratively via element tree / layout helpers rather than per-frame math.

### 4) Wire action/keymap + focus into the layout tree
- Make the layout tree the primary dispatch surface (actions and focus contexts anchored to layout nodes).
- Ensure Autopilot Desktop routes input → focused entity/view via WGPUI’s action/keymap + focus system.

### 5) Add layout test harness for the desktop UI
- Use `crates/wgpui/src/testing` to assert element-tree structure and computed bounds for the workspace shell, lists, and composer.
- Add regression tests for DPI scaling and font-size changes (no overlap).

### 6) Guardrails to prevent regressions
- Add a lint/check for manual layout patterns in desktop UI paths (e.g., `Bounds::new` in `autopilot_ui`, `autopilot/src/app/ui/rendering`).
- Document the “layout engine only” rule in the relevant migration doc.

### Follow‑ups (outside Phase 0 but necessary for full consistency)
- Migrate Storybook section internals and non-storybook WGPUI examples to layout engine.
- Convert Compute and Onyx UIs to layout engine (or deprecate if they are demo-only).

## Open questions / decisions to confirm
- Is `crates/autopilot/src/app/ui/rendering` still live or can it be deprecated in favor of `autopilot_ui`?
- Should layout helpers live in WGPUI (generic) or in `autopilot_ui` (product-specific)?
- What is the intended ownership of layout tests (WGPUI vs Autopilot UI)?

## Work log
- 2026-01-30: Converted `crates/autopilot_ui/src/lib.rs` session list columns, status line columns, status action badges, and command bar hints to use `wgpui::row_bounds` instead of manual x-offset math.
- 2026-01-30: Converted header badges and status pills in `crates/autopilot_ui/src/lib.rs` to layout-engine driven right-aligned rows (`right_aligned_row_bounds`).
- 2026-01-30: Rebuilt the input/composer bar layout in `crates/autopilot_ui/src/lib.rs` using `LayoutEngine` rows/columns instead of manual y/x offsets.
- 2026-01-30: Added a small layout prelude (`h_flex`, `v_flex`, `flex_1`, `gap`, `min_w`) and applied it across the main panel layout and helper layouts in `crates/autopilot_ui/src/lib.rs`.
- 2026-01-30: Converted Autopilot Desktop panes to layout-engine column/row primitives (chat pane, identity, pylon, wallet, sell-compute, DVM history, NIP-90, events, threads, file editor) and added `column_bounds` + centered layout helpers in `crates/autopilot_ui/src/lib.rs` to eliminate manual y-offset layout.
