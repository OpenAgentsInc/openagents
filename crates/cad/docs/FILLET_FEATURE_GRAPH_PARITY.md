# Fillet Feature Graph Parity

Issue coverage: `VCAD-PARITY-030`

## Purpose

Lock production fillet-op parity contracts for planar-safe feature-graph paths (`fillet.v2`).

## Implemented Fillet Feature-Graph Contract Layer

- Hardened fillet feature-op tests/contracts in `crates/cad/src/finishing_ops.rs`:
  - explicit fillet node round-trip canonicalization checks.
  - deterministic hash differentiation by edge-selection signature.
- Added deterministic parity lane:
  - `crates/cad/src/parity/fillet_feature_graph_parity.rs`
  - `crates/cad/src/bin/parity-fillet-feature-graph.rs`
  - `crates/cad/tests/parity_fillet_feature_graph.rs`
  - `scripts/cad/parity-fillet-feature-graph-ci.sh`
  - `crates/cad/parity/fillet_feature_graph_parity_manifest.json`

## Contracts Locked

- `FilletFeatureOp::to_feature_node` and `from_feature_node` round-trip deterministically.
- edge refs are canonicalized for deterministic replay.
- planar-safe fillet path applies deterministically for safe radius values.
- fallback path emits `FINISHING_TOPOLOGY_RISK` classification deterministically.
- applied fillet hashes include edge-selection signature.

## Parity Artifact

- `crates/cad/parity/fillet_feature_graph_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-fillet-feature-graph
scripts/cad/parity-fillet-feature-graph-ci.sh
```
