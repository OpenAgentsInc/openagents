# Khala External Head-to-Head — Recurring Published Quality Bar (first run)

> **Status:** machinery shipped, 2026-06-26 (#6308, Khala GTM master #6303).
> This is the recurring, dereferenceable quality bar that compares
> `openagents/khala` against **the tools/models a developer would otherwise reach
> for** — not just our own supply lanes — on identical prompts and BOTH headline
> axes: **solve-rate** (verified-rate) AND **cost-per-accepted-outcome**. It
> generalizes the supply-lane decision sweep (#6307) and the gym ladder (#6309)
> into the comparison a developer actually cares about: "is Khala better than the
> thing I'd otherwise use?" The planning memo is
> `docs/opencode/khala-head-to-head-gym-final-output.md`; this doc records the
> shipped publishing machinery and the honest first state.

## What shipped

The head-to-head is the **publishing layer** on top of the already-shipped
benchmark harness (matrix / runner / report from #6088 + #6307) and the flat
`buildGymLeaderboardProjection` (#6309). It does **not** re-run or re-implement
the harness, the cost math, the verification math, the real-sweep owner-arm gate,
or the public-safety boundary — it consumes them.

| Surface | Path | Owner |
|---------|------|-------|
| Builder (pure) | `workers/api/src/inference/benchmark/head-to-head.ts` | comparator set, recurring config, `buildKhalaHeadToHead`, two-axis verdict |
| Publish store (D1) | `workers/api/src/inference/benchmark/head-to-head-store.ts` | `khala_head_to_head_snapshots` table, decode-on-read |
| Routes | `workers/api/src/inference/benchmark/head-to-head-routes.ts` | public GET + operator publish POST |
| Migration | `workers/api/migrations/0241_khala_head_to_head_snapshots.sql` | snapshot table |
| Public dereference | `GET /api/public/khala/head-to-head` | latest published bar (no auth) |
| Recurring publish | `POST /api/operator/khala/head-to-head` | admin-bearer; scheduler/operator publish boundary |

## The comparator set ("what a developer would otherwise reach for")

`KHALA_HEAD_TO_HEAD_COMPARATORS` in `head-to-head.ts`, grouped by the category a
developer is choosing between, every comparator reusing the existing benchmark
matrix lane vocabulary (no parallel taxonomy):

| Comparator lane | Category | Bar Khala aims to clear |
|-----------------|----------|--------------------------|
| `bigpickle` | default coding-agent model | beat the default free coding model on cost-per-accepted-outcome AND solve-rate |
| `gemini-free` | free / open | match/beat the free field on solve-rate at equal-or-lower cost |
| `openai-gpt` | paid frontier | measure the gap to paid GPT-class and track it shrinking |
| `claude` | paid frontier | measure the gap to paid Claude-class and track it shrinking |
| `fireworks` | paid frontier | match/beat the paid open-weight serving lane on cost at equal solve-rate |

`khala` is always the protagonist, never a comparator.

## Scored on BOTH axes (the unique value)

Each published matchup carries a Khala side and a comparator side (from
decision-grade leaderboard rows), plus a two-axis **verdict**:

- `khala_wins_both` — Khala wins solve-rate AND cost-per-accepted-outcome
- `khala_wins_cost` — Khala cheaper per accepted outcome, comparable/lower solve
- `khala_wins_quality` — Khala higher solve-rate, comparable/higher cost
- `comparator_ahead` — the comparator leads on the axis(es) that matter
- `even` — within the tolerance band on both axes (±200 bps solve, ±5% cost)

`solveRateDeltaBps` (khala − comparator) and `costPerAcceptedOutcomeDeltaMsat`
(comparator − khala; positive ⇒ Khala cheaper) make the gap explicit.

## Recurring run mechanism

`KHALA_HEAD_TO_HEAD_RECURRING_CONFIG` in `head-to-head.ts` is the single
recurring contract:

- `headToHeadRef`: `head_to_head.public.khala_vs_developer_defaults.v1`
- `cadence`: `per_khala_release` (re-run + re-publish on every significant Khala
  change so the bar tracks Khala improving; `weekly` and `on_demand` are also
  valid cadences)
- `publishPath`: `/api/public/khala/head-to-head`
- `experimentConfigId`: `khala-vs-fireworks-vertex-decision-suite-oq5-v1` (the
  owner-armed real decision suite from #6307 the sweep arms)
- `demandKind` / `demandSource`: **`internal` / `head_to_head`** — every Khala
  request the bar drives MUST carry these tags (#6298) so head-to-head traffic
  stays segmented from external/user traffic.

A scheduler (or operator) re-runs the **same owner-armed real sweep gate** #6307
uses each cadence, then POSTs the resulting decision-grade
`GymLeaderboardReportInput[]` to `/api/operator/khala/head-to-head`. The Worker
re-builds the bar through `buildKhalaHeadToHead` (which runs the flat projection —
decision-grade + public-safety-checked rows only) and upserts the public-safe
artifact by `headToHeadRef`. The public route serves the latest snapshot.

The public and operator read envelopes deliberately distinguish **read time**
from **snapshot time**:

- `generatedAt` is when the API response was composed.
- `publishedAt` is the stored snapshot's publish timestamp (or `null` for the
  honest empty pre-publish shape).
- `dataAgeSeconds` and `staleExceeded` are computed from `publishedAt` against
  the declared stored-snapshot staleness contract, so an old recurring
  head-to-head cannot look fresh just because someone read it today.

## Honesty / owner-arm gate (NEEDS-OWNER)

Numbers are published **only** from a `decisionGrade: true`, public-safety-checked
report over **realistic** traffic — the same bar the gym leaderboard enforces.

- **Khala side runs now at no third-party cost** (own-capacity public
  `/api/v1`), so the protagonist row is producible immediately.
- **Every comparator that needs real third-party spend is owner-armed.** It is
  wired to the **same single owner-arm gate #6307 uses**
  (`preflightRealBenchmarkSweep` / `runRealSweep`): explicit owner confirmation,
  a public-safe approval ref, a positive msat budget cap, a billable-sample cap,
  and realistic-traffic evidence refs. The runner refuses to spend without a
  green preflight, and an unarmed comparator lane is recorded as a **skipped**
  run (honest absence), never a fabricated number.

**NEEDS-OWNER to publish decision-grade matchups:**

1. Provide paid API keys + spend approval for the comparator lanes the owner
   wants measured (`openai-gpt`, `claude`, `gemini-free` paid tier, `fireworks`)
   and a real `bigpickle` model id, wired through the owner-armed real-lane
   transports (`real-lane-transports.ts`).
2. Confirm/refresh the realistic observed-traffic shapes from the live
   `token_usage_events` ledger at arm time, then run `runRealSweep` with the
   owner confirmation, approval ref, budget cap, and billable-sample cap.
3. POST the resulting decision-grade `GymLeaderboardReportInput[]` to
   `/api/operator/khala/head-to-head`. Each armed comparator's matchup flips from
   `awaiting_owner` to `published` with the two-axis verdict.

## First published state (2026-06-26)

In this environment the owner-armed real seam is **not armed** and the comparator
lanes are `fixture_only` (no real executor wired) — so there is no decision-grade
real measurement to publish. The honest first published state is the **empty
shape**: every matchup `awaiting_owner` with its owner-gate refs visible, and
`khala: null`. Fixture/synthetic numbers are **never** published.

```jsonc
{
  "schemaVersion": "openagents.khala.head_to_head.v1",
  "headToHeadRef": "head_to_head.public.khala_vs_developer_defaults.v1",
  "cadence": "per_khala_release",
  "demandKind": "internal",
  "demandSource": "head_to_head",
  "decisionGradeRowCount": 0,
  "khala": null,
  "matchups": [
    {
      "lane": "bigpickle", "category": "default_coding_agent_model",
      "state": "awaiting_owner", "verdict": null,
      "blockerRefs": [
        "blocker.khala.head_to_head.no_decision_grade_khala_row",
        "blocker.khala.head_to_head.comparator_lane_fixture_only.bigpickle",
        "gate.owner.khala.head_to_head.bigpickle.real_seam_with_model_id"
      ]
    }
    // gemini-free / openai-gpt / claude / fireworks: same awaiting_owner shape
  ]
}
```

This is a real published artifact: the recurring quality bar is live and
dereferenceable, and it honestly shows that no matchup has decision-grade numbers
yet, naming the exact gate that must resolve for each comparator. As soon as an
owner-armed real sweep publishes decision-grade rows for a comparator, that
matchup flips to `state: "published"` with both sides, the two-axis verdict, and
an empty `blockerRefs`.

### Illustrative published-shape example (NOT a measurement)

When the Big Pickle matchup publishes, it looks like the following. These numbers
come from the deterministic fixture harness purely to show the SHAPE — they are
illustrative and are never served by the public route (the builder drops every
non-decision-grade report before ranking):

```jsonc
{
  "lane": "bigpickle", "state": "published", "verdict": "khala_wins_both",
  "khala":      { "lane": "khala",     "costPerAcceptedOutcomeMsat": 400,  "solveRateBps": 10000 },
  "comparator": { "lane": "bigpickle", "costPerAcceptedOutcomeMsat": 1800, "solveRateBps": 5000 },
  "solveRateDeltaBps": 5000,
  "costPerAcceptedOutcomeDeltaMsat": 1400,
  "blockerRefs": []
}
```

## Tests

- `workers/api/src/inference/benchmark/head-to-head.test.ts` — comparator set +
  internal demand tagging, empty/awaiting-owner shape, Khala-only protagonist,
  the two-axis verdicts (`khala_wins_both`, `comparator_ahead`), fixture
  exclusion, caveats.
- `workers/api/src/inference/benchmark/head-to-head-routes.test.ts` — public
  empty surface + staleness contract, non-GET rejection, operator publish +
  public serve, unauthorized/malformed/unsafe-ref rejection.
