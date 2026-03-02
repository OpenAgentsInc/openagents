# Drafting Projection Parity

Issue coverage: `VCAD-PARITY-068`

## Purpose

Lock orthographic/isometric projection behavior to vcad semantics for
front/back/top/bottom/right/left/isometric drafting views.

## Parity Contracts

The parity manifest validates:

1. Canonical projection outputs for sample point `(1,2,3)` match vcad
   reference coordinates/depth per view direction.
2. View matrices remain orthonormal for all seven drafting directions.
3. Standard isometric projection keeps +Z mapped to positive 2D Y.
4. Projection computations replay deterministically.

## Parity Evidence

- Reference corpus fixture:
  - `crates/cad/parity/fixtures/drafting_projection_vcad_reference.json`
- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-drafting-projection -- --check`
- Manifest fixture:
  - `crates/cad/parity/drafting_projection_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_drafting_projection --quiet`

## Failure Modes

- Any view-direction projection drift beyond tolerance fails parity.
- Non-orthonormal view matrix components fail parity.
- Isometric +Z not projecting upward in 2D fails parity.
- Nondeterministic replay outputs fail parity.
