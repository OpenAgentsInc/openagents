# CAD Background Rebuild Worker

Desktop CAD pane rebuilds are queued to a background worker in:

- `apps/autopilot-desktop/src/cad_rebuild_worker.rs`
- `apps/autopilot-desktop/src/input/reducers/cad.rs`

## Flow

1. CAD action queues `CadRebuildRequest`.
2. Worker thread evaluates deterministic feature graph rebuild.
3. UI reducer drains ready responses and commits successful rebuilds.

## Last-Good Mesh Strategy

- While a request is pending, pane state remains usable and keeps `last_good_mesh_id`.
- On commit, `last_good_mesh_id` updates to the new rebuild hash-derived mesh token.
- Stale worker responses (older revision) are ignored to avoid regressions/flicker.

## Receipt/Event Integration

- Successful commits emit `CadRebuildReceiptState`.
- Receipts are stored in pane history and mirrored into activity events (`source_tag = cad.eval`).

## Verification

- `cargo test -p autopilot-desktop input::reducers::cad::tests -- --nocapture`
- `cargo test -p autopilot-desktop cad_rebuild_worker::tests -- --nocapture`
