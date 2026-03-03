# Drafting Drawing-Mode UI Parity

Issue coverage: `VCAD-PARITY-074`

## Purpose

Lock desktop drawing-mode UI behavior to vcad-compatible state contracts for mode
switching, view-direction resets, visibility toggles, zoom/pan bounds, and
detail-view lifecycle semantics.

## Parity Contracts

The parity manifest validates:

1. Drawing mode defaults to 3D/front with hidden lines and dimensions enabled.
2. Switching 2D view direction resets zoom and pan to defaults.
3. Hidden-line and dimension toggles update deterministically.
4. Zoom clamps to `0.1x..10x` and pan updates are deterministic.
5. Detail-view IDs are monotonic and clear does not reset the ID counter.

## Parity Evidence

- Reference corpus fixture:
  - `crates/cad/parity/fixtures/drafting_drawing_mode_ui_vcad_reference.json`
- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-drafting-drawing-mode-ui -- --check`
- Manifest fixture:
  - `crates/cad/parity/drafting_drawing_mode_ui_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_drafting_drawing_mode_ui --quiet`

## Failure Modes

- Default drawing mode or view-direction behavior drifts from vcad contracts.
- Zoom/pan clamp semantics drift and produce out-of-range values.
- Hidden-line or dimension toggle semantics drift.
- Detail-view ID generation/clear semantics become nondeterministic.
