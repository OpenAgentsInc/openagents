# Sketch Undo/Redo Parity

Issue coverage: `VCAD-PARITY-053`

## Purpose

Align sketch undo/redo behavior with vcad history semantics and deterministic
replay invariants.

## OpenAgents Sketch Undo/Redo Contract

- `crates/cad/src/sketch_interaction.rs` maps sketch transitions to typed
  `CadHistoryCommand::ApplySketchInteraction` history entries.
- `crates/cad/src/parity/sketch_undo_redo_parity.rs` records a deterministic
  sketch session and replays it through `CadHistoryStack`.
- Parity command bindings follow vcad keyboard behavior:
  - Undo: `Cmd/Ctrl+Z`
  - Redo: `Cmd/Ctrl+Shift+Z`
- History invariants:
  - trace order is exact reverse on undo and exact forward on redo
  - new edit after undo clears redo stack
  - history depth follows vcad parity limit of 50 steps

## Parity Evidence

- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-sketch-undo-redo -- --check`
- Manifest fixture:
  - `crates/cad/parity/sketch_undo_redo_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_sketch_undo_redo --quiet`

## Failure Modes

- History command mapping drift fails fixture equivalence.
- Undo/redo ordering regressions fail reverse/forward replay checks.
- Redo stack branch-clear regressions fail deterministic replay parity checks.
