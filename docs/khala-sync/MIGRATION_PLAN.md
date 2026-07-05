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
read cutover. Health/runtime snapshot retention (Analytics Engine vs
row-porting history) is a cutover-time decision: the backfill ports rows
as-is today; bounding retention before cutover shrinks the port. Prod
cutover procedure: [`RUNBOOK.md`](./RUNBOOK.md) "Artanis supervision
domain cutover"; cutover evidence + D1 drop tracked in the decommission
follow-up filed off [#8317](https://github.com/OpenAgentsInc/openagents/issues/8317).

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
batch-job-metering, inference-abuse-controls, serving-node-payout,
cloud-metering, product-promises, business-starter-credit). Prod cutover
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
the §3.13 ref-set surface; `postgres` serving is deferred to the cutover
follow-up and behaves as compare with a one-time
`khala_sync_forge_postgres_reads_deferred`). Resumable backfill +
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

### 3.5 KS-8.8 — Treasury, payouts, tips settlement

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
(`forum-content-repository.contract.test.ts`). The issue's REMAINING
tables — `forum_private_message_threads`, `forum_private_messages`,
`forum_acl_grants`, `forum_actor_forum_trust`, `forum_trust_edges`
(recompute-and-compare), `forum_score_snapshots` (derived — recompute on
Postgres), `forum_notification_reads`, and `forum_work_request_*` (6;
set-membership referential verification against KS-8.1 assignments and
KS-8.8 tips at cutover) — move in the follow-up remainder lane tracked
with the read cutover + D1 decommission follow-up. The forum MONEY
tables stay with KS-8.8 (D1 authority, that lane's mirror discipline) —
this lane never touches them. Prod cutover procedure:
[`RUNBOOK.md`](./RUNBOOK.md) "Forum content domain cutover"; cutover
evidence + D1 drop tracked on epic
[#8282](https://github.com/OpenAgentsInc/openagents/issues/8282).

- **What:** the forum content core: forums/boards/categories, topics,
  posts + bodies + revisions, private messages, trust edges/scores, ACLs,
  moderation, watches/bookmarks/notifications, work-request lifecycle.
  (The money half went in KS-8.8.)
- **Tables (~26):** `forum_forums`, `forum_boards`, `forum_categories`,
  `forum_topics`, `forum_posts`, `forum_post_bodies`,
  `forum_post_revisions`, `forum_private_message_threads`,
  `forum_private_messages`, `forum_acl_grants`, `forum_actor_follows`,
  `forum_actor_forum_trust`, `forum_trust_edges`, `forum_score_snapshots`,
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

**KS-8.15 remainder status (2026-07-04, #8355): LANDED.** The gym /
mullet / blueprint / replay-clip / mirrorcode remainder (21 tables)
now has its Postgres twins (khala-sync migration
`0026_gym_evals_domain.sql`), shared registry
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
equality), NOT recomputed in Postgres; the five write-dead
`gym_agentcl_eval_*` tables take the KS-8.17 short path (copy + verify
only, no dual-write). Live dual-write is wired for the gym stores
(run-progress, mirrorcode, ladder, mutalisk delegation, harbor); the
transactional `mullet_*` / `blueprint_*` / `replay_clip_jobs` call-site
mirror wiring lands with the read-cutover follow-up (RUNBOOK "Gym/evals
domain cutover"). Destructive D1 retirement stays in KS-8.19
[#8330](https://github.com/OpenAgentsInc/openagents/issues/8330).

- **What:** training runs/windows/leases/verification, trace
  contributions, gym eval runs + delegation + leaderboards, mullet
  simulations, blueprint program runs, replay clips, mirrorcode runs.
- **Tables (~29):** `training_*` (7), `gym_*` (11), `mullet_*` (5),
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
   explicitly retained (see 6.7). `wrangler d1 execute … "SELECT name
   FROM sqlite_master"` output reconciled against this plan, table by
   table.
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
