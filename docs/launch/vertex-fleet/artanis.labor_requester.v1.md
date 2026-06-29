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

## What this run adds (durable backing + live route registration)

This run cleared remaining items (1) and (2) of the previous "What remains":

- `apps/openagents.com/workers/api/migrations/0215_artanis_labor_unattended_receipts.sql`
  — durable D1 table `artanis_labor_unattended_receipts`: one row per
  content-addressed receipt ref (PK), the canonical serialized bytes as source
  of truth, a denormalized `terminal_state` for audit/query, and `created_at`.
  Idempotent by construction (the ref is the primary key).
- `apps/openagents.com/workers/api/src/artanis-labor-receipt-store.ts` (extended)
  — `makeD1ArtanisLaborUnattendedReceiptStore(db, nowIso)` implements the
  existing `ArtanisLaborUnattendedReceiptStore` interface against D1, mirroring
  `makeD1HygieneDebtReceiptStore`. `put` uses `INSERT OR IGNORE` on the
  content-addressed PK (idempotent: a re-put returns `already_stored` via
  `meta.changes === 0`), refusing an inconsistent sealed receipt first.
  `get`/`list` re-verify the persisted bytes against the ref they are keyed
  under (`verifyArtanisLaborUnattendedRequestReceipt`), so a corrupted/edited
  row can never be served — tamper-evident reads. `list` is `created_at, rowid`
  ordered. Mints no payment, identity, or settlement authority.
- `apps/openagents.com/workers/api/src/artanis-labor-receipt-store-d1.test.ts`
  — 6 cases over a minimal in-memory D1 fake: put/get round-trip, idempotent put
  on the content-addressed ref, created-at-ordered list, unknown-ref miss, the
  tamper-evident read (a corrupted persisted row rejects), and the
  inconsistent-sealed-receipt refusal.
- `apps/openagents.com/workers/api/src/index.ts` (wired) — registered the
  read-only feed route `GET /api/public/artanis/labor-receipts` backed by the
  durable D1 store, no longer theater because the store survives across requests.
- Ledger/inventory: added the route as `staleness_declared` in
  `scripts/check-zero-debt-architecture.mjs` and to the Public Projection
  Staleness Declaration inventory in `INVARIANTS.md`.

## What this run adds (tick driver: run-and-seal seam)

This run builds the seam that previous item (3) named — connecting the gated
requester surface to the durable receipt store so a **real tick ends with a
sealed receipt persisted by content address**, not just a fixture:

- `apps/openagents.com/workers/api/src/artanis-labor-tick-driver.ts`
  - `runAndPersistArtanisLaborRequestTick({ store, requesterDeps, artanisActorRef, tickRef })`
    runs `runArtanisLaborRequestTick`, seals the consolidated receipt from its
    typed outcome, and `store.put`s it. **Every** terminal state is sealed —
    `skipped_config_disabled` and `refused` included — so an operator can audit
    that the gates ran on a tick even when no work request was placed. Returns
    `{ requestOutcome, sealed, put }`. Persistence is idempotent by content
    address: re-running the same tick is an `already_stored` no-op.
  - `resolveAndPersistArtanisLaborDelivery({ store, acceptanceDeps, delivery, requestOutcome, artanisActorRef, nowIso, tickRef })`
    runs `handleArtanisLaborResultDelivery` and seals the consolidated receipt
    that folds the original request stage with the validator-pass release or
    validator-fail refund (`accepted_released` / `rejected_refunded`), then
    persists it. The original `'requested'` outcome is required and narrowed at
    the type level (`ArtanisLaborRequestedOutcome`) so a delivery can only be
    resolved against a request that actually reserved escrow.
  - The driver mints no payment, identity, or settlement authority — it only
    runs the already-gated surface, seals its public-safe projection, and
    persists it.
- `apps/openagents.com/workers/api/src/artanis-labor-tick-driver.test.ts`
  - 6 cases: placed tick seals + persists a `requested_pending_delivery` receipt
    readable by ref; disabled tick still seals a `skipped_config_disabled`
    receipt without proposing; over-budget tick seals a `refused` receipt with a
    null work-request id; idempotent re-run by content address; validator-pass
    delivery seals `accepted_released`; validator-fail delivery seals
    `rejected_refunded`.

## What this run adds (frozen wire-format regression guard)

The receipt is content-addressed and persisted durably in D1 keyed by its ref,
and `get`/`list` re-verify that the stored bytes still address that ref. The
existing tests prove the ref is *deterministic* and *divergent*, but they
recompute both sides of every comparison — so a refactor of `serialize`/`derive`
(key order, spacing, field rename, digest length) would silently change every
persisted ref while all of those tests still pass, orphaning every
already-stored receipt (its tamper-evident read would fail and it would drop out
of the public feed). This run closes that gap:

- `apps/openagents.com/workers/api/src/artanis-labor-request-receipt-golden.test.ts`
  — golden vectors that freeze the **exact** canonical wire bytes and
  content-addressed ref for every terminal state against fixed inputs (16 cases:
  per-state frozen bytes, frozen ref, and the durable-store contract that the
  frozen bytes still parse/verify under the frozen ref today, plus a
  full-terminal-state coverage pin). Re-blessing these values now forces an
  explicit decision (format change ⇒ migration/version bump for stored
  receipts) instead of a silent break. Mints no payment, identity, or settlement
  authority.

## What this run adds (consumer-side feed verification)

The serving side already re-verifies every sealed receipt against the ref it is
keyed under before projecting it, but a **third party reading the public JSON
feed** had no library function to confirm the same thing for itself — it had to
trust the server's `receiptRef`. A feed row carries exactly the eight canonical
receipt fields plus that served ref, so a consumer can reconstruct the canonical
receipt from the row's own public fields, re-derive its content-address, and
confirm it matches the served name without trusting the server. This run closes
that gap:

- `apps/openagents.com/workers/api/src/artanis-labor-receipt-feed-verify.ts`
  - `verifyArtanisLaborReceiptFeedRow(row)` reconstructs the canonical receipt
    from a feed row's public fields, re-serializes it through the canonical-form
    gate, re-derives its content-address, and confirms it matches the served
    `receiptRef` — throwing `ArtanisLaborReceiptError` if the row is internally
    inconsistent (e.g. a budget on a non-placed state) or any public field was
    mutated away from the ref it is served under. Returns the validated receipt.
  - `verifyArtanisLaborReceiptFeed(feed)` runs that check over every row in a
    feed projection, throwing on the first inconsistent row, so a non-throwing
    return means every served receipt is self-consistent. This is the entry
    point an external auditor calls against
    `GET /api/public/artanis/labor-receipts`. Mints no payment, identity, or
    settlement authority; reads only public fields.
- `apps/openagents.com/workers/api/src/artanis-labor-receipt-feed-verify.test.ts`
  - 8 cases: clean row re-derives its served ref, derived receipt equals the
    sealed source, tampered served ref rejected, internally inconsistent row
    rejected (budget on a skipped state), mutated public field rejected, full
    clean feed verified-and-counted, empty feed verifies to zero, and a feed with
    one forged row throws on that row.

## What this run adds (untrusted-bytes consumer parse boundary)

The consumer-side verifier (`verifyArtanisLaborReceiptFeed` /
`verifyArtanisLaborReceiptFeedRow`) re-derives each row's content-address from
its public fields, but it took an **already-typed**
`ArtanisLaborReceiptFeedProjection`. A real third party does not start with a
typed projection — it starts with raw JSON bytes downloaded from `GET
/api/public/artanis/labor-receipts`. To use the verifier it had to cast that
untrusted JSON to the typed shape (an unchecked `as`), which defeats the point of
independent verification. This run closes that gap:

- `apps/openagents.com/workers/api/src/artanis-labor-receipt-feed-verify.ts` (extended)
  - `parseArtanisLaborReceiptFeed(serialized)` is the untrusted-bytes parse
    boundary: it decodes through the sanctioned `parseJsonUnknown` json-boundary,
    validates the envelope (`schemaVersion`) and every row's field types and the
    terminal-state enum, and returns typed
    `ArtanisLaborReceiptFeedRow`s — or throws `ArtanisLaborReceiptError`. It does
    only structural validation; it does not re-derive content-addresses.
  - `parseAndVerifyArtanisLaborReceiptFeed(serialized)` is the one-call third-party
    entry point: parse the raw feed bytes, then re-derive every row's
    content-address and confirm it matches the served ref. A non-throwing return
    means every served receipt is structurally valid AND self-consistent under its
    own content address — all from untrusted bytes, with no `as`-cast and no trust
    in the server. Mints no payment, identity, or settlement authority; reads only
    public fields.
- `apps/openagents.com/workers/api/src/artanis-labor-receipt-feed-verify.test.ts` (extended)
  - 9 new cases: raw bytes parse+verify end to end, parse-only typed rows, empty
    feed, non-JSON rejection, non-object envelope rejection, unrecognized
    `schemaVersion`, non-array `rows`, unrecognized row `terminalState`, and a
    structurally-valid-but-tampered row that parses yet fails verification.

## What remains

- The receipt is now **minted by a run, persistable, dereferenceable,
  tamper-evident, durable, and publicly readable** end-to-end: the requester
  surface → seal → store path exists as a single driver call. The last step
  before the blocker can be dropped is operational, not structural: **wiring the
  driver into Artanis's live minute-tick scheduler** (passing the production
  `requesterDeps` and the D1 store) so receipts accrue from the real cron path.
  That wiring is gated behind operator enablement and is therefore part of
  `blocker.product_promises.artanis_labor_live_enablement_missing`.
- `blocker.product_promises.artanis_labor_live_enablement_missing` is untouched and
  still open: Artanis is not operator-enabled for a live unattended labor request.

Both blockers stay listed on the promise; the promise state is left at `yellow`.
