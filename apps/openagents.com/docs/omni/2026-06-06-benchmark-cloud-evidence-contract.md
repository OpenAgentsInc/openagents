# Benchmark Cloud Evidence Contract

Date: 2026-06-06

Issue: #383 / `OPENAGENTS-LAB-004`

Status: implemented as a read-only schema/projection contract in
`workers/api/src/omni-model-lab-benchmark-cloud.ts`.

## Purpose

Benchmark Cloud evidence records describe evaluation suites, tasks, eval jobs,
scorecards, regressions, flakes, and comparisons that can inform Model Lab and
Blueprint release gates.

The contract is evidence-only. It can prove that a candidate was evaluated,
that a scorecard passed or failed, that a regression blocks promotion, or that
a task is flaky. It cannot launch benchmark jobs, execute evals, mutate
provider state, copy raw benchmark inputs, spend money, promote runtime
behavior, mutate routing, pay out, settle, or upgrade public claims.

## Records

- `OmniBenchmarkSuiteRecord`: suite ref, task refs, policy refs, evidence
  refs, and caveats.
- `OmniBenchmarkTaskRecord`: task ref, suite refs, input class, dataset refs,
  expected-output refs, evidence refs, and caveats.
- `OmniBenchmarkEvalJobRecord`: eval job ref, suite/task refs, runner/provider
  refs, candidate/artifact/training/retained-failure refs, scorecard,
  regression, flake, comparison, evidence, receipt, caveat, and blocker refs.
- `OmniBenchmarkScorecardRecord`: eval refs, metric refs, pass threshold,
  observed score, evidence refs, receipt refs, state, and caveats.
- `OmniBenchmarkRegressionRecord`: source and baseline eval refs, affected
  tasks, severity, promotion-blocking flag, promotion gate refs, evidence, and
  caveats.
- `OmniBenchmarkFlakeRecord`: eval refs, task refs, flake rate, evidence, and
  caveats.
- `OmniBenchmarkComparisonRecord`: baseline and candidate artifact/eval refs,
  scorecards, regressions, evidence, and caveats.
- `OmniBenchmarkCloudRecord`: the aggregate evidence packet linking the
  records above to Model Lab candidates, artifacts, training runs, retained
  failures, and promotion gates.

## Validation Rules

- A packet requires at least one suite, task, eval job, and scorecard.
- Suite/task/eval/scorecard/regression/flake/comparison refs must be unique.
- Suites must reference tasks in the packet. Tasks must reference suites in the
  packet. Eval jobs must reference suites and tasks in the packet.
- Scorecards must reference eval jobs in the packet and use basis-point scores
  from `0` through `10000`.
- Passed scorecards require observed score at or above threshold and receipt
  refs.
- Failed scorecards must be below threshold.
- Passed eval jobs require scorecard, evidence, and receipt refs.
- Failed eval jobs require regression refs.
- Flaky eval jobs require flake refs.
- Running eval jobs require runner and provider refs.
- Promotion-blocking regressions require evidence, high or critical severity,
  and promotion gate refs.
- Failed aggregate packets require promotion-blocking regression evidence and
  blocker refs.
- Private handles can be present for team/operator projections, but raw
  benchmark inputs, raw datasets, raw provider payloads, raw logs, model
  weights, payment or wallet material, secrets, private repos, and raw
  timestamps are rejected.

## Projection

`projectOmniBenchmarkCloud(record, audience, nowIso)` returns a
`OmniBenchmarkCloudProjection` with:

- friendly time labels instead of raw timestamps,
- aggregate counts for suites, tasks, eval jobs, scorecards, regressions,
  flakes, and comparisons,
- pass/fail/flaky/promotion-blocked labels,
- audience-specific redaction for public, agent, customer, team, and operator
  projections,
- hard false authority booleans for benchmark launch, eval execution,
  provider mutation, raw-input copy, payment spend, runtime promotion, routing,
  payout, settlement, and public claim upgrade.

## Tests

Coverage lives in `workers/api/src/omni-model-lab-benchmark-cloud.test.ts`.
The tests cover:

- read-only projection of passing benchmark evidence,
- suite/task/eval/scorecard/comparison linkage,
- scorecard threshold validation,
- duplicate ref rejection,
- regression and flake state labels,
- promotion-blocking evidence requirements,
- public redaction of private benchmark refs,
- rejection of raw/private unsafe material and mutable authority.
