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
- `apps/openagents.com/workers/api/src/artanis-labor-request-receipt.ts` (extended)
  - `serializeArtanisLaborUnattendedRequestReceipt(receipt)` emits a canonical,
    deterministic wire form: top-level keys in fixed order so the same lifecycle
    always serializes to identical bytes, with `lifecycleRefs` array order
    preserved (it encodes the propose -> reserve -> validate -> release/refund
    sequence). Re-runs `assertArtanisLaborPublicSafe` before emitting.
  - `deriveArtanisLaborUnattendedRequestReceiptRef(receipt)` mints a
    content-addressed identity
    (`receipt.artanis_labor.unattended_request.<sha256-16>`) over that canonical
    form, following the existing `debt-receipt-key.ts` digest pattern. The receipt
    projection previously carried no id of its own, so it could neither be
    persisted alongside the tick ledger nor dereferenced from a route; this gives
    it a stable, collision-resistant name without minting any payment, identity,
    or settlement authority.
- `apps/openagents.com/workers/api/src/artanis-labor-request-receipt.ts` (extended again)
  - `parseArtanisLaborUnattendedRequestReceipt(serialized)` is the read side of
    `serialize`: before any public route or the tick ledger store can serve a
    persisted receipt it must turn untrusted wire bytes back into a validated,
    typed, public-safe receipt — or refuse. It validates every field type and the
    terminal-state enum, enforces the placed-vs-pre-request invariant
    (budget/`workRequestId` presence must match the terminal state), re-runs
    `assertArtanisLaborPublicSafe`, and requires the input to already be in
    canonical form by re-serializing the reconstructed receipt and rejecting any
    mismatch (extra keys, reordered keys, non-canonical spacing all fail). It
    parses through the sanctioned `parseJsonUnknown` json-boundary, not raw
    `JSON.parse`, so the zero-debt architecture check stays green.
  - `verifyArtanisLaborUnattendedRequestReceipt(serialized, expectedRef)` is the
    tamper check: it parses the wire form and confirms its content-addressed ref
    matches the name it was stored or served under, returning the validated
    receipt or throwing so a route/store can never hand back a receipt addressed
    by the wrong name. Neither function mints any payment, identity, or
    settlement authority.
- `apps/openagents.com/workers/api/src/artanis-labor-request-receipt.test.ts`
  - 17 cases: every terminal state, ref folding, public-safety refusal, the
    impossible-combination guard, canonical-serialization determinism, ref
    stability across rebuilds, distinct-state ref divergence, plus the new read
    side — round-trip for every terminal state, non-JSON/non-object rejection,
    unrecognized schema/terminal-state rejection, the placed-vs-pre-request
    invariant on read, non-canonical wire-form rejection, and ref verify
    match/mismatch.

- `apps/openagents.com/workers/api/src/artanis-labor-receipt-store.ts`
  - `sealArtanisLaborUnattendedRequestReceipt(input)` folds
    build -> serialize -> derive-ref into ONE `ArtanisLaborSealedReceipt`
    (`{ receipt, receiptRef, serialized }`) a caller can hand straight to a
    store/route. Because the ref is content-addressed over the canonical bytes,
    the same lifecycle always seals to the same ref, so idempotent persistence
    falls out for free.
  - `ArtanisLaborUnattendedReceiptStore` (interface) + in-memory
    `makeInMemoryArtanisLaborUnattendedReceiptStore()` — the persistence
    boundary that was still missing. `put` writes a sealed receipt keyed by its
    own content address (idempotent: a re-put of the same lifecycle returns
    `already_stored`, never overwrites; refuses an internally inconsistent sealed
    receipt). `get` re-verifies the persisted bytes still address the ref they
    are keyed under (tamper-evident read), and `list` returns rows in
    deterministic insertion order. It mirrors the hygiene debt-receipt store
    contract so a durable KV/D1 backing can later replace the in-memory map, and
    mints no payment, identity, or settlement authority.
- `apps/openagents.com/workers/api/src/artanis-labor-receipt-store.test.ts`
  - 8 cases: seal consistency + determinism, put/get round-trip, idempotent put
    on the content-addressed ref, distinct lifecycles under distinct refs,
    unknown-ref miss, and the two refusal guards (ref that does not address its
    bytes; object that disagrees with its bytes).

## What remains

- The receipt now has a canonical wire form, a content-addressed ref, a
  validating read/verify path (`parse` + `verify`), a one-shot `seal`, and an
  in-memory tick-ledger **store** with idempotent writes and tamper-evident
  reads, so it is **persistable, dereferenceable, and tamper-evident** through a
  real store contract. What still remains is wiring that store into a public
  route (`/api/public/...`), a durable backing (D1/KV) behind the same
  interface, and a real unattended tick producing one of these receipts
  end-to-end. Those remain before the blocker can be dropped.
- `blocker.product_promises.artanis_labor_live_enablement_missing` is untouched and
  still open: Artanis is not operator-enabled for a live unattended labor request.

Both blockers stay listed on the promise; the promise state is left at `yellow`.
