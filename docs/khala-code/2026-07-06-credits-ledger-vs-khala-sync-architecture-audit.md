# Credits/billing on D1 vs. Khala Sync on Cloud SQL — architecture audit

Date: 2026-07-06
Status: investigation-only audit, no code changed. Triggered by an owner
question asked in frustration: *"obviously a user's credits should be synced
to their device, but we implemented it in D1???? not our new cloud SQL/khala
sync????"*

Scope: this document traces the real data flow for credits/billing, checks
whether keeping money on D1 was a reasoned decision or an accident of
sequencing, and gives a concrete recommendation for #8479 (in flight) and a
scoped follow-up issue for live balance sync. Every claim below cites the
file, table, or issue it was checked against.

## 0. The one-paragraph verdict

**Money staying on D1 is a deliberate, heavily-reasoned, still-current
policy — not a scheduling accident.** The repo already built a Postgres
mirror of the money tables *early* in the migration (Wave B of the KS-8
plan, before most product surfaces), specifically to get reconciliation
practice while the stakes were still low-volume. Every time a domain
lead considered flipping decision-critical reads (balance checks,
idempotency, charge/payout decisions) onto that Postgres mirror, they found
or cited a real staleness/consistency risk and explicitly kept those reads
on D1 **forever**, not just "for now." A real production bug (a
Postgres-mirror row silently reverting a `pending`→`ran` status transition
in the Artanis domain, KS-8.6/#8335) is the concrete evidence the team
points to for that caution. So the owner's instinct — "credits should sync
live" — is right, and it is achievable **without** touching where the money
lives. The gap is narrower than the frustration suggests: nobody has
projected a balance-changed signal into Khala Sync yet, but the pattern to
do it (`khala_sync_public_counters`, already shipped for the public
tokens-served counter) is proven, cheap, and doesn't require relitigating
the D1-authority decision at all.

## 1. What exists today, exactly

### 1.1 Where a credit actually lives

Three D1-backed pools, all in the `openagents-autopilot` D1 database, all
still D1-authoritative:

- **Pool A** — `billing_ledger_entries` (USD, Autopilot-oriented; `billing.ts`).
- **Pool B** — `agent_balances` (msat, `CHECK (balance_msat >= 0)`,
  `payments-ledger.ts`). This is the one the mobile app, the coding-run
  metering path, and the Aiur credits console all actually read/write. The
  inference gateway charges it via `apps/openagents.com/workers/api/src/inference/metering-hook.ts`'s
  `makeLedgerMeteringHook`; a parallel cloud-primitive charge path
  (`apps/openagents.com/workers/api/src/cloud/cloud-metering.ts`'s
  `settleCloudPrimitiveCharge`) exists for fine-tuning/sandbox/agent-computer
  compute time and shares the exact same discipline (one atomic `pay_ins`
  adjustment, `idempotency_key UNIQUE`, `CHECK balance_msat >= 0`).
- **Pool C** — `inference_free_allowance` (free-tier tracking,
  `inference-free-allowance.ts`).

The $10 GitHub-signup grant (#8478, closed, shipped `dfd4d848c2`) mints
directly into Pool B via `apps/openagents.com/workers/api/src/inference/github-signup-credit-grant.ts`,
reusing `usd-credit-bridge.ts`'s `usdCreditGrantStatements` primitive, idempotent on
`signup:github:<githubUserId>` plus a UNIQUE `github_user_id` column
(migration `0305`), RL-3-tagged `revenueAsset: 'free'` (promotional, never
Bitcoin-withdrawable). The Aiur credits console (#8500, closed, shipped
`8ff4c9b051`) grants/claws back through a new
`apps/openagents.com/workers/api/src/inference/admin-credit-grant.ts`,
which again reuses `usdCreditGrantStatements` for grants and the existing
`clawbackInferenceCredits` for claws — same ledger, same idempotency
discipline, same D1 tables. **All three surfaces — inference metering,
cloud-compute metering, the signup grant, and the Aiur admin console —
converge on the same `agent_balances` row in D1.** There is one balance,
not several competing sources of truth.

### 1.2 How the mobile app learns about balance changes today: plain REST poll, not sync

`clients/khala-mobile/src/components/credits-balance-chip.tsx` and
`clients/khala-mobile/src/screens/credits-history-screen.tsx` both call
`fetchKhalaMobileCreditsBalance` / `fetchKhalaMobileCreditsTransactions`
(`clients/khala-mobile/src/sync/khala-mobile-credits-api.ts`) from a plain
`useEffect` on mount, with a manual "Load more" button for pagination. There
is no Khala Sync scope subscription anywhere in this code path — it is a
bare `fetch()` with a bearer token, exactly like a REST client from ten
years ago, and it never re-fetches unless the screen remounts.

More importantly: **the server routes these client functions call don't
exist yet.** `khala-mobile-credits-api.ts`'s own header comment says so
explicitly:

> "KNOWN GAP (honest, tracked): neither route exists on the server yet
> ... This module defines the CONTRACT this issue proposes for whoever
> builds those routes ... a 404/unimplemented response degrades to an
> honest 'not yet available' UI state, never a fabricated balance."

Confirmed by grep: `GET /api/mobile/credits/balance` and
`GET /api/mobile/credits/transactions` do not exist anywhere under
`apps/openagents.com/workers/api/src/`. The #8480 closeout comment
(2026-07-06) confirms the same thing from the shipping side: "Neither the
balance nor the transaction-history HTTP endpoint exists yet." So today the
mobile balance chip silently renders nothing, and the history screen shows
"not yet available." The honest starting point is not "polling instead of
syncing" — it's "no live data source at all yet, built against a proposed
polling contract."

### 1.3 What Khala Sync already carries live for this same product

Meanwhile the rest of the mobile app's real-time behavior already goes
through Khala Sync (`packages/khala-sync`, `packages/khala-sync-server`, Cloud
SQL Postgres via Hyperdrive): `clients/khala-mobile/src/sync/khala-mobile-sync-runtime.ts`
subscribes to `scope.user.<ownerUserId>` (personal/thread-list scope) and
`scope.thread.<threadId>` per open thread. Chat messages, runtime turns, and
— critically — **runtime events already sync live into `scope.thread.<id>`**
via the `runtime.recordEvent` mutator (`docs/khala-sync/MUTATORS.md` line 84):
"`KhalaRuntimeEvent` → `khala_sync_runtime_events` row + full `runtime_event`
post-image **only** in `scope.thread.<threadId>`." Event kinds already
flowing through this path include `turn.started`, `text.delta`, `tool.call`,
`usage.recorded`, and `turn.finished` (`apps/pylon/src/orchestration/runtime-intent-enforcement.ts`).
So the live-push machinery the owner is asking for already exists and is
already proven in production for this exact app — it just has never been
pointed at the credit ledger.

## 2. Why it ended up this way: deliberate policy, not an accident — with one real nuance

The owner's phrasing implies money/auth were simply "scheduled last" and
never revisited. That's half right and half wrong, and the wrong half
matters:

### 2.1 What's true: auth genuinely does go last, by explicit blast-radius policy

`docs/khala-sync/MIGRATION_PLAN.md` §5 ("Waves and rationale") lays out five
waves. Wave E is explicit: *"Auth goes last by blast-radius policy, after ~14
proven cutovers"* (line ~2518). KS-8.18 (identity/auth) is literally titled
"last" in the plan (§3.15, line 2382: *"a bad cutover breaks literally
everything, which is why it goes **last**"*). That part of the owner's
intuition — "it's just sequencing" — is correct for auth.

### 2.2 What's NOT true: money was not "left for later" — it was migrated early, specifically because of its correctness stakes, and its decision-reads were deliberately kept on D1 forever

Money is **Wave B**, the *second* wave overall (`MIGRATION_PLAN.md` §5),
running *before* forum, CRM, Sites, and every other product surface. The
plan's own rationale for that ordering: *"Low volume, highest correctness
stakes, freshest reconciliation muscle"* — i.e., money was moved early
**on purpose**, to build and prove the exact-reconciliation tooling (row
hashes, per-currency/per-rail sums, idempotency-key set equality) while the
volume was still small, specifically so later domains could reuse that
tooling with confidence.

What happened after the mirror landed (KS-8.7 billing/#8318+#8337, KS-8.8
treasury/payouts/tips/#8319, KS-8.9 entitlements/#8320+#8336) is the part
that answers the owner's question directly:

- **The mirror is real and live.** `agent_balances`, `pay_ins`,
  `billing_ledger_entries`, and 40+ other money tables are already
  dual-written, fail-soft, into the same Cloud SQL Postgres database Khala
  Sync uses — confirmed production reconciliation output, e.g. #8337's
  closeout: `billing_ledger_entries: rows: d1=2264 postgres=2264 ... PER-USER
  BALANCE (SUM(amount_cents) to the cent): exact match`, and #8319's
  closeout for the treasury/`agent_balances` domain (migration
  `0016_treasury_domain.sql`, verified "exact counts, per-state/rail money
  sums, and newest-N hashes match"). This mirroring happens through
  `runLedgerStatements` in `apps/openagents.com/workers/api/src/payments-ledger.ts`
  (line ~600): every ledger batch commits to D1 first, then (a) an optional
  KS-8.7 billing mirror for the `pay_ins`/`pay_in_legs` rows, and (b) an
  always-on KS-8.8 `mirrorTreasuryRows` call for any row annotated
  `mirror: balanceMirror(partyRef)` — which is exactly how `agent_balances`
  itself gets copied.
- **But every read that DECIDES something — stays on D1, permanently, by
  explicit finding, not by omission.** #8337's second closeout comment names
  this precisely: *"Charge-decision isolation holds as documented.
  `chargeAutoTopUp` reads the authoritative D1 balance via
  `readBillingBalanceCents` with no routed runtime... The new
  `recentEntriesRead`/`autoTopUpStateRead` hooks are reachable only from the
  display path."* Only **display-only** reads (recent-ledger-entries
  projection, receipt display, auto-top-up display state) were ever routed
  toward Postgres, behind a named, tested allowlist
  (`BILLING_DOMAIN_POSTGRES_SERVED_READ_TABLES`); balance-gating, idempotency,
  and charge-decision reads were never moved and the closeout says so
  explicitly.
- **The reason isn't hypothetical — the team found a real bug and used it as
  the standing justification.** KS-8.6 (#8335, Artanis domain — lower stakes
  than money, used as the canary) found the Postgres mirror silently
  reverting rows from `ran` back to `pending`-looking state 20-40 minutes
  *after* the fix was supposedly deployed, across multiple verification
  passes. The closeout's own words: *"Per the 'money/business-adjacent
  responder data — if you find ANY real discrepancy, stop and report
  clearly' guardrail: I found one, so this pass does **not** flip
  `KHALA_SYNC_ARTANIS_READS=postgres`."* That single finding is cited
  directly by name in the entitlements pass (#8336) as the reason five of six
  candidate "safe" reads were re-classified as permanently D1-only rather
  than flipped: *"a lagging Postgres read could allow a double provider call
  or double charge"* — i.e., the exact failure mode a msat balance-gate read
  cannot tolerate.
- **The discipline is explicit and repeated: "money discipline: never in the
  same change as a flag flip."** That phrase is the literal scope-boundary
  line on #8337 itself. Every money-adjacent cutover issue in this repo
  (#8335, #8336, #8337, #8362) follows the same shape: land dual-write mirror
  → backfill → verify with exact hashes → soak in compare mode → flip ONLY
  display-only, non-decision reads → leave decision reads on D1 "permanently"
  (their word, repeatedly, not "pending" or "TODO").

**Conclusion:** this is not "we haven't gotten around to it." It is a
considered, re-confirmed-multiple-times, evidence-backed policy: D1 is kept
as the money-decision authority because Khala Sync's mirror is an
async, eventually-consistent, fail-soft copy (by design — the mirror write
never blocks or fails the D1 transaction), and that consistency model is
provably insufficient for balance checks, idempotent charge decisions, and
double-spend prevention, which need the immediate read-your-own-write
guarantee D1 (Cloudflare's transactional SQLite-in-a-DO) gives for free.
Destructive D1 *retirement* (dropping the D1 tables entirely) is a separate,
later, explicitly-deferred step (KS-8.19/#8330, post-MVP per the mobile
launch audit §8) — but that's about archiving old tables, not about where
decisions get made, which stays D1 by design.

## 3. What "credits should sync to your device" actually requires

There are two different things the owner's sentence could mean, and they
have very different costs:

**(a) Authority stays where it is; balance CHANGES get projected into Khala
Sync as a live-synced entity.** The ledger write (charge, grant, clawback)
still happens exactly as it does today — one atomic D1 batch, same
`CHECK`/`UNIQUE` guarantees, same idempotency keys. After that D1 write
commits, a small best-effort step increments (or repairs) a `scope.user.<id>`
entity in Postgres and appends its post-image to the changelog, exactly the
way `packages/khala-sync-server/src/public-counter-projection.ts` already
does for the public tokens-served counter: exact-once per source
idempotency key (`khala_sync_counter_applied`, `ON CONFLICT DO NOTHING`),
fail-soft (a failed projection never fails or blocks the real charge),
UPDATE-only with an explicit backfill/repair step so a fresh deploy can never
serve a fabricated partial total, and an audited repair path
(`khala_sync_public_counter_repairs`) for when reconciliation finds drift.
The mobile client's existing `scope.user.<id>` subscription (already
wired in `khala-mobile-sync-runtime.ts` for the thread list) would just
gain one more entity type to read.

> **Scope-taxonomy caveat (#8557):** a handful of legacy `email:`-form user
> IDs contain an `@` (and sometimes `+`), which is outside the `SyncScope`
> entity-id charset — so they can never form a valid `scope.user.<id>` on
> either the server or the mobile client (the same schema runs on both), and
> are structurally outside the sync scope taxonomy. The credit-balance
> producer and backfill pre-check with `isScopeCompatibleUserId` and treat
> these as a distinct *skip* (`scope_incompatible_user_id` /
> `skippedIncompatibleCount`), never a failure. Broadening the charset is
> explicitly not the fix (it widens the protocol surface for zero mobile
> benefit); identity migration to a `github:`/`user_` form is the only path
> to sync these accounts, tracked separately.

**(b) Ledger write authority itself moves onto Postgres/Khala Sync's own
mutator/transaction model** — i.e., a coding turn's charge becomes a Khala
Sync mutator instead of a D1 batch.

**Recommendation: (a), unambiguously, and not close.** Reasons:

1. **The correctness policy above is current, re-confirmed, and not this
   feature's to relitigate.** Four separate cutover passes (KS-8.6, 8.7, 8.9,
   8.18), each done independently and each finding or citing the same class
   of risk, arrived at the same conclusion: decision-critical money/auth
   reads stay D1. Moving the *write* authority for the coding-run credit
   ledger onto Khala Sync's mutator engine would be a strictly bigger,
   riskier version of the exact move this team has repeatedly examined and
   declined for *reads* — the recorded reasoning applies with even more
   force to writes, which carry the `CHECK balance_msat >= 0` and
   `idempotency_key UNIQUE` invariants that a whole different transaction
   engine (Postgres, via a Durable-Object-hubbed, rebase-tolerant mutator
   model designed for optimistic client writes) would have to re-prove from
   scratch, under real load, before it could be trusted with real money.
2. **The RL-3 asset boundary is enforced by a single, D1-integrated guard**
   (`workers/api/src/asset-bitcoin-boundary.ts`'s `validateAssetBoundary`),
   wired directly into `payments-ledger.ts`'s statement builders. Re-deriving
   that boundary correctly inside Khala Sync's Postgres-transaction mutator
   model (which uses a different execution shape — client-optimistic,
   server-authoritative-mutator, rebase-on-conflict) is real, non-trivial
   work with no product upside for the mobile MVP timeline.
3. **Projection is materially cheaper and the pattern already shipped.** The
   public-counter projection is ~250 lines, has a test file, and has been in
   production. Adapting it from a single global counter
   (`scope.public.tokens-served`) to a per-user balance
   (`scope.user.<userId>`) is a template-following change, not new design
   work.
4. **It doesn't block on, or interfere with, the eventual full D1
   decommission (KS-8.19/#8330).** Whenever/if that migration reaches the
   money domain's read-cutover for real, the projection this recommends
   keeps working unchanged — it reads whatever is authoritative at the time
   through the same D1-writes-first path; nothing about it assumes D1 stays
   authoritative forever, it just doesn't require deciding that today.

**What the projection needs to be honest**, following the public-counter
precedent exactly: an explicit backfill (seed `scope.user.<id>`'s
`credit_balance` entity to the current exact D1 `agent_balances` value
before any client relies on it — never let a fresh/unbackfilled row read as
zero), an idempotency key reused from the *same* key each ledger write
already carries (`inference:payin:<requestId>`,
`signup:github:<githubUserId>`, the admin-grant caller-supplied ref, etc. —
never invented fresh), fail-soft application (a lost increment is a
reconcile-job finding, never a blocked charge), and an explicit,
audited repair path for drift — not a silent overwrite.

## 4. What Codex should do right now for #8479

#8479 (coding-run metering + balance gate) is mid-implementation and, per
the parent task, was about to represent an `insufficient_credit` paywall
moment as a Khala Sync **runtime event** while the actual balance debit
stays in the D1 `payments-ledger`. **This is sound. Confirm and proceed —
do not change the debit path, and do not wait for the follow-up projection
issue below.**

Concretely:

- **The debit/charge stays exactly where the existing pattern already puts
  it**: `settleCloudPrimitiveCharge`
  (`apps/openagents.com/workers/api/src/cloud/cloud-metering.ts`) for
  agent-computer compute time, using the identical atomic-D1,
  idempotent-per-`chargeId`, `CHECK`-guarded discipline the inference
  gateway already uses (`makeLedgerMeteringHook`,
  `apps/openagents.com/workers/api/src/inference/metering-hook.ts`). Do not
  invent a second charge path and do not attempt to make this write go
  through Khala Sync's mutator engine — see §3 above.
- **The admission-time refusal is already correct and needs no change**:
  `decideCloudCodingAdmission`
  (`apps/openagents.com/workers/api/src/cloud/cloud-coding-session-routes.ts`,
  line ~332) already returns a typed `insufficient_credit` /
  `rate_limited` / `org_capacity_unavailable` refusal reading
  `availableBalanceMsat` — a D1 read, correctly, per the same
  decision-read-stays-D1 policy in §2.
- **The mid-run / paywall-moment signal that needs to reach the mobile
  client live is exactly a job for the existing `runtime.recordEvent`
  mutator** (`docs/khala-sync/MUTATORS.md` line 84), which already writes a
  `runtime_event` post-image into `scope.thread.<threadId>` — the same
  live channel `usage.recorded` and `turn.finished` already use
  (`apps/pylon/src/orchestration/runtime-intent-enforcement.ts`). Representing
  the insufficient-credit moment (and the mid-run exhaustion policy #8479
  is scoped to decide/document) as a new runtime-event kind through this
  same mutator is the "authority stays put, projection syncs" pattern
  applied at the smallest, already-proven scope: it needs no new
  infrastructure, no new scope type, and no policy exception. **This is
  exactly what Codex was about to do, and it's the right call.**
- **One thing to flag, not block on**: `runtime.recordEvent` only writes into
  `scope.thread.<threadId>` — it is thread-scoped and private. That is
  sufficient for #8479's own acceptance bar (the composer/thread screen
  sees "insufficient_credit" the moment it happens, mid-turn). It is **not**
  sufficient for the standing balance chip or Settings balance to update
  live outside of an active thread — that needs the `scope.user.<id>`
  projection from §3, which is explicitly out of #8479's scope and should
  not be pulled in to avoid scope creep on an already-large issue.

## 5. Recommended follow-up issue (sketch only — not filed here)

**Title (suggested):** "Project credit balance changes into Khala Sync for
live mobile updates"

**Scope:**

- New Postgres entity/table following the `khala_sync_public_counters`
  shape exactly, but per-user: e.g. `khala_sync_user_credit_balances`
  (`user_id` key instead of a global `counter_id`), with the same guard
  table pattern (`khala_sync_counter_applied`-style exact-once idempotency)
  and the same repair/audit table.
- A `credit_balance` entity type projected into `scope.user.<userId>`
  (reuse the existing scope the mobile app already subscribes to for the
  thread list — no new scope taxonomy needed).
- Wire the increment call, fail-soft and best-effort, at the small, known
  set of existing D1 ledger write sites: `makeLedgerMeteringHook`
  (inference charges), `settleCloudPrimitiveCharge` (agent-computer/cloud
  charges, landing with or after #8479), `github-signup-credit-grant.ts`
  (the $10 grant), and `admin-credit-grant.ts` (Aiur grants/clawbacks). Each
  already has a stable per-write idempotency key to reuse — no new key
  scheme needed.
- An explicit backfill step (seed every existing user's row to their exact
  current `agent_balances.balance_msat`, converted to USD cents) before any
  client can read the entity — same "refuse until backfilled" discipline as
  the public counter, so a fresh rollout can't show a fabricated zero.
- Mobile-side: replace the polling `fetchKhalaMobileCreditsBalance` call in
  `credits-balance-chip.tsx` with a read against the synced entity in the
  already-open `scope.user.<id>` subscription; **keep transaction history
  (`credits-history-screen.tsx`) as paginated REST against D1** — Khala Sync
  is built for "current state of a scope" and append-only event streams
  within a scope, not arbitrary offset/cursor-paginated historical lists
  across a user's whole lifetime, so forcing history through the sync
  protocol would be fighting the tool. Only the live *number* needs to sync;
  the history *list* is fine as a pull.
- Also finally build the two REST routes the mobile client already expects
  (`GET /api/mobile/credits/balance`, `GET /api/mobile/credits/transactions`)
  as the bootstrap/cold-start/no-network-fallback path — even with live sync,
  the app needs a value on first paint before the sync connection warms up,
  and a way to view balance/history without an open sync session.
- **Explicit non-goals**: does not move ledger write or read-decision
  authority off D1; does not touch `agent_balances`, `payments-ledger.ts`,
  or the RL-3 boundary guard; does not depend on or block KS-8.19/#8330 (the
  full D1 decommission). This is additive plumbing on top of the existing
  authority, following the exact precedent already proven for the public
  counter.
- **Depends on**: #8479 landing the compute-charge path (so there is a
  single, stable set of write call sites to hook), and #8478 already landed
  (the signup grant, already the case).

This keeps the money-correctness policy untouched while giving the owner
what they actually want: the balance visibly ticking down the moment a turn
charges it, without a manual refresh.

## 6. Summary answers to the five questions this audit was asked

1. **What exists**: money lives in D1 (`agent_balances`/Pool B primarily,
   plus Pools A and C), already dual-write-mirrored (fail-soft, best-effort)
   into the same Cloud SQL Postgres database Khala Sync runs on, but *not*
   through the Khala Sync changelog/scope/mutator protocol — so the mirror
   copy exists physically but produces no live push to any client. The
   mobile balance/history UI is a plain `useEffect` REST poll against
   `/api/mobile/credits/*` routes that do not exist on the server yet
   (confirmed by grep and by the #8480 closeout comment itself).
2. **Why**: deliberate, re-confirmed policy for money-decision reads (stay
   D1 forever, evidenced by a real found bug in a lower-stakes domain used
   as the standing justification), combined with a real (and separate)
   blast-radius sequencing decision that only auth, not money, goes fully
   last. Money's migration mirror actually landed *early* (Wave B) on
   purpose, to build reconciliation muscle while stakes were low.
3. **What syncing would require**: keep ledger authority exactly where it
   is; add a `khala_sync_public_counters`-style per-user projection into
   `scope.user.<id>` after each D1 write, reusing existing idempotency keys.
   Do not move ledger authority onto Postgres/Khala Sync's mutator model —
   not worth the risk or the rework for this timeline, and it's the same
   move the team has independently declined four separate times for reads
   alone.
4. **#8479 right now**: the plan already in motion — debit stays in D1,
   insufficient-credit/mid-run-exhaustion moments post as a `runtime_event`
   through the already-built, already-tested `runtime.recordEvent` mutator
   into `scope.thread.<threadId>` — is sound. Proceed with it unchanged.
5. **Follow-up issue**: yes, recommended, scoped narrowly to a per-user
   balance projection (§5 above), separate from #8479 and from the full
   D1-decommission epic (#8330/KS-8.19).
