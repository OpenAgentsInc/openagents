# Probe GEPA Validation Sweep

Date: 2026-06-08

Status: implemented for `OpenAgentsInc/openagents#4562`.

`benchmark-cloud` now has a selected SHC Terminal-Bench validation sweep for a
GEPA candidate. This is the post-Stage-1 validation lane. It uses public
Benchmark Cloud contracts, preserves Probe closeout bundles, records candidate
hash and Probe commit, and stays on the validation split. It does not use
holdout tasks and it does not make a public claim that Probe beats
Terminal-Bench.

## Sweep Shape

The sweep is built by `build_probe_gepa_validation_sweep`.

It records:

- campaign id: `probe-gepa-validation-sweep-2026-06-08`
- split manifest:
  `benchmark_split_manifest.terminal_bench_2.probe_gepa.stage_0_1.v1`
- six validation Terminal-Bench task refs:
  `db-wal-recovery`, `configure-git-webserver`, `pypi-server`,
  `filter-js-from-html`, `gcode-to-text`, and `query-optimize`
- three compared routes: current Probe champion, GEPA candidate, and baseline
  backend route
- eighteen rollout records
- Probe commit and GEPA candidate hash
- Probe closeout bundle refs
- verifier refs and verifier result refs
- artifact/proof/resource refs
- cost refs, duration records, and artifact availability flags

## Shadow Boundary

The candidate can move to shadow only if Omega and Blueprint gate refs are
present. The validator rejects shadow movement without both gate families.

The public claim is constrained to:

```text
validation measured only
```

The validator rejects any sweep that clears
`no_public_beats_terminal_bench_claim`.

## Run It Locally

Run:

```sh
cargo run -p benchmark-cloud --example probe_gepa_validation_sweep
```

The broader contract check also runs the sweep:

```sh
scripts/benchmarks/validate-benchmark-cloud-contracts.sh
```
