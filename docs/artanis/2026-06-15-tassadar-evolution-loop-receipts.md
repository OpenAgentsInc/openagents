# Tassadar Evolution Loop Receipts

Date: 2026-06-15

This note tracks the receipt-bearing surfaces for the
`artanis.tassadar_evolution_loop.v1` green path. As of this file's 2026-06-15
update, the public monitor projects all four receipt surfaces and the product
promise transition was recorded receipt-first:

- `promise_transition_f4cb0a3e-0c41-41ef-b8fe-e8a9985fdae2`
  (`yellow -> green`, registry `2026-06-14.10`, all checks passed).

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

Live status: #5029 closed on 2026-06-15 after ten Artanis admin assignments
(`assignment.artanis_admin.20260615080510` through
`assignment.artanis_admin.20260615081910`) projected as `closed_verified` from
live dispatch, Pylon closeout, accepted-work, and exact-replay verdict rows.

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

Live status: #5030 closed on 2026-06-15 after the public monitor projected
`currentConsecutiveClosedTicks: 10`, `longestConsecutiveClosedTicks: 10`,
`blockerRefs: []`, and
`receiptRef: receipt.public.artanis.unattended_tick_streak.71929a5d-fa68-41f1-bb9f-51f67abd3456.x10`.
D1 verification showed all ten 2026-06-15 Artanis admin assignments in the run
as `accepted_work` with `outcome=verified`, `accept_state=accepted`, and trace
digest prefix `f2995c4e3c959b42`.

## #5031: Public Tick Monitor Hardening

`GET /api/public/artanis/admin-ticks` is a read-only monitor. It rebuilds
`live_at_read` from persisted transition rows and declares the transition set:

- `artanis_admin_tick_decision_recorded`;
- `pylon_assignment_closeout_submitted`;
- `artanis_closeout_verdict_recorded`.

The monitor projects only bounded public-safe fields: reason strings are
truncated and redaction-scanned, raw mind output is not exposed, and closed tick
receipts run all refs through the public scanner. Tests cover state counts,
bounded limits, redaction of secret-shaped reasons, `live_at_read` staleness,
closed tick receipts, interrupted streaks, and the satisfied ten-tick receipt.

## #5032: First Distillation Dataset Receipt

`GET /api/public/artanis/admin-ticks` now projects
`distillationDatasetReceipt` once the verified ten-tick streak is satisfied.
The receipt is derived from the same public closed-tick receipt refs and
converts those accepted, replay-verified traces into a compact-binary
train/validation/test dataset manifest:

- `curationKind: dataset_curation`;
- `receiptKind: tassadar_distillation_dataset_curation`;
- `encodingRef: encoding.tassadar_trace.compact_binary.v0_1`;
- `splitPolicyRef: policy.tassadar_trace.train_val_test_split.v0_1`;
- three shard refs with SHA-256 digest refs;
- one manifest digest ref;
- source receipt refs for the included verified closed ticks.

The receipt projects only refs and digest commitments. It does not expose raw
traces, does not launch training, does not claim a trained model, and grants no
promise-transition, payout, settlement, publication, or model-capability
authority.
