# Drafting DXF Export Parity

Issue coverage: `VCAD-PARITY-076`

## Purpose

Lock drafting DXF export behavior to vcad-compatible contracts for projected-view
line output, visibility layer mapping, and deterministic R12 serialization.

## Parity Contracts

The parity manifest validates:

1. DXF output uses R12 header semantics (`$ACADVER=AC1009`, `$INSUNITS=4`).
2. Visible/hidden edges map to `VISIBLE/CONTINUOUS` and `HIDDEN/HIDDEN`.
3. Projected edge ordering emits deterministic `LINE` entities with six-decimal
   coordinate formatting.
4. Reference fixtures replay deterministically across repeated exports.

## Parity Evidence

- Reference corpus fixture:
  - `crates/cad/parity/fixtures/drafting_dxf_export_vcad_reference.json`
- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-drafting-dxf-export -- --check`
- Manifest fixture:
  - `crates/cad/parity/drafting_dxf_export_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_drafting_dxf_export --quiet`

## Failure Modes

- Header/tables serialization drifts from vcad R12 contract.
- Hidden-line edges serialize onto incorrect layer or linetype.
- Entity order or coordinate formatting drifts and changes deterministic hash.
- Export replay becomes nondeterministic across runs.
