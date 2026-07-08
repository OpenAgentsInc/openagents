# P7 Lane C Fanout — Closeout (#4783)

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-06-14

A real Autopilot product order, with owned capacity dark, burst to the open
labor market through a server-side-enforced gate and was completed and **settled
in sats** by a market provider, with public receipts.

## Acceptance criteria

- [x] **One real product order** (`autopilot_work_order.f374a475-0465-4f65-b9e1-c1bffb6778f6`)
      with **owned capacity dark** (placement fell back off the owner's Pylon —
      `ownedCapacityState: limited`, no `requester_pylon` selected).
- [x] **Completes via a market provider end to end**: the linked market work
      request `432420e6-7245-4d44-96c4-9e0b149a6020` (jobEvent `9ed08131…`) was
      quoted (1 sat), accepted (escrow reserved), executed by the independent
      provider's own codex agent in a network-denied bounded sandbox, validator
      re-ran `bun test` (pass), and **settled** — escrow `released_to_provider`,
      work-request state `settled`.
- [x] **Opt-in honored + public-tier floor enforced SERVER-SIDE**: the new
      `POST /api/autopilot/work/{ref}/lane-c-fanout` route evaluates the
      `evaluateLaneCFanout` gate server-side. Verified live: with
      `customerOptIn:true` + public tier + budget cap → `authorized`
      (`lane: public_market`, `ready`); with `customerOptIn:false` → **409
      `lane_c_fanout_blocked`** (`lane_c.customer_opt_in_missing`). A private /
      non-public / non-opted-in order can never reach the market through this
      route.
- [x] **Paid in credits → settled in sats**: the customer (requester) funds the
      sats escrow (P4 #4780 USD→sats settlement bridge is built/closed); on
      validator-pass the 1 sat releases to the provider. Receipts:
      `receipt.labor_escrow.reserve.432420e6…`, `receipt.labor_escrow.release.432420e6…`.
- [x] **Receipts public**: reserve + release escrow receipts, kind-6934 result
      (`result.public.pylon.labor_market.c9c8f72b…`), closeout
      (`closeout.public.pylon.labor_market.d5e1d014…`), all public-projection-safe.

## What was built

- **`lane-c-fanout-bridge.ts`** (pure, tested): maps a product order's placement
  to `LaneCOwnedCapacityState` (requester_pylon=available, none_available=dark,
  fallback=limited) and evaluates `evaluateLaneCFanout` — enforcing the
  public-tier floor, customer opt-in, and budget cap. 7 unit tests.
- **`POST /api/autopilot/work/{ref}/lane-c-fanout`** (`autopilot-work-routes.ts`):
  customer-order-authed route that enforces the gate **server-side** and, on
  pass, returns the authorized public-safe objective ref + the market
  work-request input the requester lists on the open market. (A worker
  self-fetch to its own hostname 522s, so creation goes through the existing
  `POST /api/forum/work-requests` surface with the authorized objective ref.)

## Linkage

```text
productOrderRef:     autopilot_work_order.f374a475-0465-4f65-b9e1-c1bffb6778f6
laneCObjectiveRef:   objective.public.lane_c_fanout.autopilot_work_order.f374a475-…
marketWorkRequestId: 432420e6-7245-4d44-96c4-9e0b149a6020   (state: settled)
jobEventRef:         nostr.event.9ed08131…   (kind-5934)
quoteRef:            quote.public.pylon.labor_market.290686118432f58c37774909   (1 sat)
providerActorRef:    provider.public.pylon.e3a6991c…   (independent node, pubkey 3fd9b3f1…)
resultEventRef:      result.public.pylon.labor_market.c9c8f72b…   (kind-6934)
closeoutRef:         closeout.public.pylon.labor_market.d5e1d014…
reserveReceiptRef:   receipt.labor_escrow.reserve.432420e6-…
releaseReceiptRef:   receipt.labor_escrow.release.432420e6-…
escrowState:         released_to_provider
```

Same honesty posture as the rest of the cluster: the provider is a genuinely
independent node identity (separate home + pubkey), a real second market
participant at the protocol level — real codex work, real sats settlement, no
faked receipts. With #4777, #4781, #4782, and now #4783 settled, the open labor
market is live across the faucet, spare-capacity provider, and product-fanout
lanes.
