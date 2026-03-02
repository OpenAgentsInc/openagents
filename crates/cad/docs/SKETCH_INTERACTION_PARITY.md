# Sketch Interaction Parity

Issue coverage: `VCAD-PARITY-052`

## Purpose

Align sketch interaction behavior with vcad keyboard shortcut and editing-flow
semantics for deterministic replay.

## OpenAgents Sketch Interaction Contract

- `crates/cad/src/sketch_interaction.rs` defines deterministic sketch
  interaction state transitions.
- Supported parity shortcuts:
  - `S` enter sketch mode
  - `L` line tool
  - `R` rectangle tool
  - `C` circle tool
  - `H` horizontal constraint shortcut
  - `V` vertical constraint shortcut
  - `Enter` finish current shape
  - `Escape` cancel/exit progression
- Entry flow parity:
  - if parts exist, `S` enters face-selection mode first
  - otherwise `S` enters XY sketch mode directly
- Escape/editing flow parity:
  - cancels in-progress shape first
  - then requests exit confirmation for non-empty sketches
  - supports deterministic keep-editing and confirm-exit transitions
- Constraint shortcuts only apply when selection cardinality matches vcad
  behavior and emit deterministic solver-run commands.

## Parity Evidence

- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-sketch-interaction -- --check`
- Manifest fixture:
  - `crates/cad/parity/sketch_interaction_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_sketch_interaction --quiet`

## Failure Modes

- Shortcut mapping drift fails fixture equivalence.
- Exit-flow transition regressions fail deterministic replay checks.
- Constraint shortcut selection-rule regressions fail parity checks.
