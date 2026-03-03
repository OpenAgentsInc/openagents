# CAD URDF Export Fixtures Parity

Issue coverage: `VCAD-PARITY-114`

## Goal

Lock deterministic parity contracts for CAD URDF Export Fixtures Parity in Phase I - Physics + URDF parity using the pinned vcad baseline.

## Contracts

- Capability scope parity is tracked for this issue ID and lane label.
- vcad source references used for this capability remain pinned and explicit.
- Generated parity manifests are deterministic across replay.

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/urdf_export_fixtures_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/urdf_export_fixtures_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-urdf-export-fixtures-ci.sh
cargo run -p openagents-cad --bin parity-urdf-export-fixtures
```
