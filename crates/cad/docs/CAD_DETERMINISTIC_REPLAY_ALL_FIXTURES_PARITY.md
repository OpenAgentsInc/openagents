# CAD Deterministic Replay All Fixtures Parity

Issue coverage: `VCAD-PARITY-133`

## Goal

Lock deterministic parity contracts for CAD Deterministic Replay All Fixtures Parity in Phase K - Hardening + parity signoff using the pinned vcad baseline.

## Contracts

- Capability scope parity is tracked for this issue ID and lane label.
- vcad source references used for this capability remain pinned and explicit.
- Generated parity manifests are deterministic across replay.

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/deterministic_replay_all_fixtures_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/deterministic_replay_all_fixtures_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-deterministic-replay-all-fixtures-ci.sh
cargo run -p openagents-cad --bin parity-deterministic-replay-all-fixtures
```
