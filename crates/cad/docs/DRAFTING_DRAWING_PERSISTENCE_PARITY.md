# Drafting Drawing Persistence Parity

Issue coverage: `VCAD-PARITY-075`

## Purpose

Lock CAD document schema persistence for drawing-mode state to vcad-compatible
contracts so drawing settings survive save/load without ambiguity.

## Parity Contracts

The parity manifest validates:

1. `CadDocument` includes optional `drawing` payload in schema.
2. Drawing payload uses vcad-compatible camelCase keys (`viewMode`,
   `viewDirection`, `showHiddenLines`, `showDimensions`, `detailViews`,
   `nextDetailId`, `centerX`, `centerY`).
3. Persisted drawing defaults match vcad drawing-store defaults.
4. Drawing persistence fixture replay is deterministic.

## Parity Evidence

- Reference corpus fixture:
  - `crates/cad/parity/fixtures/drafting_persistence_vcad_reference.json`
- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-drafting-persistence -- --check`
- Manifest fixture:
  - `crates/cad/parity/drafting_persistence_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_drafting_persistence --quiet`

## Failure Modes

- Drawing schema keys drift from expected vcad-compatible names.
- Default drawing values drift from vcad baseline contracts.
- Drawing pan/detail payload shape changes without parity fixture updates.
- Serialization replay becomes nondeterministic.
