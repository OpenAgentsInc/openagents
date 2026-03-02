# Drafting Detail Parity

Issue coverage: `VCAD-PARITY-073`

## Purpose

Lock detail-view behavior to vcad-compatible clipping and magnification semantics
for projected drawing edges.

## Parity Contracts

The parity manifest validates:

1. Detail views clip parent edges to the selected detail region.
2. Detail transform recenters around the selection center and scales deterministically.
3. Visibility classifications are preserved through clipping/transform.
4. Detail-view outputs replay deterministically.

## Parity Evidence

- Reference corpus fixture:
  - `crates/cad/parity/fixtures/drafting_detail_vcad_reference.json`
- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-drafting-detail -- --check`
- Manifest fixture:
  - `crates/cad/parity/drafting_detail_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_drafting_detail --quiet`

## Failure Modes

- Detail edge count/visibility drift fails parity.
- Detail bounds or scaled edge-length drift fails parity.
- Nondeterministic replay outputs fail parity.
