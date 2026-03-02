# Shell Feature Graph Parity

Issue coverage: `VCAD-PARITY-029`

## Purpose

Lock production shell-op parity contracts at the feature-graph layer (`shell.v1`) with deterministic node serialization, replay semantics, and diagnostics.

## Implemented Shell Feature-Graph Contract Layer

- Hardened shell feature-op behavior in `crates/cad/src/finishing_ops.rs`:
  - shell validation now supports empty `remove_face_refs` for closed-shell flows.
  - finishing geometry hashes include deterministic selection signatures.
  - shell hashes now vary deterministically with `remove_face_refs` selection.
- Added deterministic parity lane:
  - `crates/cad/src/parity/shell_feature_graph_parity.rs`
  - `crates/cad/src/bin/parity-shell-feature-graph.rs`
  - `crates/cad/tests/parity_shell_feature_graph.rs`
  - `scripts/cad/parity-shell-feature-graph-ci.sh`
  - `crates/cad/parity/shell_feature_graph_parity_manifest.json`

## Contracts Locked

- `ShellFeatureOp::to_feature_node` and `from_feature_node` round-trip deterministically.
- remove-face refs are canonicalized for node storage/replay.
- empty remove-face sets are accepted for closed-shell production paths.
- applied shell hashes include selection signature (selection changes hash).
- fallback shell behavior maps to `FINISHING_ZERO_THICKNESS_RISK` deterministically.

## Parity Artifact

- `crates/cad/parity/shell_feature_graph_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-shell-feature-graph
scripts/cad/parity-shell-feature-graph-ci.sh
```
