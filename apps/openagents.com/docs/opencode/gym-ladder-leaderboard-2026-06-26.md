# Gym Benchmark Ladder — Recurring Published Leaderboard (first run)

> **Status:** first published run, 2026-06-26 (#6309, Khala GTM master #6303).
> This is the recurring, dereferenceable Gym leaderboard that ranks Khala against
> the three-rung opponent ladder (Big Pickle → free models → paid frontier) on
> the OpenCode coding surface and our axes (cost-per-accepted-outcome,
> verified-rate, tool-call completion, wall-clock). The planning contract is
> `docs/opencode/opencode-gym-benchmark-ladder.md`; this doc records the shipped
> publishing machinery and the first published state.

## What shipped

The ladder is the **publishing layer** on top of the already-shipped benchmark
harness (matrix / runner / report) and the flat `buildGymLeaderboardProjection`.
It does not re-run or re-implement the harness — it consumes it.

| Surface | Path | Owner |
|---------|------|-------|
| Ladder builder (pure) | `workers/api/src/inference/gym/ladder.ts` | rung classification, recurring config, `buildGymLadderLeaderboard` |
| Publish store (D1) | `workers/api/src/inference/gym/ladder-store.ts` | `gym_ladder_leaderboard_snapshots` table, decode-on-read |
| Routes | `workers/api/src/inference/gym/ladder-routes.ts` | public GET + operator publish POST |
| Migration | `workers/api/migrations/0240_gym_ladder_leaderboard_snapshots.sql` | snapshot table |
| Public dereference | `GET /api/public/gym/leaderboard` | latest published ladder (no auth) |
| Recurring publish | `POST /api/operator/gym/leaderboard` | admin-bearer; scheduler/operator publish boundary |

## Recurring run mechanism

`GYM_LADDER_RECURRING_CONFIG` in `ladder.ts` is the single recurring contract:

- `ladderRef`: `ladder.public.gym.opencode_khala_vs_field.v1`
- `cadence`: `per_model_release` (re-run + re-publish on every significant Khala
  model update; `weekly` and `on_demand` are also valid cadences)
- `publishPath`: `/api/public/gym/leaderboard`
- `experimentConfigId`: `gym-opencode-khala-vs-bigpickle-fixture-v1` (the
  OpenCode head-to-head matrix the owner-armed real sweep arms)
- `demandKind` / `demandSource`: **`internal` / `gym_ladder`** — every Khala
  request the ladder drives MUST carry these tags (#6298) so gym traffic stays
  segmented from external traffic.

A scheduler (or operator) re-runs the owner-armed real sweep each cadence, then
POSTs the resulting decision-grade `GymLeaderboardReportInput[]` to
`/api/operator/gym/leaderboard`. The Worker re-builds the ladder through
`buildGymLadderLeaderboard` (which runs the flat projection — decision-grade +
public-safety-checked rows only) and upserts the public-safe ladder by
`ladderRef`. The public route serves the latest snapshot.

The public and operator read envelopes deliberately distinguish **read time**
from **snapshot time**:

- `generatedAt` is when the API response was composed.
- `publishedAt` is the stored snapshot's publish timestamp (or `null` for the
  honest empty pre-publish shape).
- `freshnessDueAt` is the timestamp when that stored snapshot exceeds the
  declared freshness window (or `null` before the first publish).
- `dataAgeSeconds` and `staleExceeded` are computed from `publishedAt` against
  the declared stored-snapshot staleness contract, so an old recurring ladder
  cannot look fresh just because someone read it today.

## First published result (2026-06-26)

In this environment the owner-armed real seam is **not armed** and the opponent
lanes (`bigpickle`, `gemini-free`, `openai-gpt`, `claude`) are `fixture_only`
(no real executor wired) — so there is no decision-grade real measurement to
publish. The honest first published state is therefore the **empty ladder**: the
three rungs in their `awaiting_owner` shape with the owner-gate refs visible.
Fixture/synthetic numbers are **never** published as a rung measurement.

```jsonc
{
  "schemaVersion": "openagents.gym.ladder_leaderboard.v1",
  "ladderRef": "ladder.public.gym.opencode_khala_vs_field.v1",
  "cadence": "per_model_release",
  "demandKind": "internal",
  "demandSource": "gym_ladder",
  "decisionGradeRowCount": 0,
  "rungs": [
    {
      "rung": "rung1", "title": "Rung 1 — Khala vs Big Pickle",
      "state": "awaiting_owner",
      "opponentLanes": ["bigpickle"],
      "blockerRefs": [
        "blocker.gym.ladder.no_decision_grade_khala_row",
        "blocker.gym.ladder.opponent_lane_fixture_only.bigpickle",
        "gate.owner.gym.ladder.rung1.real_seam_with_bigpickle_model_id"
      ]
    },
    {
      "rung": "rung2", "title": "Rung 2 — Khala vs free / open models",
      "state": "awaiting_owner",
      "opponentLanes": ["gemini-free"],
      "blockerRefs": [
        "blocker.gym.ladder.opponent_lane_fixture_only.gemini-free",
        "gate.owner.gym.ladder.rung2.free_tier_real_seam"
      ]
    },
    {
      "rung": "rung3", "title": "Rung 3 — Khala vs paid frontier",
      "state": "awaiting_owner",
      "opponentLanes": ["openai-gpt", "claude"],
      "blockerRefs": [
        "blocker.gym.ladder.opponent_lane_fixture_only.claude",
        "blocker.gym.ladder.opponent_lane_fixture_only.openai-gpt",
        "gate.owner.gym.ladder.rung3.paid_api_keys_and_spend_approval"
      ]
    }
  ]
}
```

This is a real published artifact: the recurring leaderboard is live and
dereferenceable, and it honestly shows that no rung has decision-grade numbers
yet, naming the exact gate that must resolve for each rung. As soon as an
owner-armed real sweep publishes decision-grade rows for a rung, that rung flips
to `state: "published"` with ranked entries (Khala + measured opponents, ranked
by cost-per-accepted-outcome) and an empty `blockerRefs`.

### Illustrative published-shape example (NOT a measurement)

When Rung 1 publishes, the rung looks like the following. These numbers come from
the deterministic fixture harness purely to show the SHAPE — they are
illustrative and are never served by the public route (the builder drops every
non-decision-grade report before ranking):

```jsonc
{
  "rung": "rung1", "state": "published",
  "entries": [
    { "rank": 1, "lane": "khala",     "costPerAcceptedOutcomeMsat": 400, "verificationRateBps": 10000 },
    { "rank": 2, "lane": "bigpickle", "costPerAcceptedOutcomeMsat": 900, "verificationRateBps": 10000 }
  ],
  "blockerRefs": []
}
```

## NEEDS-OWNER (the gated rungs)

The publishing machinery is shipped and live; the decision-grade numbers are
owner-gated. To publish real rungs:

- **Rung 1 (Big Pickle):** arm the OpenCode real seam and resolve + record the
  exact upstream Big Pickle model id + API version + date
  (`gate.owner.gym.ladder.rung1.real_seam_with_bigpickle_model_id`).
- **Rung 2 (free/open):** wire the free-tier real executor for `gemini-free`
  (and any other free lanes) with quota handling
  (`gate.owner.gym.ladder.rung2.free_tier_real_seam`).
- **Rung 3 (paid frontier):** provide owner-held API keys + per-lane
  `budgetCapMsat` + spend approval for `openai-gpt` and `claude`
  (`gate.owner.gym.ladder.rung3.paid_api_keys_and_spend_approval`).

Each real sweep must also pass the existing `preflightRealBenchmarkSweep` gates
(owner confirmation, approval ref, budget cap, billable-sample cap, real-traffic
evidence) before the report is `decisionGrade: true`.

## Honesty gates

Inherited from the flat projection + added at the ladder layer:

- `caveat.public.gym.ladder.decision_grade_rungs_only`
- `caveat.public.gym.ladder.fixture_or_synthetic_never_published`
- `caveat.public.gym.ladder.awaiting_owner_rungs_show_gate_not_numbers`
- `caveat.public.gym.ladder.no_beats_frontier_claim_from_single_run`
- `caveat.public.gym.ladder.gym_traffic_tagged_internal_gym_ladder`
