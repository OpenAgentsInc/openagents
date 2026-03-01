# CAD Eval Scheduler

`crates/cad/src/eval.rs` now includes deterministic feature-graph rebuild scheduling for Wave 1.

## Deterministic Rebuild Contract

- Input: `FeatureGraph`
- Ordering: stable topological order from `FeatureGraph::deterministic_topo_order()`
- Per-feature record:
  - `feature_id`
  - `operation_key`
  - deterministic dependency hash list
  - deterministic parameter fingerprint
  - deterministic `geometry_hash`
- Aggregate:
  - `feature_hashes` map by feature id
  - stable `rebuild_hash` derived from ordered `(feature_id, geometry_hash)` pairs

## Observability

- `DeterministicRebuildResult::receipt()` emits:
  - ordered feature IDs
  - rebuild hash
  - feature count

This receipt is intended for CAD pane telemetry and activity logging.

## Parameter Invalidation

`compute_parameter_invalidation_plan(...)` computes deterministic pruning after parameter edits:

- detects directly affected features from parameter bindings in `FeatureNode.params`
- propagates invalidation strictly downstream through dependency edges
- returns:
  - ordered invalidated feature IDs
  - directly affected feature IDs
  - retained upstream hashes from previous rebuild output

This allows incremental rebuild paths to avoid recomputing unaffected upstream nodes.

## Verification

- `cargo test -p openagents-cad`
