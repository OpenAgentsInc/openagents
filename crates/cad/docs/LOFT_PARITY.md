# Loft Parity

Issue coverage: `VCAD-PARITY-034`

## Purpose

Lock deterministic parity contracts for loft feature ops, including multi-profile validation and closed-loft mode semantics.

## Implemented Loft Contract Layer

- Added loft feature-op support in `crates/cad/src/features/loft.rs`:
  - `LoftFeatureProfile`
  - `LoftFeatureOp`
  - `LoftFeatureResult`
  - `evaluate_loft_feature`
- Contract alignment to vcad loft baseline:
  - requires at least 2 profiles.
  - requires uniform vertex counts across profiles.
  - `closed=false` includes start/end caps; `closed=true` removes cap faces (tube semantics).
  - deterministic topology-count receipts (`transition_count`, `lateral_patch_count`, `cap_count`).
- Added deterministic parity lane:
  - `crates/cad/src/parity/loft_parity.rs`
  - `crates/cad/src/bin/parity-loft.rs`
  - `crates/cad/tests/parity_loft.rs`
  - `scripts/cad/parity-loft-ci.sh`
  - `crates/cad/parity/loft_parity_manifest.json`

## Contracts Locked

- loft profile order is deterministic and stable across replay.
- open loft uses `profile_count - 1` transitions and `2` cap faces.
- closed loft uses `profile_count` transitions and `0` cap faces.
- lateral patch count is deterministic: `transition_count * vertices_per_profile`.
- invalid profile-count and mismatched-segment inputs emit stable CAD diagnostics.

## Parity Artifact

- `crates/cad/parity/loft_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-loft
scripts/cad/parity-loft-ci.sh
```
