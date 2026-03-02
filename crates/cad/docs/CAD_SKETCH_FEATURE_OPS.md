# CAD Sketch Feature Ops (Wave 2 MVP)

This module converts constrained sketch profiles into deterministic feature-graph
operations for:

- `extrude`
- `cut`
- `revolve`
- `sweep`
- `loft`

Implementation:

- `crates/cad/src/sketch_feature_ops.rs`

## Operation Keys

- `sketch.extrude.v1`
- `sketch.cut.v1`
- `sketch.revolve.v1`
- `sketch.sweep.v1`
- `sketch.loft.v1`

## Conversion Contract

Entry point:

- `convert_sketch_profile_to_feature_node(sketch, spec)`

Behavior:

- Validates feature/profile ids and operation-specific parameters.
- Validates sweep path ids and path-control parameters (`twist/scale`) for sketch sweep conversion.
- Requires constraints to solve before conversion.
- Computes deterministic profile bounds and profile hash.
- Emits deterministic `FeatureNode` payload with canonical params.
- Emits warnings for risky profile conditions (open loops, partial revolve seam risk).

## History/Warn/Undo Participation

- History command mapping:
  - `history_command_for_sketch_feature(spec)` -> `CadHistoryCommand::ApplySketchFeature`
- `CadHistoryCommand` includes `ApplySketchFeature` with:
  - operation key
  - profile id
  - feature id
- Undo/redo coverage includes sketch-feature transitions with warning snapshots.

## Warnings

MVP warning mapping used during conversion:

- Open loop on `extrude`/`cut` -> `CAD-WARN-NON-MANIFOLD`
- Open loop on `sweep` -> `CAD-WARN-NON-MANIFOLD`
- Open loop on `loft` -> `CAD-WARN-NON-MANIFOLD`
- Partial `revolve` angle (< 360) -> `CAD-WARN-SLIVER-FACE` advisory

## Tests

- Constrained profile conversion generates deterministic extrude/cut/revolve/sweep/loft nodes.
- Reordered profile entity ids keep stable profile hash.
- Open profiles emit conversion warnings.
- Sketch feature history entries preserve warning snapshots across undo/redo.
