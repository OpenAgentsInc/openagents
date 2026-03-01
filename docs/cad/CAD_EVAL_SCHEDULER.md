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

## Verification

- `cargo test -p openagents-cad`
