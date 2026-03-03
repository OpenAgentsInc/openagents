# VCAD Capability Crawler

Issue coverage: `VCAD-PARITY-002`

## Purpose

Produce a deterministic inventory of `vcad` capabilities for parity planning, covering:

- feature-doc capabilities (`docs/features/index.md`, `docs/features/ROADMAP.md`)
- workspace crate capabilities (`Cargo.toml` members + package names)
- CLI command surface (`crates/vcad-cli/src/main.rs` command enum)

Pinned baseline commit:

- `1b59e7948efcdb848d8dba6848785d57aa310e81`

## Output Artifact

- `crates/cad/parity/vcad_capabilities_inventory.json`

The artifact is sorted and deterministic for the pinned commit.

## Commands

Generate/update inventory:

```bash
cargo run -p openagents-cad --bin vcad-capability-crawler -- \
  --vcad-repo "${VCAD_REPO:-$HOME/code/vcad}"
```

Drift check against committed artifact:

```bash
scripts/cad/vcad-capability-crawler-ci.sh
```

## Test Coverage

- `crates/cad/tests/vcad_capability_crawler.rs`
  - fixture schema/invariant checks
  - live crawl parity check against pinned commit when local `vcad` repo is available
- parser unit tests live in:
  - `crates/cad/src/parity/vcad_crawler.rs`
