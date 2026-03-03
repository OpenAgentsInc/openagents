# CAD Convex Decomposition Parity

Issue coverage: `VCAD-PARITY-107`

## Goal

Lock deterministic parity contracts for CAD Convex Decomposition Parity in Phase I - Physics + URDF parity using the pinned vcad baseline.

## Contracts

- Capability scope parity is tracked for this issue ID and lane label.
- vcad source references used for this capability remain pinned and explicit.
- Generated parity manifests are deterministic across replay.

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/convex_decomposition_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/convex_decomposition_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-convex-decomposition-ci.sh
cargo run -p openagents-cad --bin parity-convex-decomposition
```
