# CAD CLI Scaffold Parity

Issue coverage: `VCAD-PARITY-083`

## Goal

Establish deterministic parity evidence that OpenAgents has the CLI command surface scaffold required before command behavior parity work in `VCAD-PARITY-084`.

## Contracts

- App command name is `openagents-cad-cli`.
- vcad reference command remains `vcad` (cross-reference baseline).
- Root help exits `0` and includes:
  - `USAGE:`
  - `COMMANDS:`
  - `export`, `import`, `info`
  - `VCAD-PARITY-083` and `VCAD-PARITY-084`
- For scaffold commands (`export`, `import`, `info`):
  - `<command> --help` exits `0`
  - `<command>` exits `3`
  - stderr equals:
    - `<command> command scaffold is present; implementation lands in VCAD-PARITY-084`

## Parity Artifacts

- Reference corpus:
  - `crates/cad/parity/fixtures/cad_cli_scaffold_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/cad_cli_scaffold_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-cad-cli-scaffold-ci.sh
cargo run -p openagents-cad --bin openagents-cad-cli -- --help
```
