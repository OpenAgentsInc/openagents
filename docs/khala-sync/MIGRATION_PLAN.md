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
5. **Soak, then drop.** ≥7-day soak with the D1 dual-write still on
   (instant rollback = flip the read flag back). Then: stop dual-write,
   snapshot the D1 tables to R2 (archaeology), drop the D1 tables, delete
   the flag.

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

### 3.12 KS-8.15 — Training, gym, evals

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
| `ServingRateMonitor.tick` | KS-8.2 |
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
where their "Depends" lines allow. Every issue lands via reviewed PR per
`docs/fable/EXECUTION.md`, with reconciliation evidence in the issue
before read cutover.

## 6. D1 retirement checklist (KS-8.19)

Run after wave E cutovers soak:

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
