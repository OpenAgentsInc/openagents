# Drafting GD&T Parity

Issue coverage: `VCAD-PARITY-071`

## Purpose

Lock GD&T annotation semantics to vcad-compatible symbol/material/datum contracts
for feature control frames and datum feature symbols.

## Parity Contracts

The parity manifest validates:

1. GD&T symbols preserve vcad-style DXF token mappings and datum requirements.
2. Material condition modifiers serialize with vcad-style markers.
3. Feature control frame text and render contracts (line/text counts) remain stable.
4. Datum feature symbol render contracts replay deterministically.

## Parity Evidence

- Reference corpus fixture:
  - `crates/cad/parity/fixtures/drafting_gdt_vcad_reference.json`
- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-drafting-gdt -- --check`
- Manifest fixture:
  - `crates/cad/parity/drafting_gdt_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_drafting_gdt --quiet`

## Failure Modes

- GD&T symbol token or datum-requirement drift fails parity.
- Material condition token drift fails parity.
- Feature control frame/datum render primitive count drift fails parity.
- Nondeterministic replay outputs fail parity.
