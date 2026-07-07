---
status: assessment
date: 2026-07-06
owner: OpenAgents
scope: D1 → Cloud SQL (Postgres) read-cutover readiness for every Khala Sync domain still on `d1` or `compare`
related:
  - ./2026-07-06-cloudflare-to-google-consolidation-audit.md
  - ./2026-07-06-openagents-domain-cutover-runbook.md
  - ../khala-sync/RUNBOOK.md
  - ../khala-sync/MIGRATION_PLAN.md
  - ../khala-sync/2026-07-05-forum-and-user-content-backup-verification.md
guardrail: ASSESSMENT + BACKFILL-PREP ONLY. No `KHALA_SYNC_*_READS` flag was flipped and no deploy was run in producing this report.
---

# D1 domain cutover readiness (2026-07-06)

## TL;DR

- **The D1 bridge is dead** (`d1-http bridge query failed (401): 10000 Authentication error`,
  confirmed in `openagents-monolith` Cloud Run logs). Practical consequence:
  **every domain on `d1` OR `compare` is currently serving from a dead store.**
  `compare` mode SERVES D1 (shadow-reads Postgres) — so `compare` is *not* safe
  validation right now, it is an active outage for those reads. Flipping a domain
  to `postgres` can therefore only **help** (serve the backfilled mirror) or be
  **inert** (still routes to dead D1) — it essentially never makes reads worse.
  The only genuine *risk* surface is a spend/quota **enforcement gate** read
  (ENTITLEMENTS gate), which is the one flip that changes an ALLOW/DENY authority.

- **The Postgres mirror is complete and current** (fresh read-only counts today —
  see §Evidence). The two large overnight backfills flagged "in progress" on
  2026-07-05 (`pylon_assignment_events`, `agent_traces`) have **finished**
  (428,064 and 230,345 rows, both ≥ the 2026-07-05 D1 snapshot). All 2026-07-05
  `--verify` parity evidence still stands; counts have only grown consistently
  since via dual-write.

- **Two structural truths that reframe "flip to postgres":**
  1. **`postgres` mode is never a full cutover.** Every domain serves either a
     *bounded read allowlist* (business/billing pattern) or a *bounded set of
     wired call-sites* (treasury/artanis/crm/training/agent-runtime/ledger
     pattern); the remainder still defers to D1. For **FORUM, SITES, IDENTITY-gate,
     GYM_EVALS** the `postgres` flag is **fully inert** — it behaves exactly as
     `compare` (serves D1) because the read-serving path was never built.
  2. **D1 cannot be *dropped* for any domain by flipping READS.** The write path
     is still D1-authoritative everywhere (Postgres is a read-back mirror); a true
     "off D1" needs the per-domain **write-authority** cutover, which is out of
     scope for the READS batch and cannot currently be parity-checked against dead
     D1.

- **Re-verification limitation:** the standard `backfill-*.ts --verify` compares
  D1 vs Postgres and **cannot run now** (D1 is 401-dead). The 2026-07-05 pass is
  the **last valid D1-vs-Postgres parity snapshot**. Fresh evidence in this report
  is Postgres-side row counts (proving the mirror persisted and grew), not a new
  cross-store diff.

## Current prod flag state (live, `gcloud run services describe openagents-monolith`)

| Mode | Domains |
|---|---|
| `postgres` | BUSINESS, ENTITLEMENTS_NON_GATE, FORGE, IDENTITY_NON_GATE, PYLON, SUPERVISION |
| `compare` | ARTANIS, ENTITLEMENTS (gate) |
| unset → default `d1` | AGENT_RUNTIME, BILLING, CRM, FORUM, GYM_EVALS, IDENTITY (gate), LEDGER, SITES, TRAINING, TREASURY |

## Readiness table (every domain still on `d1` or `compare`)

| Domain | Cur. mode | What `postgres` mode actually serves | PG backfill readiness | Data-backup / parity gate | Flip verdict | Exact next step |
|---|---|---|---|---|---|---|
| **AGENT_RUNTIME** | d1 | Wired call-sites only: `listDueCronTriggers`, `listInboundWebhookTriggers` from mirror (retry→D1 fallback). Other reads stay D1. | **Complete.** `agent_traces` 230,345 (overnight backfill finished), `agent_run_events` 6,801, `agent_runs`/goals exact per 2026-07-05. | Operational metadata, not money/content. Verified exact 2026-07-05 (small tables) + traces backfill now complete. | **SAFE-TO-FLIP-NOW** | Set `KHALA_SYNC_AGENT_RUNTIME_READS=postgres`. |
| **ARTANIS** | compare | Wired operator-chat / status-spine reads from mirror (retry→D1 fallback). Cron ticks stay D1. | **Complete.** `artanis_messages` 104, `artanis_responder_ticks` 9,691. One known **non-content** drift table `artanis_responder_ticks` (#8409, tick bookkeeping race — not user data). | Operational. On `compare` today = serving dead D1, so flip strictly restores reads. | **SAFE-TO-FLIP-NOW** | Set `KHALA_SYNC_ARTANIS_READS=postgres`. |
| **BILLING** | d1 | **Bounded allowlist, display-only:** `billing_ledger_entries` (recent list), `billing_auto_top_up_*` + `stripe_saved_payment_methods` (auto-top-up display), `stripe_checkout_sessions` (settled receipt). **NOT** balance SUM, charge decisions, or buyer-payment idempotency — those keep dedicated D1 reads. `pay_ins` already removed (CFG-4, now Postgres credits-ledger direct). | **Complete for served surface.** `billing_ledger_entries` 2,264, `stripe_checkout_sessions` 0. 20/21 tables exact (#8337). `pay_in_legs` has the known #8412 2-row historical gap **but is NOT in the served allowlist**. | Money domain, but every allowlisted read is decision-free / immutable-settled. No charge/balance decision routes to Postgres. | **SAFE-TO-FLIP-NOW** (display surface) | Set `KHALA_SYNC_BILLING_READS=postgres`. Caveat: balance/charge/buyer-payment reads still hit dead D1 — separate write-path cutover, not this flag. |
| **CRM** | d1 | Wired call-sites incl. the suppression-compliance gate (atomic per-read, serves one store). | **Complete.** Near-zero prod data (`crm_email_messages` 0, `crm_contacts` 0); enrichment/outreach ledgers backfilled + verified exact 2026-07-05. | No customer PII in prod today. Low volume/low risk. | **SAFE-TO-FLIP-NOW** | Set `KHALA_SYNC_CRM_READS=postgres`. |
| **LEDGER** (token ledger) | d1 | The five public-tokens-served projections (homepage counter + history/model/demand/channel mix) from Postgres (retry→D1 fallback). | **Complete.** `token_usage_events` 311,783 (was 297,198 exact on 2026-07-05; grows live). Rollups show live-counter jitter, **not** a gap. | Public counter, not a spend gate. | **SAFE-TO-FLIP-NOW** (high value) | Set `KHALA_SYNC_LEDGER_READS=postgres` — fixes the public homepage counter currently reading dead D1. |
| **TRAINING** | d1 | Exactly ONE scan: `listClaimableWindows` (the top-up cron) from Postgres. Public run summaries / proof replay / timelines stay D1. | **Complete.** 100% historical gap fixed + verified exact 2026-07-05 (7 tables). | Training run/eval artifacts. Public-projection-bearing but bounded serve. | **SAFE-TO-FLIP-NOW** (bounded) | Set `KHALA_SYNC_TRAINING_READS=postgres`. Note: most public training reads still D1/dead until their own cutover. |
| **TREASURY** | d1 | Display / lookup reads from mirror. **EVERY money-decision scan** (payout dispatch, sweep candidates, pending-txn reconcile) passes no `readPostgres` and **stays D1 regardless of the flag.** | **Complete.** 25 tables verified exact 2026-07-05. | Money domain — but no payout/settlement **decision** routes to Postgres. Flip risk is bounded to display correctness against a verified mirror. | **SAFE-TO-FLIP-NOW** (display; money crons unaffected) | Set `KHALA_SYNC_TREASURY_READS=postgres`. Caveat: payout/sweep/reconcile crons stay D1-bound and are currently broken on dead D1 — separate write cutover, not this flag. |
| **ENTITLEMENTS** (gate) | compare | **The six enforcement gate reads** (ALLOW/DENY on inference spend/quota) served from Postgres. This *changes which store enforces*. | Backfill claimed LANDED (#8336) and trusted-but-not-independently-re-verified on 2026-07-05. Gate tables near-empty in prod today (`agent_search_entitlements` 0, `agent_rate_limit_entitlements` 0). | **HIGHEST RISK.** A lagging/mismatched read can allow a double-spend or false-quota-allowance. Store's own cutover order requires a zero-divergence `compare` soak before `postgres`. | **NEEDS-PARITY-PROOF** | Do NOT batch-flip blind. Run a Postgres-**internal** consistency check (accrual `tally = SUM(events)` per entitlement key) — D1-vs-PG compare is impossible now. If the owner needs enforcement working *at all* (it's broken on dead D1 today), postgres is the only live store; near-empty tables make the current double-spend surface minimal. Owner-gated. |
| **IDENTITY** (gate) | d1 | **Nothing new — INERT.** `KHALA_SYNC_IDENTITY_READS` gate reads are DEFERRED ("no routed identity read"); flag behaves as `compare`. NOTE: `users`/`auth_identities` were **hard-cut to Postgres-authoritative** via `identity-db.ts` (CFG-4 #8519) and the OpenAuth issuer already serves from the Postgres KvStore (CFG-3 #8518) — login/session already run off Postgres **independent of this flag**. | Mirror exact 2026-07-05 (`users`/`auth_identities` 464/464 today). | Auth/custody. The flag flip does nothing; the real auth read path is the owner-gated, done-last, highest-risk step and its KV-cache + session-revocation replay tooling is unbuilt. | **HOLD** (flip inert; core already off D1) | Leave `KHALA_SYNC_IDENTITY_READS` unset. Identity core is already Postgres-served via the hard-cut, not via this flag. |
| **FORUM** | d1 | **Nothing — INERT.** `postgres` read serving is DEFERRED/UNBUILT (domain-wide read surface); flag behaves as `compare` and serves dead D1. | **Backup COMPLETE + verified** (owner's #1 requirement MET): `forum_posts` 1,313, `forum_topics` 219 in Postgres, full parity verified 2026-07-05 (per-topic post chains + thread hashes). | Data is safe to retire D1 against. But **forum pages will not render from Postgres until the read-serving path is built** — the READS flag cannot fix this. | **HOLD** — flag is inert; **needs code, not a flag** | Build the forum Postgres read-serving path (inventory the domain-wide public read surface, wire `makePostgresForumContentStore` reads). Backup ≠ serving. |
| **SITES** | d1 | **Nothing — INERT.** `postgres` read serving DEFERRED/UNBUILT (live site-serving reads must be inventoried first per KS-8.12); behaves as `compare`. | **Backup complete + verified** 2026-07-05: `site_projects` 3, `site_deployments` 4, version/deploy chains exact. `site_environment_values.plain_value` **secrets intentionally NOT mirrored** (secret_ref indirection only). | Real user-authored sites, backed up. Serving path unbuilt. | **HOLD** — flag inert; needs read-serving impl | Inventory live site-serving reads, then wire the Postgres read path. |
| **GYM_EVALS** | d1 | **Nothing — INERT.** Reads never serve from Postgres this lane ("public gym projections never regress mid-cutover"); flag parsed but unused for reads. | **Complete.** 2-of-16-table gap fixed + verified exact 2026-07-05. | Eval artifacts (public projections). Low priority. | **HOLD** — flag inert; low priority | Leave unset (or a future read-serving lane if gym pages must render from Postgres). |

## Recommended final flag set for the batched redeploy

**Set `postgres` now (7 domains — safe + effective, all serve their wired read
surface from the complete mirror, none route a money/spend DECISION to Postgres):**

```
KHALA_SYNC_AGENT_RUNTIME_READS=postgres
KHALA_SYNC_ARTANIS_READS=postgres
KHALA_SYNC_BILLING_READS=postgres
KHALA_SYNC_CRM_READS=postgres
KHALA_SYNC_LEDGER_READS=postgres
KHALA_SYNC_TRAINING_READS=postgres
KHALA_SYNC_TREASURY_READS=postgres
```

**Hold (do NOT batch-flip):**

- **ENTITLEMENTS (gate)** — `NEEDS-PARITY-PROOF`. Only flip after a Postgres-internal
  accrual-tally consistency check; it is the single flip that moves a spend/quota
  ALLOW/DENY authority. Owner-gated. (It is broken on dead D1 today, so there is a
  forcing function, but do it deliberately, not in the blind batch.)
- **IDENTITY (gate)** — flag flip is **inert**; identity core already Postgres-
  authoritative via the CFG-4 hard-cut. Leave unset.
- **FORUM, SITES, GYM_EVALS** — flag flip is **inert** (read-serving path unbuilt).
  These need implementation work, not a flag. Flipping would falsely imply progress.

## Biggest risks (surface these to the owner)

1. **FORUM: backup is done, serving is not.** The owner's explicit requirement —
   forum posts backed up and moved into Postgres — is **MET** (1,313 posts,
   verified). But the READS flag is **inert** for forum: the Postgres read-serving
   path was never built, so forum pages stay broken on dead D1 regardless of the
   flag. **Do not conflate "safe to drop D1" (true for forum data) with "forum
   works" (false until the read cutover is coded).** Same shape for SITES.
2. **ENTITLEMENTS gate is the only money/gate flip with real blast radius.** It
   moves the inference spend/quota enforcement authority to a mirror that can no
   longer be D1-parity-checked. Prove Postgres-internal accrual consistency first;
   the near-empty gate tables currently limit the exposure.
3. **Money DECISION paths are not fixed by any READS flip.** Treasury payout/
   sweep/reconcile crons and billing charge/balance/buyer-payment reads stay
   D1-bound by design and are currently broken on dead D1. They need the
   **write-authority** cutover — a green READS flip must not be read as "money
   moves again."
4. **D1 cannot be dropped yet for any domain.** Writes are still D1-authoritative
   (Postgres is a read-back mirror) and reads can no longer be re-verified against
   dead D1. Dropping D1 requires the per-domain write cutover + a fresh parity
   basis, not just `READS=postgres`.

## Evidence (fresh, read-only, 2026-07-06)

Direct Cloud SQL `khala_sync_prod` counts (role `khala_app`, public IP,
`sslmode=require`; no D1 read — bridge is dead):

| table | Postgres rows (2026-07-06) | 2026-07-05 D1 baseline | note |
|---|---:|---:|---|
| forum_posts | 1,313 | 1,303 | +10 via dual-write; backup intact |
| forum_topics | 219 | 219 | exact |
| users / auth_identities | 464 / 464 | 462 / 462 | hard-cut authoritative (CFG-4) |
| token_usage_events | 311,783 | 297,198 | live counter; corpus backed up |
| pylon_assignments | 10,665 | 10,665 | exact |
| pylon_assignment_events | 428,064 | 425,300 (backfill in progress) | **overnight backfill finished** |
| agent_traces | 230,345 | 230,331 (backfill in progress) | **overnight backfill finished** |
| agent_run_events | 6,801 | 6,801 | exact |
| billing_ledger_entries | 2,264 | — | served allowlist |
| pay_ins / pay_in_legs | 302 / 324 | — | pay_in_legs #8412 gap; not served |
| site_projects / site_deployments | 3 / 4 | 3 / 4 | exact |
| artanis_messages / artanis_responder_ticks | 104 / 9,691 | — | ticks = known #8409 drift table |
| agent_search_entitlements / agent_rate_limit_entitlements | 0 / 0 | — | gate near-empty → low blast radius |

Log evidence: `d1-http bridge query failed (401)` present in `openagents-monolith`
logs (6h window); **zero** `khala_sync_*_read_compare_mismatch` /
`*_postgres_read_serve_failed` events in the 6–24h window (consistent with
compare-mode reads failing at the dead-D1 serve step before a mismatch can be
computed, and with the served domains being clean).
