# Sweep Parity

Issue coverage: `VCAD-PARITY-033`

## Purpose

Lock deterministic parity contracts for sweep feature ops, including path sampling plus twist/scale controls.

## Implemented Sweep Contract Layer

- Added sweep feature-op support in `crates/cad/src/features/sweep.rs`:
  - `SweepFeatureOp`
  - `SweepFeatureStation`
  - `SweepFeatureResult`
  - `evaluate_sweep_feature`
- Contract alignment to vcad sweep baseline:
  - `path_segments=0` resolves to deterministic default `32`.
  - twist and scale controls are interpolated from start to end across sampled path stations.
  - zero-length paths and non-positive scales are rejected with stable CAD diagnostics.
- Added deterministic parity lane:
  - `crates/cad/src/parity/sweep_parity.rs`
  - `crates/cad/src/bin/parity-sweep.rs`
  - `crates/cad/tests/parity_sweep.rs`
  - `scripts/cad/parity-sweep-ci.sh`
  - `crates/cad/parity/sweep_parity_manifest.json`

## Contracts Locked

- sweep path controls produce deterministic station sampling and geometry hashing.
- auto segment policy is stable (`0` => `32`).
- explicit segment counts are honored deterministically.
- twist/scale interpolation remains deterministic across replay runs.
- invalid path and scale controls map to stable CAD errors.

## Parity Artifact

- `crates/cad/parity/sweep_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-sweep
scripts/cad/parity-sweep-ci.sh
```
