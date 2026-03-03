# CAD CLI Commands Parity

Issue coverage: `VCAD-PARITY-084`

## Goal

Validate deterministic parity behavior for implemented CLI command paths (`export`, `import`, `info`) against vcad command-surface intent.

## Contracts

Case set:
- `export_stl`
- `export_glb`
- `export_step`
- `import_stl`
- `import_step`
- `info_mesh`

Expected command markers:
- `Exported STL to`
- `Exported GLB to`
- `Exported STEP to`
- `Imported STL to`
- `Imported STEP to`
- `openagents cad mesh:`

Additional data parity checks:
- `import_stl` reconstructs expected mesh counts:
  - vertices: `4`
  - triangles: `4`
- `import_step` produces at least one imported feature id.

## Parity Artifacts

- Reference corpus:
  - `crates/cad/parity/fixtures/cad_cli_commands_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/cad_cli_commands_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-cad-cli-commands-ci.sh
cargo run -p openagents-cad --bin openagents-cad-cli -- export --help
cargo run -p openagents-cad --bin openagents-cad-cli -- import --help
cargo run -p openagents-cad --bin openagents-cad-cli -- info --help
```
