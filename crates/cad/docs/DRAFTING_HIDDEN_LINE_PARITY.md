# Drafting Hidden-Line Parity

Issue coverage: `VCAD-PARITY-069`

## Purpose

Lock hidden-line removal behavior to vcad-style occlusion semantics for
canonical cube drafting views.

## Parity Contracts

The parity manifest validates:

1. Front and top unit-cube views produce stable drafting edge sets with
   vcad-compatible bounds.
2. Isometric unit-cube view retains all 12 cube edges.
3. Dedicated occlusion probe scene contains hidden back-layer edges.
4. Occlusion classification is deterministic across repeated runs.
5. Full hidden-line parity snapshot replay is deterministic.

## Parity Evidence

- Reference corpus fixture:
  - `crates/cad/parity/fixtures/drafting_hidden_line_vcad_reference.json`
- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-drafting-hidden-line -- --check`
- Manifest fixture:
  - `crates/cad/parity/drafting_hidden_line_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_drafting_hidden_line --quiet`

## Failure Modes

- Missing hidden edges in front/top contract views fails parity.
- Isometric cube edge-count drift fails parity.
- Nondeterministic occlusion classification fails parity.
- Replay drift in hidden-line reports fails parity.
