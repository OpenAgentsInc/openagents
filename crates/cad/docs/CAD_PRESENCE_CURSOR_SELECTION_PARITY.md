# CAD Presence Cursor Selection Parity

Issue coverage: `VCAD-PARITY-117`

## Goal

Lock deterministic parity contracts for CAD Presence Cursor Selection Parity in Phase J - Full workspace parity lanes using the pinned vcad baseline.

## Contracts

- Capability scope parity is tracked for this issue ID and lane label.
- vcad source references used for this capability remain pinned and explicit.
- Generated parity manifests are deterministic across replay.

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/presence_cursor_selection_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/presence_cursor_selection_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-presence-cursor-selection-ci.sh
cargo run -p openagents-cad --bin parity-presence-cursor-selection
```
