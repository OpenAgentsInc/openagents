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

Desktop integration currently stores a per-cycle receipt summary in `CadDemoPaneState` and mirrors
it into the activity feed event stream (`source_tag = "cad.eval"`).
Rebuild compute is dispatched via a background worker and committed asynchronously.

## Parameter Invalidation

`compute_parameter_invalidation_plan(...)` computes deterministic pruning after parameter edits:

- detects directly affected features from parameter bindings in `FeatureNode.params`
- propagates invalidation strictly downstream through dependency edges
- returns:
  - ordered invalidated feature IDs
  - directly affected feature IDs
  - retained upstream hashes from previous rebuild output

This allows incremental rebuild paths to avoid recomputing unaffected upstream nodes.

## Eval Cache

`EvalCacheStore` provides deterministic per-feature caching keyed by:

- `document_revision`
- `feature_node_id`
- `params_hash`

Supporting contracts:

- `EvalCacheKey::from_feature_node(...)`
- `EvalCacheEntry { geometry_hash }`
- `EvalCacheStats { hits, misses, evictions }`

Behavior:

- LRU eviction when capacity is exceeded.
- Explicit hit/miss/eviction accounting for observability.

## Verification

- `cargo test -p openagents-cad`
