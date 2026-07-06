# Khala Sync — Rolling D1 → Cloud SQL Migration Plan (KS-8.3)

**Status:** Plan of record for KS-8 domain migration (issue
[#8309](https://github.com/OpenAgentsInc/openagents/issues/8309), epic
[#8282](https://github.com/OpenAgentsInc/openagents/issues/8282)).
**Spec:** [`SPEC.md`](./SPEC.md).
**Rationale:** [`../fable/2026-07-04-database-alternatives-and-postgres-sync-engine.md`](../fable/2026-07-04-database-alternatives-and-postgres-sync-engine.md)
(§1.2 the D1 audit, §5 Phase 3).
**Predecessors (not re-planned here):**
[#8307](https://github.com/OpenAgentsInc/openagents/issues/8307) KS-8.1
assignments/dispatch and
[#8308](https://github.com/OpenAgentsInc/openagents/issues/8308) KS-8.2
token ledger. This document sequences **everything after those two**.

**KS-8.1 status (2026-07-04):** machinery LANDED — Postgres schema
(`khala-sync-server` migration `0005_pylon_dispatch.sql`:
`pylon_registrations` / `pylon_assignments` / `pylon_assignment_events`,
indexes re-derived from actual query patterns), typed repository seam with
D1 + Postgres implementations and a fail-soft dual-write wrapper
(`apps/openagents.com/workers/api/src/pylon-dispatch-store.ts`), flags
`KHALA_SYNC_PYLON_DUAL_WRITE` (default on) / `KHALA_SYNC_PYLON_READS`
(d1|compare|postgres, default d1), resumable backfill + exact-verify CLI
(`packages/khala-sync-server/scripts/backfill-pylon.ts`), and a contract
suite run against BOTH stores. The prod cutover (flag flips + backfill +
verification evidence) is tracked on epic
[#8282](https://github.com/OpenAgentsInc/openagents/issues/8282); the
cutover procedure is [`RUNBOOK.md`](./RUNBOOK.md) "Pylon dispatch domain
cutover". Per owner direction on 2026-07-04, the per-domain soak/drop ticket
is not an active gate; final D1 retirement is consolidated into KS-8.19
[#8330](https://github.com/OpenAgentsInc/openagents/issues/8330).

**KS-8.1 fresh backup verification (2026-07-05, #8282/#8330 follow-up):** a
follow-up to the same-day forum-and-user-content backup pass checked this
domain fresh (that pass had deferred it as "operational, not content, out
of scope"). Fresh `backfill-pylon.ts --verify --verify-newest 50` against
production: **VERIFY FAILED** — despite the "LANDED" status above and
`KHALA_SYNC_PYLON_READS=postgres` already being committed to production for
the runner-status/dispatch read paths (§3.1), the historical backfill had
never actually run: `pylon_registrations` 114 D1 / 5 Postgres,
`pylon_assignments` 10,665 D1 / 2 Postgres, `pylon_assignment_events`
425,300 D1 / 2,393 Postgres. Ran the standard sweep: `pylon_registrations`
(109 newly inserted) and `pylon_assignments` (10,663 newly inserted) are
now **backfilled and confirmed exact**. `pylon_assignment_events` (425,300
rows, large per-row heartbeat/progress payloads) backfills at only ~11-13
rows/sec via the page-by-page `wrangler d1 execute` CLI — too slow to
finish inside this session — and was left running as a detached,
safe/idempotent/resumable background process for its remaining ~9-11
hours. Full evidence:
[`2026-07-05-forum-and-user-content-backup-verification.md`](./2026-07-05-forum-and-user-content-backup-verification.md).
**Registrations and assignments are safe to cite on KS-8.19; assignment
events backfill is IN PROGRESS as of this writing — re-verify before citing
that table specifically.**

**KS-8.2 status (2026-07-04):** machinery LANDED — Postgres schema
(`khala-sync-server` migration `0008_token_usage_ledger.sql`:
`token_usage_events` + the three `public_khala_tokens_served_*` rollup
twins + `token_usage_leaderboard_preferences`; indexes re-derived from the
actual post-#8304 read patterns — the 14 D1 indexes are deliberately NOT
ported, rationale in the migration header), the `TokenLedgerWriteStore`
repository seam with D1 + Postgres implementations, a fail-soft dual-write
wrapper and flag-routed public reads
(`apps/openagents.com/workers/api/src/token-ledger-store.ts`), flags
`KHALA_SYNC_LEDGER_DUAL_WRITE` (default on) / `KHALA_SYNC_LEDGER_READS`
(d1|compare|postgres, default d1; covers the five public tokens-served
read paths), direct-insert mirrors on the khala-chat and khala-MCP paths,
resumable backfill + exact-verify CLI
(`packages/khala-sync-server/scripts/backfill-token-ledger.ts`: exact
counts, SUM(total_tokens), the public tokens-served SUM, per-provider
tallies, newest-N row hashes), and a contract suite run against BOTH
stores including D1-vs-Postgres read equivalence. The #8304 public-counter
projection stays exactly-once per ledger row (regression-tested: the
Postgres mirror never re-fires the counter producer). Internal admin
aggregates (readAggregates / readInferenceAnalytics / readLeaderboards)
and the two low-volume unhooked direct-insert paths
(`builtin-compute-agent-grant.ts`, `provider-account-service-routes.ts` —
the same pair the #8304 runbook already tracks) stay D1-only until the final
KS-8.19 D1 retirement sweep. Prod cutover procedure:
[`RUNBOOK.md`](./RUNBOOK.md) "Token ledger domain cutover"; cutover
evidence is tracked on epic
[#8282](https://github.com/OpenAgentsInc/openagents/issues/8282), while the
per-domain soak/drop ticket is intentionally closed as not planned.

**KS-8.2 fresh backup verification (2026-07-05, #8282/#8330 follow-up):** a
follow-up to the same-day forum-and-user-content backup pass checked this
domain fresh (that pass had deferred it as "operational, not content, out
of scope"). Fresh `backfill-token-ledger.ts --verify` against production:
**VERIFY FAILED** — `token_usage_events` (the table behind the public
`/api/public/khala-tokens-served` homepage counter) was 296,908 D1 rows
against only 11,077 Postgres rows (`sum_total_tokens` 8,441,160,476 vs
182,795), and all three public rollup tables
(`public_khala_tokens_served_daily_rollups` / `_model_daily_rollups` /
`_channel_daily_rollups`) showed the identical proportional gap.
`KHALA_SYNC_LEDGER_READS` stays at its documented default `d1`, so the
public counter itself was never served from the incomplete mirror — this
was a backup-completeness gap, not an active-serving bug. Ran the standard
sweep (full sweep converged 285,831 rows + all rollups; several transient
`wrangler` API errors during the resumable catch-up sweep, each resumed
cleanly from the saved cursor with zero data loss since the upsert is `ON
CONFLICT DO NOTHING`). Full evidence and the closing `--verify` result:
[`2026-07-05-forum-and-user-content-backup-verification.md`](./2026-07-05-forum-and-user-content-backup-verification.md).

**KS-8.6 status (2026-07-04):** machinery LANDED — Postgres schema
(`khala-sync-server` migration `0011_artanis_domain.sql`: all twenty
`artanis_*` twins; indexes re-derived from the owning modules' actual
query patterns; D1's one-active-loop-per-scope partial unique index
deliberately NOT ported mid-migration, rationale in the migration
header), the `ArtanisDatabase` seam — a database-shaped handle
(`apps/openagents.com/workers/api/src/artanis-domain-store.ts`) rather
than a per-operation store, because this domain's SQL lives in eleven
owning modules — with a registry-driven Postgres converge store,
read-back full-row dual-write mirrors (`mirrorArtanisRows`, fail-soft:
the 2d46d808 operator-chat precedent holds through the seam), and
flag-routed reads (`artanisRead`), flags `KHALA_SYNC_ARTANIS_DUAL_WRITE`
(default on) / `KHALA_SYNC_ARTANIS_READS` (d1|compare|postgres, default
d1). Wired at the artanis write call sites INCLUDING all six every-minute
cron ticks (`ArtanisScheduledRunner.runTick`, `ArtanisResponder.scan`,
`ArtanisResponder.compose`, `ArtanisAdmin.tick`,
`ArtanisAdmin.closeoutVerifier`, `ArtanisFleet.tick`) — the ticks keep D1
authority and mirror to Postgres until the read-cutover evidence lands.
Resumable backfill + exact-verify CLI
(`packages/khala-sync-server/scripts/backfill-artanis.ts`: exact counts,
per-state tallies, newest-N row hashes over all twenty tables), and a
contract suite run against BOTH engines
(`artanis-domain-repository.contract.test.ts`: registry fidelity across
all twenty tables, tick double-fire idempotency, mutation convergence,
D1-vs-Postgres persistence read equivalence). Analytics-style JOIN reads
(`artanis-tick-streak.ts`, `artanis-distillation-dataset-receipt.ts`
cross-table joins) and dashboard aggregations stay D1-only and move at
read cutover.

**KS-8.6 read-cutover evidence (2026-07-05, #8335):** retention decision
made — `artanis_health_snapshots` / `artanis_runtime_snapshots` are 125
rows each in production; porting as-is (no bounding) is cheap enough that
no pre-sweep retention change is needed. Backfill ran twice (sweep 1:
first full port; sweep 2 `--restart`: 0 newly-inserted rows across all 20
tables — dual-write was already fully caught up) then `--verify
--verify-newest 50`: **19 of 20 tables exact** (rows, per-state tallies,
newest-50 row hashes all match). `artanis_responder_ticks` came back
non-exact: row COUNT matches (7940=7940) but 20 rows have a stale
`scan_state`/`compose_state` in Postgres — root-caused to a genuine
concurrent-writer race in `mirrorArtanisRows` (full-row read-D1-then-
upsert, no ordering guard) between the two independent ticks
(`ArtanisResponder.scan`, `ArtanisResponder.compose`) that both write the
same `scheduled_at` row; filed as
[#8409](https://github.com/OpenAgentsInc/openagents/issues/8409). This
table is NOT one of the eight `ArtanisPersistenceRecordKind`s routed
through `artanisRead` (only `approval_gate`, `forum_publication_intent`,
`health_snapshot`, `loop_record`, `loop_tick`,
`nexus_pylon_adapter_dispatch`, `runtime_snapshot`, and
`work_routing_proposal` are), so the drift does not block the read
cutover below, but it DOES block ever safely reading
`artanis_responder_ticks` from Postgres (the "responder scan/composer
joins" item) and blocks KS-8.19 D1 retirement for that table until fixed.

Of the six every-minute cron ticks, only `ArtanisScheduledRunner.runTick`
currently issues a flag-routed read (an idempotency check via
`readArtanisPersistedRecord('loop_record', ...)`, unconditional on every
invocation); `ArtanisResponder.scan/.compose`, `ArtanisAdmin.tick`,
`ArtanisAdmin.closeoutVerifier`, and `ArtanisFleet.tick` only mirror
writes today — their own decision-making reads (spend/grant aggregation
in `artanis-spend.ts`, responder scan/composer joins in
`artanis-responder-ticks.ts`/`artanis-responder-provenance.ts`, the labor
receipt ordered list in `artanis-labor-receipt-store.ts`) are still bare
D1 SQL with no Postgres reader wired, so `KHALA_SYNC_ARTANIS_READS` is a
no-op for them regardless of value. `KHALA_SYNC_ARTANIS_READS=compare`
shipped to production and staging in commit `07ada9d32b` (Worker version
`473b6c53-8a65-40d2-b238-2e1d5c21c449`) for a live soak; see the
issue/RUNBOOK for the soak window and flip decision. "Move remaining
D1-direct read paths" (analytics joins, dashboard/console aggregations,
spend/grant aggregation, responder scan/composer joins, labor receipt
ordered list) is real follow-up implementation work, not yet started —
each needs its own re-derived Postgres index/query and its own compare
evidence before it can be flag-routed. Prod cutover procedure:
[`RUNBOOK.md`](./RUNBOOK.md) "Artanis supervision domain cutover";
cutover evidence tracked on
[#8335](https://github.com/OpenAgentsInc/openagents/issues/8335); D1 drop
consolidated into KS-8.19
([#8330](https://github.com/OpenAgentsInc/openagents/issues/8330)) per
the 2026-07-04 owner direction.

**#8409 fix landed (2026-07-05, code only):** root-caused and fixed the
`artanis_responder_ticks` full-row-upsert race above —
`mirrorArtanisRows`/`upsertRows` now accept a column-ownership scope so
two independent writers of disjoint columns on the same key (the scan and
compose ticks) can never clobber each other's concurrent update; the same
race shape, also fixed, existed for `artanis_responder_state`. Real
Postgres+D1 regression coverage added and verified sensitive (fails
without the fix, passes with it). A fresh production `--verify` run the
same day shows the drift is real, ongoing, and has grown since the
original #8335 report (20→21/22 stale rows) — confirming the diagnosis —
but the fix is not yet deployed and the already-stale rows are not yet
corrected; see `RUNBOOK.md` "#8409 fix landed" for the full evidence and
the outstanding deploy + corrective-sweep follow-up.

**KS-8.6 follow-up (2026-07-05, #8335): fix deployed, but the clobber
recurred AFTER deploy — flip NOT taken.** Confirmed `06ee7de4c7`
(#8409) is on `main`; fast-forwarded a clean worktree and ran a
guaranteed-fresh `deploy:safe` (staging + prod; prod Worker Version
`17543300-c80f-450f-a84a-826be0b06358`, live `2026-07-05T10:00:48Z`) so
there is no worktree-staleness ambiguity about whether the fix is
actually running. A fresh `--verify` still shows 23 stale
`artanis_responder_ticks` rows (up from 20→21→23 across today's checks)
plus one row entirely missing from Postgres — and, critically, one of
the 23 (`scheduled_at=2026-07-05T09:13:24Z`) was created 27 minutes AFTER
the PRIOR deploy that chronologically should already have included the
fix. Over the ~24 minutes after the guaranteed-fresh deploy
(`2026-07-05T10:00:48Z` → `10:24:57Z`), the full-table diff stayed at
exactly 23 stale rows (zero new ones), and a ~35-minute genuinely
unfiltered production tail showed zero mentions of `artanis` at all — an
encouraging but not fully conclusive signal (historical clobber rate ~2/hr,
median gap ~22 min). Per the explicit money/business-adjacent-data
guardrail, this is a stop-and-report finding, not a paper-over: recommend
reopening #8409 rather than treating it as closed, and holding the
remaining Artanis read-path migration work (this table plus the flip
itself for the eight actually-routed record kinds) until the mirror's
dual-write reliability is re-confirmed. Full evidence in `RUNBOOK.md`
"KS-8.6 follow-up — #8409 fix deployed, fresh clobber confirmed AFTER
deploy".

**#8409 follow-up (2026-07-05): distinct root cause confirmed and fixed
(retry, code only).** The reopened issue's own candidate hypothesis was
right: `mirrorArtanisRows` attempted its D1-read-back + Postgres upsert
exactly ONCE with no retry, so a transient failure on a writer's OWN
mirror call permanently dropped that writer's column update — a DIFFERENT
bug from the #8409 clobber race (which is genuinely fixed; its regression
tests still pass), producing the identical stuck-at-`'pending'` symptom.
Fixed with bounded retry (`[100, 400]` ms backoff, matching the existing
`artanisRead` postgres-mode precedent) plus a new
`khala_sync_artanis_dual_write_retry` diagnostic for observability. Real
Postgres+D1 regression coverage added and verified sensitive. Fresh
`--verify` baseline the same day: drift has plateaued (23-row skew, same
order as the prior pass) rather than accelerated. Fix committed to `main`;
not yet deployed as of this note. See `RUNBOOK.md` "#8409 follow-up — root
cause confirmed and fixed" for full evidence and the outstanding
deploy/soak/corrective-sweep sequence.

**KS-8.8 status (2026-07-04):** machinery LIVE — Postgres schema
(`khala-sync-server` migration `0016_treasury_domain.sql`: all 27 live
money tables — treasury transactions, the six nexus payout-authority
ledgers, the forum money half, both reward ledgers, agent balances, labor
escrows + receipts, partner/site-referral payout ledgers + agreements,
revenue-event provenance, and the two MPP replay guards; idempotency/
replay keys ported EXACTLY, money amounts as bigint, indexes re-derived
from the owning modules' actual reads, `agent_claim_reward_ledger`'s
partial uniques deliberately NOT ported mid-migration — rationale in the
migration header), the `TreasuryDatabase` seam
(`apps/openagents.com/workers/api/src/treasury-domain-store.ts`: a
database-shaped handle, registry-driven Postgres converge store,
read-back full-row `mirrorTreasuryRows` fail-soft dual-write with
redacted diagnostics for the replay-guard payment identifiers, and
`treasuryRead` flag routing), plus a `LedgerStatement.mirror` annotation
seam in `payments-ledger.ts` so every `runLedgerStatements` batch
(balances, escrows, credit grants) mirrors its touched rows after the
atomic D1 commit. Flags `KHALA_SYNC_TREASURY_DUAL_WRITE` (default on) /
`KHALA_SYNC_TREASURY_READS` (d1|compare|postgres, default d1; read flips
are EPIC-GATED ops decisions on #8282, never a code default). Wired at
the money write call sites INCLUDING all six money crons
(`TipsSweep.runTick`, `TipsBuffer.reconcileForwarding`,
`TipsBuffer.backingInvariant`, `TreasuryTransactions.reconcilePending`,
`XClaimRewardTreasuryDispatcher.runTick`,
`ForumDirectTips.archiveStaleRecoveries`) — D1 stays SOLE payout
authority; every side-effect-bearing scan (payout dispatch, sweep
candidates, pending reconcile) reads exactly ONE store with no Postgres
twin, so no flag can double-dispatch. Resumable backfill + money-exact
verify CLI (`packages/khala-sync-server/scripts/backfill-treasury.ts`:
exact counts, per-(state, rail) tallies WITH exact money-column SUMs to
the millisat/cent, newest-N row hashes), and a contract suite run against
BOTH engines (`treasury-domain-repository.contract.test.ts`).

Live closeout evidence: commit `87d16a6ee7` deployed through
`deploy:safe` on 2026-07-04 (staging Worker version
`a423fc15-16a6-492b-9559-bb78e26160ed`, production Worker version
`1bcd048d-de0d-4a1e-a108-f79b4ba5e33f`). Migration
`0016_treasury_domain.sql` was dry-run then applied in staging and prod
(`1 pending, 16 already applied` in each dry-run; `applied 1, already
applied 16` in each apply). Production smokes passed: `https://openagents.com/`
HTTP 200, `/assets/index-DWcdsn2N.js` HTTP 200, and
`/api/internal/khala-sync/db-smoke` returned `{ ok: true,
khalaSyncTables: 12 }`. Production backfill copied the nonzero treasury
corpus (126 treasury transactions; 40 payout approvals; 67 payout intents;
64 payout attempts; 64 reconciliation events; 72 payment-authority
receipts; 1 release gate; 100 forum money actions; 100 forum payment
events; 58 forum receipts; 37 L402 challenges; 26 L402 redemptions; 74
direct-tip attempts; 2 direct-tip webhook events; 30 recipient wallets; 10
settlement claims; 3 X-claim rewards; 7 agent balances; 3 labor escrows;
1 MPP lightning replay guard), then the required restart sweep scanned the
same rows with zero inserts. Production `--verify --verify-newest 50`
ended `VERIFY OK: exact counts, per-state money sums, and newest-N hashes
match.`

Known
D1-only residuals until their callers pass the seam handle (all
low-volume; tracked for the KS-8.19 sweep): the referral/engagement
accrual feeds (`site-referral-payout-feed.ts`,
`business-referral-engagement-feed.ts`,
`referral-cross-category-accrual.ts`, `inference-referral-accrual.ts`),
`khala-code-paid-plan-payments.ts` / `qa-swarm-first-engagement-routes.ts`
provenance writers, the metering/cloud-metering statement paths that only
read balances, and Artanis' internal forum-route invocations. Prod
cutover procedure: [`RUNBOOK.md`](./RUNBOOK.md) "Treasury settlement
domain cutover" (flag flips are epic-gated on
[#8282](https://github.com/OpenAgentsInc/openagents/issues/8282)); final
D1 retirement is consolidated into KS-8.19
[#8330](https://github.com/OpenAgentsInc/openagents/issues/8330).

**KS-8.8 fresh re-verification + tooling cleanup (2026-07-05, #8282/#8330
follow-up):** independently re-ran `backfill-treasury.ts --verify
--verify-newest 50` against production (this domain was NOT re-checked in
the same-day forum-and-user-content backup pass, which explicitly deferred
it to "already has its own dated evidence"). Found the run hard-crashed on
"no such table: mpp_lightning_replay" — a real tooling gap, not a money
gap: D1 worker migration `0303_drop_mpp_replay_tables.sql` had already
retired both `mpp_lightning_replay` / `mpp_spt_replay` (the `/mpp/v1/chat/
completions` replay guards, removed per #8387 in favor of the Khala Code
paid-plan payment-intent ledger), but this domain's backfill/verify
registry still listed them, and the Postgres side held one stale,
never-mirrored row each with no live D1 counterpart to reconcile against.
Retired both tables from the registry (`treasury-backfill.ts`,
`backfill-treasury.ts`, tests) and added Postgres migration
`0036_drop_treasury_mpp_replay_tables.sql` (applied to staging + prod,
mirroring the D1 drop; zero live code referenced either table). After that
fix, a full `--verify --verify-newest 50` against production is **VERIFY
OK** across all 25 remaining live money tables — exact row counts,
per-(state, rail) tallies, and exact money-column SUMs to the
sat/msat/cent. D1 was never read from destructively or written to beyond
the retired-table Postgres migration (which mirrors an already-D1-dropped,
already-write-dead pair); no `KHALA_SYNC_TREASURY_*` flag was touched. Full
evidence:
[`2026-07-05-forum-and-user-content-backup-verification.md`](./2026-07-05-forum-and-user-content-backup-verification.md).
**Safe to cite on KS-8.19 (#8330).**

**KS-8.9 status (2026-07-04):** machinery LANDED — Postgres schema
(`khala-sync-server` migration `0013_inference_entitlements.sql`: 29
tables — the 15 `inference_*` entitlement/quota tables,
`builtin_compute_agent_quota_events`, `orange_check_entitlements`, the 4
`agent_rate_limit_*` and 8 `agent_search_*` tables; indexes ported from
actual reads only, each justified in the migration header;
`agent_search_metric_events` deliberately EXCLUDED — the `*_metric_events`
observability stream is an Analytics Engine candidate, not Postgres rows),
the typed mirror-op + enforcement-gate-read seam
(`apps/openagents.com/workers/api/src/inference-entitlements-store.ts`),
flags `KHALA_SYNC_ENTITLEMENTS_DUAL_WRITE` (default on) /
`KHALA_SYNC_ENTITLEMENTS_READS` (d1|compare|postgres, default d1; routes
the six serving-path enforcement reads — free-tier key membership +
daily usage, free-usage pool state, premium allowlist, operator
exemption, privacy entitlement — with compare-mode shadow decisions
scheduled OFF the response path and postgres mode falling back to D1 on
error). The dual-write mirror is FIRE-SAFE on the inference hot path:
synchronous enqueue, never awaited by the caller, never fails or delays a
completion (regression-tested); tally counters mirror EVENT-KEYED
(event insert `ON CONFLICT DO NOTHING` gates the tally increment in one
transaction) so a re-delivered mirror op can never double-count — a lost
increment is a free-tier leak, a doubled one a false denial. Wired at all
12 write modules (free tier, free allowance, earned allowance, premium
allowlist, operator exemption, privacy entitlements + receipts, referral
margin splits, batch jobs, builtin compute quota, orange check, agent
rate-limit recovery, agent search + payments). Resumable backfill +
exact-verify CLI
(`packages/khala-sync-server/scripts/backfill-inference-entitlements.ts`:
exact counts, per-group "per-plan" tallies, newest-N row hashes, and
tally = SUM(events) per key for the three enforcement tally families),
plus a contract suite that drives the PRODUCTION D1 write paths and
proves D1-vs-Postgres decision parity on the six gate reads under mirror
re-delivery. NOT mirrored in this lane: the `token_usage_events` row the
builtin-compute grant writes (KS-8.2 domain) and the metric-events stream
(Analytics Engine). Prod cutover procedure: [`RUNBOOK.md`](./RUNBOOK.md)
"Inference entitlements domain cutover"; the read cutover changes which
store ENFORCES allow/deny, so it requires the low-traffic window +
zero-divergence compare evidence; cutover evidence + D1 drop tracked on
epic [#8282](https://github.com/OpenAgentsInc/openagents/issues/8282).

**KS-8.9 decommission follow-up status (2026-07-05, #8336):** `inference_batch_jobs`
(the one write-dead table in the domain) was already retired end-to-end —
D1 migration `0302_drop_inference_batch_jobs.sql`, Postgres migration
`0032_drop_inference_batch_jobs.sql`, and the batch-job feature code removed
outright — dropping this lane's table count from 29 to 28. Real production
evidence gathered against `khala_sync_prod`: TWO backfill sweeps plus
`--verify` are GREEN (exact row counts on all 28 tables, newest-50 hashes
match, and all three tally=SUM(events) invariants hold exactly — see the
#8336 issue comment for the full verbatim output) and a live production
Worker tail sample (tens of thousands of log lines across two capture
windows, including real `/v1/chat/completions` traffic) shows ZERO
`khala_sync_entitlements_dual_write_failed` events. That is real but
partial evidence: it is not the multi-hour/day representative-window soak
the runbook's compare/postgres flip steps call for, and this pass had no
durable production log/metrics query surface (Logpush/Analytics Engine)
to reconstruct a longer historical window — only live `wrangler tail`.
Per the runbook's own caution, `KHALA_SYNC_ENTITLEMENTS_READS` was
deliberately left at the default `d1` (no compare/postgres flip attempted)
rather than flip blind. Decommission-scope findings: (1) the two
Effect-shaped `markAccountFreeTier`/`recordFreeKeyMint` variants in
`inference/inference-free-tier-key.ts` had zero production call sites (only
the mirror-wired `*Async` siblings are wired at the real write path) and
were deleted, with call sites in
`inference/inference-free-tier-key.test.ts` moved onto the `*Async`
functions (37/37 tests green); (2) the `token_usage_events` row
`builtin-compute-agent-grant.ts` writes is confirmed to belong to the
KS-8.2 ledger domain by table ownership, but that exact call site is a
KNOWN, ALREADY-TRACKED KS-8.2 gap (see the KS-8.2 status paragraph above:
it stays D1-only, unhooked, "until the final KS-8.19 D1 retirement
sweep") — not a new KS-8.9 finding, and it must close before the ledger D1
drop, tracked on KS-8.2/KS-8.19, not here; (3) `agent_search_metric_events`
(`agent-search.ts` `recordMetric`, ~line 1489) is confirmed WRITE-ONLY
(zero reads anywhere in the codebase) and is a clean Analytics Engine
candidate, but this repo has zero existing Analytics Engine binding/dataset
precedent today, so wiring it is deferred to its own properly-scoped,
locally-verifiable follow-up rather than a same-change addition; (4) the
"route the non-gate reads to Postgres" scope (admin/list reads for premium
allowlist + operator exemption, agent-search request/cache/quota/count
reads, agent-rate-limit recovery reads, privacy-receipt reads, and the two
orange-check reads `countActiveOrangeChecks` /
`readActiveOrangeCheckByActorRef`) was inventoried but NOT implemented —
the call sites span `index.ts`, `config.ts`,
`inference/inference-premium-allowlist.ts`,
`inference/inference-operator-exemption.ts`, `agent-search.ts`,
`agent-proposal-routes.ts`, `agent-rate-limit-recovery.ts`,
`inference/inference-privacy-receipt-routes.ts`,
`inference/khala-code-paid-plan-payments.ts`,
`orange-check-entitlements.ts`, and (for the orange-check reads)
`forum-routes.ts` — each needs env/flag threading through existing
Effect-based call chains, a materially larger and riskier change than fit
safely in this pass; it stays open. Per the current owner-directed policy
above the KS-8.1/8.2 status paragraphs ("the old per-domain soak/drop
follow-ups are not active gates; KS-8.19 owns final D1 retirement"), moving
write authority off D1 and dropping this domain's remaining 28 tables is
explicitly DEFERRED to KS-8.19
[#8330](https://github.com/OpenAgentsInc/openagents/issues/8330) rather
than done per-domain here, superseding the #8336 issue text's original
"drop the D1 tables" ask. #8336 stays open with this status.

**KS-8.9 decommission follow-up, part 2 (2026-07-05, #8336): bounded
non-gate read allowlist, following the KS-8.14 business-domain precedent
(#8360).** On closer per-call-site review, the "route the non-gate reads to
Postgres" item above turned out to be mostly enforcement/idempotency
hazards in disguise, not display reads:

- `countProviderRequestsSince` / `countQuotaEventsSince` /
  `readRequestByIdempotencyKeyHash` / `consumeEntitlement`
  (`agent-search.ts`) all decide a rate-limit, dedupe, or consume outcome —
  a lagging Postgres read could allow a double provider call or a double
  charge. `readFreshCache` is lower-risk but still a
  serve-vs-re-query-provider decision. All stay D1-only permanently.
- `readChallengeById` / `readChallengeByIdempotencyKeyHash` /
  `readEntitlementByRef` / `readReceiptByRef` / `readRedemptionByChallengeId`
  (`agent-rate-limit-recovery.ts`) validate redemption/entitlement legitimacy
  before a consume — same hazard class. Stay D1-only permanently.
- The two reads sometimes shorthanded as "admin/list reads for premium
  allowlist + operator exemption" turned out to BE the two existing
  enforcement gate reads (`premiumAllowlisted` / `operatorExempt`) — no
  separate admin/list route exists in source today. Already governed by
  `KHALA_SYNC_ENTITLEMENTS_READS`, unchanged.
- `khala-code-paid-plan-payments.ts`'s reads are against
  `khala_code_paid_plan_payment_intents`, a KS-8.7 BILLING-domain table
  (imports `BillingDomainMirror`, not this domain's mirror) — out of scope
  for KS-8.9 entirely; it does call this domain's
  `grantPaidPrivacyEntitlement` for the write-side fulfillment step, but
  that is a write, not a read.
- `grantPaidPrivacyEntitlement`'s and
  `recordConfidentialComputeExecutionReceipt`'s own read-backs
  (`inference-privacy-receipt-routes.ts`) read the row the SAME request just
  wrote to D1, before the async mirror could possibly have landed it in
  Postgres — read-your-own-write hazard, stays D1-only permanently.

What remained after that filter was genuinely safe: THREE public-projection
reads that decide nothing —
`orange-check-entitlements.ts`'s `countActiveOrangeChecks` (public stat) and
`readActiveOrangeCheckByActorRef` (public badge lookup), and
`inference-privacy-receipt-routes.ts`'s `readPublicPrivacyReceipt` (the
public `/api/public/inference/privacy-receipts/{receiptRef}` GET
projection). Landed in
`apps/openagents.com/workers/api/src/inference-entitlements-store.ts`:

- A brand-new, FULLY INDEPENDENT flag,
  `KHALA_SYNC_ENTITLEMENTS_NON_GATE_READS` (d1|compare|postgres, default
  d1) — flipping it can never move `KHALA_SYNC_ENTITLEMENTS_READS` (the six
  enforcement gate reads), and vice versa. The enforcement flag is left
  UNTOUCHED at its default `d1` — this pass does not attempt the multi-hour
  representative soak that changing the ALLOW/DENY authority would need.
- `InferenceEntitlementsNonGateReads` + `makeD1InferenceEntitlementsNonGateReads`
  + the Postgres side in `makePostgresInferenceEntitlementsStore` + the
  three-mode router `makeRoutedEntitlementsNonGateReads` (d1 — untouched
  inline reads; compare — D1-serve + off-path Postgres shadow-compare,
  logging `khala_sync_entitlements_non_gate_read_compare_mismatch`;
  postgres — REAL Postgres serve with single-attempt D1 fallback + the
  `khala_sync_entitlements_non_gate_postgres_read_fallback` diagnostic —
  safe to actually serve here because none of the three decides an
  allow/deny/consume outcome).
- Wired at all 5 call sites (`orange-check-entitlements.ts`'s two functions
  gained an optional `nonGateReads` param; `forum-routes.ts`'s
  `ForumRouteDependencies` gained `entitlementsNonGateReads`, threaded
  through `orangeCheckNostrExportResponse`, `agentProfileResponse`,
  `agentProfilePageResponse`, the post-detail author-badge read, and the
  `/api/forum/launch-status` count; `inference-privacy-receipt-routes.ts`'s
  `PrivacyReceiptRoutesDeps` gained `nonGateReads`, threaded through
  `handlePublicPrivacyReceiptRead`); `index.ts` constructs it from
  `makeInferenceEntitlementsRoutingForEnv(env)?.nonGateReads` at both call
  sites. Absent/`'d1'` ⇒ byte-identical inline D1 behavior everywhere.
- Contract-suite coverage
  (`inference-entitlements-repository.contract.test.ts`): a NEW test proves
  real D1-vs-Postgres ANSWER parity (not just decision parity) for the
  orange-check count/lookup and BOTH privacy-receipt kinds (entitlement +
  confidential-compute), including the not-found case on each side. Unit
  coverage (`inference-entitlements-store.test.ts`) pins the flag's
  independence from `reads`, the router's own (distinct) diagnostic event
  names, and real-serve-with-fallback behavior.
- Production evidence (2026-07-05): fresh backfill sweep + `--restart`
  catch-up sweep + `--verify` against `khala_sync_prod` — VERIFY OK, exact
  counts on all 28 tables (`orange_check_entitlements` d1=2/postgres=2,
  `inference_privacy_entitlement_receipts` and
  `inference_confidential_compute_execution_receipts` d1=0/postgres=0 — the
  privacy-receipt endpoint carries zero production traffic today, so its
  real-serve path is proven by the contract suite, not live volume),
  newest-50 hashes match, all three tally=SUM(events) invariants exact.
- Deploy + flag-flip decision recorded on epic
  [#8282](https://github.com/OpenAgentsInc/openagents/issues/8282).

**KS-8.9 decommission follow-up, part 3 (2026-07-05, #8336): enforcement-gate
compare-mode soak observability bring-up.** Parts 1-2 above left
`KHALA_SYNC_ENTITLEMENTS_READS` untouched at `d1` specifically because there
was no durable production log/metrics surface for a genuine multi-hour
representative-window soak. That blocker is now removed by the shared
compare-mode soak observability tool (#8282 follow-up, commit `6c2cf72b1a`):
`makeRoutedEntitlementsGateReads`'s `compare` branch records a durable
Analytics Engine data point (`domain: "entitlements_gate"`) on every
match/mismatch/shadow-read-error, additive to the existing
`khala_sync_entitlements_read_compare_mismatch` diagnostic — this was
already wired before this pass (`inference-entitlements-store.ts` lines
~1303-1328), it was simply never exercised because the flag itself stayed
at `d1`. This pass flips `KHALA_SYNC_ENTITLEMENTS_READS` from `d1` to
`compare` in both `staging` and production `vars`
(`apps/openagents.com/workers/api/wrangler.jsonc`), so real soak time starts
accumulating from this deploy forward. This is observation-only: `compare`
still serves every gate decision from D1 and cannot itself change an
ALLOW/DENY outcome. A future pass queries
`packages/khala-sync-server/scripts/query-compare-soak.ts` for the
`entitlements_gate` domain once a genuinely representative window has
accumulated (not `VACUOUS`) and only then evaluates a `postgres` flip —
never on this pass's evidence alone, and only with the epic-gated ops
decision on #8282. Rollback at any time: `KHALA_SYNC_ENTITLEMENTS_READS=d1`.

**KS-8.7 status (2026-07-04):** machinery LANDED — Postgres schema
(`khala-sync-server` migration `0015_billing_pay_ins.sql`: the 22 live
billing/Stripe/pay-ins/buyer-payment tables; the
`billing_ledger_entries_next` rewrite artifact was renamed away by worker
0031/0170 and does not exist live, so nothing was created for it; indexes
re-derived — PKs + every idempotency UNIQUE port exactly, plus only the
one secondary index the routed balance read uses, rationale in the
migration header), the fail-soft READ-BACK mirror seam
(`apps/openagents.com/workers/api/src/billing-store.ts`: after a D1
write, the mirror re-reads the fresh authoritative row(s) by key and
converge-upserts byte-identical copies — amounts and idempotency keys are
copied, never recomputed, and no idempotency decision is ever re-made
against Postgres), flags `KHALA_SYNC_BILLING_DUAL_WRITE` (default on) /
`KHALA_SYNC_BILLING_READS` (d1|compare|postgres, default d1; routes ONLY
the display balance read behind an explicit `routeReads` opt-in — gates,
evaluators, and receipt inputs always read D1), mirror wiring across
billing.ts (all credit/debit/policy/notification ops), the FULL Stripe
webhook ingest path (`stripe_webhook_events` dedupe gate + checkout
fulfillment + customers/sessions/saved-payment-methods), Khala Code
paid-plan intents (Stripe + Lightning rails), the buyer-payment ledger
store (decorator on all six create paths + the site-checkout challenge),
and the pay-in statement plans (`runLedgerStatements` mirrors annotated
pay_ins + legs on the tip-ladder, tips-sweep, forwarding-reconcile,
USD-credit-bridge, and MPP/Lightning mint paths), resumable backfill +
exact-verify CLI (`packages/khala-sync-server/scripts/backfill-billing.ts`:
exact counts, the FULL per-user balance map to the cent, per-(currency,
source) cents, per-(type, state)/(direction, kind) msat, webhook event-id
SET equality, newest-N row hashes), and a contract suite run against BOTH
stores including the webhook-replay idempotency regression and fail-soft
proofs. MONEY DISCIPLINE: D1 stays the SOLE authority; the production
read-flag flip is an EPIC-GATED ops decision (#8282) taken only on a
green money `--verify`. Writers still D1-only pending the decommission
follow-up (backfill-converged until then): `first_batch_payment_policies`
(operator triage), codex-usage debits from callers that build their own
`OmniRunStore` without `billingRuntime`, and the remaining low-volume
`runLedgerStatements` consumers (labor-escrow, metering-hook,
inference-abuse-controls, serving-node-payout, cloud-metering,
product-promises, business-starter-credit). Prod cutover
procedure: [`RUNBOOK.md`](./RUNBOOK.md) "Billing/Stripe/pay-ins domain
cutover"; cutover evidence + D1 drop tracked on epic
[#8282](https://github.com/OpenAgentsInc/openagents/issues/8282).

**KS-8.16 status (2026-07-04):** machinery LANDED — Postgres schema
(`khala-sync-server` migration `0021_forge_domain.sql`: ALL SIXTEEN
`forge_*` twins, column-for-column with worker migrations
0251–0256/0259/0260/0284; indexes re-derived from the five owning
stores' actual reads — D1 artifacts with no live read (token prefix,
object-by-packfile) are dropped, and the D1 uniques/partial-uniques
(active-lease-per-work, held-lock-per-ref, token-hash, packfile digest,
mirror destination tuple) are deliberately NOT ported mid-migration so a
transiently stale mirror can never reject a converge upsert — rationale
in the migration header). THE SEAM: the domain's writes are already
CLOSED behind the five typed forge stores (coordination, git canonical,
packfile archive, tenant git auth, GitHub mirror — the only writers of
the sixteen tables), so the wiring is the KS-8.5 store-factory wrap:
five `makeForge*StoreForEnv` drop-ins
(`apps/openagents.com/workers/api/src/forge-domain-store.ts`) whose
write methods read back the affected rows by composite key after the
authoritative D1 write and converge-upsert the byte-exact rows into
Postgres, fail-soft (`khala_sync_forge_dual_write_failed` is the drift
metric). Wired at ALL construction call sites: the git intake routes,
the control-plane routes, the three agent-definition webhook paths, the
dynamic run request path, assignment token revocation, and the
AgentDefinitionScheduler tick (injectable stores, same pattern as
KS-8.1/8.5). REF LOCKING: D1 stays the sole lock authority — the
held/applied/rejected dance is NOT emulated in Postgres; porting the
protocol onto real `SELECT ... FOR UPDATE` is the read/write-cutover
step (§3.13). SECRETS (invariant 9): the token twin stores exactly what
D1 stores (hashes/prefixes, no widening); custody values never appear in
diagnostics (the one token_hash-keyed mirror path redacts its refs) or
in backfill/verify output. Flags `KHALA_SYNC_FORGE_DUAL_WRITE` (default
on) / `KHALA_SYNC_FORGE_READS` (d1|compare|postgres, default d1;
`compare` shadow-compares the canonical `listRefs` ref advertisement —
the §3.13 ref-set surface; `postgres` SERVES that ref advertisement from
the Postgres twin via `makePostgresForgeGitCanonicalStore.listRefs` and is
FAIL-SOFT — any Postgres error falls back to the D1 authority for that one
call and logs `khala_sync_forge_postgres_read_serve_failed`, so the
advertisement can never break). Resumable backfill +
exact-verify CLI (`packages/khala-sync-server/scripts/backfill-forge.ts`:
exact counts, per-state tallies, per-(tenant, repository) REF-SET
digests — the storage twin of `git ls-remote`; the live ls-remote
cross-check is the runbook's cutover step — per-(tenant, queue)
merge-queue LEDGER REPLAY digests, newest-N row hashes; secret-safe
output), and a contract suite run against BOTH engines
(`forge-domain-repository.contract.test.ts`: composite-PK converge on
D1/SQLite AND Postgres, end-to-end mirror fidelity across all sixteen
tables through the real stores incl. lease double-fire idempotency and
mirror-receipt attempt bumps, fail-soft proofs, custody redaction,
compare-read drift detection). Prod cutover procedure:
[`RUNBOOK.md`](./RUNBOOK.md) "Forge domain cutover"; cutover evidence,
the FOR UPDATE lock-protocol port, read/write cutover, and the D1 drop
are tracked in the follow-up
[#8358](https://github.com/OpenAgentsInc/openagents/issues/8358).

**KS-8.16 follow-up status (2026-07-05, #8358):** production cutover
EVIDENCE landed. Migration `0021_forge_domain.sql` confirmed already
applied (`bun scripts/migrate.ts --dry-run` against the direct Cloud SQL
URL: 34/34 files already applied, `0021` among them). Ran the backfill
TWICE (`bun scripts/backfill-forge.ts --restart`, then a third
convergence sweep after an out-of-band verification write below) —
production Forge traffic is genuinely tiny today (one live tenant,
single-digit rows per table), and `--verify --verify-newest 50` is
CLEAN: exact row counts, all newest-hash checks match, the per-(tenant,
repository) ref-set digest matches (1 repository), and the
per-(tenant, queue) merge-queue replay digest matches (0 queues). THE
GIT-PROTOCOL GROUND-TRUTH CROSS-CHECK (§3.13's actual acceptance
authority): the live Forge intake surface is push-only (it implements
`GET .../info/refs?service=git-receive-pack` — the receive-pack
advertisement — but there is no `git-upload-pack` route, so plain
`git ls-remote <url>` cannot run against it as a black-box CLI
invocation). Minted a bounded 15-minute `git:receive-pack`-scoped
verification token for the one live tenant/repository
(`tenant.openagents` / `repo.openagents.issue6771.live.20260628190038-48007`),
called the real advertisement endpoint with it, and parsed the pkt-line
response by hand: it advertised `refs/heads/main` at
`a909337789007a12fa1dd48d5acf2cdfa44fe165` — an EXACT match against both
stores' `forge_git_refs` row for that ref. The token was revoked
immediately after use and the resulting D1 rows (mint + revoke) were
converged into Postgres by a follow-up backfill sweep, re-verified clean.
READ CUTOVER — DONE (`KHALA_SYNC_FORGE_READS=postgres`, second pass
2026-07-05): the canonical `listRefs` ref advertisement now SERVES from
the Postgres twin (fail-soft; any Postgres error falls back to the D1
authority for that call and logs `khala_sync_forge_postgres_read_serve_failed`,
so the advertisement can never break). Gated on this-session evidence:
(1) a FRESH full backfill `--verify --verify-newest 50` against the
direct prod Cloud SQL URL — CLEAN across all 16 tables (exact row counts,
all newest-50 row hashes match, the per-(tenant, repository) REF-SET
DIGEST — the §3.13 ls-remote twin — matches for the 1 repository,
merge-queue replay digests match); (2) the prior live git-advertisement
ground-truth cross-check (exact `refs/heads/main` object-id match); (3)
~20 minutes of `wrangler tail` on production with ZERO forge
advertisement requests and ZERO forge diagnostics (this domain has
effectively no organic traffic, so a passive soak yields no comparisons —
the fresh full `--verify` is the stronger, higher-signal evidence and was
used instead of minting further live tokens for synthetic traffic). The
serving change is unit-tested in `forge-domain-repository.contract.test.ts`
(postgres mode returns the Postgres value, not D1; a dead twin falls back
to D1 and logs the serve-failed diagnostic). Earlier compare-mode soak
(first pass): Worker `75c8132b-9994-4a59-a17a-751e185b011d`, ~7 minutes +
6 advertisement reads, zero drift. REF-LOCK PROTOCOL PORT: implemented and
tested, still NOT wired to production write authority (see REMAINING WORK).
`forge-git-canonical-postgres-store.ts`
(`makePostgresForgeGitCanonicalStore`) replaces the D1
held/applied/rejected lock-row dance with a real
`pg_advisory_xact_lock` (keyed per `tenant_ref`/`repository_ref`/
`ref_name`, transaction-scoped, released automatically at
COMMIT/ROLLBACK — needed because a 'create' target ref has no row yet
for a plain row lock to hold) PLUS a real
`SELECT ... FOR UPDATE` on the ref row when one already exists (the
literal §3.13 mechanism, re-validating the CAS precondition under lock).
No lock-row bookkeeping of any kind. `forge-git-canonical-postgres-store.test.ts`
proves it against a real ephemeral Postgres, including two genuine
concurrency races (two simultaneous CREATEs of the same brand-new ref;
two simultaneous UPDATEs racing the same `old_object_id`) — in both
cases exactly one transaction wins, the other is rejected with the same
typed `forge_git_unsafe_ref_update`/`ForgeGitCanonicalStoreError` the D1
lane raises, and the final ref state is never corrupted. REMAINING WORK
(left honestly undone — the WRITE cutover): wiring
`makePostgresForgeGitCanonicalStore` as write authority is NOT a one-store
swap. The canonical git store is one of FIVE forge stores that all write
D1-first then mirror to Postgres; flipping only it to Postgres write
authority would split authority incoherently (canonical refs on Postgres,
the other 15 tables on D1, mirror running the wrong direction). It also
requires re-adding the six deliberately-unported Postgres uniques
(active-lease-per-work, token-hash, packfile digest, mirror-destination
tuple, github-issue-number/change_ref; held-lock-per-ref is moot under the
new locking) so the Postgres write authority enforces the same integrity
D1 does. This is a coordinated, domain-wide write cutover, deliberately
left unwired this pass rather than risk a tenant's git-ref integrity on a
piecemeal flip — tracked on #8358. The D1 drop is confirmed consolidated
into the epic-wide KS-8.19 sweep (#8330), not attempted here.

**KS-8.16 follow-up status (2026-07-05, #8358, third pass): constraint
parity landed — write cutover STILL deliberately unwired.** Re-derived
the D1 uniques EXACTLY from the worker migration files
(0251/0252/0253/0255/0260) rather than trusting the prior pass's tracking
comment, and found NINE distinct unique indexes/constraints were missing
from the Postgres twin, not six: the prior count bundled two pairs
together ("packfile digest / R2 key" is two separate D1 UNIQUE indexes on
different columns — `idx_forge_git_packfile_archives_digest` and
`idx_forge_git_packfile_archives_r2_key`; "github-issue-number/change_ref"
is one unique index each on two different tables) and MISSED
`forge_dispatch_leases`' `idx_forge_dispatch_leases_idempotency`
(`UNIQUE (tenant_ref, idempotency_key_hash) WHERE idempotency_key_hash IS
NOT NULL`) entirely. Verified a real backfilled copy of BOTH `khala_sync_prod`
and `khala_sync_staging` for existing violations of all nine candidate
constraints before writing anything — zero violations on either database
(prod Forge traffic is still tiny: single/low-digit rows per table).
Wrote and applied `khala-sync-server` migration
`0035_forge_domain_ref_lock_uniques.sql` (staging then prod, via
`bun scripts/migrate.ts`) adding all nine: `forge_coordination_issues`
(tenant_ref, github_issue_number), `forge_coordination_prs` (tenant_ref,
change_ref), `forge_dispatch_leases` (tenant_ref, work_ref) WHERE
state='active', `forge_dispatch_leases` (tenant_ref,
idempotency_key_hash) WHERE NOT NULL, `forge_git_packfile_archives`
(tenant_ref, packfile_sha256) [replacing the existing plain index of the
same name], `forge_git_packfile_archives` (artifact_r2_key),
`forge_git_access_tokens` (token_hash) [a new index, distinct from the
existing (token_hash, state) auth-lookup index], `forge_git_ref_locks`
(tenant_ref, repository_ref, ref_name) WHERE state='held' [moot for the
new advisory-lock write path — kept for schema parity], and
`forge_github_mirror_receipts` (tenant_ref, promotion_ref,
destination_github_repository, destination_github_ref) [a new index,
distinct from the existing listing index that also orders by
updated_at]. Post-migration `bun scripts/backfill-forge.ts --verify
--verify-newest 50` re-ran CLEAN on prod: all 16 tables exact row counts
+ newest-hash matches, ref-set digest matches, merge-queue replay digest
matches. The new unique constraint on `forge_git_access_tokens.token_hash`
immediately caught a real test-fixture bug in
`forge-backfill.test.ts` — two `tokenRow()` fixture rows shared one
hardcoded fake hash (`"e3".repeat(32)`), which is unrealistic test data
(a real SHA-256 collision), not a production scenario; fixed to derive a
distinct hash per row. Full `khala-sync-server` suite (355 tests) and the
seven forge-prefixed suites in `workers/api` (49 tests) pass;
`workers/api` typecheck, `check:architecture` zero-debt, and
`check:deploy` (full suite) are clean. **Write cutover — RE-EVALUATED and
STILL deliberately NOT wired**, for two reasons: (1) the domain-wide
write-authority-incoherence concern from the second pass is unchanged —
flipping only the canonical git store still splits authority across the
other four forge stores, which remain D1-first with no coordinated
plan landed this pass to flip all five together; (2) a NEW finding on
inspection this pass: `forge-git-canonical-postgres-store.ts` has NO
path that mirrors its writes back into D1 at all (it is a pure
Postgres-only implementation). Wiring it as write authority today would
mean D1 goes stale for canonical git tables immediately, and the
existing FAIL-SOFT-to-D1 read fallback (used when `KHALA_SYNC_FORGE_READS
=postgres` and a Postgres call errors) would then silently serve stale
ref state instead of failing loud — worse than today's D1-authoritative
behavior. A safe write cutover needs either a reverse D1-mirror write
path added to the Postgres store, or an explicit accepted-risk decision
to drop the D1 read fallback once Postgres is write-authoritative; this
pass does not resolve that gap, so per the task's "if in doubt, leave D1
as sole write authority" guardrail, write authority stays on D1 for all
five forge stores. Still tracked on #8358; the D1 drop remains
consolidated into #8330.

**KS-8.16 follow-up status (2026-07-05, #8358, fourth pass): D1
mirror-back gap CLOSED — write cutover reduced to a routing decision.**
Closed the exact gap the third pass named:
`makePostgresForgeGitCanonicalStore` now takes an OPTIONAL second
`mirror` argument (`ForgeGitCanonicalD1MirrorDeps`); when provided, every
successful write (`applyReceivePack`, `importExternalRef`)
converge-upserts its already-RESOLVED rows (no extra read-back — the
rows are already available via `RETURNING`/in-tx reads inside the same
Postgres transaction) into a D1 twin, fail-soft with bounded retry
(100ms/400ms, matching `mirrorArtanisRows`), logging
`khala_sync_forge_postgres_write_mirror_retry` /
`khala_sync_forge_postgres_write_mirror_failed` and never throwing.
Passing no `mirror` reproduces the exact prior behavior. Six new tests
(`forge-git-canonical-postgres-store-d1-mirror.test.ts`) prove
byte-faithful mirroring for create/update/import, a fully broken D1
mirror never failing the Postgres write, transient-failure retry
recovery, and the no-mirror no-op case. Still did NOT flip the
production route handler this pass — the domain-wide-incoherence
concern (the other four Forge stores still write D1-first) is unchanged,
and there was zero production traffic history on the mirror path itself.
Reduced the remaining write cutover to exactly two things: (a) the
domain-wide-vs-scoped routing decision, (b) a soak/verification of the
new mirror path — no unbuilt mechanism left.

**KS-8.16 follow-up status (2026-07-05, #8358, fifth pass): WRITE
CUTOVER LANDED for the canonical git store — LIVE in production.**
Wired `KHALA_SYNC_FORGE_GIT_CANONICAL_WRITES` (a flag SEPARATE from and
narrower than `KHALA_SYNC_FORGE_READS`/`KHALA_SYNC_FORGE_DUAL_WRITE`) into
`makeForgeGitCanonicalStoreForEnv`
(`forgeGitCanonicalWritesFromEnv`, `forge-domain-store.ts`): when
`'postgres'`, Postgres becomes the SOLE authority for the canonical git
store's entire surface (preflight/apply/import/read/list) via
`makePostgresForgeGitCanonicalStore` with its mirror-back wired in, and
there is deliberately NO fallback to D1 on a Postgres error (a silent
D1 fallback under this flag would let two ref-lock protocols race the
same ref — a Postgres outage must fail the request loud, not silently
diverge lock authority). Default stays `'d1'`; the production route
handler call site (`index.ts`) needed NO changes — the flip is entirely
this env-var-gated branch, matching the read-cutover pattern. Scoped to
ONLY this store: the other four Forge stores are constructed by
unrelated factories and stay D1-first/mirror-to-Postgres regardless.
New env-wiring integration coverage
(`forge-git-canonical-write-authority.test.ts`, real ephemeral Postgres +
SQLite D1 double): default-`d1` regression, `postgres`-authority
read/write routing (including a drifted-D1 proof that reads are actually
served from Postgres, not D1), and safe fallback to D1 when no
`KHALA_SYNC_DB` binding exists. **Real verification (not just unit
tests):** flipped `KHALA_SYNC_FORGE_GIT_CANONICAL_WRITES=postgres` on
STAGING first, minted a `git:receive-pack`-scoped token for a dedicated
canary tenant/repo (`tenant.ks8-16-write-cutover-canary` /
`repo.ks8-16-write-cutover-canary` — never the real customer
tenant/repo), and ran TWO real `git push` operations through the actual
deployed Worker route: a CREATE (new ref, new orphan commit) and a
fast-forward UPDATE. Both succeeded through the real HTTP smart-protocol
git client, and direct queries against BOTH the staging Postgres
database and staging D1 confirmed byte-identical convergence — exact
`object_id`/`previous_object_id`/`state` on `forge_git_refs`, matching
rows on `forge_git_objects` and `forge_git_receive_pack_intakes` — for
both the create and the fast-forward update. Only after that real
end-to-end proof did production get the same flag flip and a matching
canary-tenant verification (never against the real live
`tenant.openagents` repository). Rollback is one flag flip back to `d1`
(or unset); D1 authority for the other four stores is unaffected either
way. The D1 drop stays out of scope, consolidated into the epic-closing
KS-8.19 sweep (#8330).

**KS-8.17 status (2026-07-04):** core machinery LANDED — Postgres schema
(`khala-sync-server` migration `0024_supervision_longtail.sql`: all 29
supervision long-tail twins — `adjutant_*` (10), `omni_*` (9),
`autopilot_*` (6), `relay_health_*` (2), `backend_incident_events`,
`hygiene_debt_receipts` — column-for-column with the worker migrations
incl. every later ADD COLUMN; type-fidelity TEXT/bigint/smallint, no
FK/CHECK, and the D1 uniqueness constraints deliberately NOT ported
mid-migration except `omni_idempotency_keys` whose PRIMARY KEY *is* the
idempotency key, ported exactly — rationale in the migration header).
Write-dead audit: `autopilot_token_usage` has ONE live writer
(`omni-runs.ts`, so it dual-writes, NOT reconcile-and-freeze);
`omni_idempotency_keys` is unwritten today (twin + verified copy only). One
shared registry
(`packages/khala-sync-server/src/supervision-longtail-domain-tables.ts`) +
row-level converge store, fail-soft read-back mirror, and flags
`KHALA_SYNC_SUPERVISION_DUAL_WRITE` (default on) /
`KHALA_SYNC_SUPERVISION_READS` (d1|compare|postgres, default d1; postgres
serving deferred to the read-cutover follow-up)
(`apps/openagents.com/workers/api/src/supervision-longtail-domain-store.ts`;
drift metric `khala_sync_supervision_dual_write_failed`). LIVE WIRING: the
three re-homed crons + the funded-hygiene store are wired as clean
store-factory drop-ins at their construction sites — `RelayHealth.probeTick`
(`makeRelayHealthStoreForEnv`, incl. the retention-prune converge),
`AutopilotContinuationPolicy.sweep`
(`makeAutopilotContinuationStoreForEnv`),
`AutopilotScheduledLaunches.dispatchDue` (`makeAutopilotWorkStoreForEnv`,
mirroring every work-order write by `work_order_ref` + closeout receipts),
and `makeHygieneDebtReceiptStoreForEnv`. Resumable backfill + exact-verify
CLI
(`packages/khala-sync-server/scripts/backfill-supervision-longtail.ts`:
exact counts, per-state/sum tallies, **idempotency-key-set equality**
(`omni_idempotency_keys`), **public proof-bundle digests**
(`omni_public_proof_bundles`), newest-N row hashes; secret-safe by
construction), and a contract suite run against BOTH engines
(`supervision-longtail-domain-repository.contract.test.ts`: composite-PK
converge on D1/SQLite AND Postgres, read-back mirror byte-fidelity incl. the
prune path, fail-soft drift, flag routing) plus pure-core unit tests
(`supervision-longtail-backfill.test.ts`). REMAINDER (filed as the
decommission follow-up
[#8361](https://github.com/OpenAgentsInc/openagents/issues/8361)): the
scattered `adjutant_*` (per-table services + raw writers in
`customer-orders.ts` / `index.ts` / `operator-email-inspection-routes.ts` /
`adjutant-run-lifecycle.ts`) and inline `omni_*`
(`createOmni…`/`recordOmni…`/`promoteOmniWorkroom`) writers, the
Effect-based onboarding store, `autopilot_token_usage`
(`omni-runs.ts:tokenUsageInsert`), and `backend_incident_events`
(`recordBackendIncidentEvent`) keep their twins + backfill + verify from
this lane but plug their per-site live mirror into
`makeSupervisionLongtailMirrorForEnv` in that follow-up; the follow-up also
carries the compare/postgres read cutover, the shadow-compared public
proof-bundle endpoint serving, and the D1 drop. Prod cutover procedure:
[`RUNBOOK.md`](./RUNBOOK.md) "Supervision long-tail cutover"; cutover
evidence is tracked on epic
[#8282](https://github.com/OpenAgentsInc/openagents/issues/8282).

**KS-8.17 follow-up status (2026-07-05, #8361):** scattered writer wiring
CONFIRMED live and green (commit `4acd3704c2`, deployed) — all 34 previously
unwired `adjutant_*`/`omni_*`/`autopilot_*`/`relay_health_*`/
`hygiene_debt_receipts` call sites mirror through
`makeSupervisionLongtailMirrorForEnv`. Backfill CATCH-UP SWEEP (second pass)
completed across all 29 tables + a full `--verify`: exact row counts,
idempotency-key-set equality (`omni_idempotency_keys`), public proof-bundle
digests (`omni_public_proof_bundles`), and newest-50 row hashes ALL MATCH —
zero drift, confirming dual-write has been converging cleanly since the
writer-wiring deploy. READ-COMPARE MACHINERY BUILT (the piece the parent
lane deferred): `makeOmniPublicProofBundleCompareReader` — a fail-soft
shadow-compare reader, inline-awaited by its call sites (never
fire-and-forget, since a Worker can cancel an un-awaited async tail once the
response is sent) wired into the one public projection surface this domain
serves (`omni_public_proof_bundles`, read by both the
redacted public handoff page and the operator JSON view in
`omni-bundle-routes.ts`). D1 remains the ONLY store that ever serves a
response, at every flag value; `compare`/`postgres` only widen when the
shadow diff fires, logging
`khala_sync_supervision_read_compare_mismatch`/`_failed`/
`_postgres_reads_deferred`. `KHALA_SYNC_SUPERVISION_READS=compare` is set in
prod + staging `wrangler.jsonc` to start a genuine soak. HONEST CAVEAT: this
is a cold, near-zero-traffic domain today (`omni_public_proof_bundles` is 0
rows in both D1 and Postgres as of this backfill; most other tables are
single/double-digit rows) — a short soak window is thin-by-vacuity evidence,
matching the Forge (#8358) precedent, so the real read/write cutover (and
any bounded postgres-serving allowlist, per the #8360 business-domain
pattern) stays a further follow-up gated on a genuinely representative
soak window, not force-flipped here. D1 drop stays consolidated in the
epic-wide KS-8.19 sweep (#8330), not attempted here.
**KS-8.18 status (2026-07-04, #8329):** machinery LANDED (CORE) — the
LAST and most sensitive domain. Postgres schema (`khala-sync-server`
migration `0028_identity_auth_domain.sql`: the SEVENTEEN canonical
identity/auth twins — `users`, `auth_identities`, `openauth_storage`,
`openauth_agent_links`, the three `github_write_*` tables, and the
provider (BYOK) account custody family incl. `provider_account_token_custody`
+ `_audit`; column-for-column with worker migrations
0002/0003/0004/0009/0011/0044–0050/0173/0234/0237/0283; the `_0173_new`/
`_0173_data`/`_0237_data` rebuild artifacts are deliberately NOT twinned;
indexes re-derived from the owning stores' reads; D1 uniques and FKs NOT
ported mid-migration so a transiently stale mirror can never reject a
converge upsert — rationale in the migration header). SECRETS (invariant
9 — the invariant this domain motivated): the twin stores EXACTLY what D1
stores with NO widening and the same at-rest encryption posture; the
custody columns (token ciphertext/IVs/key ids on
`provider_account_token_custody`, `openauth_storage.value_json`,
`provider_account_connection_attempts.user_code`,
`github_write_connection_attempts.state`) are declared in the shared
registry and NEVER appear in diagnostics or backfill/verify output —
row KEYS (ids/refs/owner_user_id) and sha256 row hashes ONLY. Seam +
flags: the KS-8.16 fail-soft read-back mirror
(`apps/openagents.com/workers/api/src/identity-auth-domain-store.ts`)
exposed two ways — `identityAuthMirrorFromEnv(env)` (the uniform adoption
handle every writer calls after its authoritative D1 write) and the
flagship drop-in `makeProviderAccountTokenCustodyStoreForEnv(env)` (the
encrypted-token vault), WIRED in this lane at its two centralized
`index.ts` construction sites. Flags `KHALA_SYNC_IDENTITY_DUAL_WRITE`
(default ON) / `KHALA_SYNC_IDENTITY_READS` (default `d1`); there is NO
read cutover in this lane — `postgres` DEFERS and logs
`khala_sync_identity_postgres_reads_deferred` once (auth read serving +
KV cache + session-revocation verification is the highest-risk,
OWNER-GATED, done-LAST step). Resumable backfill + secret-safe
exact-verify CLI
(`packages/khala-sync-server/scripts/backfill-identity-auth.ts`: exact
counts / identity set equality, custody-safe scalar tallies, newest-N row
hashes), and contract suites on BOTH engines
(`identity-auth-domain-repository.contract.test.ts` — composite-PK
converge on D1/SQLite AND Postgres incl. a custody round-trip, end-to-end
token-custody mirror fidelity, fail-soft, custody redaction;
`identity-auth-backfill.test.ts` — all seventeen twins converge, rotation
idempotency, no tally references a custody column).
REMAINDER (wiring the rest of the write surface is intentionally
NOT in this lane — dozens of construction sites across ~10 hot auth
files is too broad for one secret-bearing change): the other five typed
factories (`makeD1GitHubWriteRepository`, `makeD1ProviderAccountRepository`,
`makeD1Storage`, `makeD1AgentRegistrationStore`, `makeD1AgentOwnerClaimStore`)
and the scattered inline writers (`index.ts` user upserts,
`onboarding/repository.ts`, `auth/email-otp-hardening.ts`,
`operator-provider-account-routes.ts`, `provider-account-pool-routes.ts`,
`artanis-operator-dashboard-routes.ts`) adopt `identityAuthMirrorFromEnv`
in the decommission/wiring follow-up
[#8362](https://github.com/OpenAgentsInc/openagents/issues/8362); until
then their rows converge on the backfill sweep. Prod cutover procedure:
[`RUNBOOK.md`](./RUNBOOK.md) "Identity/auth domain cutover"; the
OWNER-GATED auth read cutover, KV cache, session-revocation proof, and D1
drop are the same follow-up.
**#8362 follow-up, bounded non-gate read allowlist (2026-07-05):** a
SECOND, fully independent read surface, following the entitlements
domain's `*_NON_GATE_READS` precedent (#8336) and the billing/business
bounded-allowlist precedent (#8337/#8360).
`KHALA_SYNC_IDENTITY_NON_GATE_READS` (d1|compare|postgres, default `d1`)
governs ONLY `IdentityAuthNonGateReads.providerAccountPoolStateByUserId`
(`provider-account-usage-routes.ts`'s `listPoolState`) — the ONE read (of
six re-audited candidates) that cleared the conservative bar; the other
five (admin user listing joined to a not-yet-Postgres-served
`software_orders`; the operator-account-status reset route, a
read-your-own-write hazard; the operator triage lease/failover/users
reads, blocked by the domain's own documented `provider_account_leases`
staleness gap and a cross-domain blob shared with an existence-gate
consumer; the CRM target-resolution reads, which feed real money-grant and
account-linking decisions; and the forum author-profile join, which also
gates a follow-creation existence/self-follow check) stay D1-only. Fully
independent of `KHALA_SYNC_IDENTITY_READS`, which stays untouched at `d1`
forever in this follow-up. Machinery:
`identity-auth-domain-store.ts`'s `IdentityAuthNonGateReads`,
`makeD1IdentityAuthNonGateReads`, `makeRoutedIdentityAuthNonGateReads`, and
the `identityAuthNonGateReadsForEnv(env)` factory. Prod cutover procedure
and flag-flip evidence: [`RUNBOOK.md`](./RUNBOOK.md) "Identity/auth domain
cutover" §"2026-07-05 follow-up (#8362): bounded non-gate read allowlist".

Everything except the Verse world runs through one D1 database
(`openagents-autopilot`, binding `OPENAGENTS_DB`). As of `main` today the
migrations directory (`apps/openagents.com/workers/api/migrations/`) holds
**312 migration files** creating **~330 logical tables**. The end state
(fable report §5): Cloud SQL Postgres HA is the authoritative relational
core reached via Hyperdrive; telemetry firehose lives on R2 + Analytics
Engine + DO buffers, never on the relational core; D1 is retired from the
hot path and survives, if at all, only as bounded staging.

## 1. The standard migration recipe

Every domain below runs the same five-step recipe (the KS-8.1/8.2 shape,
generalized). Deviations are called out per domain.

1. **Dual-write behind a flag.** Postgres schema lands first (khala-sync
   migration runner, KS-0.3). The domain's write paths gain a flag-gated
   second write to Postgres (same request, after the D1 write; Postgres
   write failure logs + counts but never fails the request during this
   phase). All writes go through single-transaction mutator-shaped
   functions — Hyperdrive is transaction-mode pooling: no session state, no
   cross-request transactions (see [`MUTATORS.md`](./MUTATORS.md)).
2. **Backfill.** Batch copy D1 → Postgres (bounded pages, resumable
   cursor, `ON CONFLICT DO NOTHING` so backfill and dual-write never
   fight). Backfill runs while dual-write is on, then re-sweeps once.
3. **Exact reconciliation.** The 2026-06-29 after-action methodology:
   per-table row counts, plus domain-specific exactness (money totals to
   the cent/millisat, receipt content hashes, counter equality, event-chain
   contiguity). Reconciliation evidence goes in the migration issue before
   any read cutover. Nothing "close enough": exact or explain.
4. **Cut reads.** Flag-flip reads to Postgres, optionally after a
   shadow-read window (read both, compare, serve D1). Public projections
   (receipt pages, counters) must never 404 or regress mid-cutover —
   dual-read fallback until soak completes. Cron tasks that read/write the
   domain re-home in the same cutover (§4).
5. **Retire D1 only in KS-8.19.** The original recipe used a per-domain
   ≥7-day soak followed by stopping dual-write, snapshotting D1 tables to
   R2, dropping those tables, and deleting the flag. Owner direction on
   2026-07-04 skips those per-domain soak/drop tickets so the migration
   fanout keeps moving. Until KS-8.19, keep D1 rollback/fallback paths
   explicit and documented; destructive drops happen only in the final D1
   retirement sweep.

**Universal porting rules** (apply to every domain):

- **Idempotency keys port exactly.** D1's `INSERT OR IGNORE` +
  `UNIQUE(...)` becomes `INSERT ... ON CONFLICT (...) DO NOTHING` with the
  same key columns. Every dedupe-SELECT-then-INSERT pair in the D1 code
  collapses to a single upsert in Postgres — this is a correctness
  improvement (no TOCTOU window), but the key set must be identical.
- **Counters and rollups are derived, never invented.** Any counter that
  crosses the store boundary must reconcile to exact source rows (SPEC
  invariant 8).
- **Indexes are re-derived from actual query patterns**, not blindly
  ported (the KS-8.2 rule, generalized — the token ledger's 13+ D1
  indexes are the cautionary example).
- **Types:** v1 keeps D1's representations byte-compatible where
  reconciliation depends on it (TEXT ISO-8601 timestamps, INTEGER 0/1
  booleans stored as `smallint`/`boolean` with lossless mapping, JSON as
  `jsonb` only when key-order-independent comparison is acceptable,
  otherwise `text`). Tightening to native types is a post-retirement
  cleanup per domain, never mid-migration.
- **Rewrite artifacts collapse.** D1's no-ALTER workarounds left
  table-rewrite pairs behind (`*_0173_data`, `*_0193_new`, `*_0261_new`,
  `*_next`, `*_0275`). Only the live canonical table migrates; the
  artifact twins are verified-empty-or-superseded and dropped with the
  domain.
- **Firehose exclusion.** Append-only telemetry streams (heartbeat/status
  events, `*_metric_events` counters) are candidates for Analytics Engine
  + R2, not Postgres rows. Each domain section flags its firehose split.
- **Khala Sync exposure is optional per domain.** Migrating a domain to
  Postgres does not require putting it on the sync engine; domains whose
  surfaces poll today (fleet cockpit, threads) should adopt scopes +
  changelog writes at cutover, others just get transactional Postgres.

## 2. Ground truth: table families on `main`

Census from `CREATE TABLE` statements across the 312 migration files
(counts are distinct logical tables incl. rewrite artifacts):

| Family (prefix) | Tables | What it is |
|---|---:|---|
| `forum_*` | 36 | Forum: boards/topics/posts/trust + tips/money actions |
| `site_*` (incl. `site_builder_*`, `site_commerce_*`) | 33 | Sites product + builder sessions + site commerce |
| `agent_*` | 31 | Agent runtime (definitions/runs/traces) + entitlement/payment sub-family |
| `artanis_*` | 20 | Artanis supervision loops (per-minute cron writer) |
| `business_*` | 19 | Business funnel/fulfillment/outreach/pipeline |
| `provider_account_*` | 18 | Provider (BYOK) account custody, leases, probes |
| `forge_*` | 16 | Forge git intake/coordination/mirroring |
| `targeted_site_*` | 15 | Targeted-site prospecting/capture/campaigns |
| `inference_*` | 15 | Free-tier/entitlement/allowance accounting |
| `email_*` | 15 | Email deliveries/campaigns/templates/suppression |
| `pylon_*` | 13 | Pylon control plane (2 tables → KS-8.1) + marketplace + raw-event chunks |
| `crm_*` | 13 | CRM accounts/contacts/opportunities/MCP grants |
| `gym_*` | 11 | Gym eval runs / delegation / leaderboards |
| `adjutant_*` | 10 | Adjutant assignment enrichment/research |
| `omni_*` | 9 | Omni workrooms/outcome contracts/evidence |
| `khala_*` | 9 | Khala product surfaces (feedback, acceptance, receipts) |
| `billing_*` | 8 | Credits ledger, auto-top-up, coupons |
| `buyer_payment_*` | 7 | Buyer payment challenges/receipts/limits |
| `training_*` | 7 | Training runs/windows/verification |
| `autopilot_*` | 6 | Autopilot work orders/continuations/onboarding |
| `nexus_*` | 6 | Treasury payout intents/attempts/gates |
| `order_*` | 6 | Order triage/fulfillment/receipts |
| `exa_*` | 6 | Exa enrichment cache/budget/runs |
| `mullet_*` | 5 | Mullet simulation runs/exports |
| `stripe_*` | 4 | Stripe customers/sessions/webhook events |
| `labor_*` | 4 | Labor escrows + receipts |
| `team_*` + `teams` | 5 | Teams, memberships, chat, invites |
| `thread_*` | 3 | Threads/messages/files |
| `sync_*` | 3 | Legacy `sync-worker` outbox — superseded by Khala Sync, decommissioned not migrated |
| `training`/`gym`-adjacent (`blueprint_*`, `mirrorcode_runs`, `replay_clip_jobs`) | 5 | Eval/experiment lanes |
| everything else | ~60 | users/auth/openauth/github_write, referrals, treasury, mpp, relay health, event ledger, prefilled workspaces, workroom templates, cloud sandbox/fine-tuning, share projections, misc receipts |

Two tables (`pylon_api_assignments`, `pylon_api_events`) migrate under
KS-8.1; `token_usage_events` + `token_usage_leaderboard_preferences` +
`public_khala_tokens_served_*` rollups (4) under KS-8.2. Everything else
is sequenced below.

## 3. Domains

Each domain becomes one implementation issue (map in §7). Format: what it
is / tables / heat / risks / verification / cron / dependencies.

### 3.1 KS-8.4 — Pylon control-plane remainder + telemetry split

**KS-8.4 substrate status (2026-07-04):** schema/backfill spine landed in
progress — `khala-sync-server` migration
`0009_pylon_control_plane_remainder.sql` creates Postgres twins for the
remaining Pylon control-plane metadata tables, including the raw Codex event
metadata indexes (R2 payload bodies stay out of Postgres). The direct
backfill/verify tool is
`packages/khala-sync-server/scripts/backfill-pylon-control-plane.ts`; it
copies bounded rowid pages from D1, preserves natural-key `ON CONFLICT DO
NOTHING` semantics, and verifies exact row counts, per-domain tallies, and
newest-N hashes. Partial Worker mirrors are live in source for
assignment-derived provider lifecycle, explicit provider lifecycle updates,
Pylon quarantines, Pylon marketplace intake/assignment/triage writes, raw
Spark payout target registrations, scheduled
`PylonCapacityFunnel.recordSnapshots` capacity-funnel snapshot upsert/prune
writes, registered-agent `pylon_agent_runner_status_events` ingest, and
Artanis Fleet tick `pylon_agent_runner_status_events` writes, plus
`fleet_alerts` cron alert rows from FleetBurnStallDetector and
ServingRateMonitor. The raw Codex event metadata path now writes payloads to
R2 on the request path and enqueues metadata rows through
`PYLON_CODEX_RAW_EVENT_METADATA_QUEUE`; its consumer preserves D1 indexing and
fail-soft mirrors Postgres when `KHALA_SYNC_PYLON_DUAL_WRITE` is armed. These
paths remain D1-first and read-authoritative until verification/cutover.
Runner-status spine reads now honor `KHALA_SYNC_PYLON_READS` for D1,
compare, and Postgres modes across the operator fleet-status route and the
in-worker Artanis status-spine loader; compare serves D1 while adding the
Postgres shadow source ref and logging drift. Pylon Codex proof and
trace-status closeout reads now route their raw-event metadata sections through
the same D1/compare/Postgres seam, exposing D1/Postgres-shadow source refs and
logging raw-metadata drift. The backfill CLI now has `--raw-event-reconcile`
for live raw Codex metadata aggregate parity and per-turn chunk-chain
contiguity evidence. Production backfill/reconciliation has established exact
D1/Postgres parity for raw Codex metadata rows, but the historical source data
contains 678 unique per-turn chunk-chain gaps under start-at-1 semantics,
mirrored identically into both stores. The Pylon Codex runner now retries
failed chunk reports under the same next chunk index instead of burning an
index on a transient send failure, so new chunk streams should not create the
same hole pattern. The reconcile tool now reports D1/Postgres/shared/unique gap
counts and can focus the chunk-gap gate on chains whose latest observed row is
after an explicit post-fix cutoff. It also classifies unique gapped chains by
final turn-event presence and by shape (missing first chunk, internal missing
chunk, duplicate chunk indexes); production classification is currently
`turn_event=511`, `live_stream_only=167`, `missing_first=19`, `internal=659`,
and `duplicate_indexes=0`. The bounded historical exception is explicit:
`--raw-event-accept-historical-gaps-before` accepts only duplicate-free chains
observed before the cutoff and fails newer/unknown-observed or duplicate gaps.
The Worker config now commits `KHALA_SYNC_PYLON_READS=postgres` for production
and staging, so Pylon dispatch, runner-status, and raw Codex proof metadata
reads use Cloud SQL with bounded retry and D1 fallback. Remaining #8315 closure
work is recorded on
[#8315](https://github.com/OpenAgentsInc/openagents/issues/8315); destructive
D1 decommission is deferred to the final KS-8.19 retirement sweep rather than
blocking Wave A.

**KS-8.4 fresh backup verification (2026-07-05, #8282/#8330 follow-up):** a
follow-up to the same-day forum-and-user-content backup pass checked this
domain fresh. Fresh `backfill-pylon-control-plane.ts --verify
--verify-newest 50` against production: **VERIFY FAILED** on 5 of 11
tables — `pylon_provider_job_lifecycle` (10,657 D1 / 2 Postgres),
`pylon_agent_runner_status_events` (25,287 / 18),
`pylon_capacity_funnel_snapshots` (521 / 21), `pylon_spark_payout_targets`
(23 / 0), `fleet_alerts` (2,440 / 111). `pylon_codex_raw_events` (1,354)
and `pylon_codex_raw_event_chunks` (139,086) were ALREADY exact, matching
this section's claim above for that specific raw-event-metadata sub-lane.
Ran the standard sweep: all 5 gap tables are now **backfilled and confirmed
exact**, except `pylon_capacity_funnel_snapshots`, which shows an exact row
count (521/521) but 11 of its newest 50 rows have a genuinely different
content hash between stores — a continuously re-upserted rolling snapshot
table (the cron re-writes hourly/daily buckets in place), so this reads as
live-write timing drift (the same *class* of issue as the already-tracked
`artanis_responder_ticks` clobber, #8409) rather than a missing-row gap; not
chased further, flagged here rather than silently left undocumented. Full
evidence:
[`2026-07-05-forum-and-user-content-backup-verification.md`](./2026-07-05-forum-and-user-content-backup-verification.md).
**10 of 11 tables safe to cite on KS-8.19; `pylon_capacity_funnel_snapshots`
has an open, low-severity, non-blocking hash-drift note above.**

- **What:** the rest of the Pylon control plane after KS-8.1:
  registrations, quarantines, marketplace intake/assignments/triage,
  provider job lifecycle, runner status, capacity funnel, Spark payout
  targets, runner sessions, fleet alerts — plus the **raw-event firehose
  split**: `pylon_codex_raw_events` / `pylon_codex_raw_event_chunks` are
  the #1 write amplifier (per-item unbatched chunk flush, ~60+ statements
  per 30-command turn). Payload bodies already live in R2; only a bounded
  metadata index moves to Postgres, and the ingest path goes through a
  Queue (Phase 0 item) so the relational core never sees per-item writes.
- **Tables (~14):** `pylon_api_registrations`, `pylon_api_quarantines`
  (boundary note: if KS-8.1 already pulled these for dispatch-gate read
  consistency, this issue just confirms), `pylon_marketplace_assignments`,
  `pylon_marketplace_job_intakes`, `pylon_marketplace_triage_actions`,
  `pylon_provider_job_lifecycle`, `pylon_agent_runner_status_events`,
  `pylon_capacity_funnel_snapshots`, `pylon_spark_payout_targets`,
  `pylon_codex_raw_events`, `pylon_codex_raw_event_chunks` (metadata
  index only), `runner_sessions`, `fleet_alerts`.
- **Heat:** hot writes (raw-event chunks per item; presence heartbeats do
  wide full-row UPDATEs per pylon per heartbeat); hot reads (closeout and
  proof scans over raw-event + assignment tables during fleet
  supervision).
- **Risks:** chunk dedupe key (`INSERT OR IGNORE`) must port exactly —
  closeout/proof verification depends on chunk-chain completeness;
  presence heartbeat UPDATE shape should slim to changed columns on the
  Postgres side; status-event streams are firehose candidates (Analytics
  Engine) — decide per stream before porting rows.
- **Verification:** chunk-chain contiguity per turn (no gaps, no dupes)
  across stores; closeout verifier runs against Postgres shadow and
  produces identical verdicts on a sampled window.
- **Cron re-homed:** `PylonCapacityFunnel.recordSnapshots`;
  `FleetBurnStallDetector.tick` reads move here + KS-8.2.
- **Depends:** KS-8.1 (same code neighborhood, dispatch gate already on
  Postgres), KS-8.2 (stall detector reads the ledger).

### 3.2 KS-8.5 — Agent runtime metadata (definitions / runs / traces)

**KS-8.5 machinery status (2026-07-04, #8316):** LANDED for the eight
core metadata tables — Postgres schema (`khala-sync-server` migration
`0010_agent_runtime.sql`: `agent_definitions`, `agent_definition_runs`,
`agent_definition_triggers`, `agent_runs`, `agent_run_events`,
`agent_traces`, `agent_goals`, `agent_goal_events`; dedupe keys ported
exactly — run-event `INSERT OR IGNORE` collapses to bare
`ON CONFLICT DO NOTHING` over the same three uniques, the trace
owner+idempotency and owner+content-digest partial uniques (the
training-consent / trace-plugin revenue-share keys) port verbatim, and
the one-active-goal-per-scope partial expression unique is preserved;
indexes re-derived from actual reads). Typed row-level repository seam
with D1 + Postgres implementations, a fail-soft dual-write wrapper, and
a read-back mirror wired at ALL worker write call sites for those tables
(`apps/openagents.com/workers/api/src/agent-runtime-store.ts`; wired
via `make*ForEnv` factories in `index.ts`, `omni-handlers.ts`,
`agent-goal-routes.ts`, `adjutant-assignments.ts`,
`operator-adjutant-routes.ts`, and the `AgentDefinitionScheduler`
dependencies — the cron this domain re-homes). Flags
`KHALA_SYNC_AGENT_RUNTIME_DUAL_WRITE` (default on) /
`KHALA_SYNC_AGENT_RUNTIME_READS` (d1|compare|postgres, default d1;
covers the scheduler due-trigger scans in this lane — all other reads
stay on D1 until cutover). Resumable backfill + exact-verify CLI
(`packages/khala-sync-server/scripts/backfill-agent-runtime.ts`: exact
counts, per-run/per-goal event-chain comparison, trace content-hash
sampling + visibility/consent tallies, goal usage sums, newest-N row
hashes), and a contract suite run against BOTH stores plus dual-write
fail-soft / flag-routing / diagnostics-privacy tests. `agent_traces`
stay owner-private: the Postgres twin carries visibility/owner/consent
columns verbatim, no new read path is exposed, and migration
diagnostics reference row keys only — never trajectory content.

**KS-8.5 CORE fresh backup verification (2026-07-05, #8282/#8330
follow-up):** unlike the remainder lane below (#8334, which recorded real
production backfill evidence on 2026-07-04), this CORE lane's "machinery
LANDED" status above never recorded an actual production backfill/verify
run. A follow-up to the same-day forum-and-user-content backup pass (which
had deferred this domain as unexamined) ran `backfill-agent-runtime.ts
--verify` fresh against production: **VERIFY FAILED** — `agent_runs` (73
D1 rows), `agent_run_events` (6,801), the small `agent_goals`/
`agent_goal_events` families, and `agent_traces` (230,331 — the largest,
highest-content table) were ALL 0 rows in Postgres despite the "LANDED"
claim (`agent_definitions`/`_runs`/`_triggers` are correctly 0/0 on both
sides — genuinely no production traffic yet). Ran the standard sweep:
`agent_runs`, `agent_run_events`, and the goal tables are now
**backfilled and confirmed converging** (0 newly inserted on the resumed
sweep). `agent_traces` backfills at only ~17-18 rows/sec (full trace
bodies per row) via the page-by-page CLI — too slow to finish inside this
session — left running as a detached, safe/idempotent/resumable background
process for its remaining ~3.5-4 hours. Full evidence:
[`2026-07-05-forum-and-user-content-backup-verification.md`](./2026-07-05-forum-and-user-content-backup-verification.md).
**`agent_runs`/`agent_run_events`/goals are safe to cite on KS-8.19;
`agent_traces` backfill is IN PROGRESS as of this writing — re-verify
before citing that table specifically.**

**KS-8.5 remainder status (2026-07-04, #8334):** COMPLETE for the
follow-up remainder lane — Postgres schema (`khala-sync-server` migration
`0012_agent_runtime_remainder.sql`) creates twins for
`agent_profiles`, `agent_credentials`, `agent_owner_claims`,
`agent_owner_x_claim_challenges`, `agent_proposals`,
`event_ledger_entries`, `khala_acceptance_jobs`, and
`khala_acceptance_verdicts`. `event_ledger_entries_next` is treated as
the D1 rewrite artifact from migration 0287: the live canonical table is
`event_ledger_entries`, and the verifier fails if the artifact still
exists with rows. The exact backfill/verify CLI is
`packages/khala-sync-server/scripts/backfill-agent-runtime-remainder.ts`;
it checks counts, scalar status/attempt/verdict tallies, newest-N row
hashes, and per-owner `event_ledger_entries.ordering_sequence` density.
Credential diagnostics remain key/hash-only: row hashes may include the
private `token_hash` byte value, but output prints credential ids and
sha256 row hashes only, never token hashes or payloads. Runtime mirror
seams are wired for `event_ledger_entries`, `agent_profiles`,
`agent_credentials`, `agent_owner_claims`, and
`agent_owner_x_claim_challenges`, `agent_proposals`,
`khala_acceptance_jobs`, and `khala_acceptance_verdicts`
(`apps/openagents.com/workers/api/src/agent-runtime-remainder-store.ts`,
`event-ledger.ts`, `agent-registration.ts`, `agent-owner-claim-routes.ts`,
`agent-proposal-routes.ts`, `inference/acceptance-job-queue-store.ts`,
`inference/acceptance-dispatch.ts`):
D1 remains authority, and Postgres mirror failures log
`khala_sync_agent_runtime_remainder_dual_write_failed` without blocking
event-ledger ingest, handled-state updates, registration, credential touch,
OpenAuth credential linking, credential reissue, owner-claim approval, X
claim verification, proposal submission, proposal transition, acceptance
job enqueue/lease/ack, or acceptance verdict backfill. Acceptance job
mirror deletes delivered jobs when the D1 queue deletes them so the
Postgres twin does not retain stale queue rows. Production evidence for
[#8334](https://github.com/OpenAgentsInc/openagents/issues/8334) was recorded
on 2026-07-04 after deploying Worker version
`afaf8272-a654-4dd6-ba1c-69418f12dcae`: `deploy:safe` completed, the
served document and concrete `/assets/index-DWcdsn2N.js` asset both returned
HTTP 200, `/api/internal/khala-sync/db-smoke` returned `ok: true` with 12
Khala Sync tables, the production backfill touched 416 `agent_profiles`,
418 `agent_credentials`, 14 `agent_owner_claims`, 5
`agent_owner_x_claim_challenges`, and 1 `agent_proposals` row, and
`--verify --verify-newest 50` reported clean counts/hashes for all eight
tables with `event_ledger_entries_next` absent-or-empty. Destructive D1
retirement is deferred to KS-8.19
[#8330](https://github.com/OpenAgentsInc/openagents/issues/8330), not a
per-domain soak/drop follow-up. Prod cutover procedure:
[`RUNBOOK.md`](./RUNBOOK.md) "Agent runtime metadata domain cutover" and
"Agent runtime remainder backfill"; cutover evidence is tracked on epic
[#8282](https://github.com/OpenAgentsInc/openagents/issues/8282).

- **What:** the agent execution record: definitions, scheduled triggers,
  runs, run events, traces, artifacts, goals, profiles, proposals, owner
  claims, credentials, plus the external event ledger (GitHub intake).
- **Tables (~18):** `agent_definitions`, `agent_definition_runs`,
  `agent_definition_triggers`, `agent_runs`, `agent_run_events`,
  `agent_traces`, `agent_artifacts`, `agent_goals`, `agent_goal_events`,
  `agent_profiles`, `agent_proposals`, `agent_owner_claims`,
  `agent_owner_x_claim_challenges`, `agent_credentials`,
  `event_ledger_entries` (+ `event_ledger_entries_next` artifact),
  `khala_acceptance_jobs`, `khala_acceptance_verdicts`.
- **Heat:** warm writes on every agent run (trace insert per ingest is
  part of the measured 10–11-statement turn cost); reads are per-run UIs
  + acceptance verifiers.
- **Risks:** `agent_traces` dedupe keys feed training-consent and
  trace-plugin revenue-share surfaces — key semantics must not drift;
  `event_ledger_entries` has a per-owner dense `ordering_sequence`
  (`UNIQUE(owner_agent_user_id, ordering_sequence)`) — allocate it inside
  the Postgres transaction exactly like khala-sync scope versions, never
  read-then-insert; credentials are secret-bearing (no post-images into
  broad scopes, SPEC invariant 9).
- **Verification:** per-run event-chain contiguity; trace row counts +
  content-hash sampling; ordering_sequence density per owner.
- **Cron re-homed:** `AgentDefinitionScheduler.tick`.
- **Depends:** KS-8.1 (runs reference assignments), KS-8.2 (traces
  correlate with ledger events).

### 3.3 KS-8.6 — Artanis supervision loops

- **What:** the Artanis operator/supervision state machine — loop records,
  ticks, responder state/actions, admin tick decisions, fleet overseer
  decisions, closeout verdicts, spend decisions/grants, health/runtime
  snapshots, threads/messages, owner memory.
- **Tables (20):** all `artanis_*`.
- **Heat:** the single biggest **cron** writer — six of the ~23
  every-minute tasks are Artanis ticks. Fixed write floor under
  everything.
- **Risks:** tick idempotency (a re-run minute must not double-act);
  spend decisions reference treasury state (keep as ID references, no
  cross-store transactions until KS-8.8 lands); snapshots are
  firehose-ish — consider bounded retention or Analytics Engine for
  `artanis_health_snapshots` / `artanis_runtime_snapshots` instead of
  row-porting history.
- **Verification:** tick-chain contiguity per loop; decision counts;
  replaying one tick against both stores yields identical decisions.
- **Cron re-homed:** `ArtanisScheduledRunner.runTick`,
  `ArtanisResponder.scan`, `ArtanisResponder.compose`,
  `ArtanisAdmin.tick`, `ArtanisAdmin.closeoutVerifier`,
  `ArtanisFleet.tick` — six tasks leave the D1 contention floor at once;
  this is the highest-leverage cron move.
- **Depends:** KS-8.1 (fleet overseer reads assignments), KS-8.4
  (closeout verifier reads raw events).

### 3.4 KS-8.7 — Billing, credits, Stripe, pay-ins

- **CFG-4 HARD CUTOVER (2026-07-06, #8519):** `pay_ins` and `pay_in_legs`
  left this lane's dual-write posture entirely — they are Cloud SQL
  Postgres-AUTHORITATIVE with the D1 code path deleted (executor:
  `payments-ledger-db.ts`; see the RUNBOOK "Credits domain HARD cutover"
  section). The rest of the ~23 billing tables keep the flag/dual-write
  machinery described below unchanged.

- **What:** customer-facing money-in: the credits ledger, auto-top-up,
  coupons, Stripe integration, pay-ins, buyer payment
  challenges/receipts/limits.
- **Tables (~23):** `billing_*` (8, incl. `billing_ledger_entries_next`
  artifact), `stripe_*` (4), `pay_ins`, `pay_in_legs`,
  `buyer_payment_*` (7), `first_batch_payment_policies`,
  `khala_code_paid_plan_payment_intents`.
- **Heat:** low volume, maximum correctness stakes.
- **Risks:** ledger-entry idempotency (Stripe webhook replays MUST hit
  `ON CONFLICT DO NOTHING` on the event id — `stripe_webhook_events` is
  the dedupe gate for everything downstream); balance = SUM(ledger) must
  reconcile **to the cent** before any read cutover; auto-top-up policies
  must not double-fire during dual-write (the Postgres side is
  shadow-only until cutover — side-effectful evaluators run against
  exactly one store at any time).
- **Verification:** per-account balance equality (exact), ledger row
  counts, Stripe event id set equality, receipts content-hash equality.
- **Cron re-homed:** none directly (auto-top-up evaluation is
  request-path today); webhook consumers repoint.
- **Depends:** KS-8.2 (usage cursors correlate with the token ledger via
  `billing_usage_cursors`).
- **KS-8.7 follow-up status (#8337, 2026-07-05):** the RUNBOOK-listed
  "still D1-only" writers are now rehomed — `first_batch_payment_policies`
  (operator-order-triage `OrderTriageRuntime.firstBatchPaymentPolicyMirror`),
  `business-starter-credit.ts` `createGrant`'s `usdCreditGrantStatements`
  pay-in, `cloud/cloud-metering.ts` `settleCloudPrimitiveCharge` (covers
  fine-tuning + sandbox-compute charges), `inference/metering-hook.ts`
  `makeLedgerMeteringHook`, and `inference/inference-abuse-controls.ts`
  `clawbackInferenceCredits` (this last one has no production call site
  yet — the wiring is forward-compatible). `labor-escrow.ts`'s
  `runLedgerStatements` calls were AUDITED and found to touch only
  treasury-domain (`agent_balances`/`labor_escrows`, KS-8.8) tables via
  their own always-on annotated mirror — not a billing-domain gap, so no
  change was needed there. A full production `--restart` sweep +
  `--verify` ran clean on 20/21 tables (exact row counts, the full
  per-user `billing_ledger_entries` balance map, Stripe event-id set
  equality, grouped msat/cents sums) — see the RUNBOOK for the exact
  output. `pay_in_legs` came back 2 rows short (321/323 mirrored) because
  of a genuine pre-existing production data bug found during this sweep
  (swapped `party_ref`/`amount_msat` params in
  `usd-credit-bridge.ts`'s `usdCreditGrantStatements` audit-leg INSERT —
  fixed in this pass; the 2 already-corrupted historical rows are tracked
  in [#8412](https://github.com/OpenAgentsInc/openagents/issues/8412) for
  an owner-gated historical-correction decision, not silently patched).
  `billing_ledger_entries_next` confirmed ABSENT in both D1 and Postgres.
  The epic-gated `KHALA_SYNC_BILLING_READS=postgres` decision on #8282 has
  NOT been made — reads stay `d1`; dual-write stays on; no D1 table was
  dropped (per the #8330 KS-8.19 consolidation policy: bulk domain drops
  wait for that closing sweep, not per-domain follow-ups).
- **KS-8.7 follow-up #2 status (#8337, 2026-07-05): bounded read allowlist
  landed.** Mirroring KS-8.14's `BUSINESS_DOMAIN_POSTGRES_SERVED_READ_TABLES`
  discipline (#8360), `billing-store.ts` now names
  `BILLING_DOMAIN_POSTGRES_SERVED_READ_TABLES` — a bounded, ALWAYS
  narrower-than-the-flag allowlist that unlocks real Postgres serving
  (`KHALA_SYNC_BILLING_READS=postgres`) for exactly four DISPLAY-ONLY,
  non-decision-critical read surfaces, each wired through its own
  hand-audited function (never a generic "SQL touches table X"
  classifier, since this domain's reads are not routed through a single
  mirroring `D1Database` proxy the way the business-funnel lane is):
  - `billing_ledger_entries` — the recent-entries display projection
    (`readRecentLedgerEntries`, `billing.ts`, LIMIT 12). NOT the balance
    SUM (that keeps its separate, still-`d1`-default `balanceRead`
    opt-in from the original KS-8.7 pass).
  - `billing_auto_top_up_policies` / `billing_auto_top_up_events` /
    `stripe_saved_payment_methods` — the auto-top-up DISPLAY state
    (`readBillingAutoTopUpState`). NEVER the charge decision —
    `chargeAutoTopUp` (`stripe-billing.ts`) always reads its own
    dedicated D1 query directly and takes no runtime hook.
  - `stripe_checkout_sessions` — the public checkout-receipt read
    (`stripe-checkout-receipts.ts`), an already-settled, immutable
    projection.
  - `pay_ins` — the public inference-receipt read
    (`inference-receipts.ts`), scoped to `pay_in_type IN ('adjustment',
    'usd_credit_grant')` and an immutable `public_receipt_ref`. The
    free-allowance branch (a different domain's `inference_free_usage_events`
    table, no live Postgres mirror in this lane) is explicitly NOT
    servable — the Postgres store throws
    `InferenceReceiptPostgresNotServableError` and the router transparently
    falls back to D1 for that ref shape, in every mode.

  Deliberately NOT allowlisted this pass — candidates for a future,
  individually reviewed pass: the buyer-payment pipeline
  (`buyer_payment_challenges`/`receipts`/`entitlements`/`redemptions`/
  `reconciliation_events`, `buyer-payment-ledger.ts`), because every read
  there is SHARED between the read-only checkout-return/payment-proof
  status routes AND the challenge/webhook/redemption idempotency-dedupe
  decision paths, and cannot be split into a decision-free surface without
  further store-interface surgery; and the forum tip-earnings
  leaderboard/creator-earnings projections (`forum/tip-earnings.ts`),
  which JOIN `pay_ins`/`pay_in_legs` against `forum_posts` (a different
  domain's mirror) in a single statement.

  Migration `0034_billing_bounded_read_indexes.sql` re-derives the two
  missing accelerators for the newly-served surfaces
  (`billing_auto_top_up_events_user_created_idx`,
  `pay_ins_public_receipt_ref_idx`,
  `pay_ins_receipt_listing_covering_idx`) — the other two surfaces already
  hit an existing PK/UNIQUE index. Contract tests (real local Postgres,
  `billing-repository.contract.test.ts` +new
  `stripe-checkout-receipts.test.ts` + new `inference-receipts.test.ts`)
  prove: parity between the D1 and Postgres-served answers; REAL serving
  (a value diverged directly on the Postgres twin is what `postgres` mode
  reads back — not a shadow compare that always answers D1); fail-soft
  fallback to D1 on any Postgres error, including a broken connection; and
  compare-mode logs divergence only on a genuine disagreement. No flag was
  flipped in production this pass — `KHALA_SYNC_BILLING_READS` stays `d1`
  until an operator deploys with `postgres`/`compare` and records that
  decision on #8282, same epic-gated discipline as the balance read.

### 3.5 KS-8.8 — Treasury, payouts, tips settlement

- **CFG-4 HARD CUTOVER (2026-07-06, #8519):** `agent_balances`,
  `labor_escrows`, and `labor_escrow_receipts` left this lane's dual-write
  posture entirely — Postgres-AUTHORITATIVE, D1 code path deleted (see the
  RUNBOOK "Credits domain HARD cutover" section). The other treasury
  tables keep the flag/dual-write machinery unchanged.

- **What:** money-out and settlement: treasury transactions, Nexus payout
  intents/attempts/reconciliation, forum tip settlement (the money half of
  the forum family), L402 challenges, reward ledgers, labor escrows,
  partner/site referral payout ledgers, MPP replay guards.
- **Tables (~28):** `treasury_transactions` (+`_0197_new` artifact),
  `nexus_*` (6), `forum_money_actions` (+`_new`), `forum_payment_events`,
  `forum_receipts`, `forum_l402_challenges`, `forum_l402_redemptions`,
  `forum_direct_tip_attempts`, `forum_direct_tip_webhook_events`,
  `forum_tip_recipient_wallets`, `forum_tip_settlement_claims`,
  `x_claim_reward_ledger`, `agent_claim_reward_ledger`, `agent_balances`,
  `labor_escrows` (+ artifacts, 4), `partner_payout_ledger_entries`,
  `partner_agreements`, `site_referral_payout_ledger_entries`,
  `revenue_event_provenance`, `mpp_lightning_replay`, `mpp_spt_replay`.
- **Heat:** low volume; settled receipts are public projections
  (`/direct-tips` evidence etc.) that must stay continuously servable.
- **Risks:** the highest-stakes domain. Replay guards (`mpp_*_replay`)
  are pure idempotency tables — port key-exactly; payout intents must
  never double-dispatch during dual-write (Postgres shadow rows carry a
  `shadow` marker or the dispatcher reads exactly one store); tip
  settlement claims reference forum posts by id across the domain split
  with KS-8.10 — ID references only, no joins across stores; millisat
  totals reconcile exactly.
- **Verification:** payout-intent set equality; settled-amount totals to
  the millisat; public receipt endpoints byte-identical (modulo
  timestamps) under shadow reads; replay-guard key set equality.
- **Cron re-homed:** `TipsSweep.runTick`,
  `TipsBuffer.reconcileForwarding`, `TipsBuffer.backingInvariant`,
  `TreasuryTransactions.reconcilePending`,
  `XClaimRewardTreasuryDispatcher.runTick`,
  `ForumDirectTips.archiveStaleRecoveries` — six money crons.
- **Depends:** KS-8.7 (shared money-rail plumbing lands there first).

### 3.6 KS-8.9 — Inference entitlements and quotas

- **What:** the free-tier/entitlement accounting sitting on the serving
  path: free-tier keys/usage/tallies, earned allowance, privacy
  entitlements, premium allowlist, referral margin splits, agent rate
  limits and agent-search payment/entitlement sub-family, built-in
  compute quotas.
- **Tables (~30):** `inference_*` (15),
  `builtin_compute_agent_quota_events`, `orange_check_entitlements`,
  `agent_rate_limit_*` (4), `agent_search_*` (9).
- **Heat:** **hot** — quota checks and usage-event writes ride the
  serving path of every free/public completion; second only to the token
  ledger.
- **Risks:** quota counters are enforcement, not telemetry — a lost
  increment is a free-tier leak, a doubled one is a false denial;
  dual-write must therefore be increment-idempotent (event-keyed, tally
  derived); the read cutover changes which store *enforces* — do it in a
  low-traffic window with shadow-compare evidence; `*_metric_events` and
  usage-event streams here are Analytics Engine candidates (enforcement
  tallies stay relational, observability streams do not).
- **Verification:** tally = SUM(events) equality per key; denial-decision
  shadow comparison (same request → same allow/deny) over a sampled
  window.
- **Cron re-homed:** GLM pool heartbeat ledger writes leave for Analytics
  Engine under Phase 0/KS-8.2; nothing else scheduled.
- **Depends:** KS-8.2 (same serving-path code, ledger already moved).

### 3.7 KS-8.10 — Forum (content + trust)

**KS-8.10 machinery status (2026-07-04, #8321):** LANDED for the
thirteen content-core tables — Postgres schema (`khala-sync-server`
migration `0014_forum_content.sql`: `forum_boards`, `forum_categories`,
`forum_forums`, `forum_topics`, `forum_posts`, `forum_post_bodies`,
`forum_post_revisions`, `forum_actor_follows`, `forum_watches`,
`forum_bookmarks`, `forum_reports`, `forum_moderation_events`,
`forum_context_links`; every idempotency-key and natural-key unique
ports verbatim, incl. the moderation-events partial unique). The seam is
the forum repository's OWN interface: every scoped write lives in
`apps/openagents.com/workers/api/src/forum/repository.ts` and takes
`db: D1Database`, so the production wiring is a mirroring D1Database
(`forumContentDatabaseForEnv` in
`apps/openagents.com/workers/api/src/forum/forum-content-store.ts`)
dropped in at the forum write entry points in `index.ts` (the forum API
route, both Artanis composer paths, the Artanis forum-update writer,
Artanis publication delivery, and the two agent-definition forum
webhooks). Repository SQL is untouched; after each successful D1 write
to a scoped table the proxy reads the row back by PK and
converge-upserts the exact D1 row into Postgres (fail-soft; the drift
metric is `khala_sync_forum_dual_write_failed`, and any write shape the
classifier cannot key logs `khala_sync_forum_write_unclassified` instead
of guessing). Column/PK registry is SHARED between the Worker store and
the backfill via `@openagentsinc/khala-sync-server`
(`forum-content-tables.ts`) — one source of truth. Flags
`KHALA_SYNC_FORUM_DUAL_WRITE` (default on) / `KHALA_SYNC_FORUM_READS`
(d1|compare|postgres, default d1; `compare` shadow-runs scoped-table
SELECTs against Postgres and serves D1 — the thread-page shadow-compare
evidence; `postgres` serving is deferred to the read-cutover follow-up
and downgrades to compare with a loud diagnostic). Resumable backfill +
exact-verify CLI
(`packages/khala-sync-server/scripts/backfill-forum-content.ts`: exact
counts, domain tallies, PER-TOPIC post-chain comparison, per-thread
body-content spot hashes over sampled topics, newest-N row hashes), and
a contract suite that runs the row seam against BOTH stores plus the
REAL repository write functions end-to-end through the mirror
(`forum-content-repository.contract.test.ts`). The forum MONEY
tables stay with KS-8.8 (D1 authority, that lane's mirror discipline) —
this lane never touches them. Prod cutover procedure:
[`RUNBOOK.md`](./RUNBOOK.md) "Forum content domain cutover"; cutover
evidence + D1 drop tracked on epic
[#8282](https://github.com/OpenAgentsInc/openagents/issues/8282).

**KS-8.10 REMAINDER machinery status (2026-07-04, #8338; #8379 cleanup on
2026-07-05):** LANDED for the eleven active remainder tables after the
write-dead trust pair was removed. Historical migration
`0027_forum_remainder.sql` created the original thirteen Postgres twins;
`0030_drop_forum_trust_remainder.sql` drops `forum_trust_edges` and
`forum_actor_forum_trust` to match Worker D1 migration
`0300_drop_forum_trust_tables.sql`. The active Postgres schema now covers
`forum_private_message_threads`, `forum_private_messages`,
`forum_acl_grants`, `forum_score_snapshots`, `forum_notification_reads`,
and the work-request lifecycle family (6)
`forum_work_requests`, `forum_work_request_relay_links`,
`forum_work_request_offers`, `forum_work_request_lifecycle_posts`,
`forum_work_request_acceptances`, `forum_work_request_results`; every
idempotency/natural-key unique ports verbatim, including the offers
`provider_pubkey` ALTER (0179) trailing in D1 physical order and the
notification-read partial uniques). Shared registry
`packages/khala-sync-server/src/forum-remainder-tables.ts`
(`upsertForumRemainderRows`) is the ONE source of truth for both the
Worker mirror and the backfill. The Worker seam
(`apps/openagents.com/workers/api/src/forum/forum-remainder-store.ts`,
`wrapForumRemainderMirroring`) is COMPOSED around the content lane's
`forumContentDatabaseForEnv`, so the SAME forum write call sites cover the
remainder tables with no new wiring; the content classifier treats
remainder tables as passthrough and the remainder classifier treats content
tables as passthrough, so the two nested mirroring wrappers never
double-mirror. PRIVACY: private-message threads/messages store exactly what
D1 stores (bodies behind `content_ref`); diagnostics carry row keys/hashes
only — never subjects, participants, or content. The remaining DERIVED
table, `forum_score_snapshots`, is recomputed from events in D1; this lane
mirrors the D1 snapshot and VERIFIES against D1 rather than re-running the
recompute on Postgres.
Resumable backfill + exact-verify CLI
(`packages/khala-sync-server/scripts/backfill-forum-remainder.ts`: exact
counts, domain tallies, newest-N row hashes, and WORK-REQUEST
SET-MEMBERSHIP referential checks: within-store orphan counts plus
cross-store equality of the cross-domain reference sets that point at
KS-8.1 assignments / KS-8.8 tips by id, no cross-store joins). Contract
suite runs the row seam against BOTH stores
plus the REAL private-message + notification-read repository writers
end-to-end through the mirror
(`forum-remainder-repository.contract.test.ts`,
`forum-remainder-store.test.ts`, `forum-remainder-backfill.test.ts`).
Flags/binding are SHARED with the content lane
(`KHALA_SYNC_FORUM_DUAL_WRITE` / `KHALA_SYNC_FORUM_READS`); Postgres read
serving stays deferred lane-wide (the content wrapper logs the single
`khala_sync_forum_postgres_reads_deferred`; the remainder wrapper treats
`postgres` as `compare` silently). Prod cutover procedure:
[`RUNBOOK.md`](./RUNBOOK.md) "Forum content domain cutover" (the remainder
tables ride the same sequence, with the extra trust-recompute and
work-request set-membership verify gates); actual Postgres read serving +
final D1 drop remain epic-gated on
[#8282](https://github.com/OpenAgentsInc/openagents/issues/8282) /
KS-8.19 [#8330](https://github.com/OpenAgentsInc/openagents/issues/8330).

**KS-8.10 backup verification (2026-07-05):** a dedicated owner-directed
pass (the "forum posts must be backed up" gate ahead of KS-8.19) found that,
despite both closeout comments above, **the production Postgres mirror for
BOTH the content core AND the remainder tables had never actually been
backfilled** — Postgres held only a single fresh dual-write-converged row
per table (e.g. 1 topic/1 post) while D1 held the real corpus (219 topics,
1,303 posts, 91 notification-reads, 6 work-requests, etc.). This is the
exact gap the independent Orrery audit on #8338 flagged as "no production
execution receipt" and that was never subsequently closed. Ran the existing
backfill CLIs (two sweeps + `--verify`) against production for real this
time: **VERIFY OK** on both the content-core (13 tables, incl. per-topic
post-chain comparison and 25 sampled thread spot-hashes) and the remainder
(11 active tables, incl. work-request cross-domain ref-set digest equality
against KS-8.1/KS-8.8). Full evidence:
[`2026-07-05-forum-and-user-content-backup-verification.md`](./2026-07-05-forum-and-user-content-backup-verification.md).
D1 was not read from, written to, or altered; no flag was flipped. **Forum
content + remainder mirror-completeness is now SAFE TO CITE on KS-8.19
(#8330).**

- **What:** the forum content core: forums/boards/categories, topics,
  posts + bodies + revisions, private messages, score snapshots, ACLs,
  moderation, watches/bookmarks/notifications, work-request lifecycle.
  (The money half went in KS-8.8.)
- **Tables (~26):** `forum_forums`, `forum_boards`, `forum_categories`,
  `forum_topics`, `forum_posts`, `forum_post_bodies`,
  `forum_post_revisions`, `forum_private_message_threads`,
  `forum_private_messages`, `forum_acl_grants`, `forum_actor_follows`,
  `forum_score_snapshots`,
  `forum_moderation_events`, `forum_notification_reads`,
  `forum_bookmarks`, `forum_watches`, `forum_reports`,
  `forum_context_links`, `forum_work_request_*` (6).
- **Heat:** read-heavy public surface (agents + humans poll it); writes
  are bursty but modest. Public projections everywhere.
- **Risks:** largest single family — backfill is the long pole (post
  bodies are big rows; page the backfill and checksum content);
  work-request lifecycle couples to assignments (KS-8.1) and tips
  (KS-8.8) by id — verify referential integrity by set-membership, not
  FK enforcement, at cutover; scoring snapshots are derived — recompute
  on Postgres and compare rather than blind-copy.
- **Verification:** row counts per table; post-body content hashes
  (sampled + full count); public thread pages shadow-compared; trust
  recomputation equality.
- **Cron re-homed:** none content-side (tips crons moved in KS-8.8).
- **Khala Sync exposure:** forum topics/posts are a natural later scope
  (`scope.public.<channel>`-style) — optional follow-up, not part of the
  cutover.
- **Depends:** KS-8.8 (money actions referenced from posts are already
  on Postgres).

### 3.8 KS-8.11 — CRM, email, enrichment

**KS-8.11 machinery status (2026-07-04, #8322):** LANDED — Postgres schema
(`khala-sync-server` migration `0022_crm_email_domain.sql`: the 36
canonical tables — `crm_*` (13), `email_*` (11 live; the `_0193_new` D1
names were transient rebuild artifacts, verified superseded, no twins),
`subscriber_lists` + `list_subscribers`, `business_outreach_*` (4),
`exa_enrichment_*` (6); every idempotency/dedupe UNIQUE ports verbatim —
including the campaign-send enrollment×step key, webhook
`(provider, provider_event_id)` replay safety, and the outreach
`(subject_ref, reason)` suppression key — so the Postgres side can never
double-email where D1 could not). The seam is the `CrmEmailDatabase`
union handle (`apps/openagents.com/workers/api/src/crm-email-domain-store.ts`,
artanis-style: the domain's SQL lives in ~15 owning modules): a plain
`D1Database` keeps working unchanged; the dual-write handle read-back
mirrors the RESOLVED D1 rows fail-soft after every authoritative write
(`mirrorCrmEmailRows` never throws; drift metric
`khala_sync_crm_dual_write_failed`), and `crmEmailRead` flag-routes reads
(d1 | compare | postgres with bounded retry + D1 fallback) — the
suppression/preference compliance gate rides this seam, so the send path
reads exactly ONE authoritative suppression store at every moment of the
cutover. Flags `KHALA_SYNC_CRM_DUAL_WRITE` (default on) /
`KHALA_SYNC_CRM_READS` (default d1). Wired at the write entry points:
CRM routes/import/batch/send/command/MCP surfaces, email
campaigns/sequences/preferences/suppression, native lists +
list→sequence enrollment, the Resend webhook ingest, transactional email
ledger writers, business outreach, Exa enrichment ledger/operations, and
the `EmailCampaignDispatcher.dispatchDue` cron. PII discipline: Postgres
stores exactly what D1 stores; every diagnostic carries table names,
keys, and hashes only, email-valued keys as sha256 prefixes. Resumable
(rowid-cursor) idempotent backfill + exact-verify CLI
(`packages/khala-sync-server/scripts/backfill-crm-email.ts`: exact
counts, per-status tallies over non-PII columns, newest-N row hashes,
whole-set digests for the compliance tables — suppression set equality
without printing an address). Contract tests cover both engines, the
fail-soft paths, and backfill idempotency
(`crm-email-domain-store.test.ts`, `crm-email-backfill.test.ts`). Prod
cutover procedure: [`RUNBOOK.md`](./RUNBOOK.md) "CRM / email / enrichment
domain cutover"; flag flips are epic-gated on
[#8282](https://github.com/OpenAgentsInc/openagents/issues/8282); final
D1 retirement is consolidated into KS-8.19
[#8330](https://github.com/OpenAgentsInc/openagents/issues/8330).

**KS-8.11 backup verification (2026-07-05):** the same dedicated pass that
found the KS-8.10 gap found the identical pattern here — the production
Postgres mirror had never actually been backfilled (business-outreach
acceptances, transactional email delivery events, and all six
`exa_enrichment_*` ledgers were present in D1 and absent in Postgres; no
real `crm_contacts` rows exist in production today, so no customer PII was
at risk). Ran the existing `backfill-crm-email.ts` CLI (two sweeps +
`--verify`) against production: **VERIFY OK** — exact counts, tallies,
newest-N hashes, and the suppression-list compliance-gate set digest all
match. Full evidence:
[`2026-07-05-forum-and-user-content-backup-verification.md`](./2026-07-05-forum-and-user-content-backup-verification.md).
D1 untouched, no flag flipped. **Safe to cite on KS-8.19 (#8330).**

- **What:** CRM accounts/contacts/opportunities/activities/lists, MCP
  grants, email messages/deliveries/campaigns/templates/suppression,
  subscriber lists, business outreach sends/suppressions, Exa enrichment.
- **Tables (~40):** `crm_*` (13), `email_*` (15, incl. three `_0193_new`
  artifacts), `subscriber_lists`, `list_subscribers`,
  `business_outreach_*` (4), `exa_enrichment_*` (6).
- **Heat:** cold-to-warm; campaign dispatch is scheduled batch; contact
  imports are bulk.
- **Risks:** **suppression lists are a compliance gate** — the send path
  must read exactly one authoritative suppression store at every moment
  of the cutover (flip atomically, verify by attempting a suppressed
  send in staging); campaign send dedupe (enrollment × step) keys port
  exactly or we double-email real people; provider webhook events
  (`email_provider_events`) are the delivery-state source of truth —
  replay-safe upserts.
- **Verification:** suppression set equality (exact); per-campaign send
  counts; contact/account row counts + email-address set equality.
- **Cron re-homed:** `EmailCampaignDispatcher.dispatchDue`.
- **Depends:** none hard; sequenced in wave C for risk budget, and CRM
  semantic-selector routing (workspace rule) is unaffected by the store
  move.

### 3.9 KS-8.12 — Sites, site builder, targeted sites

**KS-8.12 source status (2026-07-04):** CORE machinery LANDED — Postgres
schema (`khala-sync-server` migration `0020_sites_core.sql`) covers the
FIFTEEN content/builder tables the version-chain and deployment state
machine live on (`site_projects`, `site_versions`, `site_deployments`,
`site_deployment_attempts`, `site_access_grants`, `site_events`, and the
nine `site_builder_*` tables); the shared registry
`packages/khala-sync-server/src/sites-content-tables.ts` owns column/key
order for both the Worker mirror and the backfill verifier; the Worker
seam `apps/openagents.com/workers/api/src/sites-content-store.ts` is a
mirroring D1Database (`sitesContentDatabaseForEnv`) — sites module SQL is
untouched; after a successful D1 write the proxy reads back the affected
rows (PK or the registered PARENT keys `site_id`/`session_id` for the
rollback/disable and archival transitions) and converge-upserts them into
Cloud SQL, fail-soft. Wired at the sites write call sites: the
`AutopilotSitesService.layer` db seam, agent site routes `dbForEnv`,
site-library, builder orchestration routes, customer orders, operator
sites/triage/adjutant/email-inspection routes, omni runner lifecycle, and
the index.ts site-event notification writers. Backfill + exact verify
lives at `packages/khala-sync-server/scripts/backfill-sites-content.ts`
(counts, domain tallies, PER-PROJECT VERSION CHAINS, the deployment
state-machine census, builder sequence chains, newest-N row hashes;
rowid-cursor resumable — file snapshots/manifests page small). Read
authority remains D1 (`KHALA_SYNC_SITES_READS` default `d1`; `compare`
shadow-reads; `postgres` defers).

**KS-8.12 remainder status (2026-07-04, #8357):** the REMAINDER 36 tables
are now LANDED behind the SAME shared registry
(`sites-content-tables.ts`) and mirroring seam — Postgres twins in
`khala-sync-server` migration `0025_sites_remainder.sql`. Coverage:
Scope A content satellites (`site_build_validations`,
`site_revision_feedback`, `site_compatibility_checks`,
`site_provisioning_plans`, `site_storage_bindings`,
`site_source_exports`, and the referral family `site_referral_sources` /
`referral_invites` / `site_referral_policy_events`); Scope B
`site_environment_values` — SECRET-BEARING, so `plain_value` is EXCLUDED
from the registry column list and the twin carries metadata + the
`secret_ref` INDIRECTION only (SPEC invariant 9; the KS-8.5 credential
posture); Scope C site COMMERCE/money (`site_commerce_*` (5),
`site_mdk_checkout_intents`, `site_mdk_account_bindings`,
`site_payment_catalog_items`, `site_referral_payout_ledger_entries`) —
D1 stays money authority, twin is MIRROR-ONLY, KS-8.7/8.8 rails
referenced BY ID (never forked, no FKs), verified by commerce totals to
the cent (`SUM(amount)` per asset) + set-membership referential checks
(no cross-store joins); Scope D `targeted_site_*` (14) +
`tenant_custom_hostnames` + legacy `deployments`/`deployment_events`.
DELIBERATELY EXCLUDED: `targeted_site_campaign_metric_events` — the
Analytics-Engine-candidate campaign firehose stays on D1/AE pending a
telemetry-sink decision, not blind-copied into a relational twin. The
dual-write mirror auto-covers these tables wherever a sites write call
site is already wrapped; `scripts/backfill-sites-content.ts` now
backfills + verifies the full core+remainder set
(`ALL_SITES_CONTENT_TABLES`). SITES READ CUTOVER (Scope E,
`KHALA_SYNC_SITES_READS=postgres`) stays DEFERRED — reads remain `d1`;
read-serving indexes are re-derived at that cutover. Final destructive
D1 retirement stays in KS-8.19
[#8330](https://github.com/OpenAgentsInc/openagents/issues/8330).

**KS-8.12 backup verification (2026-07-05):** the same dedicated pass found
the identical undetected gap here — `site_projects`, `site_versions`,
`site_deployments`, `site_deployment_attempts`, `site_access_grants`, one
`site_builder_*` table, `site_build_validations`, and
`site_revision_feedback` were all present in D1 (real user-authored site
content) and entirely absent from the Postgres mirror. Ran the existing
`backfill-sites-content.ts` CLI (two sweeps + `--verify`) against
production: **VERIFY OK** — exact counts, domain tallies (incl. commerce
totals), per-project version-chain contiguity, deployment state-machine
census, builder sequence chains, and referential set-membership all match.
Full evidence:
[`2026-07-05-forum-and-user-content-backup-verification.md`](./2026-07-05-forum-and-user-content-backup-verification.md).
D1 untouched, no flag flipped. **Safe to cite on KS-8.19 (#8330).**

- **What:** the Sites product: projects/versions/deployments/grants,
  builder sessions (messages, phase runs, file snapshots, previews,
  repair attempts), site commerce, environment values, custom hostnames,
  targeted-site prospecting/capture/campaign machinery.
- **Tables (~51):** `site_*` (33), `targeted_site_*` (15),
  `tenant_custom_hostnames`, `deployments`, `deployment_events`.
- **Heat:** builder sessions are hot *during a build* (file snapshots,
  message streams), cold otherwise; capture runs are batch; commerce is
  low-volume money (payment events reference KS-8.7 rails).
- **Risks:** file snapshots may be large — confirm payload homes (R2 for
  bodies, Postgres for metadata) before porting; site commerce payment
  events must not fork from the KS-8.7 money rails (ID references);
  `site_environment_values` may carry secrets — same invariant-9
  handling as credentials; capture/campaign `*_metric_events` are
  Analytics Engine candidates.
- **Verification:** per-project version-chain contiguity; deployment
  state-machine equality; commerce totals to the cent; live site serving
  unaffected (serving reads mostly hit R2/KV already — verify inventory
  first).
- **Cron re-homed:** none scheduled today (orchestration is
  request/queue-driven).
- **Depends:** KS-8.7 (commerce), otherwise independent.

### 3.10 KS-8.13 — Khala Code product state (threads, teams, workspaces)

**KS-8.13 source status (2026-07-04):** source machinery LANDED —
Postgres schema (`khala-sync-server` migration
`0017_khala_code_product_state.sql`) covers the product-state tables
listed below; the shared registry
`packages/khala-sync-server/src/khala-code-product-state-tables.ts`
owns column/key order for both the Worker mirror and the backfill
verifier; the Worker seam
`apps/openagents.com/workers/api/src/khala-code-product-state-store.ts`
wraps the existing D1 write handle, read-backs accepted D1 rows, mirrors
them to Cloud SQL, and appends Khala Sync changelog entries for
`scope.team.<id>` / `scope.thread.<id>` from day one. The seam is wired
through chat/message/file/workspace/invite/share/cloud/Khala Code
receipt write factories. Backfill + exact verify lives at
`packages/khala-sync-server/scripts/backfill-khala-code-product-state.ts`
(counts, newest-N row hashes, active membership set equality, and
message-chain fingerprints). Read authority remains D1 until the runbook
shadow window posts evidence; final destructive D1 retirement stays in
KS-8.19 [#8330](https://github.com/OpenAgentsInc/openagents/issues/8330).
Tables with no current Worker writer registration, such as workroom
template rows and Khala Code download events, are still covered by the
schema/backfill verifier and move when their owning route/factory starts
using the wrapped D1 handle.

**KS-8.13 entity-contract status (2026-07-04, second pass):** scope
post-images are now TYPED PUBLIC-SAFE CONTRACT ENTITIES, not raw D1 rows.
`@openagentsinc/khala-sync` (`src/khala-code.ts`) defines the ten
thread/team/workspace entity contracts (team, membership, project,
invite, team chat message, thread message, thread file, file↔message
ref, prefilled workspace, share projection) with golden wire fixtures
(`fixtures/KhalaCode*.json`) registered in the conformance suite;
`packages/khala-sync-server/src/khala-code-product-state-projection.ts`
owns the allowlist row→entity mappings, scope routing, and a
forbidden-material scan over structural fields (invite `token_hash` /
invitee emails, R2 `object_key`s, `metadata_json`, team `credits`, and
share `projection_json` payloads never ride a post-image; bounded chat
bodies/filenames are product content for the authorized scope).
Projection failures are fail-soft
(`khala_sync_khala_code_state_projection_skipped`): the Cloud SQL twin
still converges and D1 authority is untouched. Money-bearing receipt
tables (trace-plugin revenue-share precedents, outside-user-run
receipts) and cloud/feedback/workroom rows remain Postgres-mirror-only
with NO scope fan-out; their public surfaces stay on the continuously
servable Worker read paths, and any future scope-native consumption is a
follow-up contract lane, not a raw-row projection. Scope-read auth for
the produced scopes is the existing KS-7.1 resolver seam: `scope.team.*`
gates on LIVE D1 team membership and `scope.thread.*` on the Worker's
thread-capability callback (`khala-sync-scope-auth.ts`), both
matrix-tested; `khala_sync_scope_owners` remains fleet/owner-scope
machinery and is deliberately NOT used for team scopes while D1
membership is authoritative. Deletion tombstones for the few hard-delete
paths (e.g. `share_projection_recipients`) are a named follow-up; the
interactive surfaces soft-delete via `deleted_at`, which projects as a
normal upsert.

**KS-8.13 delete-tombstone + recipient-scope status (2026-07-04, #8356,
third pass):** the named follow-up LANDED. Hard-delete paths now append
`op:"delete"` changelog entries so removals replicate: the Worker seam
(`khala-code-product-state-store.ts`) reads the rows a `mirrored-delete`
will remove BEFORE the D1 delete commits (an `onBeforeWrite` capture on
the wrapped statement, since the scope/key columns are gone afterward),
converges the Postgres twin, and appends one tombstone per resolved scope
via `scopeTombstonesForKhalaCodeProductStateRow` (scope/type/id only — no
post-image mapper or redaction guard, so a row that fails post-image
mapping on a non-key column still replicates its removal). The one
hard-delete family with a scope consumer,
`share_projection_recipients` (`replaceRecipients` deletes the audience
by `share_id` then re-inserts), is now SCOPE-NATIVE: a new PUBLIC-SAFE
contract `KhalaCodeShareProjectionRecipientEntity` (+
`fixtures/KhalaCodeShareProjectionRecipientEntity.json` in the
conformance suite) projects each recipient into the SUBJECT's own scope
(`subject_kind='user'` → `scope.user.<id>`, `'team'` →
`scope.team.<id>`); `'email'` subjects have no sync scope (PII, never a
`scope.*.<id>`) and stay Postgres-mirror-only, and `display_name` is
structurally absent. All OTHER remainder families
(`workroom_*`, `cloud_*`, `khala_feedback` /
`khala_head_to_head_snapshots` / `khala_unsupported_requests`,
`khala_code_download_events` /
`khala_code_outside_user_run_receipts` /
`khala_code_trace_plugin_revenue_share_precedents`,
`prefilled_workspace_seeded_memory` /
`prefilled_workspace_starter_workflows`) stay Postgres-mirror-only with
NO scope fan-out by design — a scope-native consumer for any of them
remains a future contract lane, and money-bearing receipt families would
project public-safe state only. Fail-soft holds throughout: a failed
pre-read or tombstone append leaves the Cloud SQL twin converged and D1
authoritative. The remaining item before read cutover is the
unclassified-write sweep (task 3): drive
`khala_sync_khala_code_state_write_unclassified` +
`khala_sync_khala_code_state_projection_skipped` to steady-state zero in
staging/prod (tracked on epic #8282).

**KS-8.13 backup verification (2026-07-05):** the dedicated owner-directed
forum-and-user-content backup pass also checked this domain fresh, since it
is the single most sensitive user-generated-content lane (actual chat/thread
messages) and — unlike KS-8.8/8.9/8.14/8.16/8.17 — this section had never
recorded a production backfill/verify run. Ran
`backfill-khala-code-product-state.ts --verify` against production:
**exit 0, zero mismatches**, cross-checked directly against D1 (e.g.
`team_chat_messages` 41/41, `teams` 2/2, `team_memberships` 11/11,
`khala_feedback` 7/7 — genuinely non-zero and exact, not a trivial
both-empty result). No backfill was needed; dual-write has kept this domain
converged. `thread_messages` itself is 0/0 in both stores — live Khala Code
chat already rides the Khala Sync scope-native path per this domain's own
"migration = sync adoption" framing, not this D1 table. Full evidence:
[`2026-07-05-forum-and-user-content-backup-verification.md`](./2026-07-05-forum-and-user-content-backup-verification.md).
**Safe to cite on KS-8.19 (#8330).**

- **What:** the product-surface state that Khala Sync exists to serve:
  threads/messages/files, teams + memberships + chat + invites, prefilled
  workspaces, workroom templates, cloud sandbox/fine-tuning sessions,
  Khala feedback/head-to-head/download/receipt surfaces.
- **Tables (~28):** `thread_*` (3), `thread_file_message_refs`, `teams`,
  `team_*` (4), `prefilled_*` (3), `workroom_*` (3), `cloud_*` (3),
  `khala_feedback`, `khala_head_to_head_snapshots`,
  `khala_unsupported_requests`, `khala_code_download_events`,
  `khala_code_outside_user_run_receipts`,
  `khala_code_trace_plugin_revenue_share_precedents`,
  `share_projections`, `share_projection_recipients`.
- **Heat:** warm; interactive chat surfaces poll today.
- **Risks:** this is the domain where **migration = sync adoption**:
  threads and team scopes should land as khala-sync scopes
  (`scope.thread.<id>`, `scope.team.<id>`) with changelog writes from day
  one, replacing polling — migrating it as plain tables and retrofitting
  scopes later would be double work; revenue-share precedents and outside
  -user-run receipts are public projections (continuously servable).
- **Verification:** thread message-chain contiguity; membership set
  equality; a desktop client on the synced scope converges with the D1
  read path during shadow window.
- **Cron re-homed:** none.
- **Depends:** KS-5.x client engine + KS-6.2 desktop consumption (this
  domain rides the sync engine, not just Postgres); KS-8.5 (agent runs
  referenced from threads).

### 3.11 KS-8.14 — Business funnel, orders, referrals

- **What:** business signup/fulfillment/pipeline, funnel events, service
  promises, commitment ledger, checkout kickoffs, orders + triage +
  fulfillment artifacts, buy-mode campaigns, referral
  invites/attributions across user/agent/order axes, viral funnel, QA
  swarm engagements, promise transition receipts.
- **Tables (~35):** `business_*` remaining (15, incl. `_0275` artifacts),
  `order_*` (6), `software_orders`, `buy_mode_*` (3),
  `customer_one_cohort_rows`, `referral_*` (3),
  `user_referral_attributions`, `agent_referral_attributions`,
  `viral_agent_funnel_events`, `qa_swarm_first_engagements`,
  `promise_transition_receipts`.
- **Heat:** cold/append-mostly; funnel events are firehose-ish
  (Analytics Engine candidate for `business_funnel_events` volume — keep
  the attributions relational, they pay money).
- **Risks:** referral attributions feed payouts (KS-8.8) — attribution
  uniqueness keys port exactly; promise transition receipts back the
  public product-promises registry — continuously servable; fulfillment
  escalation state must not double-page during dual-write.
- **Verification:** attribution set equality; promise-receipt hash
  equality; funnel counts per cohort.
- **Cron re-homed:** `BusinessFulfillmentLoop.dailyMotion`.
- **Depends:** KS-8.7/8.8 (attributions → payouts), KS-8.11 (outreach
  already moved).

**KS-8.14 machinery status (2026-07-04, #8325):** LANDED for all 32 LIVE
tables — Postgres schema (`khala-sync-server` migration
`0023_business_funnel.sql`; the `business_funnel_events_0275` /
`business_service_promises_0275` rewrite artifacts were renamed back by
worker 0277/0275 and do not exist live, so nothing was created for them;
indexes re-derived — PKs + every attribution/idempotency UNIQUE port
exactly, the two partial-unique CONSTRAINTS
(`software_orders_agent_idempotency_idx`,
`order_triage_records_active_order_idx`) port verbatim, all D1 read
accelerators dropped because this lane routes zero reads; the
starter-credit window-cap TRIGGER is deliberately NOT ported — it is a
D1 write-authority gate), the MIRRORING D1Database seam
(`apps/openagents.com/workers/api/src/business-domain-store.ts`,
`businessDomainDatabaseForEnv` — the KS-8.10 pattern generalized with
per-table LOOKUP COLUMNS for update-by-unique statements and
`ON CONFLICT(col)` read-back so the surviving
`business_signup_fulfillments` row mirrors correctly), flags
`KHALA_SYNC_BUSINESS_DUAL_WRITE` (default on) /
`KHALA_SYNC_BUSINESS_READS` (d1|compare|postgres, default d1; `postgres`
serving is deferred-to-compare until the read-cutover follow-up), wiring
at the core write boundaries (public business-signup intake + its
referral capture/consume/affiliate/fulfillment chain, the intake-chat and
signup funnel-event recorders, the funnel dashboard, pipeline /
starter-credit operator stores, buy-mode dispatcher, QA-swarm
engagements, customer-one cohort stores, promise transition receipt
stores incl. hosted-gemini readiness, the six viral-funnel read
recorders, session-bootstrap referral consumption, site-referral
capture routes, and the `BusinessFulfillmentLoop.dailyMotion` cron —
whose escalation pager keeps its dedupe on D1, so dual-write cannot
double-page), resumable backfill + exact-verify CLI
(`packages/khala-sync-server/scripts/backfill-business.ts`: exact counts,
ATTRIBUTION SET DIGESTS over the payout-feeding tuples,
PROMISE-RECEIPT full-row hash-set equality, funnel counts per cohort,
money sums for checkout/starter-credit/buy-mode/workflow-event/order/QA
amounts, newest-N row hashes), and a contract suite run against BOTH
stores incl. consume-once replay, fail-soft, and compare-read proofs.
Funnel-event volume stays relational in this lane; the Analytics Engine
split is a post-cutover follow-up decision.

**KS-8.14 remainder status (2026-07-04, #8359):** FULL WRITER COVERAGE —
the `software_orders`/`order_*`/checkout/referral boundaries that #8325
left D1-only are now wired into the same mirror seam. Because the domain
has no single store object (D1 itself is the repository interface), the
wiring drops `businessDomainDatabaseForEnv` in at the db-construction
boundary that feeds each writer, COMPOSING over the domain a route file
already rides via the new `options.d1` override (the KS-8.12/8.13
pattern): business/order statements read-back mirror to the business
Postgres twin while the wrapped domain's statements pass through to (and
mirror via) their own seam. Live boundaries wired: customer-orders and
operator-order-triage (business OVER the sites proxy, file-local
`openAgentsDatabase` helper); operator-adjutant launch-state UPDATE
(business UNDER the CRM seam and OVER the sites proxy at the two
`makeCrmEmailDatabaseForEnv` sites feeding it); adjutant-run-lifecycle
via `omni-handlers` (business OVER the sites proxy at the single
`applyAdjutantRunLifecycleEvents` call); the stripe-billing–fed
`business_checkout_kickoffs` + `business-referral-engagement-feed` funnel
writes (billing-routes webhook + return-URL fulfillment boundaries — the
1c323deac3 sourceRef hardening is untouched, only the db handle is
wrapped); onboarding referral consumption (`consumePendingReferralForUser`
/ `linkPendingReferralToOrder` in `onboarding/routes.ts`); and the
operator QA-swarm first-engagement path
(`business_commitment_ledger`/`qa_swarm_first_engagements`) which #8325
had left on raw D1 while the public routes path already rode the factory.
Already-wired-in-8325 and confirmed still covered: `handleBusinessSignupApi`
referral capture/consume (`referral-source-capture`,
`business_signup_referral_attributions`), `site-referral-routes` capture,
session-bootstrap consumption, and the pipeline/QA-swarm public stores.
`business-new-routes` itself writes nothing — it only renders and
validates referral codes (`isSafeReferralSourceRef`); the capture write it
implies happens in `business-signup-routes`, already on the factory.

**Decommission audit (write-dead / dormant boundaries).** These have NO
live worker write statement, so there is nothing to wire — they take a
`db` param but are reached only by their own unit tests:
`github-writeback-authority` (`order_github_write_authority_receipts`,
`order_fulfillment_artifacts`, `software_orders` UPDATE) and
`github-pr-fulfillment` (`order_fulfillment_artifacts`, `software_orders`
delivered flip), whose only callers are their tests; and
`site-referral-workflow-events.recordReferralWorkflowEvent`
(`referral_workflow_events`) via `site-referral-policy`, both test-only.
`order_fulfillment_feedback` and `referral_invites` likewise have no live
write statement (already noted in #8325). Their exact SQL shapes are still
pinned in the classifier contract suite so that IF a production caller is
added later it classifies as a mirrored write (never a silent
`khala_sync_business_write_unclassified` drift); they take the
snapshot-and-drop short path at cutover unless a live writer lands first.

Prod cutover procedure: [`RUNBOOK.md`](./RUNBOOK.md) "Business funnel
domain cutover"; cutover evidence + D1 drop tracked on epic
[#8282](https://github.com/OpenAgentsInc/openagents/issues/8282).

**KS-8.14 read-cutover follow-up status (2026-07-05, #8360): BOUNDED READ
SERVING LANDED, production backfill/verify GREEN.** Production backfill +
`--restart` catch-up sweep + `--verify --verify-newest 50` are green across
all 32 tables (exact counts, the five attribution-table set digests,
`promise_transition_receipts` full-row hash-set equality, funnel cohort
tallies, money sums, newest-50 hashes) — evidence posted on #8282 and
#8360. `business-domain-store.ts` now implements REAL Postgres read
serving for `KHALA_SYNC_BUSINESS_READS=postgres`, but only for the bounded
allowlist `BUSINESS_DOMAIN_POSTGRES_SERVED_READ_TABLES` (today:
`business_funnel_events` alone — the public funnel dashboard's two
full-table aggregate reads, re-derived index
`business_funnel_events_dashboard_covering_idx` added in khala-sync
migration `0033_business_funnel_events_dashboard_read_index.sql`). Every
OTHER comparable-select in this domain — the escalation pager's
`listBlockedPromises`/`listDuePromises`, referral-attribution
existence-checks feeding `INSERT OR IGNORE` consume-once decisions,
pipeline/order/referral list reads — stays D1-served under `postgres`
exactly like `compare` mode, PERMANENTLY, not as a staging step: those
reads feed write-path decisions or cron evaluators where a lagging mirror
read could silently under-attribute a payout-feeding referral or
double/skip a cron action. Widening the allowlist to another table is a
separate, individually reviewed follow-up, never a blanket flag flip.
Fail-soft in both directions: a Postgres read error on the allowlisted
surface falls back to D1 (`khala_sync_business_postgres_read_serve_failed`)
and never fails the request.

**Analytics Engine decision (§3.11's "AE candidate" flag, item 4 of
#8360):** KEEP `business_funnel_events` RELATIONAL for now. Production
volume is currently tiny (D1 row count was 0 at verify time — this domain
has not yet seen meaningful signup-funnel traffic), so there is no cost or
scan-latency pressure that would justify sacrificing the exact
byte-hash-verified dual-write/backfill machinery already built and tested
for this table in exchange for an Analytics Engine dataset (a distinct,
larger lane: a new AE binding, a rewritten writer at the funnel-event
insert call site, and a rewritten dashboard reader against the AE SQL
API). Revisit this decision if/when funnel-event volume or Postgres
storage/scan cost genuinely requires it — it is a dedicated follow-up, not
a component of this read-cutover.

**Decommission audit confirmation (2026-07-05):** production D1 has ZERO
rows in `order_fulfillment_feedback` and `referral_invites` (confirmed via
direct `wrangler d1 execute --remote` query), and zero
`business_funnel_events_0275` / `business_service_promises_0275` tables
exist (confirmed via `sqlite_master` query) — the #8325 rename-back
completed cleanly in both worker migrations `0275`/`0277`, matching the
`0023_business_funnel.sql` migration header's expectation.

**D1 drop status:** NOT done in this pass. Per the epic's KS-8.19
consolidation policy (confirmed by re-reading #8330's current wave
evidence, e.g. `06f5b91793`, `87e6992d1e`), destructive D1 retirement is
batched into #8330's own wave sweeps rather than per-domain — this
follow-up intentionally stops at read-cutover + decommission-audit
confirmation and leaves the 32 D1 tables live for #8330 to retire in its
own reviewed wave.

**KS-8.14 independent fresh re-confirmation (2026-07-05, #8282/#8330
follow-up):** the 2026-07-05 forum-and-user-content backup-verification pass
noted this domain's #8360 evidence looked real but was not itself
independently re-run. Re-ran `backfill-business.ts --verify
--verify-newest 50` against production fresh, from scratch, in a separate
session: **VERIFY OK** across all 32 tables — exact row counts, the five
attribution-table set digests, `promise_transition_receipts` full-row
hash-set equality, funnel cohort tallies, and money sums, matching the
#8360 evidence exactly (e.g. `promise_transition_receipts` 78/78,
`buy_mode_jobs` 51/51). Confirms the earlier claim was genuine, same-day
evidence, not a stale re-quote. **Still safe to cite on KS-8.19 (#8330).**

### 3.12 KS-8.15 — Training, gym, evals

**KS-8.15 source status (2026-07-04):** training CORE machinery LANDED —
Postgres schema (`khala-sync-server` migration
`0019_training_domain.sql`) covers the seven `training_*` tables (runs,
windows, window events, window leases, verification challenges/events,
trace contributions) with indexes re-derived from the live reads and
the lease/contribution idempotency keys ported exactly. The shared
registry `packages/khala-sync-server/src/training-domain-tables.ts`
owns column/key order for both the Worker mirror and the backfill
verifier; the Worker seam
`apps/openagents.com/workers/api/src/training-domain-store.ts` wraps
the three existing D1 stores (authority / verification / trace
contribution) with fail-soft read-back mirroring at every write call
site, behind `KHALA_SYNC_TRAINING_DUAL_WRITE` (default ON) /
`KHALA_SYNC_TRAINING_READS` (default `d1`; routes the
`listClaimableWindows` scan behind the re-homed
`SelfServeWindowProducer.topUp` cron). Backfill + exact verify lives at
`packages/khala-sync-server/scripts/backfill-training.ts` (counts,
newest-N row hashes, window/verification event-chain fingerprints,
per-window lease-set fingerprint, state tallies). Lease claiming stays
D1-authoritative until cutover, where it becomes a real Postgres
row-lock transaction (RUNBOOK "Training domain cutover").

**KS-8.15 remainder status (2026-07-05, #8355/#8380): LANDED.** The active
gym / mullet / blueprint / replay-clip / mirrorcode remainder (16 tables after
retiring the five write-dead `gym_agentcl_eval_*` tables) has its Postgres
twins (khala-sync migration `0026_gym_evals_domain.sql`, followed by
`0031_drop_gym_agentcl_eval_tables.sql`), shared registry
`packages/khala-sync-server/src/gym-evals-domain-tables.ts`, Worker seam
`apps/openagents.com/workers/api/src/gym-evals-domain-store.ts` (row-level
dual-write store + fail-soft read-back mirror + `make*ForEnv` drop-ins,
flag `KHALA_SYNC_GYM_EVALS_DUAL_WRITE` default ON /
`KHALA_SYNC_GYM_EVALS_READS` default `d1`), cursor-resumable backfill +
exact verify (`packages/khala-sync-server/scripts/backfill-gym-evals.ts`),
and a contract suite against both stores
(`gym-evals-domain-repository.contract.test.ts`). Confirmed findings:
`gym_harbor_full_trace_archives` is R2-body-split (D1 carries only
`artifact_r2_key`/`artifact_sha256`/`artifact_bytes`; the twin never
carries a body — no table skipped); the derived
`gym_ladder_leaderboard_snapshots` / `gym_run_progress_snapshots` are
verified by newest-N byte-exact copy-equality (leaderboard recomputation
equality), NOT recomputed in Postgres. Live dual-write is wired for the gym stores
(run-progress, mirrorcode, ladder, mutalisk delegation, harbor); the
transactional `mullet_*` / `blueprint_*` / `replay_clip_jobs` call-site
mirror wiring lands with the read-cutover follow-up (RUNBOOK "Gym/evals
domain cutover"). The retired AgentCL eval family is dropped by Worker
migration `0301_drop_gym_agentcl_eval_tables.sql`; broad D1 retirement stays
in KS-8.19
[#8330](https://github.com/OpenAgentsInc/openagents/issues/8330).

**KS-8.15 fresh backup verification (2026-07-05, dedicated forum-and-
user-content backup follow-up, #8282/#8330):** this domain was explicitly
flagged in the 2026-07-05 backup-verification pass as "not reviewed this
pass — not asserted safe" (see
[`2026-07-05-forum-and-user-content-backup-verification.md`](./2026-07-05-forum-and-user-content-backup-verification.md)).
An independent same-day follow-up ran `backfill-training.ts --verify` fresh
against production and found the SAME gap shape as the forum/CRM/Sites
finding: all seven training-core tables were **100% unbackfilled** —
Postgres held 0 rows across the board while D1 held the real corpus
(`training_runs` 9, `training_windows` 33, `training_window_events` 53,
`training_window_leases` 2,153, `training_verification_challenges` 43,
`training_verification_events` 129, `training_trace_contributions` 1,006).
Ran the standard recipe (full sweep, resumed after one transient wrangler
API error, `--restart` catch-up sweep) then `--verify --verify-newest 50`:
**VERIFY OK** — `{"chainMismatches":[],"countMismatches":[],"newestHashMismatches":[],"stateTallyMismatches":[]}`
(exit 0), all seven tables exact.

The gym/mullet/blueprint/replay-clip/mirrorcode remainder (16 tables) had a
smaller, PARTIAL gap: 14 of 16 tables were already correctly converging via
live dual-write, but `gym_run_progress_snapshots` (3 D1 rows) and
`mirrorcode_runs` (5 D1 rows) were fully absent from Postgres — confirmed by
direct D1 row counts on all 16 tables before backfilling (the other 14 are
genuinely 0 rows in production today, not silently skipped). Ran
`backfill-gym-evals.ts` (sweep, `--restart` sweep, `--verify`): **VERIFY OK**
— `{"countMismatches":[],"newestHashMismatches":[],"stateTallyMismatches":[]}`
(exit 0).

Both training-core and gym-evals-remainder are now **SAFE TO CITE ON KS-8.19
(#8330)** for the mirror-completeness precondition. D1 was never read from
in a mutating way, written to beyond the additive `ON CONFLICT DO NOTHING`
converge upsert, or schema-changed; no `KHALA_SYNC_*_READS` /
`KHALA_SYNC_*_DUAL_WRITE` flag was flipped.

- **What:** training runs/windows/leases/verification, trace
  contributions, gym run progress + delegation + leaderboards, mullet
  simulations, blueprint program runs, replay clips, mirrorcode runs.
- **Tables (~23 active):** `training_*` (7), `gym_*` (6), `mullet_*` (5),
  `blueprint_*` (3), `replay_clip_jobs`, `mirrorcode_runs`.
- **Heat:** bursty during runs, cold otherwise; leaderboard snapshots are
  derived.
- **Risks:** window leases are correctness-bearing (double-lease =
  double-payout risk upstream) — lease acquisition becomes a Postgres
  row-lock transaction, which is strictly better than D1's; large trace
  archives (`gym_harbor_full_trace_archives`) — confirm R2 payload split
  before porting.
- **Verification:** window/lease chain equality; leaderboard
  recomputation equality; verification-event chains contiguous.
- **Cron re-homed:** `SelfServeWindowProducer.topUp`.
- **Depends:** KS-8.2 (verification correlates with ledger), KS-8.5
  (trace contributions reference agent traces).

### 3.13 KS-8.16 — Forge (git intake + coordination)

- **What:** Forge git object/ref storage, packfile archives, receive-pack
  intakes, ref locks, tenants, access tokens, GitHub mirroring, merge
  queue, coordination status, verification receipts, dispatch leases,
  promotion decisions.
- **Tables (16):** all `forge_*`.
- **Heat:** bursty on push/mirror; ref updates are contention-sensitive.
- **Risks:** `forge_git_objects` payloads belong in R2 (packfile archives
  already are) — Postgres carries refs/locks/metadata only; ref locking
  gains real `SELECT ... FOR UPDATE` semantics — port the lock protocol
  deliberately, don't emulate the D1 dance; access tokens are
  secret-bearing (invariant 9).
- **Verification:** ref-set equality against a live `git ls-remote` of
  each tenant repo (ground truth is git itself); merge-queue ledger
  replay equality.
- **Cron re-homed:** none.
- **Depends:** none hard (self-contained bounded context) — scheduled
  late purely by risk budget.

### 3.14 KS-8.17 — Supervision long tail (Adjutant, Omni, Autopilot, ops)

**KS-8.17 read-cutover follow-up status (2026-07-05, #8361):** write-wiring +
backfill + verify + shadow-compare are DONE and re-confirmed fresh (a repeat
`--verify` after real organic writes landed in `relay_health_probes` since the
first pass — 2024 → 2028 rows — still matched exactly across all 29 tables).
`omni_public_proof_bundles`, the domain's one public projection surface, is
genuinely zero-traffic (0 rows in D1 AND Postgres, prod AND staging,
re-confirmed this pass) — the Analytics Engine soak-observability query
script (`query-compare-soak.ts`) also remains blocked on a Cloudflare API
token permission gap (`NEEDS_OWNER.md`, "Account Analytics: Read", still
unresolved as of this pass). A live-traffic soak is therefore structurally
unavailable for this surface and would stay vacuous indefinitely if waited
on, so this pass built the bounded real-Postgres-serve reader
(`makeOmniPublicProofBundlePostgresServerForEnv` in
`supervision-longtail-domain-store.ts`, wired via the new
`serveProofBundleFromPostgres` dependency in `omni-bundle-routes.ts`) and
flipped `KHALA_SYNC_SUPERVISION_READS=postgres` in prod + staging — matching
the KS-8.14 business-domain precedent (#8360) of accepting a bounded,
single-table, already-shadow-compared allowlist backed by contract-suite +
row-for-row `--verify` evidence instead of a traffic-based soak. Every other
comparable read in this domain stays D1-only by construction (no reader
wired for anything else). D1 table drop stays consolidated in the epic's
KS-8.19 sweep (#8330), per the same convention as #8358/#8360/#8362. See the
RUNBOOK "Supervision long-tail cutover" section for the full flag-flip
history.

- **What:** Adjutant assignment enrichment/research, Omni workrooms +
  outcome contracts + evidence bundles, Autopilot work orders /
  continuation policies / onboarding / legacy token usage, relay health
  probes, backend incident events, hygiene debt receipts.
- **Tables (~30):** `adjutant_*` (10), `omni_*` (9), `autopilot_*` (6,
  incl. legacy `autopilot_token_usage` — reconcile-and-freeze rather than
  live-migrate if it is write-dead; verify first), `relay_health_*` (2),
  `backend_incident_events`, `hygiene_debt_receipts`.
- **Heat:** cold-to-warm; several tables may already be write-dead —
  each gets a freshness check (last-write timestamp) and write-dead
  tables take the short path: snapshot to R2 + verified copy + drop, no
  dual-write phase.
- **Risks:** `omni_idempotency_keys` is a pure idempotency table (port
  key-exactly); evidence/proof bundles are public projections.
- **Verification:** row counts; idempotency key set equality; public
  proof-bundle endpoints shadow-compared.
- **Cron re-homed:** `AutopilotScheduledLaunches.dispatchDue`,
  `AutopilotContinuationPolicy.sweep`, `RelayHealth.probeTick`.
- **Depends:** KS-8.5 (adjutant references assignments/runs), KS-8.8
  (spend/payout references).

### 3.15 KS-8.18 — Identity and auth core (last)

**KS-8.18 follow-up status (2026-07-05, #8362):** write-site wiring COMPLETE.
Every remaining identity/auth write call site (the five typed factories
beyond the flagship token-custody vault, plus every scattered inline
writer across `index.ts`, `onboarding/repository.ts`,
`auth/email-otp-hardening.ts`, `operator-provider-account-routes.ts`,
`provider-account-pool-routes.ts`, `artanis-operator-dashboard-routes.ts`)
now adopts `identityAuthMirrorFromEnv`. A production `--restart` backfill
sweep + `--verify` ran clean immediately after: all 17 tables exact row
counts, newest-50 row hashes all matched, zero scalar-tally mismatches
(evidence on #8362). D1 remains the SOLE authority; `KHALA_SYNC_IDENTITY_READS`
is untouched (still `d1`, still defers on `postgres`); NO flags were
flipped. The owner-gated read cutover (KV/cache layer, real auth-matrix
shadow-read replay tooling, session-revocation staging drill) is NOT
built and remains a separate, not-yet-scheduled follow-up — see the
RUNBOOK "Identity/auth domain cutover" section for the full flag-flip
order and the one known, deliberately-accepted drift source
(`openauth_storage`'s TTL-shaped row-count asymmetry).

**Second follow-up (2026-07-05, same day, #8362):** confirmed the write-site
wiring above is now LIVE in production (first post-commit deploy
`2026-07-05T06:53:56Z`, version `b34ad490-450b-4728-b945-ad858983917a`) and
re-ran `--restart` + `--verify` against production hours into live traffic on
the new code: all 17 tables still exact (same row counts as the pre-deploy
snapshot; a restart sweep re-converges regardless), zero mismatches. Also
produced a read call-site classification inventory — which identity/auth
reads are permanent D1-only auth-decision paths vs. candidates for a future
bounded non-gate-read allowlist (mirroring `inference-entitlements-store.ts`)
— see the RUNBOOK section for the full list. No candidate was implemented or
flipped this pass; that remains real, separate follow-up work needing its own
flag, its own compare-mode soak, and (for the actual auth-decision reads) the
KV/cache layer and auth-matrix replay tooling that still do not exist.
Forum posts are confirmed out of scope for this domain's backup concerns —
they live under the separate, already-landed KS-8.10 Forum lane (#8321).

**KS-8.18 fresh re-verification (2026-07-05, dedicated forum-and-user-content
backup pass):** re-ran `backfill-identity-auth.ts --verify` (read-only,
secret-safe by construction) against production as an independent
confirmation, not a re-quote of the #8362 evidence above. Result: exit 0, all
17 tables exact (`users`/`auth_identities` 462/462, `openauth_storage`
176/176, `openauth_agent_links` 21/21, the provider custody family
155/478/64/31/26/12, etc.). No secret values were read into this report or
that pass — only row counts and the script's own custody-safe scalar
tallies. Confirms the domain's own #8362 evidence still holds today. Full
evidence:
[`2026-07-05-forum-and-user-content-backup-verification.md`](./2026-07-05-forum-and-user-content-backup-verification.md).

- **What:** the tables every request touches: users, auth identities,
  OpenAuth storage + agent links, GitHub write connections/grants,
  provider (BYOK) account custody family.
- **Tables (~26):** `users`, `auth_identities`, `openauth_storage`,
  `openauth_agent_links`, `github_write_*` (3), `provider_accounts`
  (+`_0173_new`), `provider_account_*` (16 more, incl. `_0173_data` /
  `_0237_data` artifacts).
- **Heat:** hottest *read* family in the system (auth on every request)
  but highly cacheable; writes are rare.
- **Risks:** maximum blast radius — a bad cutover breaks literally
  everything, which is why it goes **last**, after the recipe has been
  proven ~14 times; token custody + auth grants are secret-bearing
  (invariant 9: never into changelog post-images beyond owner scope; at
  rest, same encryption posture as today); auth reads must gain a
  KV/cache layer as part of this move so Postgres doesn't inherit a
  per-request read storm; session invalidation semantics verified
  explicitly (revoke in staging, observe both stores deny).
- **Verification:** identity set equality; a full auth matrix (each
  credential class × allow/deny) replayed against shadow reads; custody
  audit-chain contiguity.
- **Cron re-homed:** none.
- **Depends:** everything before it (deliberately).
- **CFG-4 Domain 2 HARD CUTOVER (2026-07-06, #8519):** `users` and
  `auth_identities` are Cloud SQL Postgres-AUTHORITATIVE with the D1 code
  path DELETED — including the auth-GATE reads (agent bearer-token
  resolution, session-subject upserts), the owner-approved supersession of
  this section's "reads stay D1 until the owner-gated last step" plan for
  exactly these two tables. Store: `workers/api/src/identity-db.ts`
  (reuses the CFG-4 `PaymentsLedgerDb` executor over `KHALA_SYNC_DB`).
  Schema: khala-sync migration `0042_identity_hard_cut.sql` (users
  onboarding columns from worker 0025, UNIQUE
  `(provider, provider_subject)`, read accelerators). The Worker mirror
  surface is now the registry minus these two tables
  (`IdentityAuthMirrorTable`); `backfill-identity-auth.ts` excludes them
  from the default sweep (explicit `--table` = pre-deploy catch-up ONLY).
  The other fifteen tables in this section keep the staged plan above
  unchanged.

### 3.16 Explicit non-migrations

- **`sync_scopes` / `sync_changes` / `sync_mutations`** (legacy
  `@openagentsinc/sync-worker` outbox): superseded by the khala-sync
  substrate itself. Decommission path: confirm no readers, snapshot,
  drop. No Postgres twin.
- **Verse world state:** not on `OPENAGENTS_DB`; out of scope.
- **Firehose streams** flagged per domain (heartbeats, `*_metric_events`,
  runtime/health snapshots, funnel event volume): re-home to Analytics
  Engine + R2, not Postgres rows.

## 4. Cron consolidation (the 25-task/min floor)

The API Worker runs `crons: ["* * * * *"]` with ~23 observed tasks in the
`scheduled` handler (`apps/openagents.com/workers/api/src/index.ts`),
every task hitting `OPENAGENTS_DB` every minute — the fixed contention
floor from fable §1.2 item 4. Each task re-homes **with its domain**
(listed per domain above); consolidated view:

| Cron task | Re-homes with |
|---|---|
| `HydraliskGlmPoolHeartbeat.run` | KS-8.2 / Phase 0 → Analytics Engine |
| `FleetBurnStallDetector.tick` | KS-8.1 + KS-8.2 reads; residuals KS-8.4 |
| `ServingRateMonitor.tick` | KS-8.2 reads; `fleet_alerts` rows KS-8.4 |
| `PylonCapacityFunnel.recordSnapshots` | KS-8.4 |
| `AgentDefinitionScheduler.tick` | KS-8.5 |
| `ArtanisScheduledRunner.runTick`, `ArtanisResponder.scan`, `ArtanisResponder.compose`, `ArtanisAdmin.tick`, `ArtanisAdmin.closeoutVerifier`, `ArtanisFleet.tick` | KS-8.6 |
| `TipsSweep.runTick`, `TipsBuffer.reconcileForwarding`, `TipsBuffer.backingInvariant`, `TreasuryTransactions.reconcilePending`, `XClaimRewardTreasuryDispatcher.runTick`, `ForumDirectTips.archiveStaleRecoveries` | KS-8.8 |
| `EmailCampaignDispatcher.dispatchDue` | KS-8.11 |
| `BusinessFulfillmentLoop.dailyMotion` | KS-8.14 |
| `SelfServeWindowProducer.topUp` | KS-8.15 |
| `AutopilotScheduledLaunches.dispatchDue`, `AutopilotContinuationPolicy.sweep` | KS-8.17 |
| `RelayHealth.probeTick` | KS-8.17 |

Consolidation rules, enforced as domains move (final sweep in KS-8.19):

1. **Cadence honesty:** re-homed tasks get per-task cadence (most do not
   need every-minute; daily motion is daily). The Worker cron becomes a
   thin dispatcher (or a scheduler DO with alarms) that enqueues named
   task runs; heavy periodic aggregates become Postgres-side scheduled
   jobs (pg_cron on Cloud SQL) where they are pure SQL.
2. **A task never straddles stores:** a cron task cuts over atomically
   with its domain's read cutover — never reads D1 and writes Postgres
   or vice versa.
3. **Tick idempotency is a landing requirement:** every re-homed task
   proves a double-fire is a no-op before it leaves D1.

## 5. Waves and rationale

Ordering = risk × value × coupling, aligned with fable §5 Phase 3
("Ledger + traces, then forum, CRM, sites, billing — each domain:
dual-write → backfill → verify → cut reads → drop"). One deliberate
refinement over the report's illustrative order: the **money wave runs
before the forum/CRM/sites wave**, because (a) the exact-reconciliation
tooling built for KS-8.2 transfers directly while fresh, and (b) forum
tips and site commerce cut over cleanly only when the money rails they
reference are already on Postgres — otherwise those cutovers would
straddle stores.

| Wave | Issues | Theme | Rationale |
|---|---|---|---|
| **A — finish the hot path** | KS-8.4, KS-8.5, KS-8.6 | Pylon remainder + agent runs/traces + Artanis | Completes the June-29 failure neighborhood: after wave A the entire fleet execution/supervision write path (and 7 more cron tasks) is off D1. Highest measured write volume; verification culture already in place. "Ledger + traces" per Phase 3. |
| **B — money** | KS-8.7, KS-8.8, KS-8.9 | Billing/Stripe → treasury/payouts/tips → inference entitlements | Low volume, highest correctness stakes, freshest reconciliation muscle; unblocks the store-straddle-free cutover of forum tips and site commerce in wave C; moves 6 more cron tasks. Entitlements (8.9) close the serving-path remainder. |
| **C — product surfaces** | KS-8.10, KS-8.11, KS-8.12, KS-8.13 | Forum, CRM/email, Sites, Khala Code state | The Phase 3 "forum, CRM, sites" core. Big backfills, read-heavy public projections, compliance gates (suppression). KS-8.13 doubles as sync-engine adoption for threads/teams (needs KS-5/KS-6 client lanes done). |
| **D — long tail** | KS-8.14, KS-8.15, KS-8.16, KS-8.17 | Business/referrals, training/gym, Forge, supervision tail | Cold/bursty domains; several likely write-dead tables take the snapshot-and-drop short path. Forge is self-contained and benefits from real transactions. |
| **E — core + retirement** | KS-8.18, KS-8.19 | Identity/auth, then cron consolidation sweep + D1 retirement | Auth goes last by blast-radius policy, after ~14 proven cutovers. KS-8.19 closes the epic: final cron sweep, retirement checklist, invariant registration. |

Waves are sequential; issues **within** a wave may run as parallel lanes
where their "Depends" lines allow. Reconciliation evidence must land in the
issue before read cutover. The old per-domain soak/drop follow-ups are not
active gates; KS-8.19 owns final D1 retirement after the domain migration
fanout is complete.

## 6. D1 retirement checklist (KS-8.19)

Run after the domain migration fanout reaches wave E and the owner opens the
final D1 retirement gate:

1. **Inventory zero:** every table in §2 is either migrated (Postgres),
   re-homed (Analytics Engine/R2), decommissioned (legacy sync), or
   explicitly retained (see 6.7). Before batching any drop migration, rerun
   `bun run d1:zero-reference-sweep` from `apps/openagents.com` and compare
   it to
   [`../cleanup/2026-07-05-d1-zero-reference-sweep.md`](../cleanup/2026-07-05-d1-zero-reference-sweep.md)
   (#8378). `wrangler d1 execute … "SELECT name FROM sqlite_master"` output
   still reconciles against this plan, table by table.
2. **Code zero:** no production code path reaches `OPENAGENTS_DB` — grep
   gate on the binding and on `openAgentsDatabase(` outside any retained
   staging module; the ~50 local `d1Effect` wrappers deleted.
3. **Cron zero:** the `scheduled` handler contains no D1-touching task;
   per-task cadences documented; the every-minute cron either removed or
   reduced to the thin dispatcher.
4. **Archive:** final full D1 export snapshot to R2 (retention: 1 year)
   with a manifest mapping each table to its successor store.
5. **Invariants:** SPEC §7 set registered in the owning `INVARIANTS.md`;
   per-domain migration flags deleted; behavior contracts for cutover
   indicators retired or repointed.
6. **Bindings:** `OPENAGENTS_DB` removed from `wrangler.jsonc` — or, if
   6.7 applies, repointed to a **new, empty, small** staging database
   (never the historical one, so accidental legacy reads fail loudly).
7. **What may remain on D1 — bounded staging only:** nothing is
   *required* to remain. The only admissible residents are bounded,
   non-authoritative, TTL-swept staging buffers (e.g. webhook intake
   dedupe with a 7-day sweep) where a Queue or KV genuinely does not fit.
   Each such resident needs: a size bound, a sweep job, a named owner,
   and an entry here. Authoritative state, ledgers, receipts, counters,
   and anything a public projection reads may **never** remain on D1.
8. **After-action:** close the loop on the 2026-06-29 after-action — a
   20-worker fleet accept burst plus a Vertex-style burn produces zero
   `overloaded` denials with D1 absent.

## 7. Issue map

Epic [#8282](https://github.com/OpenAgentsInc/openagents/issues/8282).
KS-8.1 = [#8307](https://github.com/OpenAgentsInc/openagents/issues/8307),
KS-8.2 = [#8308](https://github.com/OpenAgentsInc/openagents/issues/8308),
KS-8.3 (this plan) = [#8309](https://github.com/OpenAgentsInc/openagents/issues/8309).

| Issue | Domain | Wave |
|---|---|---|
| KS-8.4 [#8315](https://github.com/OpenAgentsInc/openagents/issues/8315) | Pylon control-plane remainder + telemetry split | A |
| KS-8.5 [#8316](https://github.com/OpenAgentsInc/openagents/issues/8316) | Agent runtime metadata (definitions/runs/traces) | A |
| KS-8.6 [#8317](https://github.com/OpenAgentsInc/openagents/issues/8317) | Artanis supervision loops | A |
| KS-8.7 [#8318](https://github.com/OpenAgentsInc/openagents/issues/8318) | Billing, credits, Stripe, pay-ins | B |
| KS-8.8 [#8319](https://github.com/OpenAgentsInc/openagents/issues/8319) | Treasury, payouts, tips settlement | B |
| KS-8.9 [#8320](https://github.com/OpenAgentsInc/openagents/issues/8320) | Inference entitlements and quotas | B |
| KS-8.10 [#8321](https://github.com/OpenAgentsInc/openagents/issues/8321) | Forum (content + trust) | C |
| KS-8.11 [#8322](https://github.com/OpenAgentsInc/openagents/issues/8322) | CRM, email, enrichment | C |
| KS-8.12 [#8323](https://github.com/OpenAgentsInc/openagents/issues/8323) | Sites, site builder, targeted sites | C |
| KS-8.13 [#8324](https://github.com/OpenAgentsInc/openagents/issues/8324) | Khala Code product state (threads/teams/workspaces) | C |
| KS-8.14 [#8325](https://github.com/OpenAgentsInc/openagents/issues/8325) | Business funnel, orders, referrals | D |
| KS-8.15 [#8326](https://github.com/OpenAgentsInc/openagents/issues/8326) | Training, gym, evals | D |
| KS-8.16 [#8327](https://github.com/OpenAgentsInc/openagents/issues/8327) | Forge | D |
| KS-8.17 [#8328](https://github.com/OpenAgentsInc/openagents/issues/8328) | Supervision long tail (Adjutant/Omni/Autopilot/ops) | D |
| KS-8.18 [#8329](https://github.com/OpenAgentsInc/openagents/issues/8329) | Identity and auth core | E |
| KS-8.19 [#8330](https://github.com/OpenAgentsInc/openagents/issues/8330) | Cron consolidation sweep + D1 retirement | E |

Closed/not-planned tracker cleanup (owner direction, 2026-07-04):

- [#8331](https://github.com/OpenAgentsInc/openagents/issues/8331) — the
  KS-8.1 per-domain D1 dispatch-table decommission/soak ticket is not an
  active gate. Its destructive work is covered by KS-8.19.
- [#8333](https://github.com/OpenAgentsInc/openagents/issues/8333) — the
  KS-8.2 per-domain token-ledger decommission/soak ticket is not an active
  gate. Its destructive work is covered by KS-8.19.
