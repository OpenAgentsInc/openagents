# artanis.labor_requester.v1 — unattended labor request receipts

Promise: `artanis.labor_requester.v1` (state: yellow — unchanged by this run).

## Blocker advanced

`blocker.product_promises.artanis_labor_unattended_request_receipts_missing`

The requester surface (`apps/openagents.com/workers/api/src/artanis-labor-requester.ts`)
already validates proposals, applies the per-tick labor budget and seeded-balance
gates, reserves escrow, and routes delivered work through validator-pass release or
validator-fail refund. It emitted per-stage tick receipts through injected callbacks
and returned typed outcomes, but there was **no single, dereferenceable receipt** that
folds a whole unattended tick lifecycle into one public-safe artifact an operator or
reviewer can read to confirm the flow ran under its gates. That missing artifact is
exactly what this blocker names.

## What this change builds

- `apps/openagents.com/workers/api/src/artanis-labor-request-receipt.ts`
  - `buildArtanisLaborUnattendedRequestReceipt(input)` folds the request outcome
    (`ArtanisLaborRequesterOutcome`) and the optional acceptance outcome
    (`ArtanisLaborAcceptanceOutcome`) into one
    `ArtanisLaborUnattendedRequestReceipt` (schema `artanis.labor.unattended_request_receipt.v1`).
  - Terminal states: `skipped_config_disabled`, `refused`,
    `requested_pending_delivery`, `accepted_released`, `rejected_refunded`.
  - Projects only refs already present on the outcomes (work request, nostr event,
    reserve/release/refund receipt refs, validator reason refs); mints no payment,
    identity, or settlement authority.
  - Re-uses `assertArtanisLaborPublicSafe` so no private or payment material can
    appear in a projected receipt; rejects impossible combinations (acceptance
    without a reserved request) via `ArtanisLaborReceiptError`.
- `apps/openagents.com/workers/api/src/artanis-labor-request-receipt.test.ts`
  - 8 cases covering every terminal state, ref folding, public-safety refusal, and
    the impossible-combination guard.

## What remains

- This builds the receipt **projection**; it is not yet wired into a public route
  (`/api/public/...`) or persisted alongside the tick ledger. That route/persistence
  wiring, plus a real unattended tick producing one of these receipts end-to-end,
  remains before the blocker can be dropped.
- `blocker.product_promises.artanis_labor_live_enablement_missing` is untouched and
  still open: Artanis is not operator-enabled for a live unattended labor request.

Both blockers stay listed on the promise; the promise state is left at `yellow`.
