# vcad-eval Receipts Parity

Issue coverage: `VCAD-PARITY-037`

## Purpose

Align OpenAgents rebuild receipts with vcad-eval timing semantics so parity fixtures can assert:

- vcad-style timing envelope fields are present (`total_ms`, `parse_ms`, `serialize_ms`, `tessellate_ms`, `clash_ms`, `assembly_ms`)
- per-node timing payloads are keyed by feature id and include `op`, `eval_ms`, and `mesh_ms`
- receipt totals are deterministic and internally self-consistent

## OpenAgents Contract

`DeterministicRebuildResult::receipt()` now emits `DeterministicRebuildReceipt` with:

- existing rebuild identity (`ordered_feature_ids`, `rebuild_hash`, `feature_count`)
- `vcad_eval_timing` envelope:
  - `total_ms`
  - `parse_ms` (`None` in native deterministic lane)
  - `serialize_ms` (`None` in native deterministic lane)
  - `tessellate_ms`
  - `clash_ms`
  - `assembly_ms`
  - `nodes: BTreeMap<feature_id, VcadEvalNodeTiming>`

Timing values are deterministic synthetic timings derived from stable rebuild records and hashes,
which preserves replay determinism while matching vcad-eval receipt field behavior.

## Parity Evidence

- Manifest generator:
  - `cargo run -p openagents-cad --bin parity-vcad-eval-receipts -- --check`
- Manifest fixture:
  - `crates/cad/parity/vcad_eval_receipts_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad parity_vcad_eval_receipts --quiet`

## Failure Modes

- Missing dependency hashes in feature graphs emit stable eval errors.
- Timing envelope drift (field mismatch or total inconsistency) fails parity fixture checks.
- Any replay mismatch for identical graphs fails the deterministic replay contract.
