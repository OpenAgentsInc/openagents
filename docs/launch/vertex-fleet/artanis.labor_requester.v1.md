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

- `apps/openagents.com/workers/api/src/artanis-labor-receipt-routes.ts`
  - `buildArtanisLaborReceiptFeedProjection(input)` is the public **read side**:
    it folds a set of `ArtanisLaborSealedReceipt`s into ONE public-safe feed
    (`schema openagents.artanis_labor_receipt_feed.v1`) with a summary (total
    count, per-terminal-state counts in lifecycle order, placed-request count)
    and projected rows. The summary is always computed over the full set before
    the filter, so a filter can narrow `rows` but never hide the headline
    counts. It re-runs `verifyArtanisLaborUnattendedRequestReceipt` on every
    sealed receipt, so the feed can never serve a receipt whose bytes no longer
    address its ref — even if the store is later swapped for a backing that
    trusts itself.
  - `handlePublicArtanisLaborReceiptsApi(request, { store, nowIso })` is the
    GET-only, no-store handler. With `?receiptRef=` it does a point read
    (`store.get`) instead of a list scan; an unknown ref returns an empty feed
    (rows `[]`, all-zero summary) rather than a 404 so a client can poll a ref
    it expects to appear without branching on status codes. `?terminalState=`
    narrows the listed rows. It mints no payment, identity, or settlement
    authority and carries the same `authorityBoundary` disclaimer as the other
    public audit projections.
- `apps/openagents.com/workers/api/src/artanis-labor-receipt-routes.test.ts`
  - 9 cases: zeroed empty summary, mixed-set per-terminal-state folding,
    public-safe row shape, terminal-state filter (rows narrow, summary does
    not), tampered-receipt refusal, non-GET rejection (405), GET list ordering,
    single-ref point dereference, and the unknown-ref empty-feed contract.

## What remains

- The receipt now has a canonical wire form, a content-addressed ref, a
  validating read/verify path (`parse` + `verify`), a one-shot `seal`, an
  in-memory tick-ledger **store** with idempotent writes and tamper-evident
  reads, and a public-safe **read projection + GET handler** that dereferences
  the whole feed or a single receipt by ref. So the receipt is now
  **persistable, dereferenceable through a route handler, and tamper-evident**
  end to end at the library layer. What still remains before the blocker can be
  dropped: (1) registering the handler on a concrete path in `index.ts` backed
  by a **durable store** — the in-memory store is per-request, so registering it
  on its own would always serve an empty feed (theater), which is why this run
  stops at the handler; (2) a durable backing (D1/KV) behind the
  `ArtanisLaborUnattendedReceiptStore` interface, mirroring
  `makeD1HygieneDebtReceiptStore`; and (3) a real unattended tick producing one
  of these receipts end-to-end.
- `blocker.product_promises.artanis_labor_live_enablement_missing` is untouched and
  still open: Artanis is not operator-enabled for a live unattended labor request.

Both blockers stay listed on the promise; the promise state is left at `yellow`.
