# Expanded Finishing Parity

Issue coverage: `VCAD-PARITY-032`

## Purpose

Lock deterministic parity contracts for expanded fillet/chamfer constraints beyond planar-safe thresholds.

## Implemented Expanded Finishing Contract Layer

- Extended finishing op contracts in `crates/cad/src/finishing_ops.rs`:
  - added `FinishingConstraintMode` (`planar_safe`, `expanded`) for fillet/chamfer ops.
  - added deterministic node serialization/parsing for `constraint_mode`.
  - preserved backward compatibility: missing legacy `constraint_mode` defaults to `planar_safe`.
  - expanded-mode threshold policy allows larger deterministic fillet/chamfer values (1.5x planar-safe base threshold).
- Added deterministic parity lane:
  - `crates/cad/src/parity/expanded_finishing_parity.rs`
  - `crates/cad/src/bin/parity-expanded-finishing.rs`
  - `crates/cad/tests/parity_expanded_finishing.rs`
  - `scripts/cad/parity-expanded-finishing-ci.sh`
  - `crates/cad/parity/expanded_finishing_parity_manifest.json`

## Contracts Locked

- planar-safe mode rejects values beyond deterministic finishing risk thresholds.
- expanded mode accepts the same deterministic samples that planar-safe rejects.
- fillet/chamfer `constraint_mode` round-trips in feature-node serialization.
- legacy nodes missing `constraint_mode` parse as `planar_safe`.
- expanded-mode replay remains deterministic across repeated runs.

## Parity Artifact

- `crates/cad/parity/expanded_finishing_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-expanded-finishing
scripts/cad/parity-expanded-finishing-ci.sh
```
