# Chamfer Feature Graph Parity

Issue coverage: `VCAD-PARITY-031`

## Purpose

Lock production chamfer-op parity contracts for feature-graph paths and diagnostics (`chamfer.v2`).

## Implemented Chamfer Feature-Graph Contract Layer

- Hardened chamfer diagnostics in `crates/cad/src/finishing_ops.rs`:
  - added operation-specific fallback warning code mapping.
  - added chamfer fallback warning-code coverage (`CAD-WARN-CHAMFER-FAILED`).
- Added deterministic parity lane:
  - `crates/cad/src/parity/chamfer_feature_graph_parity.rs`
  - `crates/cad/src/bin/parity-chamfer-feature-graph.rs`
  - `crates/cad/tests/parity_chamfer_feature_graph.rs`
  - `scripts/cad/parity-chamfer-feature-graph-ci.sh`
  - `crates/cad/parity/chamfer_feature_graph_parity_manifest.json`

## Contracts Locked

- `ChamferFeatureOp::to_feature_node` and `from_feature_node` round-trip deterministically.
- edge refs are canonicalized for deterministic replay.
- planar-safe chamfer path applies deterministically for safe distances.
- fallback diagnostics map to `FINISHING_TOPOLOGY_RISK` deterministically.
- fallback warning code is stable: `CAD-WARN-CHAMFER-FAILED`.

## Parity Artifact

- `crates/cad/parity/chamfer_feature_graph_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-chamfer-feature-graph
scripts/cad/parity-chamfer-feature-graph-ci.sh
```
