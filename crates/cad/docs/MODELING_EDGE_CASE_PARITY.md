# Modeling Edge-Case Parity

Issue coverage: `VCAD-PARITY-039`

## Purpose

Capture deterministic parity fixtures for core modeling edge cases called out by vcad docs:

- coincident operand geometry
- tangent contact geometry
- seam topology behavior

## OpenAgents Edge-Case Fixtures

The parity lane builds and checks three deterministic snapshots:

1. Coincident intersection fixture
- staged boolean intersection for identical cubes
- stable stage sequence, diagnostics, and deterministic signature

2. Tangent intersection fixture
- staged boolean intersection for face-touching cubes
- stable empty-intersection diagnostics and deterministic signature

3. Seam topology fixture
- cylinder/sphere seam edge and degenerate-edge counts
- stable seam contract assertions for primitive topology

## Parity Evidence

- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-modeling-edge-cases -- --check`
- Manifest fixture:
  - `crates/cad/parity/modeling_edge_case_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_modeling_edge_cases --quiet`

## Failure Modes

- Any stage-order or diagnostic drift for coincident/tangent fixtures fails parity checks.
- Seam edge/degenerate edge contract drift fails seam topology assertions.
- Replay nondeterminism across identical edge-case inputs fails the deterministic replay gate.
