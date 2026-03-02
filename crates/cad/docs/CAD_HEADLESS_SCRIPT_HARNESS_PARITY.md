# CAD Headless Script Harness Parity

Issue coverage: `VCAD-PARITY-091`

## Goal

Validate deterministic headless workflow scripting parity for CLI/MCP pipelines aligned to vcad scripting workflows.

Reference sources:

- `~/code/vcad/docs/features/headless-api.md`
- `~/code/vcad/packages/docs/content/tutorials/cli/04-scripting.mdx`

## OpenAgents Parity Surface

- Harness module: `crates/cad/src/headless_script_harness.rs`
- Parity builder: `crates/cad/src/parity/headless_script_harness_parity.rs`
- CLI parity generator: `crates/cad/src/bin/parity-headless-script-harness.rs`
- Fixture: `crates/cad/parity/fixtures/headless_script_harness_vcad_reference.json`
- Manifest: `crates/cad/parity/headless_script_harness_parity_manifest.json`

## Contracts Enforced

- Canonical CLI headless workflow script executes deterministic import/info/export chains.
- Canonical MCP headless workflow script executes deterministic create/inspect/export chain.
- Fail-fast script halts and skips trailing steps after first failure.
- Script report snapshots replay deterministically.

## Commands

Generate/refresh manifest:

```bash
cargo run -p openagents-cad --bin parity-headless-script-harness
```

Check manifest lock:

```bash
cargo run -p openagents-cad --bin parity-headless-script-harness -- --check
```

CI lane:

```bash
scripts/cad/parity-headless-script-harness-ci.sh
```
