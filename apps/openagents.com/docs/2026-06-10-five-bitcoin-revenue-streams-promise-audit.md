# Five Bitcoin Revenue Streams Promise Audit

Date: 2026-06-10 (revised same day after Episode 213/214/215 transcript review)

Promise: `pylon.five_bitcoin_revenue_streams.v1`

Registry version at audit time: `2026-06-10.1`; this audit shipped the
`2026-06-10.2` labor-stream correction and the `2026-06-10.3` no-resale
promise consolidation described below, and now carries the full
implementation issue plan.

Status: full status audit of the promise and an exact get-to-green plan.

## Correction From The First Revision

The first revision of this audit (and the registry claim text through
`2026-06-10.1`) mischaracterized the fifth stream as "subscription/token-
capacity arbitrage" — i.e. reselling provider account access. That is wrong.

Per Episodes 213–215, the fifth stream is the **agent labor market**: the
contributor *uses* their own capacity (their Claude Code / Codex subscription,
their idle agent, their hardware) to do jobs and **sells the result**.
Episode 214: "If your Claude Code or Codex is just sitting idle overnight,
you're not using it, why don't you sell access to it? We want this 'go online'
button to be able to let you sell agent compute while you're sleeping."
Episode 215: "Labor next week. You can put your Claude Code or Codex to work
overnight."

This distinction matters operationally: selling work output is ordinary labor
and removes the provider-ToS resale gate this audit previously treated as the
long pole. There is no resale anywhere in the plan: OpenAgents does not
resell, rent, proxy, or broker anyone's subscription seat, provider account,
or API access — it pays contributors for accepted work output produced with
their own compliant provider usage. Registry `2026-06-10.2` renamed the
blocker `capacity_stream_not_live` → `labor_stream_not_live` and fixed the
claim text; registry `2026-06-10.3` removed the resale-framed
`provider.subscription_capacity.v1` and
`provider.prepaid_capacity_monetization.v1` promises and replaced both with a
single `provider.compliant_usage_labor.v1` promise that carries the no-resale
boundary explicitly.

The first revision also estimated "weeks-scale" for several streams. That
ignored the fact that the market rails already exist in this repo's own
history (see below). Corrected estimate: **a couple of days for everything.**

## The Promise

> Pylon stacks compute, data, Forum tips, referrals, and agent labor markets
> in one install.

Current registry record (as corrected in `2026-06-10.2`):

- state: `red`
- blockerRefs:
  - `blocker.product_promises.compute_stream_not_broadly_live`
  - `blocker.product_promises.data_stream_not_live`
  - `blocker.product_promises.referral_stream_not_live`
  - `blocker.product_promises.labor_stream_not_live`
- lastVerifiedAt: none (no transition receipt has ever been recorded for this
  promise).

The blocker set has four entries for five streams: Forum tips is the only
stream considered live enough that it does not block this promise.

## Source Material: Episodes 213–215

The canonical product framing for the streams is the March 2026 agent-markets
launch series, not just the June 8 transcript:

- `docs/transcripts/213.md` — Agent Markets (2026-03-07): five interlocking
  markets launching one per week — **compute, data, labor, liquidity, risk** —
  with Bitcoin rev-share, the economy kernel enforcing rules across all five,
  and Autopilot as the entry point. Open protocols explicitly invited outside
  Nostr/Bitcoin developers into the same liquidity pool.
- `docs/transcripts/214.md` — Compute market launch at the Austin Startup
  Rodeo (2026-03-11): live NIP-90 kind 5050 text inference jobs on Apple FM,
  Spark Lightning wallet + Nostr keypair from one BIP-39 seed, buy mode
  dispatching 2-sat jobs, real sats paid to attendees. Also names the draft
  NIPs: Skills (SKL), Sovereign Agents (SA), Agent Credit (AC).
- `docs/transcripts/215.md` — Data market launch: draft **NIP-DS** open spec
  for dataset sales (kind 30404 dataset listing, kind 30406 offer, optional
  NIP-90 delivery flow), conversational dataset listing through the agent,
  with labor named as the next launch.

The June 8 transcript reframed the set for the Pylon install as compute,
data, Forum tips, referrals, and the fifth stream now correctly read as the
labor market.

## The Critical Fact: The NIP-90 Rails Are Already Built

All three machine markets (compute, data, labor) ride the same NIP-90 data
vending machine rails, and those rails were implemented in this repository
and removed only two days ago in the Bun/Effect rebuild (commit
`f5919c766`, 2026-06-09). In git history:

- `crates/nostr/core/src/nip90/` — builders, data_vending, kinds, model,
  integration tests (the protocol library).
- `crates/nostr/nips/DS.md` (864 lines), plus `SKL.md`, `SA.md`, `AC.md`,
  `TRN.md` — the draft open specs from the launch series.
- `apps/autopilot-desktop/src/provider_nip90_lane.rs`,
  `nip90_compute_flow.rs`, `nip90_compute_semantics.rs`,
  `nip90_compute_domain_events.rs`, buyer payment attempt/fact state, and
  sent-payments UI panes — the full provider/buyer compute market.
- `data_seller_control.rs` + the Data Market NIP-90 profile (commit
  `1fc3d3e01`) — the data market lane.
- `apps/deprecated/pylon/src/nip90_runtime.rs` — the Pylon-side NIP-90
  runtime, including provider admission enforcement (`22a162b11`) and the
  Artanis MDK settlement bridge carrying authority through NIP-90 settlement
  runs (`f574f123d`, 2026-06-07 — two days before removal, i.e. this was
  current, working code, not bitrot).

The current workspace also already contains `apps/nostr-relay` — the scoped
OpenAgents Nostr relay POC (Durable Object relay from `nostr-effect`, NIP-11
handshake live, issue #4621). So the porting story is: resurrect proven
NIP-90 contracts from our own history into the Bun/Effect workspace, pointed
at our own relay, settled by the same MDK bridge that already has 2,323 sats
of public settlement receipts. That is a port, not a build.

## Live Evidence Snapshot (2026-06-10)

Checked live during this audit:

- `GET /api/public/product-promises` → version `2026-06-10.1`, promise `red`.
- `GET /api/public/pylon-stats`:
  - `pylonsOnlineNow: 0`, `pylonsWalletReadyNow: 0`,
    `sellablePylonsOnlineNow: 0`
  - `pylonsSeen24h: 4`, `pylonsRegisteredTotal: 7`, with `0.3.0` and
    `0.3.0-rc1` clients registering
  - `nexusAcceptedWorkPayoutSatsPaidTotal: 2323` with 8 public settlement
    receipt refs (including both paid GEPA multi-Pylon settlements)
- `GET /api/forum/launch-status` → `status: ready`, `orangeChecksSold: 2`
- `GET /api/forum/tip-leaderboards` → creators with nonzero settled sats

## Stream-By-Stream Status

### Stream 1: Compute — partially live; previously fully live

Blocker: `compute_stream_not_broadly_live`.

Live today: 2,323 sats of receipted accepted-work payouts; full assignment
lifecycle routes proven end-to-end on production (no-spend loop, #4633);
v0.3 clients heartbeating. Previously live (March): the NIP-90 kind 5050
compute market paid real sats to real attendees' Apple Silicon machines.

To clear: pass the v0.3 live GEPA endpoint smoke, settle one paid v0.3
assignment, keep at least one Pylon continuously online, and/or port the
NIP-90 compute provider lane from history so GO ONLINE sells inference again.
Days, not weeks — both halves (NIP-90 lane, MDK settlement) have already
worked in production.

### Stream 2: Data — spec and lane exist in history

Blocker: `data_stream_not_live`. Dependent promise
`pylon.data_trace_revenue.v1` is red with `settled_trace_sale_missing`.

The first revision called this "weeks-scale, full build." Wrong: NIP-DS is a
written 864-line spec, the data market lane shipped in the desktop app
(Episode 215 demoed listing a dataset for 50 sats conversationally), and the
data-market NIP-90 profile exists (`1fc3d3e01`). What was never completed is
one *settled* public-safe sale with redaction evidence. Port the NIP-DS
listing/offer flow, run one redacted Claude Code/Codex conversation-bundle
sale for small sats, record the receipt. Days.

### Stream 3: Forum tips — live (already non-blocking)

No blocker on this promise. `forum.content_tipping.v1` is yellow with
`lastVerifiedAt: 2026-06-10T02:44:34Z`; strict funded smooth-path BOLT 12
smokes passed 2026-06-09 against two independent live recipients with
creator-spendable settlement. Orange check rail adjacent and live (2 sold).

### Stream 4: Referrals — simple; days

Blocker: `referral_stream_not_live`. Dependent promise
`sites.referral_bitcoin_stream.v1` is yellow.

Attribution capture is live. What is missing is consumption + a payout
ledger + one settled small-sats referral payout receipt through the
already-proven MDK settlement bridge. There is no research here; it is one
table, one policy doc, and one smoke. Days (the first revision's "week-scale"
was padding).

### Stream 5: Agent labor market — NIP-90 jobs; not capacity resale

Blocker: `labor_stream_not_live` (renamed from `capacity_stream_not_live` in
`2026-06-10.2`).

The stream: a contributor's idle agent (Claude Code, Codex, or any runtime
behind Pylon) accepts NIP-90 job requests — "fix this PR", review work,
inference beyond kind 5050 — does the work using the contributor's own
capacity, and gets paid sats for the *result*. Episode 214 frames the
economics: subscription holders sit on heavily subsidized capacity; putting
that agent to work overnight sells output. This is labor, not account
resale: the contributor's own account stays in the contributor's own custody
under the contributor's own provider terms, and nothing about provider
access is transferred, metered for resale, or brokered. The
`provider.compliant_usage_labor.v1` promise carries that boundary.

What exists: the NIP-90 runtime, provider admission, job lifecycle, and MDK
settlement bridge in repo history; Pylon v0.3's runtime package already
carries provider-neutral LLM/tool contracts and the OpenCode/Codex
diagnostic path; the assignment lifecycle on openagents.com already proves
accept→work→closeout→settle.

To clear: port the NIP-90 job intake into the v0.3 worker loop (or run the
labor job through the existing openagents.com assignment rail as the v1),
execute one real job on a contributor's own agent capacity, settle sats,
record the public receipt. Days.

### Liquidity and risk (Episodes 213/214 streams six and seven)

The March framing also named liquidity (Lightning yield) and risk
(verification derivatives per Catalini's "Some Simple Economics of AGI")
markets. These are explicitly speculative in the transcripts, are not part
of this promise's five streams as registered, and are not gated here. They
should get their own promise records if/when they become claims.

## Exact Path To Green — Couple Of Days Total

All four blockers are port-and-smoke work against rails that have already
moved real bitcoin:

1. **Labor** (~1 day): NIP-90 job intake into the v0.3 loop or the existing
   assignment rail; one real overnight-agent job; settled receipt → clear
   `labor_stream_not_live`.
2. **Compute** (~1 day, overlaps with labor since it is the kind 5050 case
   of the same rail): live endpoint smoke + one paid v0.3 settlement +
   nonzero online counters → clear `compute_stream_not_broadly_live`.
3. **Data** (~1 day): port NIP-DS listing/offer; one redacted conversation
   bundle sold and settled → clear `data_stream_not_live`.
4. **Referrals** (~half day): attribution consumption + payout ledger + one
   settled referral payout receipt → clear `referral_stream_not_live`.
5. **Stacking smoke** (~hours, after any two streams): one install, one
   identity, receipts from two streams in one session window. Add an
   explicit `one_install_stacking_smoke_missing` blocker at the red→yellow
   flip so the composition claim stays mechanically gated until this passes.

Record each transition via
`POST /api/operator/product-promises/transitions` **before** shipping the
registry edit so receipts evaluate cleanly (not as backfill exceptions) and
`lastVerifiedAt` finally populates for this promise.

## Registry Changes Shipped With This Audit

### `2026-06-10.2`

- Claim corrected: "...and agent labor markets in one install" (was
  "subscription/token-capacity arbitrage").
- Blocker renamed: `capacity_stream_not_live` → `labor_stream_not_live`.
- safeCopy now records that the NIP-90 compute/data/labor rails shipped in
  Episodes 213–215 and exist in repo history.
- unsafeCopy now forbids describing the labor stream as provider-capacity
  resale.
- evidenceRefs now include `docs/transcripts/213.md`, `214.md`, `215.md`,
  and this audit.
- State stays `red` (no state transition; wording/blocker correction only).

### `2026-06-10.3` — no-resale consolidation

- Removed `provider.subscription_capacity.v1` and
  `provider.prepaid_capacity_monetization.v1`. Both were framed around
  capacity metering/marketplace resale, which OpenAgents is not doing and
  will not do.
- Added `provider.compliant_usage_labor.v1` (red, blocked on
  `labor_stream_not_live`): contributors connect their own provider accounts
  or prepaid API budgets and earn Bitcoin for accepted work output produced
  with that compliant usage. Its unsafeCopy and authorityBoundary state the
  no-resale boundary explicitly: no subscription seat, provider account,
  session, or API access is ever resold, rented, proxied, metered for
  resale, or brokered.
- Public launch dashboard: the two historical transcript capacity rows keep
  their transcript claims but now project the no-resale labor framing in
  safeCopy/unsafeCopy, share the
  `blocker.launch_dashboard.compliant_usage_labor.paid_labor_jobs_missing`
  blocker, and cite this audit; the five-streams row text now reads "agent
  labor markets".
- Registry notes now state the no-resale boundary as a standing rule.

## Implementation Plan: GitHub Issues

Every step from the current codebase (Bun/Effect monorepo: `apps/openagents.com`
worker + web, `apps/pylon` v0.3 OpenTUI, `apps/nostr-relay` POC,
`apps/forum`; NIP-90 Rust stack in git history at `f5919c766^`) to all five
streams implemented. **Filed on GitHub 2026-06-10 as #4635–#4653**, one per
plan step in order (plan 1 = #4635 … plan 19 = #4653); each filed body is
self-contained and inlines the Delegation Contract below. Sequencing: rails
(1–6 / #4635–#4640) unblock everything; streams (7–17 / #4641–#4651) can
then run in parallel; 18 (#4652) composes; 19 (#4653) is optional polish.

### Delegation Contract (binding for every issue below)

This plan was drafted by Fable (registered agent
`fable-promise-auditor`) and will be executed by delegated coding agents who
were not present for the wave-1 campaign. The working conventions are the
ones already adopted on the Forum — topic `promise-flip-campaign-conventions`
in the product-promises forum, plus the wave-1 priorities/wrap-up topics —
and the repo-specific expectations below. **When these issues are filed,
every issue body must link this section.** Wave 1 closed fourteen issues in
one day because each issue carried explicit surfaces, acceptance criteria,
and authority boundaries; ambiguity is what makes delegation fail.

**Process conventions (from the Forum conventions topic, adopted):**

1. Claim before you build: post in (or create) a `Working: <promiseId>`
   topic in the product-promises forum naming the issue, the blockerRef you
   are taking, and your approach in 2–3 sentences. Yield if someone is
   visibly further along.
2. Evidence over narrative: progress posts and issue comments carry commit
   SHAs on `main`, route names, smoke names with pass/fail, receipt refs,
   and the registry version — not prose about intent.
3. Done means the promise `verification` field (or the issue's acceptance
   list) passes **from integrated state**: merged to `main`, pushed, and
   deployed where the issue says deployment is in scope. Branch work is
   in-progress evidence, never completion. Close the issue only after that,
   with a closing comment containing the evidence refs.
4. Nobody flips their own promise. Propose the transition; the
   operator/maintainer flips it. Record transitions via
   `POST /api/operator/product-promises/transitions` (admin token) **before**
   shipping the registry edit, or the receipt evaluates as a backfill
   exception.
5. Lane discipline: each issue is tagged Lane A (agent can finish alone),
   Lane B (a human operator must fund or execute the live step — say
   explicitly which part is yours and which waits on the operator), or
   Lane C (a human decision is required first — deliver a draft proposal,
   not code). If your work suddenly needs spend, deploy-to-prod judgment,
   settlement, moderation, or provider-account authority you were not
   granted, that is a lane signal: stop and flag it on the issue.
6. File-surface ownership: the lane map table below names each issue's
   primary surfaces. Do not edit another issue's surfaces without
   coordinating in the Working topic first; this is what allows the issues
   to run in parallel without collisions.

**Owner pre-approvals and operating posture (recorded 2026-06-10):**

- The maintainer **pre-approved** the two Lane C policy gates: the
  compliant-usage labor policy as scoped in #4646 (and worded in
  `provider.compliant_usage_labor.v1`), and the referral payout policy
  direction in #4650. Approval comments are on those issues. Implementing
  agents finalize the docs and concrete parameters with their own judgment
  and proceed — solicit the owner's input after the fact, do not block on
  sign-off. Dependents (#4647, #4648, #4651) are unblocked on these gates.
- General posture is **default-yes**: agents make their own decisions and
  request review post-hoc. Escalate before acting only for (a) steps that
  require the owner's physical participation — wallet funding, production
  spend enablement, account approvals — and (b) material deviations from a
  stated policy boundary. The no-resale rule is never waivable, by anyone,
  under any autonomy grant.

**Repo-specific expectations (things you cannot guess; learned the hard way):**

- **Tests:** run `bunx vitest run <files>` from
  `apps/openagents.com/workers/api`. Do NOT use `bun test` — it cannot load
  `cloudflare:workers` imports and reports ~70 false failures. Web tests run
  from `apps/openagents.com/apps/web`.
- **Architecture gates (zero-debt):** the deploy gate enforces a budget on
  Worker `Response` return-type surfaces — new HTTP code should use a
  `type HttpResponse = globalThis.Response` alias instead of adding `Response`
  annotations. No `try/catch` inside `Effect.gen` (custom TS error); HTTP
  route files follow the `-routes.ts` naming convention. Never pipe gate
  output through `tail`/`head` inside `&&` chains — it masks failures and
  has let bad commits through twice.
- **D1 discipline:** multi-statement writes that must be consistent go
  through one `db.batch([...])` call — sequential `.run()` calls are NOT
  transactional and caused a live orphaned-receipt incident (#4634). SQLite
  `CHECK` constraint changes require a full table rebuild migration
  (`PRAGMA defer_foreign_keys`; create-new → insert-select → drop → rename →
  recreate indexes; see migration `0151`).
- **Registry edits:** bump `PublicProductPromisesVersion` in
  `product-promises.ts` AND the version pin in `product-promises.test.ts`;
  check `public-launch-dashboard.ts` for mirrored rows; transition receipt
  first (rule 4).
- **AGENTS.md:** `docs/live/AGENTS.md` and `apps/web/public/AGENTS.md` must
  stay byte-identical, with the sha pin in `openagents-agent-onboarding.ts`
  updated in the same commit.
- **Deploy:** `bun run deploy` from `workers/api` runs the full gate chain
  (checks → remote D1 migrations → web build → wrangler deploy). Deploy only
  when the issue scope says so. Verify live with a cache-busting query param;
  the first read after deploy can serve stale cache.
- **Copy boundaries:** do not change user-facing copy beyond issue scope.
  unsafe-copy scan gates run over live docs and seed copy; the orange-check
  copy boundary (economic participation signal, never identity verification)
  and the no-resale labor boundary (this audit) are load-bearing.
- **Payments evidence:** a wallet-side "pending" record is not a payment.
  Only provider-confirmed settlement (e.g. hosted MDK `payment_received`,
  settled BOLT 12 state) counts. Never post mnemonics, agent tokens, raw
  invoices, payment hashes, preimages, or wallet-home paths into issues,
  Forum posts, commits, or tracked files. (The checkout page showing an
  invoice to the payer is the one deliberate exception.)
- **MDK operational gotchas** (for any issue touching wallets): always set
  `MDK_WALLET_PORT` (CLI restart respawns on 3456 and cross-talks to the
  wrong wallet); BOLT 12 offers are session-bound and must be re-claimed
  after daemon restarts; mnemonic-only restore does NOT restore outbound
  capacity; cold-channel first payments settle but exceed the CLI's 120s
  wait — a fail-then-pass pattern on first attempts is expected and the
  smoke's `failureClassification` tells you which case you hit.
- **NIP-90 source of truth:** the removed Rust implementation is at git rev
  `f5919c766^` — `crates/nostr/core/src/nip90/`, `crates/nostr/nips/*.md`,
  `apps/deprecated/autopilot-deprecated/src/*nip90*`,
  `apps/deprecated/pylon/src/nip90_runtime.rs`. Treat it as the contract
  reference; port behavior and tests, do not blind-rewrite.

**Lane map (plan step → issue → lane → primary file surfaces → depends on):**

| Plan | Issue | Lane | Primary surfaces | Depends on |
|------|-------|------|------------------|-----------|
| 1 | #4635 | A | NEW `packages/nip90` (or nostr-effect contribution) | — |
| 2 | #4636 | A | `apps/nostr-relay/*`, AGENTS.md mirrors | — |
| 3 | #4637 | A | NEW `docs/nips/*`, AGENTS.md mirrors | — |
| 4 | #4638 | A | `apps/pylon/src/*` (provider loop), pylon runtime contracts | #4635, #4636 |
| 5 | #4639 | B (operator enables; caps approval) | NEW worker dispatcher module + operator routes | #4635, #4636 |
| 6 | #4640 | A | NEW worker receipts module, `public-pylon-stats.ts` | #4635 |
| 7 | #4641 | B (funded jobs + contributor machine) | smoke scripts, registry files | #4638, #4639, #4640 |
| 8 | #4642 | B (paid settlement step) | `apps/pylon/src/assignment.ts`, smoke scripts | — |
| 9 | #4643 | A | `packages/nip90` (NIP-DS module), NEW CLI/skill script | #4635, #4637 |
| 10 | #4644 | A | NEW export/redaction script + fixtures | — |
| 11 | #4645 | B (small-sats buy) | smoke scripts, registry files | #4643, #4644, #4639 |
| 12 | #4646 | C → pre-approved 2026-06-10; proceed | NEW policy doc, `packages/nip90` labor schema | #4635 |
| 13 | #4647 | A | `apps/pylon/src/*` (labor intake), runtime contracts | #4646, #4638 |
| 14 | #4648 | B (paid job + acceptance) | smoke scripts, registry files | #4647, #4639, #4640 |
| 15 | #4649 | A | site referral routes/store in worker | — |
| 16 | #4650 | C → pre-approved 2026-06-10; proceed | NEW referral ledger module + policy doc | #4649 |
| 17 | #4651 | B (operator-approved payout) | payout dispatch path, registry files | #4650 |
| 18 | #4652 | B (composition smoke) | registry files, smoke scripts | any two of #4641/#4645/#4648 (+ tips) |
| 19 | #4653 | B | forum tip webhook/refund surfaces | — |

Registry-file edits (issues 7, 11, 14, 17, 18) all touch
`product-promises.ts`; those five issues must serialize their registry
commits (coordinate in Working topics) even though their build work is
parallel. Issue 5's spend caps and every Lane B live step require explicit
operator approval before any sats move.

### Rails

**Issue 1 — `nip90: port the NIP-90 protocol library into the Bun/Effect workspace`**

> The full NIP-90 data vending machine implementation lived in this repo as
> `crates/nostr/core/src/nip90/` (kinds, builders, data_vending, model,
> integration tests) until the Bun rebuild removed it (`f5919c766`).
> Port it as a TypeScript package (e.g. `packages/nip90` or as a contribution
> surface on `nostr-effect`) with: typed job request kinds (5000–5999),
> result kinds (6000–6999), feedback kind 7000 with status tags
> (`payment-required`, `processing`, `success`, `error`); `i`/`output`/
> `relays`/`bid`/`amount` tag builders and parsers; bolt11 amount handling on
> results/feedback; optional encrypted params; Effect Schema validation for
> every event shape. Use the Rust source at `f5919c766^` as the contract
> reference and port its test cases. Acceptance: package builds in the
> workspace, round-trips all event shapes against fixtures derived from the
> Rust tests, and rejects malformed kind/tag combinations. No relay,
> wallet, or execution behavior in this issue.

**Issue 2 — `nostr-relay: promote the POC relay to the scoped market relay`**

> `apps/nostr-relay` is the deploy/handshake POC from #4621 (NIP-11, REQ over
> a SQLite Durable Object). Promote it to the scoped relay for market
> events: explicit allowed-kind policy (NIP-90 job/result/feedback ranges,
> NIP-DS kinds 30404/30406, NIP-89 handler info), retention policy,
> per-pubkey rate limits, REQ filter limits, and a health/metrics route.
> Deploy to a production hostname, add a Live Public Surfaces row to
> `docs/live/AGENTS.md` (and mirror + sha pin), and add a relay smoke
> (publish job event → REQ readback) to CI or a runnable script. Acceptance:
> a NIP-90 kind 5050 request published by a test key is readable by a second
> connection, disallowed kinds are rejected, and the health route reports
> event counts. This relay is event transport only: it grants no payment,
> identity, or moderation authority.

**Issue 3 — `nips: restore the draft NIP specs (DS, SKL, SA, AC, TRN) as living docs`**

> The draft NIPs from the Episode 213–215 launch series (`crates/nostr/nips/`:
> DS.md 864 lines, SKL.md, SA.md, AC.md, TRN.md) were removed in the Bun
> rebuild. Restore them from `f5919c766^` into `docs/nips/` with a README
> index, unchanged content plus a status header (draft, last-shipped-in
> ref), and link them from the public AGENTS.md so outside Nostr/Bitcoin
> developers can implement against the same liquidity pool (the Episode 213
> open-protocol commitment). Acceptance: all five specs render in the repo,
> the README maps spec → market stream, and AGENTS.md links the index.

**Issue 4 — `pylon: NIP-90 provider loop behind GO ONLINE`**

> Port the provider lane (history: `provider_nip90_lane.rs`,
> `nip90_runtime.rs`, provider admission from `22a162b11`) into the v0.3
> OpenTUI app: subscribe to the scoped relay, advertise capability via
> NIP-89 handler info, accept kind 5050 text-inference jobs, execute on the
> local runtime (Apple FM bridge first, runtime-neutral contract), publish
> kind 6050 results with a bolt11 amount from the contributor's MDK agent
> wallet, and mark feedback states. Wallet boundaries: the loop submits only
> redacted readiness refs; mnemonics, raw invoices on the OpenAgents API,
> preimages, and wallet-home paths never leave the machine. Reuse provider
> admission gating before execution. Acceptance: with a funded buyer (Issue
> 5), GO ONLINE on a real machine results in a paid job visible in local
> earnings state and a public-safe receipt ref.

**Issue 5 — `worker: operator-gated buy-mode dispatcher with spend caps`**

> Port buy mode (Episode 214: 2-sat jobs every 12 seconds) as a worker-side
> dispatcher: operator-gated start/stop, per-day and per-job spend caps,
> idempotent job issuance, payment of provider bolt11s through the proven
> MDK settlement bridge, and duplicate-settlement guards (reuse the
> campaign payment-mode ladder patterns). OpenAgents is the first buyer to
> bootstrap sell-side liquidity, exactly as in March. Acceptance: dispatcher
> issues NIP-90 jobs against the scoped relay, pays only on valid results,
> respects caps, halts on cap breach with an operator alert, and every
> payment lands as a receipt (Issue 6). No autonomous spend outside the
> configured caps; operator approval required to enable.

**Issue 6 — `worker: public NIP-90 market receipts and stats projection`**

> Project NIP-90 market settlements into the public receipt surface (same
> discipline as `receipt.nexus_pylon.settlement.*`): receipt ref, settled
> state, amount sats, stream kind (compute/data/labor), and public-safe
> projection only — no invoices, preimages, payment hashes, or
> counterparty wallet detail. Extend `GET /api/public/pylon-stats` (or a
> sibling market-stats route) with per-stream counters (jobs settled 24h /
> total, sats settled per stream). Acceptance: a settled buy-mode job from
> Issue 5 is publicly retrievable by receipt ref, the stats route counts it
> under the right stream, and the copy-scan gates pass.

### Stream: Compute

**Issue 7 — `compute: live paid kind 5050 smoke and clear compute_stream_not_broadly_live`**

> With Issues 1–6 landed: run the provider loop on at least one real
> contributor machine (not operator-only), keep it online with fresh
> heartbeats so `pylonsOnlineNow`/`sellablePylonsOnlineNow` are nonzero,
> dispatch funded small-sats kind 5050 jobs via buy mode, and verify
> settlement receipts publicly. Record the
> `compute_stream_not_broadly_live` blocker clear via a transition receipt
> BEFORE shipping the registry edit. Acceptance: ≥2 settled compute receipts
> to a contributor wallet, nonzero online counters at verification time,
> transition receipt recorded, registry bumped.

**Issue 8 — `pylon v0.3: live OpenAgents GEPA endpoint smoke and one paid settlement`**

> Clears the two `pylon.gepa_worker_loop_v03.v1` blockers
> (`live_openagents_gepa_endpoint_smoke_missing`,
> `paid_gepa_settlement_v03_missing`). Run the v0.3 assignment loop against
> live production endpoints (the #4633 no-spend loop already proved the
> rail), then repeat in paid mode with operator-approved spend cap and a
> settled receipt. This is the assignment-rail complement to Issue 7's
> NIP-90 rail; both feed the compute stream. Acceptance: live endpoint smoke
> green in repo evidence, one settled paid GEPA assignment receipt, both
> blocker clears recorded as transition receipts.

### Stream: Data

**Issue 9 — `data: implement the NIP-DS listing/offer flow (kinds 30404/30406)`**

> Implement NIP-DS (restored spec, Issue 3) on the NIP-90 package: kind
> 30404 dataset listing (canonical digest, size, schema/provenance
> metadata), kind 30406 offer with terms, and delivery via public, DVM, or
> NIP-90 flow. Surface as a `scripts/`-style CLI or agent skill so listing
> is conversational (the Episode 215 demo path). Acceptance: a test dataset
> can be listed, offered, and delivered end-to-end against the scoped relay
> with digests verified on receipt; malformed listings rejected by schema.

**Issue 10 — `data: conversation-bundle export and redaction tool`**

> A script/skill that exports recent Claude Code/Codex conversations from
> the local machine, runs a redaction pass (secrets, tokens, emails, repo
> paths, names — deny-by-default with an allowlist), produces a manifest +
> canonical digest, and refuses to bundle anything matching secret
> patterns. Redaction evidence (what classes were removed, counts) ships in
> the manifest; raw source never leaves the machine unredacted. Acceptance:
> running against a fixture conversation set produces a clean bundle, a
> seeded secret in fixtures causes a hard refusal, and the manifest digest
> matches the NIP-DS listing digest.

**Issue 11 — `data: first settled dataset sale and clear data_stream_not_live`**

> Sell one redacted conversation bundle (Issues 9–10) for small sats:
> listing → offer → payment → entitlement/delivery → public settlement
> receipt (Issue 6 projection). Buyer is OpenAgents buy-side initially
> (Episode 215: "I'll buy it from you"). Clears `data_stream_not_live` on
> the five-streams promise and flips `pylon.data_trace_revenue.v1` red →
> yellow with the receipt as evidence; both via transition receipts before
> registry edits. Acceptance: one public receipt ref for a settled dataset
> sale with redaction evidence in the manifest, registry updated.

### Stream: Labor

**Issue 12 — `labor: define labor job kinds and the compliant-usage policy`**

> Define the labor job contract on NIP-90: job kind(s) for agent work tasks
> (code task, review, document work) with input refs, acceptance criteria,
> bid/amount, and result artifact refs. Ship the compliant-usage policy doc
> stating the boundary that governs the whole stream: contributors run jobs
> on their OWN provider accounts/API budgets under their OWN provider
> terms; OpenAgents pays for accepted work output only; no provider
> credentials, sessions, or account access are ever transferred, metered
> for resale, or brokered; contributors are responsible for their own
> provider-terms compliance and the runtime never exfiltrates provider
> auth material. Acceptance: schema lands in the NIP-90 package with tests;
> policy doc linked from AGENTS.md and from
> `provider.compliant_usage_labor.v1` evidenceRefs.

**Issue 13 — `labor: Pylon labor intake running jobs on the contributor's own agent`**

> Wire labor jobs (Issue 12) into the v0.3 runtime: accept a labor job,
> execute through the contributor's locally configured agent (Codex /
> OpenCode / Claude Code via the existing runtime contracts), apply sandbox
> and approval policy (bounded working dir, no provider-auth exfiltration,
> operator-visible approval for first runs), produce result artifacts, and
> publish the NIP-90 result. The openagents.com assignment rail is an
> acceptable v1 transport if relay-first slips — the contract, not the
> transport, is the deliverable. Acceptance: a real job ("fix this
> fixture's failing test" class) runs end-to-end on a contributor machine
> using the contributor's own agent capacity and returns verifiable output.

**Issue 14 — `labor: first paid overnight labor job and clear labor_stream_not_live`**

> Run one real paid labor job on a contributor's idle agent: job dispatched
> (buy mode or operator), executed overnight-unattended on the
> contributor's own capacity, result accepted against stated criteria,
> sats settled to the contributor wallet, public receipt recorded. Clears
> `labor_stream_not_live` on the five-streams promise and flips
> `provider.compliant_usage_labor.v1` red → yellow; transition receipts
> before registry edits. Acceptance: one settled labor receipt, acceptance
> evidence retained, no provider-auth material anywhere in artifacts or
> receipts.

### Stream: Referrals

**Issue 15 — `referrals: consume attribution at signup/order`**

> Site referral capture (`/r/site/{publicSourceRef}`) records pending
> attribution but nothing consumes it. Add consumption: when a referred
> user completes a qualifying event (signup or paid order), bind the
> pending attribution to that event exactly once (idempotent, atomic with
> the qualifying write via db.batch), with an attribution window and
> last-touch rule documented. The attribution window is thirty days from
> capture; the latest pending cookie is the last-touch winner until first
> signup, agent-claim, or paid-order consumption. Acceptance: tests cover
> consume-once, window expiry, and no-attribution paths; consumed attributions
> are queryable through the admin-gated
> `/api/operator/sites/referrals/consumed` route.

**Issue 16 — `referrals: payout policy and referral payout ledger`**

> Policy doc + ledger converting consumed attributions into payout-eligible
> records: reward amount/percentage, caps per referrer/period, abuse rules
> (self-referral, ring detection), dispute/reversal handling, and payout
> state machine (eligible → approved → dispatched → settled, reusing the
> proven MDK settlement bridge semantics). Ledger rows are append-only
> with reversal entries, not mutations. Acceptance: schema + tests for the
> state machine and caps; policy doc linked from
> `sites.referral_bitcoin_stream.v1` evidenceRefs.

**Issue 17 — `referrals: first settled referral payout and clear referral_stream_not_live`**

> Run one real referral end-to-end: captured attribution → qualifying event
> → eligible ledger row → operator-approved small-sats payout via the MDK
> bridge → public settlement receipt. Clears `referral_stream_not_live` on
> the five-streams promise and advances `sites.referral_bitcoin_stream.v1`
> (clears `referral_settlement_receipts_missing`; policy blocker cleared by
> Issue 16); transition receipts before registry edits. Acceptance: one
> public referral settlement receipt ref, ledger states consistent,
> registry updated.

### Composition

**Issue 18 — `five-streams: one-install stacking smoke and registry flip`**

> The promise claims one install stacks the streams. At the red → yellow
> flip, add `blocker.product_promises.one_install_stacking_smoke_missing`
> so the composition claim stays mechanically gated. Then run the smoke:
> one registered Pylon identity, one session window, public receipts from
> at least two different streams (compute + labor is the natural pair; tips
> to the same wallet-readiness identity also counts) projected under the
> same pylonId. Green copy for the promise only after all four stream
> blockers AND the stacking blocker are cleared, each with its own
> transition receipt. Acceptance: stacking receipts visible publicly under
> one identity; promise green with `lastVerifiedAt` populated.

### Optional polish (not blocking this promise)

**Issue 19 — `tips: webhook live callback, refund/reversal, and checkout polish (yellow→green)`**

> `forum.content_tipping.v1` already does not block the five-streams
> promise, but its own yellow → green needs: live MDK webhook callback
> smoke, a public refund/reversal smoke, browser checkout polish (the
> worker-served `/checkout/{id}` QR page shipped 2026-06-10 covers part),
> and broader wallet coverage. File separately so tips polish never gates
> the five-streams campaign.

## Evidence Reviewed

- `docs/transcripts/213.md`, `docs/transcripts/214.md`,
  `docs/transcripts/215.md`
- Git history: `f5919c766` (Bun rebuild removing the NIP-90 stack),
  `f574f123d` (Artanis authority through NIP-90 settlement),
  `22a162b11` (provider admission before NIP-90 execution),
  `1fc3d3e01` (Data Market NIP-90 profile),
  `558729282` (draft NIP-DS), `crates/nostr/core/src/nip90/`,
  `crates/nostr/nips/{DS,SKL,SA,AC,TRN}.md`
- `apps/nostr-relay/README.md` (live relay POC, issue #4621)
- `apps/openagents.com/workers/api/src/product-promises.ts`
- `apps/openagents.com/docs/2026-06-08-pylon-agentic-revenue-gap-audit.md`
- `apps/pylon/docs/2026-06-09-pylon-v0.3-launch-promise-reconfiguration-audit.md`
- Live: `GET /api/public/product-promises`, `GET /api/public/pylon-stats`,
  `GET /api/forum/launch-status`, `GET /api/forum/tip-leaderboards`
- Forum (product-promises forum, all authored by Fable, reviewed for the
  Delegation Contract): `promise-flip-campaign-conventions` (lane taxonomy,
  claim/evidence/done/flip rules), `campaign-priorities-wave-1` (issue map
  with file-surface parallelization lanes — the delegation pattern this plan
  reuses), `wave-1-wrapup-2026-06-09` (fourteen issues closed in one day
  under those conventions; MDK operational findings),
  `fable-registry-review-2026-06-09-15` (registry critique; safeCopy/
  unsafeCopy discipline)
