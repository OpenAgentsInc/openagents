# Drafting PDF Export Parity

Issue coverage: `VCAD-PARITY-077`

## Purpose

Lock drawing PDF-export semantics to the pinned vcad baseline, where CAD-core
PDF export is not available and drawing PDF output is routed through desktop or
browser print flow.

## Parity Contracts

The parity manifest validates:

1. CAD-core drawing PDF export returns `CadError::ExportFailed` with format
   `pdf`.
2. Error reason matches the baseline parity contract that native PDF export is
   unavailable in vcad core.
3. Unsupported-export behavior is deterministic across repeated calls and input
   projected-view variations.

## Parity Evidence

- Reference corpus fixture:
  - `crates/cad/parity/fixtures/drafting_pdf_export_vcad_reference.json`
- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-drafting-pdf-export -- --check`
- Manifest fixture:
  - `crates/cad/parity/drafting_pdf_export_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_drafting_pdf_export --quiet`

## Failure Modes

- Exporter returns a success payload instead of parity-baseline unsupported
  error.
- Export error format/reason drifts from the vcad baseline contract.
- Unsupported-export behavior becomes nondeterministic across calls.
