# Probe GEPA Stage 1 Retained-Failure Sprint

Date: 2026-06-08

Status: implemented for `OpenAgentsInc/openagents#4561`.

`benchmark-cloud` now has a deterministic Stage 1 retained-failure sprint for
Probe GEPA text-bundle optimization. This extends the Stage 0 smoke from a
small proof into a larger Pylon-style rollout batch while keeping the same
public-safe boundary: retained evidence only, no LoRA, no model training, no
public leaderboard claim, and no active production promotion.

## Sprint Shape

The sprint is built by `build_probe_gepa_stage1_retained_sprint`.

It records:

- campaign id: `probe-gepa-stage1-retained-failure-sprint-2026-06-08`
- split manifest:
  `benchmark_split_manifest.terminal_bench_2.probe_gepa.stage_0_1.v1`
- seven retained Terminal-Bench fixture refs
- eight Pylon worker assignment refs
- ten text-bundle candidates: baseline, champion, and eight mutations
- 210 metric-call records
- explicit `unpaid_smoke` payment mode on every rollout
- candidate hash, task ref, verifier ref, artifact ref, proof ref, resource
  ref, and failure classification ref where applicable
- accepted and rejected Pylon closeout states
- retained summaries for every candidate
- selected candidate decision:
  `optimizer_accepted`

The public summary is constrained to retained evidence only.

## Run It Locally

Run:

```sh
cargo run -p benchmark-cloud --example probe_gepa_stage1_retained_sprint
```

The broader contract check also runs the sprint:

```sh
scripts/benchmarks/validate-benchmark-cloud-contracts.sh
```

## Decision Boundary

The selected candidate can be `optimizer_accepted` or `rejected`; it cannot be
marked active by this sprint. Validation rejects missing payment modes, missing
rollout evidence refs, non-retained public summaries, model-training/LoRA
overclaims, and optimizer acceptance when the candidate fails retained
preservation or policy gates.
