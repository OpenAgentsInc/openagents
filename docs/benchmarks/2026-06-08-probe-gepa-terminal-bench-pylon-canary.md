# Probe GEPA Terminal-Bench Pylon Canary

Date: 2026-06-08

Status: public `benchmark-cloud` contract for
`OpenAgentsInc/openagents#4566`.

This document records the first Probe GEPA Terminal-Bench 2 canary that uses a
live Omega Pylon assignment lifecycle. The Probe repo retains the closeout
bundle under `docs/benchmarks/canaries/20260608151057/`. This repo retains the
public Benchmark Cloud side of that evidence.

## Source Refs

- Pylon: `pylon.artanis.gepa_stats_canary.20260608150415`
- assignment:
  `assignment.public.probe_gepa.terminal_bench_2.canary.20260608151057`
- Probe run:
  `probe_run.public.probe_gepa.terminal_bench_2.canary.20260608151057`
- receipt:
  `receipt.public.probe_gepa.terminal_bench_2.canary.20260608151057`
- Psionic import:
  `psionic_import.public.probe_gepa.terminal_bench_2.canary.20260608151057`

The live assignment completed:

- heartbeat
- wallet readiness for unpaid smoke
- assignment offer
- worker acceptance
- progress
- artifact/proof submission
- operator accepted-work closeout

## Benchmark Cloud Contract

The bundle schema is
`openagents.probe_gepa_terminal_bench_pylon_canary_bundle.v1`.

The builder is:

```sh
cargo run -p benchmark-cloud --example probe_gepa_terminal_bench_pylon_canary
```

The bundle maps the Probe/Pylon task refs to canonical Benchmark Cloud split
refs:

- `benchmark_task.terminal_bench.retained.configure_git_webserver.v1`
- `benchmark_task.terminal_bench.retained.filter_js_from_html.v1`

The primary metric call is for `configure-git-webserver`. Its benchmark status
is `failed` with a retained `service_readiness` failure classification. Its
Pylon work closeout is `accepted`: the worker evidence bundle was accepted, but
the benchmark did not become a public pass or score.

The bundle preserves:

- Pylon assignment ref
- live Pylon event refs
- Probe closeout ref
- Probe closeout bundle ref
- artifact refs
- proof refs
- accepted-work refs
- closeout refs
- resource receipt ref
- verifier import and result refs
- failure classification ref
- Psionic import ref

It also records the full Probe closeout bundle file set:

- `probe-run-record.json`
- `probe-closeout.json`
- `decision-trace-summary.json`
- `selected-signatures.json`
- `tool-menu.json`
- `candidate-ref.json`
- `artifact-refs.json`
- `resource-usage-ref.json`
- `policy-findings.json`
- `failure-classification.json`
- `route-scorecard.json`

## Claim Boundary

Allowed public wording is `initial retained Pylon canary evidence only`.

The validator rejects any claim that this canary is:

- a public Terminal-Bench score
- paid work
- settled bitcoin
- LoRA or model training
- runtime promotion
- production activation

This canary is useful because it proves the live Pylon assignment/closeout path
can carry Probe GEPA Terminal-Bench evidence into public Benchmark Cloud and
then into Psionic review. It does not prove the whole network is ready for full
launch.

## Verification

Run:

```sh
scripts/benchmarks/validate-benchmark-cloud-contracts.sh
```

The validator runs the Pylon canary example in addition to the retained Stage
0 smoke, Stage 1 retained sprint, validation sweep, observed runner, SHC
Harbor live-smoke, and SHC live receipt examples.
