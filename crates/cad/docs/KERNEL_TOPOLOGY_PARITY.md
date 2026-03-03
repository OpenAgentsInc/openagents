# Kernel Topology Parity

Issue coverage: `VCAD-PARITY-013`

## Purpose

Integrate a vcad-aligned half-edge topology model into `openagents-cad` with deterministic IDs,
entity links, and validation behavior.

## Implemented Surface

`crates/cad/src/kernel_topology.rs` now provides:

- typed IDs: `VertexId`, `HalfEdgeId`, `EdgeId`, `LoopId`, `FaceId`, `ShellId`, `SolidId`
- entities: `Vertex`, `HalfEdge`, `Edge`, `Loop`, `Face`, `Shell`, `Solid`
- enums: `Orientation`, `ShellType`
- `Topology` store with deterministic insertion order and topology mutation helpers:
  - `add_vertex`, `add_half_edge`, `add_edge`, `add_loop`
  - `add_face`, `add_inner_loop`, `add_shell`, `add_solid`
  - `validate_loop_ring`, `counts`

## Parity Artifact

- `crates/cad/parity/kernel_topology_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-kernel-topology
scripts/cad/parity-kernel-topology-ci.sh
```

## Determinism Contract

- sample manifest captures stable topology counts and invariants for a reference loop/face/shell/solid build.
- `crates/cad/tests/parity_kernel_topology.rs` verifies fixture equivalence.
- lane is integrated into `scripts/cad/parity_check.sh`.
