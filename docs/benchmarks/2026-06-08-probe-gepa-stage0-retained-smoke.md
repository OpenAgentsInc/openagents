# Probe GEPA Stage 0 Retained Smoke

Date: 2026-06-08

Status: implemented for `OpenAgentsInc/openagents#4560`.

`benchmark-cloud` now has a deterministic Stage 0 smoke campaign for Probe GEPA
retained-fixture optimization. This is not model training, not LoRA, not a
public leaderboard claim, and not a promotion path. It is a public-safe local
proof that the benchmark assignment, candidate, Probe closeout, Benchmark Cloud
proof, verifier import, and Pylon assignment-ref surfaces fit together.

## Campaign Shape

The campaign is built by `build_probe_gepa_stage0_smoke_campaign`.

It records:

- campaign id: `probe-gepa-stage0-retained-smoke-2026-06-08`
- split manifest:
  `benchmark_split_manifest.terminal_bench_2.probe_gepa.stage_0_1.v1`
- five retained Terminal-Bench fixture refs
- one baseline text-bundle candidate
- three mutated text-bundle candidates
- twenty metric-call records
- Probe assignment refs
- Probe closeout refs and closeout bundle refs
- Benchmark Cloud result, artifact manifest, proof bundle, and resource receipt
  refs
- verifier import refs
- Pylon assignment refs when Pylon-style distribution is represented
- accepted and rejected closeout states

The public status is constrained to
`public_status.probe_gepa.measured_retained_smoke.v1`.

## Run It Locally

Run:

```sh
cargo run -p benchmark-cloud --example probe_gepa_stage0_smoke
```

The example prints a JSON summary with candidate count, retained fixture count,
metric-call count, accepted/rejected closeout counts, Pylon assignment-ref
count, public status, promotion state, and the no-training/no-claim flags.

The broader contract check also runs this smoke:

```sh
scripts/benchmarks/validate-benchmark-cloud-contracts.sh
```

## Promotion Boundary

The validator rejects Stage 0 smoke campaigns that enable LoRA, model training,
public leaderboard claims, or automatic promotion. The only safe projection is
that the system measured retained smoke. Any validation, holdout, live, paid, or
promoted claim needs a later issue and an external release gate.
