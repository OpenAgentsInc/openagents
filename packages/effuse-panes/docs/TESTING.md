# Testing

This package is structured so the important logic can be unit-tested without a browser.

## What We Test Today

Unit tests live in `test/`:

- `test/paneStore.test.ts`:
  - `addPane` / `removePane` / `togglePane` semantics
  - `normalizePaneRect` min size + NaN handling
- `test/resizablePane.test.ts`:
  - resize edge helpers (corner/affectsWidth/affectsHeight)
  - hit test parity with the Rust test cases in
    `crates/wgpui/src/components/hud/resizable_pane.rs`

## Running Tests

```bash
cd packages/effuse-panes
bun install
bun run typecheck
bun test
```

## What We Are Not Testing (Yet)

The DOM adapter is intentionally thin, but it is still easy to regress:

- pointer capture behavior (dragging outside bounds)
- title-bar hit testing vs close button exclusion
- resize edge behavior in the DOM coordinate system
- keydown routing (Escape / Cmd+1..9)

If/when we add DOM tests, options:

1. Add `happy-dom` and run adapter tests in a simulated DOM.
2. Add an `apps/web` storybook story and add visual/E2E coverage via `packages/effuse-test`.

Both approaches can be done without changing the core store/resizer tests.

Related host-level coverage exists in `apps/web` for Effect lifecycle wiring:

- `apps/web/tests/worker/pane-system-service.test.ts`
  - verifies mount path delegates to the DOM adapter
  - verifies release triggers destroy
  - verifies release is idempotent

## Suggested DOM Test Cases (Future)

These mirror the Rust `MinimalRoot` behavior:

- Dragging:
  - pointer down on title -> move -> pointer up
  - pane rect changes accordingly
  - active pane becomes dragged pane
- Resizing:
  - pointer down near right edge -> move -> pointer up
  - width increases and clamps to min size
- Canvas pan:
  - pointer down on background -> move -> pointer up
  - all panes offset
- Keyboard:
  - press Escape closes active dismissable pane
  - press Cmd+1 triggers `onHotbarSlotClick(1)`
