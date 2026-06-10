# Five Bitcoin Revenue Streams Promise Audit

Date: 2026-06-10

Promise: `pylon.five_bitcoin_revenue_streams.v1`

Registry version at audit time: `2026-06-10.1`

Status: full status audit of the promise and an exact get-to-green plan.

## The Promise

> Pylon stacks compute, data, Forum tips, referrals, and subscription/token-capacity
> arbitrage in one install.

Current registry record:

- state: `red`
- safeCopy: "Forum tipping and multiple future revenue gates exist, but
  one-install multi-stream Bitcoin earning is not live."
- blockerRefs:
  - `blocker.product_promises.compute_stream_not_broadly_live`
  - `blocker.product_promises.data_stream_not_live`
  - `blocker.product_promises.referral_stream_not_live`
  - `blocker.product_promises.capacity_stream_not_live`
- verification: "Each revenue stream needs its own evidence refs, public-safe
  receipts, policy gates, and settlement state."
- lastVerifiedAt: none (no transition receipt has ever been recorded for this
  promise).

Note the blocker set has exactly four entries for five streams: the Forum tips
stream is the only stream considered live enough that it does not block this
promise at the per-stream level.

## Why This Promise Is Special

This is a **composition promise**. It can only go green when:

1. each of the five streams independently has live, receipt-backed,
   settlement-state evidence under its own gate; and
2. one Pylon install demonstrably participates in (at minimum) more than one
   of those streams at once — "stacks ... in one install" is itself a claim
   that needs its own smoke.

Every constituent stream maps to other promises in the registry, so this
promise's state is effectively `min()` over its dependencies plus a stacking
proof nobody has attempted yet.

## Live Evidence Snapshot (2026-06-10)

Checked live during this audit:

- `GET /api/public/product-promises` → version `2026-06-10.1`, promise `red`
  with the four blockers above.
- `GET /api/public/pylon-stats`:
  - `pylonsOnlineNow: 0`, `pylonsWalletReadyNow: 0`,
    `pylonsAssignmentReadyNow: 0`, `sellablePylonsOnlineNow: 0`
  - `pylonsSeen24h: 4`, `pylonsRegisteredTotal: 7`
  - client versions include `0.3.0` and `openagents.pylon@0.3.0-rc1`
  - `nexusAcceptedWorkPayoutSatsPaidTotal: 2323` with 8 public settlement
    receipt refs (including both paid GEPA multi-Pylon settlements from
    2026-06-08)
- `GET /api/forum/launch-status` → `status: ready`, `orangeChecksSold: 2`
- `GET /api/forum/tip-leaderboards` → live creators with nonzero
  `totalSettledSats` (settled, not just paid, distinguished in public copy)

## Stream-By-Stream Status

### Stream 1: Compute — partially live (closest to clearing)

Blocker: `compute_stream_not_broadly_live`.

What is live:

- 2,323 sats of accepted-work payouts with public settlement receipts,
  including two paid GEPA Pylon assignments settled with real bitcoin
  (2026-06-08).
- Full assignment lifecycle routes (register → heartbeat → wallet readiness →
  assignment lease → accept → progress → artifacts → closeout → receipt) are
  live; a complete no-spend loop ran end-to-end on production on 2026-06-09
  (work order with promiseRef, live Pylon `pylon.fable.live_smoke_4633`,
  accepted closeout) as part of issue #4633.
- Pylon v0.3 clients are registering against production (`0.3.0`,
  `0.3.0-rc1` heartbeats visible in stats).

What is missing:

- Zero Pylons online/wallet-ready/sellable at audit time. "Broadly live"
  requires a continuously online sellable network, not episodic smokes.
- Pylon v0.3 (standalone repo) still gates its own worker loop:
  `pylon.gepa_worker_loop_v03.v1` is yellow with
  `live_openagents_gepa_endpoint_smoke_missing` and
  `paid_gepa_settlement_v03_missing`.
- `pylon.compute_revenue_modes.v1` is red (no live paid GEPA network, no
  sellable local inference, no remote Qwen training).

To clear this blocker:

1. Pass the v0.3 live OpenAgents GEPA endpoint smoke (clears the first
   gepa_worker_loop blocker).
2. Settle at least one paid GEPA assignment from a v0.3 client (clears the
   second; reuses the proven MDK agent-wallet settlement bridge).
3. Keep ≥1 real contributor Pylon continuously online with fresh heartbeats
   so `pylonsWalletReadyNow`/`sellablePylonsOnlineNow` are nonzero at
   verification time (heartbeat keeper or operator runbook).
4. Move assignment creation from one-off operator flow to the controlled
   dispatcher (spend cap, no duplicate assignment, pause/rollback).

### Stream 2: Data — not live (furthest, with capacity)

Blocker: `data_stream_not_live`. Dependent promise
`pylon.data_trace_revenue.v1` is red with `settled_trace_sale_missing`.

What is live: the gate doc only
(`2026-06-08-data-trace-marketplace-gate.md`). No trace submission,
redaction, valuation, purchase, entitlement, or payout path exists.

To clear this blocker, one public-safe settled trace sale smoke with receipt
refs covering: trace submission → redaction → consent → valuation → purchase →
buyer entitlement → contributor payout contract → settlement receipt. This is
a build, not a smoke — it is the only stream with zero implemented surface.

### Stream 3: Forum tips — live (already non-blocking)

No blocker on this promise. `forum.content_tipping.v1` is yellow with
`lastVerifiedAt: 2026-06-10T02:44:34Z`:

- Strict smooth-path funded production smokes passed 2026-06-09 against two
  independent live ready recipients with verified creator-spendable BOLT 12
  settlement and no timeout recovery.
- Live tip leaderboards distinguish paid vs settled sats and show nonzero
  settled totals.
- Adjacent paid rail also live: orange check self-purchase
  (`identity.orange_check_forum_signal.v1` yellow, 2 sold, atomic redemption,
  provider-gated fulfillment).

Remaining yellow→green work (webhook live callback, refund/reversal smoke,
browser checkout polish, broader wallet coverage) does **not** block the
five-streams promise — but the stacking smoke (below) should use this stream
because it is the most reliable.

### Stream 4: Referrals — attribution live, payout not

Blocker: `referral_stream_not_live`. Dependent promise
`sites.referral_bitcoin_stream.v1` is yellow with
`referral_payout_policy_missing` and `referral_settlement_receipts_missing`.

What is live: Site referral capture routes (`/r/site/{publicSourceRef}`) and
pending attribution persistence. Attribution is explicitly not payout
eligibility.

To clear this blocker:

1. Referral payout policy: attribution consumption at signup/order, abuse and
   dispute handling, caps, reversal rules.
2. A payout ledger that converts consumed attribution into payout-eligible
   records.
3. At least one settled referral payout with a public settlement receipt
   (small-sats is fine; the MDK settlement bridge already proven for Pylon
   payouts can be reused).

### Stream 5: Subscription/token-capacity arbitrage — not live (furthest, with data)

Blocker: `capacity_stream_not_live`. Dependent promises both red:
`provider.subscription_capacity.v1` (`capacity_metering_missing`,
`provider_tos_policy_missing`, `capacity_settlement_missing`) and
`provider.prepaid_capacity_monetization.v1`
(`prepaid_provider_policy_missing`).

What is live: ChatGPT/Codex provider-account connection, device login, the
six-account operator runbook, provider-account contracts in the Pylon v0.3
runtime, and the per-provider capacity marketplace gate (unsupported /
configured / healthy / assignable / payable / settled states). The capacity
funnel (`pylon.no_dark_capacity_accounting.v1`, yellow, lastVerifiedAt
2026-06-10) gives the accounting substrate.

To clear this blocker, for at least one provider (ChatGPT first):

1. Capacity metering: usage records bound to a connected provider account.
2. An explicit ToS/policy boundary doc for reselling that provider's
   capacity (this is a policy decision, not just code — it may legitimately
   conclude "metered internal assignment only, no resale", in which case the
   promise claim itself should be narrowed in the same change).
3. Assignment routing that consumes metered capacity for real work.
4. Pricing and a settled bitcoin payment to the capacity contributor with a
   public receipt.

### The stacking proof (no blocker ref exists for it yet)

Even with all four blockers cleared, green copy for "stacks ... in one
install" needs a one-install smoke: a single registered Pylon identity that,
in one session window, (a) completes a paid compute assignment and (b)
receives a Forum tip to the same wallet readiness identity — plus public
receipts for both, projected under the same pylonId. When the registry flips
this promise past yellow, add an explicit
`blocker.product_promises.one_install_stacking_smoke_missing` so the
composition claim is mechanically gated rather than implied.

## Exact Path To Green

Honest sequencing, cheapest first:

1. **Compute (days-scale):** v0.3 live GEPA endpoint smoke → one paid v0.3
   settlement → heartbeat keeper so online counters are nonzero → clear
   `compute_stream_not_broadly_live`. Most of the machinery already has
   production receipts; this is integration plus uptime, not new product.
2. **Referrals (week-scale):** payout policy + ledger + one settled
   small-sats referral payout receipt → clear `referral_stream_not_live`.
3. **Capacity (weeks-scale, policy-gated):** ChatGPT metering + ToS boundary
   decision + one settled capacity payment → clear
   `capacity_stream_not_live`. The ToS decision is the real gate; do it
   before writing the metering code.
4. **Data (weeks-scale, full build):** the settled trace sale smoke per the
   data trace marketplace gate → clear `data_stream_not_live`.
5. **Stacking smoke:** one install, two streams, same identity, public
   receipts → only then green copy.

Intermediate red→yellow flip is defensible once compute and referrals are
both cleared (three of five streams live-evidenced, tips already live), with
safeCopy stating exactly which streams have settled receipts and which remain
gated. Record the transition via
`POST /api/operator/product-promises/transitions` **before** shipping the
registry edit, so the receipt evaluates cleanly instead of as a backfill
exception, and so `lastVerifiedAt` finally populates for this promise (it is
currently null — it has never had a transition receipt).

## What This Audit Does Not Change

This document does not modify the registry. Follow-ups when work starts:

- add this doc to the promise's `evidenceRefs` in the next registry version
  bump;
- add the `one_install_stacking_smoke_missing` blocker at the red→yellow
  flip;
- per stream, clear blockers one at a time with transition receipts rather
  than batching.

## Evidence Reviewed

- `apps/openagents.com/workers/api/src/product-promises.ts` (registry source,
  version 2026-06-10.1)
- `apps/openagents.com/docs/2026-06-08-pylon-agentic-revenue-gap-audit.md`
- `apps/pylon/docs/2026-06-09-pylon-v0.3-launch-promise-reconfiguration-audit.md`
- Live: `GET /api/public/product-promises`, `GET /api/public/pylon-stats`,
  `GET /api/forum/launch-status`, `GET /api/forum/tip-leaderboards`
