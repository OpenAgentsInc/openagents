# VCAD Parity Baseline Manifests

Issue coverage: `VCAD-PARITY-001`

## Purpose

Freeze the exact parity reference baseline into machine-readable manifests so every
later parity issue compares against the same pinned source state.

Pinned commits:

- `vcad`: `1b59e7948efcdb848d8dba6848785d57aa310e81`
- `openagents` starting point: `04faa5227f077c419f1c5c52ddebbb7552838fd4`

## Manifest Artifacts

- `crates/cad/parity/vcad_reference_manifest.json`
- `crates/cad/parity/openagents_start_manifest.json`

Each manifest entry freezes:

- source file path
- SHA-256 digest
- byte length

## Regeneration and Drift Check

Generate/update manifests:

```bash
scripts/cad/freeze-parity-baseline.sh
```

Check committed manifests for drift from pinned commits:

```bash
scripts/cad/freeze-parity-baseline.sh --check
```

Use `VCAD_REPO` to override local vcad path if needed:

```bash
VCAD_REPO=/path/to/vcad scripts/cad/freeze-parity-baseline.sh --check
```

## Test Coverage

`crates/cad/tests/parity_baseline_manifests.rs` enforces:

- manifest schema and pinned-commit integrity
- deterministic ordering of frozen source paths
- digest/byte verification against pinned openagents commit
- digest/byte verification against pinned vcad commit when local vcad repo is present
