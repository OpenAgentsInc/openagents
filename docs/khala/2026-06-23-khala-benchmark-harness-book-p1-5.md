# Khala provider/engine benchmark harness — book P1-5 / #6088

_2026-06-23. This note ties the new typed Khala benchmark harness to the
inference-engineering book's P1-5 ("build a provider and engine benchmark
matrix") and Open Question #5 (the minimum lane decision suite). It is the
fixture-driven, no-spend foundation for an eventual owner-armed real sweep._

## What the book asked for (P1-5)

The book's Ch.4 §4.5 lesson is that **"faster" is meaningless until you say
faster at _what_** — TTFT vs inter-token latency / perceived TPS vs throughput vs
cost vs verification rate — on **which lane**, under **which traffic shape**,
judged on **which outcome**. A benchmark is therefore not an ad-hoc script with
hard-coded numbers; it is a **matrix** that varies the dimensions production
actually exhibits, and the **best benchmark shadows real production traffic**.
Synthetic traffic is useful only insofar as it matches the real input/output
lengths, prompt contents, cache behaviour, and concurrency the system sees
(§4.5: "if you're maximizing benchmark performance against bad inputs,
performance in production won't match expectations"). Latency must be read in
**percentiles** (Ch.1 §1.4.1) because the inference time distribution is
right-skewed — the mean hides the P90/P99 outliers that erode user trust.

P1-5 also folds in the notes' Open Question #5: _what is the minimum benchmark
suite for deciding between Fireworks, Vertex, Pylon whole-small, and later
shard-WAN lanes?_

## What shipped

A typed benchmark harness under
`apps/openagents.com/workers/api/src/inference/benchmark/`. It is pure,
framework-agnostic, deterministic, and reuses the merged P0-1 telemetry schema
(`openagents.khala.telemetry.v1`) rather than forking a parallel metric
vocabulary.

### 1. The matrix (`matrix.ts`)

A declarative, typed cross-product spec that varies exactly the book's
dimensions:

- **lane** — `vertex-anthropic`, `vertex-gemini`, `fireworks`,
  `partner-passthrough` (real today), plus `pylon-whole-small` and
  `psionic-shard-wan` (named but **not-yet-available** future lanes). A single
  `LANE_AVAILABILITY` table is the source of truth for which lanes are real;
  unbuilt lanes are still first-class matrix axes so the decision suite is shaped
  for them, but they are never measured.
- **engine** — `provider-native`, `vllm`, `sglang`, `tensorrt-llm`. Engines are
  _paired_ with lanes (not blindly crossed) so impossible cells (e.g. a managed
  provider on our own vLLM) are never fabricated.
- **workload** — `chat`, `khala-code-artifact-gen`, `verifier-run`,
  `long-context-codebase-question`.
- **sequence shape** — input length (ISL), output length (OSL), cacheable prefix
  length, and concurrency — each shape tagged `realistic` or `synthetic`.
- **streaming vs batch** transport, and **temperature / reasoning effort**.
- **verification outcome** — the expected verification class is _derived from the
  workload_ (chat → `none`; artifact-gen/verifier → executed `test_passed`;
  long-context → `seeded`), so every cell is scored on outcome, not just token
  speed.

`expandMatrix` deterministically expands a config into ordered cells with a
stable, axis-encoded `cellId`; `expectedCellCount` is the cross-product
cardinality used to assert coverage.

### 2. The runner + pluggable lane seam (`runner.ts`, `lane-seam.ts`)

The runner executes each cell against a pluggable **`BenchmarkLaneSeam`** and
records a canonical `KhalaTelemetryRecord` per sample (TTFT, tokens, wall-clock,
cached tokens, perceived TPS / ITL, verification class + executed verdict +
reward, cost basis). Two seams exist:

- **`makeFixtureLaneSeam` (default)** — fully deterministic, network-free,
  spend-free. It derives every sample arithmetically from the cell + a per-lane
  fixture profile (no clock, no randomness), so the same config always produces
  the same runs. A small index-keyed jitter spreads repeated samples so
  percentiles are non-degenerate. `canSpend: false` always.
- **`makeRealLaneSeam` (flag/owner-gated, default OFF)** — the seam that _would_
  hit a live provider adapter and measure a real, billable request. It is armed
  only by an explicit `armRealSweep: true` **and** an injected live executor;
  absent that it throws `RealLaneNotArmedError` and reports `canSpend: false`, so
  a test or an un-armed environment can **never** issue a billable request. This
  module implements the **gate**, not the live calls — the live provider sweep is
  the owner-armed work.

A **not-yet-available** lane is never executed against any seam: the runner emits
a skipped run with a `lane_not_yet_available:<lane>` reason and a null telemetry
record (honest absence, never a fabricated zero).

### 3. The dereferenceable report (`report.ts`)

`buildBenchmarkReport` aggregates a run set into per-`(lane × workload)` metrics —
the book's "best is product-specific, _measured_" framing as a typed artifact:

- **latency percentiles** (P50 / P90 / P99 + mean) for TTFT, total wall-clock,
  perceived TPS, and inter-token latency, over _measured_ samples only (the
  `not_measured` sentinel is dropped, never coerced to a number);
- **cost-per-accepted-outcome** (msat) = total cost basis / accepted outcomes —
  the only cost metric that respects verification: a cheap lane that fails
  verification is not cheap. A group with **zero** accepted outcomes has a
  **null** cost-per-outcome (a finding — money spent, nothing accepted — not a
  fake 0);
- **verification rate** = executed-passed / executed-attempted (null when the
  group has no verification, e.g. chat);
- **cache hit rate** = cached input tokens / prompt tokens (book P0-2).

The report is **public-safe**: it carries only token counts, durations, neutral
lane/engine/workload classifiers, the coarse region, and the aggregates — never a
prompt, completion, account ref, raw cache key, price, or margin. A structural
`checkReportPublicSafety` tripwire serializes the report and asserts no forbidden
key (`prompt`, `account`, `pricemsat`, `margin`, `secret`, …) ever appears.

### 4. Honesty — realistic traffic is required for the numbers to mean anything

The report header records the seam that produced it. A fixture-lane report is
`decisionGrade: false` and carries an `illustrativeNotice`: the numbers exercise
the harness and report math; they are **not** measurements of any real lane. A
report is decision-grade **only** when an owner-armed real seam ran over
**realistic** traffic (real input/output lengths, real cacheable prefixes, real
concurrency) **and** no group is synthetic-only. Even an owner-armed real seam
run over synthetic shapes stays out of decision-grade — real numbers need real
traffic. Each group is independently flagged `syntheticOnly`.

### Minimum decision suite (Open Question #5)

`fixtures.ts` ships `SAMPLE_DECISION_SUITE_CONFIG` — the minimum lane decision
suite shape: Fireworks vs Vertex-Anthropic on chat / khala-code / verifier /
long-context, with Pylon whole-small and Psionic shard-WAN as labeled future
lanes, over three synthetic shapes (short chat, large-prefix code artifact,
32k-context codebase question), both transports, two sampling settings, 5 samples
per cell (book §4.5.2: enough traffic to read percentiles, not be swayed by one
outlier). Running it through the fixture lane produces the first dereferenceable
report comparing Fireworks vs Vertex on chat + khala-code — **illustrative**
until real traffic and an owner-armed sweep replace the synthetic shapes.

## What is fixture-proven vs owner-gated

- **Fixture-proven (no spend, in CI):** the matrix expands to the expected cells;
  the fixture runner is deterministic and records canonical telemetry records;
  the report aggregation (percentiles, cost-per-accepted-outcome, verification
  rate, cache hit rate) is correct on a hand-checkable fixture set; the real-lane
  seam is flag-gated OFF by default and refuses to sample unarmed; the report is
  public-safe and labeled non-decision-grade for fixture/synthetic runs.
- **Owner-gated (real spend, NOT in this change):** arming `makeRealLaneSeam`
  with a live provider executor, sourcing **realistic** Khala traffic shapes from
  observed production traffic, and running the suite for real, billable numbers
  that earn `decisionGrade: true`.

## Owner step for a real sweep

1. Replace the synthetic `SequenceShape`s with shapes sourced from **real
   observed Khala traffic** (provenance `realistic`) and attach public-safe
   evidence refs for each shape.
2. Run `preflightRealBenchmarkSweep(...)` against the matrix with an explicit
   owner approval ref, positive msat budget cap, maximum billable sample cap, and
   observed-traffic evidence for every realistic shape. A synthetic matrix may be
   used for a real-lane smoke, but the preflight marks it **not decision-grade**
   until every shape is realistic and backed by actual Khala usage evidence.
3. Provide a live `RealLaneExecutor` that drives the production provider adapters
   and measures real samples.
4. Construct `makeRealLaneSeam({ armRealSweep: true, executor })` only after the
   preflight is green for arming (this is the only path that can spend), run the
   suite, and publish the now-`decisionGrade` report only when the preflight also
   says it is decision-grade eligible.

## Verification bar (green)

The inference test suites (742 tests, 39 new under `benchmark/`), `typecheck`,
`check:architecture`, `check:effect-topology`, and
`check:public-projection-freshness`.

## Scope / boundaries

New module under `workers/api/src/inference/benchmark/` only. No proof /
claim-upgrade, settlement, Psionic, Pylon-runtime, or autopilot-desktop changes;
no new routes; the telemetry schema is reused, never forked.
