# Drafting Section Parity

Issue coverage: `VCAD-PARITY-072`

## Purpose

Lock section view behavior to vcad-compatible contracts for plane intersections,
segment chaining, section projection, and hatch generation.

## Parity Contracts

The parity manifest validates:

1. Plane intersection contracts for triangle and coplanar polyline inputs.
2. Deterministic open/closed section chaining behavior.
3. Section mesh output parity for with-hatch and no-hatch options.
4. Stable section-plane projection and hatch line generation contracts.

## Parity Evidence

- Reference corpus fixture:
  - `crates/cad/parity/fixtures/drafting_section_vcad_reference.json`
- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-drafting-section -- --check`
- Manifest fixture:
  - `crates/cad/parity/drafting_section_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_drafting_section --quiet`

## Failure Modes

- Intersection segment count/length drift fails parity.
- Open/closed chain semantics drift fails parity.
- Section mesh curve/hatch count drift fails parity.
- Projection or hatch scalar drift beyond tolerance fails parity.
