# Five Bitcoin Revenue Streams Promise Audit

Date: 2026-06-10 (revised same day after Episode 213/214/215 transcript review)

Promise: `pylon.five_bitcoin_revenue_streams.v1`

Registry version at audit time: `2026-06-10.1`; this audit ships with the
`2026-06-10.2` correction described below.

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
and removes the heavy provider-ToS resale gate this audit previously treated
as the long pole. The provider-account *resale* questions remain owned by the
separate `provider.subscription_capacity.v1` and
`provider.prepaid_capacity_monetization.v1` promises and do not block this
one. Registry `2026-06-10.2` renames the blocker
`capacity_stream_not_live` → `labor_stream_not_live` and fixes the claim,
safeCopy, and verification text accordingly.

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
that agent to work overnight sells output, which is labor, not account
resale, so the provider-ToS marketplace gate owned by
`provider.subscription_capacity.v1` does not block this stream.

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

## Registry Changes Shipped With This Audit (`2026-06-10.2`)

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
