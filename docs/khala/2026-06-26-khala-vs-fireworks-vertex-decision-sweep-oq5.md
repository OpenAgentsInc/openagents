# Khala vs Fireworks/Vertex — Open Question #5 decision sweep (#6307)

> Status: **harness + Khala-side runnable now; owner-armed real spendful run is
> owner-gated.** This is the report scaffold + owner-arm runbook for the FIRST
> `decisionGrade: true` Khala-vs-Fireworks/Vertex report. It produces no public
> claim until the owner arms the spendful comparators and the real report is run.

Master: GTM #6303 (§4), roadmap Phase 4. GLM serving/throughput is tracked
separately (#6253, #6312) — this issue is the **quality ladder**, not throughput.

## What this is

The minimum lane decision suite (book P1-5 §4.5 / notes Open Question #5):
**Khala vs Fireworks vs Vertex on chat / khala-code / verifier / long-context**,
run for real over **realistic** observed Khala traffic, scored on the
decision-grade metrics (P50/P90/P99 latency, cost-per-accepted-outcome,
verification rate, cache hit rate). A fixture/synthetic run is illustrative only;
a `decisionGrade: true` report requires the owner-armed real seam over realistic
traffic.

## What is built (this change)

All NEW files under `apps/openagents.com/workers/api/src/inference/benchmark/`,
built ON TOP of the already-merged harness (matrix, fixture/real seam gate,
preflight, public-safe report) — nothing in the existing harness was rewritten:

- `real-lane-executor.ts` — the owner-armed `RealLaneExecutor` mapping layer: it
  turns a measured live provider exchange into the canonical `BenchmarkLaneSample`
  the existing runner records. Holds **no credentials and does no IO**; the
  transport is injected. Tags the sweep's own Khala load
  `demand_kind=internal`, `demand_source=benchmark_real_sweep` (#6298). A lane
  with no armed transport is a **typed refusal**, never a fabricated number.
- `real-lane-transports.ts` — the concrete OpenAI-compatible lane transports (the
  only place credentials/IO live, all injected `fetch` + clock):
  - `makeKhalaPublicTransport` → `https://openagents.com/api/v1/chat/completions`,
    model `openagents/khala`. **Own-capacity / no third-party cost** — runs now.
  - `makeOpenAICompatibleTransport` → generic builder the owner uses for Fireworks
    and Vertex (OpenAI-compatible) with their base URL + key + per-1k rate card.
    **Billable** — dark until armed.
- `real-sweep-config.ts` — `KHALA_VS_FIREWORKS_VERTEX_DECISION_SUITE` (Khala +
  Fireworks + Vertex Anthropic + Vertex Gemini × 4 workloads × 4 realistic
  shapes, streaming, 5 samples/cell) and `KHALA_ONLY_DECISION_SLICE` (Khala only,
  runs now). Every shape is `realistic` with a public-safe
  `observedTrafficEvidenceRef` so the preflight clears the realistic-traffic gate.
- `real-sweep-runner.ts` — `runRealSweep(...)`: awaits each live transport per
  cell, assembles the canonical telemetry run set, and refuses to start unless
  `preflightRealBenchmarkSweep` clears the owner gate. Un-armed lanes are skipped
  (honest absence). The report's `decisionGrade` rule is unchanged: it is `true`
  only when a **billable comparator** actually ran over realistic-only traffic.
- `khala-decision-sweep.ts` (script) — the runnable entrypoint (NOT in CI). No
  token → prints the NEEDS-OWNER arm steps and exits 0. Token only → runs the
  Khala-only slice (no spend). Owner-arm env + keys → runs the spendful real
  sweep and emits the `decisionGrade: true` report.

Tests: `real-sweep-runner.test.ts` (11 new) — preflight eligibility, #6298
attribution, the Khala transport (fake fetch), billable cost-basis, the gating
(refuses unarmed), the decision-grade rule (Khala-only = not decision-grade;
Khala + billable over realistic = `decisionGrade:true`), un-armed-lane skip, and
public-safety on a real-lane report. Full `benchmark/` suite: 70 pass.

## Honesty / spend discipline (held)

- The default path is provably spend-free: the real seam refuses to spend unarmed
  (existing `RealLaneNotArmedError`), and `runRealSweep` refuses to start without a
  green preflight (`RealSweepNotArmedError`).
- A Khala-only run is honestly `decisionGrade: false` (no billable comparator).
- No synthetic shape is published as a measurement; the report flags any
  synthetic-only group and stays out of decision-grade.
- The sweep's own Khala inference is tagged internal + segmented (#6298) and
  still counts in the public all-demand counter when Khala serves tokens.

## NEEDS-OWNER: arm the real spendful sweep

The Khala side runs now at no third-party cost. The first `decisionGrade:true`
cross-provider report needs the owner to arm the billable comparators:

1. **Credentials** (in `~/work/.secrets/`, never committed):
   - `OPENAGENTS_AGENT_TOKEN` — Khala `/api/v1` caller (no third-party cost).
   - `FIREWORKS_API_KEY` (`~/work/.secrets/fireworks.env`) — **billable**.
   - `VERTEX_API_BASE_URL` + `VERTEX_API_KEY` (OpenAI-compatible Vertex endpoint),
     or a Vertex SA + region wired as an OpenAI-compatible transport — **billable**.

2. **Explicit arm env** (the preflight blocks the spend without ALL of these):
   - `OA_BENCH_OWNER_CONFIRM=1`
   - `OA_BENCH_OWNER_APPROVAL_REF="<public-safe owner approval ref>"`
   - `OA_BENCH_BUDGET_CAP_MSAT=<positive msat cap>`
   - `OA_BENCH_MAX_BILLABLE_SAMPLES=<>= 320 per armed billable lane>`
     (suite expands to 4 workloads × 4 shapes × 5 samples × 1 streaming × 1
     sampling = **320 billable samples per billable lane**).

3. **Refresh realistic traffic shapes** from the live `token_usage_events` ledger
   so each shape's evidence ref + observed request count reflect **current** Khala
   traffic. The shapes shipped in `real-sweep-config.ts` are seeded from the
   2026-06-25 observed export — confirm/replace them before the spendful run.

4. **Run:**
   ```sh
   bun run apps/openagents.com/workers/api/scripts/khala-decision-sweep.ts
   ```
   With all of the above set it preflights, runs Khala + Fireworks + Vertex over
   the realistic shapes, and emits the public-safe `decisionGrade:true` report.
   Publish that report only when both `report.decisionGrade === true` and
   `checkReportPublicSafety(report).safe === true`.

## Report scaffold (filled by the owner-armed run)

The emitted report is `openagents.khala.benchmark-report.v1` with, per
(lane × workload) group: `ttftMs`/`totalWallClockMs`/`perceivedTps`/
`interTokenLatencyMs` (P50/P90/P99/mean), `cacheHitRate`, `verificationRate`,
`costPerAcceptedOutcomeMsat`, and the routing recommendation. `decisionGrade`,
`illustrativeNotice`, and per-group `syntheticOnly` carry the honesty state. No
prompt/account/price/margin ever appears (structural `checkReportPublicSafety`
tripwire).
