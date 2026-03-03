# STEP Import Entity Parity

Issue coverage: `VCAD-PARITY-079`

## Goal

Expand OpenAgents STEP import parity to vcad-supported entity families for the pinned baseline commit:

- `MANIFOLD_SOLID_BREP` and `FACETED_BREP`
- `CLOSED_SHELL` and `OPEN_SHELL`
- `FACE` and `ADVANCED_FACE`
- vcad topology/geometry dependencies used by import paths (`EDGE_*`, `VERTEX_*`, placements, line/circle/surface entities)

## Contracts

- Structural checker treats `OPEN_SHELL` as valid shell coverage.
- Structural checker counts both `FACE` and `ADVANCED_FACE`.
- STEP import emits deterministic entity coverage:
  - `supported_entity_types_present`
  - `unsupported_entity_types_present`
  - `entity_type_counts`
- Unsupported entities are surfaced deterministically but do not fail import by themselves.

## Parity Artifacts

- Reference corpus:
  - `crates/cad/parity/fixtures/step_import_entity_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/step_import_entity_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-step-import-entity-ci.sh
cargo test -p openagents-cad step_import --quiet
cargo test -p openagents-cad step_checker --quiet
```
