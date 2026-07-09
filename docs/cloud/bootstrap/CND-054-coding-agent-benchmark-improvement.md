# CND-054 Coding Agent Benchmark Improvement

Status: implemented retained-regression improvement
Date: 2026-06-02

This note records the first concrete improvement pass for the OpenAgents coding
agent benchmark path.

## Goal

Improve the Codex-backed Benchmark Cloud runner without changing the base model.
The intended product path is:

```text
Vortex training-run API
-> Cloud Codex control assignment
-> Benchmark runner
-> Probe/Codex selected signatures
-> retained Terminal-Bench fixtures
-> measurable improvement signal
```

The improvement should be visible in a regression harness before any public
benchmark claim is made.

## Baseline

The existing runner already selected expected signatures for retained
Terminal-Bench failure fixtures, but the prompt addendum was thin:

```text
selected signature ids
failure fingerprints
required evidence
closeout artifacts
```

That helped inspectability, but it did not give Codex concrete task-family
procedure. The retained raw Codex rewards in the fixtures were all `0.0`.

## Change

`runners/py-bench-runner/openagents_bench/signature_routing.py` now includes
versioned playbooks for the retained failure families:

- `coding.service_readiness`
- `coding.python_package_index`
- `coding.query_optimizer_workflow`
- `coding.sqlite_wal_recovery`
- `coding.gcode_parser_guard`
- `coding.xss_sanitizer_policy`
- `benchmark.runner_supervisor`

For `probe-codex`, the prompt addendum now carries:

- selected signatures;
- failure fingerprints;
- required evidence;
- closeout artifacts;
- concrete playbook steps;
- evidence filenames;
- retained raw reward to expected signature reward target.

Raw `codex` still receives no signature addendum, preserving the ablation.

## SQLite/WAL Learned Rule

The strongest evidence-backed change is the SQLite/WAL rule from the preserved
SHC rerun:

```text
Before opening SQLite, copy the DB, WAL, and SHM files as a matched set.
Open only the copied DB.
Then run integrity/checkpoint/recovery on the copy.
```

The preserved account-backed rerun in `CND-053` records the first package
attempt at `0.0` reward and the revised package at `1.0` reward. This rule is
now present in both:

- the Python benchmark runner prompt addendum for `coding.sqlite_wal_recovery`;
- the Rust `oa-codex-control` prompt generated from a Vortex
  `openagents.training_run_assignment.v1` assignment carrying
  `probe.signature.db-wal-recovery`.

## Vortex API Path

The Vortex API contract checked here is `/api/training-runs/start`, which is
the route that creates a training run, child Codex run, benchmark run/task
records, variants, selected-signature state, and learning events.

Verification command:

```bash
cd /Users/christopherdavid/work/vortex
npm run test -- server/trainingRunRoutes.test.ts
```

Result:

```text
1 test file passed
11 tests passed
```

This validates the API route contract without mutating the dirty Vortex
worktree.

## Improvement Measurement

New evaluator:

```bash
cd runners/py-bench-runner
python3 -m openagents_bench.evaluate_signatures --fixture-dir fixtures/signature-routing
```

Observed result:

```text
retained fixtures: 7
improved: 7
raw mean: 0.000
probe+signature mean: 0.900
delta: +0.900
```

Per fixture:

| Task | Raw Codex | Probe+signature target | Delta | Signature |
| --- | ---: | ---: | ---: | --- |
| `configure-git-webserver` | 0.000 | 1.000 | +1.000 | `coding.service_readiness` |
| `db-wal-recovery` | 0.000 | 1.000 | +1.000 | `coding.sqlite_wal_recovery` |
| `filter-js-from-html` | 0.000 | 1.000 | +1.000 | `coding.xss_sanitizer_policy` |
| `gcode-to-text` | 0.000 | 0.500 | +0.500 | `coding.gcode_parser_guard` |
| `pypi-server` | 0.000 | 1.000 | +1.000 | `coding.python_package_index` |
| `query-optimize` runner stall | 0.000 | 1.000 | +1.000 | `benchmark.runner_supervisor` |
| `query-optimize` workflow | 0.000 | 0.800 | +0.800 | `coding.query_optimizer_workflow` |

This is an internal retained-fixture score, not a public Terminal-Bench
leaderboard result.

## Verification

Commands run:

```bash
cd runners/py-bench-runner
python3 -m unittest discover -s tests -v
python3 -m openagents_bench.evaluate_signatures --fixture-dir fixtures/signature-routing --json

cd /Users/christopherdavid/work/cloud
cargo test -p oa-codex-control

cd /Users/christopherdavid/work/vortex
npm run test -- server/trainingRunRoutes.test.ts
```

Results:

- Python benchmark runner: `22` tests passed.
- Retained fixture evaluator: `+0.900` expected mean reward delta.
- `oa-codex-control`: `16` tests passed.
- Vortex training-run route API tests: `11` tests passed.

## Caveats

- The `+0.900` result is a retained-regression expected reward metric, not a
  fresh live Terminal-Bench run.
- Only `db-wal-recovery` has preserved account-backed evidence for an actual
  `0.0 -> 1.0` rerun after playbook revision.
- The next live benchmark step is to rerun the remaining retained failures
  through the Vortex training-run API and SHC/GCP runner, preserving artifacts
  and public-safe proof bundles.
