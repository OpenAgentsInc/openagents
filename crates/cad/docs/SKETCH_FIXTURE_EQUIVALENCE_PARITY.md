# Sketch Fixture Equivalence Parity

Issue coverage: `VCAD-PARITY-054`

## Purpose

Validate sketch parity fixtures against a pinned vcad reference corpus across
the full sketch parity sequence (`VCAD-PARITY-041` through `VCAD-PARITY-053`).

## OpenAgents Sketch Fixture Equivalence Contract

- `crates/cad/parity/fixtures/sketch_vcad_reference_corpus.json` pins expected
  deterministic signatures and replay flags for sketch parity manifests.
- `crates/cad/src/parity/sketch_fixture_equivalence_parity.rs` compares current
  OpenAgents sketch manifests to corpus expectations:
  - deterministic signature equality
  - deterministic replay flag equality
  - repeated evaluation stability
- Any mismatch is surfaced explicitly as a parity fixture mismatch with case id,
  issue id, manifest path, and expected vs actual values.

## Parity Evidence

- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-sketch-fixture-equivalence -- --check`
- Reference corpus fixture:
  - `crates/cad/parity/fixtures/sketch_vcad_reference_corpus.json`
- Manifest fixture:
  - `crates/cad/parity/sketch_fixture_equivalence_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_sketch_fixture_equivalence --quiet`

## Failure Modes

- Signature drift in any sketch parity manifest fails corpus equivalence.
- Replay-flag drift in any sketch parity manifest fails corpus equivalence.
- Non-deterministic repeated evaluation fails fixture replay stability checks.
