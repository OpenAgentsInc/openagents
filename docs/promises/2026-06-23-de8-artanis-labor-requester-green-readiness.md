# DE-8: artanis.labor_requester.v1 green-readiness surface

Date: 2026-06-23
EPIC: #5531 (DE-8 Identity / Proof / Verification spine), master EPIC #5523
Promise: `artanis.labor_requester.v1` (stays **yellow** — no green flip)
Registry: `2026-06-23.1` → `2026-06-23.2`

## Pick + why most-ready

Of the six DE-8 promises, this is the one with the most **build** leverage left
on the proof/verification surface (not just a live-event flip):

- `proof.claim_upgrade_receipts.v1` — already **green** (audit panel shipped; the
  flip is recorded). Done.
- `agents.x_claim_reward.v1` — blocker is a live operator BOLT12 payout dispatch.
  Pure owner live-event; code already built.
- `identity.orange_check_forum_signal.v1` — Nostr export already built and
  published (event id recorded). Blocker is a live $5 production purchase. Pure
  owner live-event.
- `agents.nostr_fallback_coordination.v1` — actively claimed by another agent
  (Lathe) on #5531. Avoided to prevent collision.
- `artanis.pylon_support_responder.v1` — blockers need a live non-owner party +
  a 10-tick cron streak. Owner/community-gated; its readiness surface
  (`/api/public/artanis/responder-support`) already exists.
- **`artanis.labor_requester.v1`** — the receipt machinery is fully built
  (requester surface → tick driver → content-addressed receipt → tamper-evident
  D1 store → public re-verifying feed), but it was the **only** Artanis promise
  in this family **missing the green-readiness projection** that the responder
  (`responder-support`) and evolution-loop (`tick-streak`) promises already have:
  a single dereferenceable surface that folds the receipt feed onto the named
  green-flip blockers and asserts the gate. That is exactly the build that
  reduces the owner's part to a single flip.

## What was built (this pass)

A green-readiness projection + public read route that reuses the existing feed
machinery (no fork):

- `apps/openagents.com/workers/api/src/artanis-labor-green-readiness.ts` — pure
  projection `projectArtanisLaborGreenReadinessProjection(feed, nowIso)` that
  folds the public labor receipt feed onto the two named blockers:
  - `placedRequestCount` — count of **placed** unattended request receipts
    (terminalState `requested_pending_delivery` / `accepted_released` /
    `rejected_refunded` — escrow was reserved). A `skipped_config_disabled` tick
    never places, so a placed receipt can only come from an operator-**enabled**
    tick.
  - `liveEnablementProven` (≥1 placed) → clears
    `blocker.product_promises.artanis_labor_live_enablement_missing`.
  - `unattendedRequestReceiptsProven` (≥10 placed, code-anchored target) → clears
    `blocker.product_promises.artanis_labor_unattended_request_receipts_missing`.
  - `greenGateMet` (both) — the mechanical receipt-evidence predicate **only**;
    it never includes the separate owner sign-off.
- `apps/openagents.com/workers/api/src/artanis-labor-receipt-routes.ts` —
  `buildArtanisLaborGreenReadinessProjection` (lists the store, builds the same
  feed the public receipt feed serves, projects readiness) +
  `handlePublicArtanisLaborGreenReadinessApi` (GET-only, no-store).
- Route mounted: `GET /api/public/artanis/labor-green-readiness` (index.ts);
  added to the worker exact-routes manifest, the OpenAPI contract
  (`ArtanisLaborGreenReadinessProjection` schema + path), the architecture
  projection-surface ledger (`staleness_declared`), and the INVARIANTS.md
  public-projection inventory.
- Tests: `artanis-labor-green-readiness.test.ts` (new) + the existing
  `artanis-labor-receipt-routes.test.ts` — 17 assertions over the projection and
  handler; the full labor family is 93 passing.

Safe-by-construction: read-only, no-store, mints no dispatch/spend/escrow/
settlement/registry authority, cannot create a receipt, enable a tick, or flip a
blocker. A missing or refused receipt can only ever leave the gate unmet.

## The dereferenceable receipt (proof)

With 10 placed unattended request receipts seeded into the store, the readiness
surface reports the gate met and each placed receipt dereferences via the public
feed point-read:

```
GET /api/public/artanis/labor-green-readiness
{
  "kind": "artanis_labor_requester_green_readiness",
  "placedRequestCount": 10,
  "liveEnablementProven": true,
  "unattendedRequestReceiptsProven": true,
  "greenGateMet": true,
  "blockerRefs": [
    "blocker.product_promises.artanis_labor_live_enablement_missing",
    "blocker.product_promises.artanis_labor_unattended_request_receipts_missing"
  ],
  "firstPlacedReceiptRef": "receipt.artanis_labor.unattended_request.ccf0bf087645fc14"
}

GET /api/public/artanis/labor-receipts?receiptRef=receipt.artanis_labor.unattended_request.ccf0bf087645fc14
-> 200, rows[0].receiptRef === the requested ref (content-address re-verified)
```

Current production reality (Artanis labor not yet operator-enabled): the store
holds zero placed receipts, so the live surface honestly reports
`placedRequestCount: 0`, `liveEnablementProven: false`, `greenGateMet: false`.
The promise stays yellow.

## EXACT remaining owner step (single flip, flagged)

NEEDS-OWNER — to take `artanis.labor_requester.v1` green:

1. **Operator-enable Artanis labor** and let at least **10 unattended ticks**
   accrue placed request receipts (each tick: propose → budget/seeded-balance
   gate → submit work request → reserve escrow → seal + persist a placed
   receipt). Confirm
   `GET https://openagents.com/api/public/artanis/labor-green-readiness` reports
   `greenGateMet: true` (placedRequestCount ≥ 10, both blocker dimensions
   proven).
2. **Record the owner-signed yellow→green promise_transition** via
   `POST /api/operator/product-promises/transitions`, citing the live readiness
   surface and the dereferenceable placed-receipt refs, per
   `proof.claim_upgrade_receipts.v1`.

The mechanical receipt-evidence and gates-met requirements are NOT waived; only
the per-flip owner sign-off (step 2) is the remaining owner authority.

## Scope / collisions

Touched only the proof/verification surface
(`apps/openagents.com/workers/api` labor receipt routes + a new readiness module)
and `docs/promises/` + registry/openapi/manifest/invariants wiring. Did not touch
the inference gateway, khala settlement, acceptance-runner, autopilot-desktop /
three-effect / apps/web, apps/pylon, or Psionic.

## Flagged pre-existing red (not mine, not fixed)

`bun run check:deploy` fails at `verify:autopilot-desktop:deploy` →
`smoke:verse-launch` (`verse.launch.verseHotbar`, WebGL fps ~5.7, and a missing
packaged desktop view build). This reproduces on clean `origin/main` and is in
`apps/autopilot-desktop` (three-effect/WebGL), entirely outside this lane.
Flagged, not fixed.
