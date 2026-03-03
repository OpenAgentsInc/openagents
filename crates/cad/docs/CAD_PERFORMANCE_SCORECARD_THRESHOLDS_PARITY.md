# CAD Performance Scorecard Thresholds Parity

Issue coverage: `VCAD-PARITY-134`

## Goal

Lock deterministic parity contracts for CAD Performance Scorecard Thresholds Parity in Phase K - Hardening + parity signoff using the pinned vcad baseline.

## Contracts

- Capability scope parity is tracked for this issue ID and lane label.
- vcad source references used for this capability remain pinned and explicit.
- Generated parity manifests are deterministic across replay.

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/performance_scorecard_thresholds_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/performance_scorecard_thresholds_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-performance-scorecard-thresholds-ci.sh
cargo run -p openagents-cad --bin parity-performance-scorecard-thresholds
```
