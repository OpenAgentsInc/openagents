# CAD CLI Scaffold

Issue coverage: `VCAD-PARITY-083`

## Scope

In scope:
- `openagents-cad-cli` command-surface scaffold for headless CAD workflows.
- Root help surface and subcommand help surfaces.
- Scaffold command IDs matching vcad headless command vocabulary baseline:
  - `export`
  - `import`
  - `info`
- Stable stub diagnostics that point to implementation issue `VCAD-PARITY-084`.

Out of scope:
- Command business behavior (`export`, `import`, `info` execution logic).
- File format handlers and geometry IO wiring (land in `VCAD-PARITY-084`).

## Command Surface

Binary entrypoint:
- `cargo run -p openagents-cad --bin openagents-cad-cli -- --help`

Scaffold behavior:
- `openagents-cad-cli --help` prints deterministic usage and command list.
- `openagents-cad-cli <command> --help` prints deterministic subcommand help.
- `openagents-cad-cli <command>` returns stub exit code `3` and error:
  - `<command> command scaffold is present; implementation lands in VCAD-PARITY-084`

Unknown command behavior:
- Exit code `2` with usage + unknown-command diagnostic.

## API / Implementation

`crates/cad/src/cli.rs`

- `run_cli_env_args(args) -> CadCliRunOutcome`
- `run_cli_tokens(tokens) -> CadCliRunOutcome`
- constants:
  - `CAD_CLI_APP_NAME`
  - `CAD_CLI_REFERENCE_COMMAND`
  - `CAD_CLI_SCAFFOLD_COMMANDS`
  - `CAD_CLI_STUB_EXIT_CODE`

`crates/cad/src/bin/openagents-cad-cli.rs`
- Thin binary wrapper over `run_cli_env_args`.
