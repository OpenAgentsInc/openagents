# Drafting Dimension Parity

Issue coverage: `VCAD-PARITY-070`

## Purpose

Lock linear/angular/radial/ordinate dimension semantics to vcad-compatible
measurement and render contracts.

## Parity Contracts

The parity manifest validates:

1. Linear dimensions match horizontal, vertical, aligned, and rotated
   measurement values.
2. Angular, radial (radius/diameter), and ordinate measurements match
   vcad reference values.
3. Render contracts (line/arc/text counts + primary labels) are stable.
4. Dimension parity snapshots replay deterministically.

## Parity Evidence

- Reference corpus fixture:
  - `crates/cad/parity/fixtures/drafting_dimension_vcad_reference.json`
- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-drafting-dimension -- --check`
- Manifest fixture:
  - `crates/cad/parity/drafting_dimension_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_drafting_dimension --quiet`

## Failure Modes

- Measurement value drift beyond tolerance fails parity.
- Render primitive/label drift fails parity.
- Nondeterministic replay outputs fail parity.
