# Drafting Checkpoint Parity

Issue coverage: `VCAD-PARITY-078`

## Purpose

Provide a deterministic Phase F checkpoint gate that verifies all drafting
parity manifests from `VCAD-PARITY-067` through `VCAD-PARITY-077` are present,
correctly labeled, and fully checked in the parity plan.

## Parity Contracts

The checkpoint manifest validates:

1. All required Phase F manifest files exist and expose expected `issue_id`
   values for `067..077`.
2. Corresponding Phase F plan entries are checked in
   `crates/cad/docs/VCAD_PARITY_PLAN.md`.
3. Phase F completion is exactly `100.0%`.
4. Checkpoint signature remains deterministic for identical inputs.

## Parity Evidence

- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-drafting-checkpoint -- --check`
- Manifest fixture:
  - `crates/cad/parity/drafting_checkpoint_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_drafting_checkpoint --quiet`

## Failure Modes

- Any Phase F manifest is missing.
- Manifest `issue_id` differs from expected sequence item.
- Phase F plan checklist items are not fully marked complete.
- Computed completion percent is less than `100.0`.
