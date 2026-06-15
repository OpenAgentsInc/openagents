# Tassadar Evolution Loop Receipts

Date: 2026-06-15

This note tracks the receipt-bearing surfaces for the
`artanis.tassadar_evolution_loop.v1` green path. The promise stays yellow until
the live blockers are cleared receipt-first:

- `blocker.product_promises.artanis_unattended_tick_streak_missing`
- `blocker.product_promises.tassadar_distillation_dataset_receipt_missing`

## #5029: Tetrahedron-Closed Executor Ticks

`GET /api/public/artanis/admin-ticks` now projects
`closedTickReceipts` for Artanis administrator assignments only when all four
faces of the `tick_closure.v0.1` predicate are present:

- intent: a persisted Artanis admin dispatch decision for the assignment;
- execution: public-safe Pylon artifact and closeout refs;
- state delta: accepted-work refs plus the public Artanis closeout receipt ref;
- evaluation: a verified Artanis closeout verdict from exact replay.

The receipt kind is `artanis_tetrahedron_closed_tick`. It is operational
evidence only: it does not claim payout settlement, trained-model capability, or
ungated Artanis authority. Spend and publication remain approval-gated.

This machinery makes repeated closed ticks visible without fabricating the
sustained evidence. #5029 can close only when multiple real closed ticks appear
in the monitor from live assignment and replay rows.

## #5030: Unattended Ten-Tick Streak Gate

`GET /api/public/artanis/admin-ticks` also projects an
`unattendedTickStreak` gate. The gate is derived from the same persisted
decision, accepted-work closeout, and exact-replay verdict rows as the
closed-tick receipts.

The projection reports:

- the current consecutive closed-tick count from the latest decision backward;
- the longest consecutive closed-tick run in the bounded monitor window;
- the public-safe closed-tick receipt refs and decision refs for that longest
  run;
- `blocker.product_promises.artanis_unattended_tick_streak_missing` until a
  real ten-tick run is visible.

The gate emits an `artanis_unattended_tick_streak` receipt ref only when the
bounded read model contains at least ten consecutive `closed_verified` ticks.
That receipt is still operational evidence only. It grants no dispatch, spend,
publication, promise-transition, model-capability, payout, or settlement
authority, and it does not by itself flip
`artanis.tassadar_evolution_loop.v1` green.

The no-spend Artanis admin dispatch budget is 10 per UTC day so one fully
unattended cron span can satisfy the streak gate. The budget still applies only
to `unpaid_smoke` executor-trace assignments; spend, publication, training
launch, payout, and settlement remain separately approval-gated.

## Remaining Gates

#5030 remains gated on live evidence: the public monitor must show at least ten
consecutive real closed ticks from a healthy online Pylon fleet. Fixture data
or source edits must not replace that live receipt trail.

#5032 requires the first `dataset_curation` receipt converting accepted,
replay-verified traces into a curated distillation dataset artifact. The
existing closed-tick and run-corpus projections are inputs to that receipt, not
the receipt itself.
