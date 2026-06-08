# Probe GEPA Stage 0 Live Receipt Bundle

Date: 2026-06-08

Status: public `benchmark-cloud` contract for `OpenAgentsInc/openagents#4565`.

This document records the first campaign-level live receipt bundle for the
Probe GEPA path. It wraps the live SHC/Harbor smoke from
`2026-06-08-shc-harbor-probe-live-smoke.md` into one public-safe Stage 0
bundle that Psionic and Omega can import without treating the run as a public
Terminal-Bench score.

## Source Run

- host ref: `shc.oa_shc_katy_01`
- dataset: `terminal-bench@2.0`
- task: `terminal-bench/db-wal-recovery`
- Harbor job id: `e487217a-715e-448c-8d45-e528b76980e7`
- Harbor trial id: `a6c6c245-b9c0-44a8-a8c0-0c7fe5cc3383`
- normalized run ref:
  `benchmark_run.probe.shc_harbor.db_wal_recovery.20260608`
- status: failed
- normalized failure classification:
  `failure_classification.probe.shc_harbor.db_wal_recovery.nonzero_agent_exit`
- payment mode: `unpaid_smoke`
- public claim level: `none`

## Bundle Contract

The bundle schema is
`openagents.probe_gepa_stage0_live_receipt_bundle.v1`.

The builder is:

```sh
cargo run -p benchmark-cloud --example probe_gepa_stage0_live_receipt_bundle
```

The bundle preserves:

- live assignment IDs, including Probe assignment, Harbor job, and Harbor trial
  refs
- `probe-run-record.json`
- `probe-closeout.json`
- verifier and verifier-result refs
- artifact manifest refs
- proof bundle refs
- resource receipt refs, including the explicit unavailable-meter receipt
- resource unavailable reason
- route scorecard refs
- failure classification refs
- normalized event refs
- Psionic import refs

The required normalized artifact files remain:

- `result.json`
- `events.jsonl`
- `metadata.json`
- `artifact_manifest.json`
- `proof_bundle.json`
- `resource_usage_receipt.json`
- `route_scorecard.json`
- `probe-run-record.json`
- `probe-closeout.json`

## Claim Boundary

The bundle is live failure-closeout evidence only. It does not claim:

- Probe passed Terminal-Bench
- Probe has a public benchmark score
- GEPA improved a production candidate
- LoRA, Qwen, MLX, or any model training ran
- paid Pylon work happened
- settlement happened

Allowed public wording is `live smoke measured only`.

## Current Fit

This is the public receipt shape for the next network step:

1. Omega can attach a real or demo unpaid Pylon lease record to a Probe GEPA
   rollout.
2. Psionic can import the bundle into the same candidate frontier state as
   retained and validation runs.
3. Artanis can summarize the evidence through projection authority without
   claiming a public benchmark result.

GEPA distribution here is rollout optimization. Pylons run independent
candidate/task/verifier assignments and return receipts. This is not
distributed neural-network training.

## Verification

Run:

```sh
scripts/benchmarks/validate-benchmark-cloud-contracts.sh
```

The validator runs the live receipt bundle example in addition to the retained
Stage 0 smoke, Stage 1 retained sprint, validation sweep, observed runner, and
SHC Harbor live-smoke examples.
