# OpenAgents CAD Capability Crawler

Issue coverage: `VCAD-PARITY-003`

## Purpose

Produce a deterministic OpenAgents CAD capability inventory at the parity baseline
starting commit:

- `04faa5227f077c419f1c5c52ddebbb7552838fd4`

The crawler captures:

- CAD docs surfaces (headings + checklist items from selected CAD docs)
- CAD crate/module surfaces (`openagents-cad` + `autopilot-desktop` CAD lanes)
- CAD command surfaces (`CadIntent` variants + OpenAgents CAD tool constants)

## Output Artifact

- `crates/cad/parity/openagents_capabilities_inventory.json`

## Commands

Generate/update inventory:

```bash
cargo run -p openagents-cad --bin openagents-capability-crawler --
```

Drift check against committed artifact:

```bash
scripts/cad/openagents-capability-crawler-ci.sh
```

## Test Coverage

- `crates/cad/tests/openagents_capability_crawler.rs`
  - fixture schema/invariant checks
  - live crawler parity against pinned baseline commit
- parser unit tests live in:
  - `crates/cad/src/parity/openagents_crawler.rs`
