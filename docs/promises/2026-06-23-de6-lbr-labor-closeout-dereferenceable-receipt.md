# DE-6: NIP-LBR Labor-Market Closeout Dereferenceable Receipt

Date: 2026-06-23

Part of EPIC **#5529** (DE-6 Open markets + marketplace), under the Weekend
Promise Assault master EPIC **#5523**.

## Promise advanced

`markets.open_protocol_markets.v1` (DE-6) — specifically the **labor** market's
dippable + dereferenceable-receipt rung.

This is receipt machinery on the `packages/nip90` protocol surface. It flips
**no** promise state and changes **no** green count. The promise STAYS `planned`;
its named blockers (`liquidity_market_unbuilt`, `risk_market_unbuilt`,
`compute_data_markets_not_broadly_live`) are untouched — none of them is what
this change addresses, and none can be cleared without real liquidity/risk
implementations and broad live compute/data markets.

## Why this promise was the most-ready buildable now

DE-6's receipt acceptance for `markets.open_protocol_markets.v1` is "all six
markets dippable + unified surface receipt." Of the six markets:

- **Data (NIP-DS)** already had a *dippable end-to-end producer with a
  verifiable receipt*: `apps/openagents.com/scripts/nip-ds.ts` signs a
  listing/offer/access-request/access-result, publishes them to a scoped relay,
  reads them back, and verifies the delivered-bundle digest
  (`verifyDatasetDeliveryDescriptorDigest`).
- **Labor (NIP-LBR)** had the full ref-only lifecycle protocol in
  `packages/nip90/src/lbr.ts` — request `5934`, quote/acceptance `7000`, result
  `6934` — but **no composed closeout object and no dereferenceable receipt**.
  There was no labor-market analogue of NIP-DS's digest verification: nothing
  bound one complete lifecycle into a single content-addressed receipt a reader
  could independently re-verify.

That was the exact missing "dippable + receipt" rung for the labor market, and
it is buildable purely on the `packages/nip90` surface with no money, no live
market, and no other lane. The remaining DE-6 promises are either money/owner
gated (the capstone `monetize_any_layer_with_referral`, signature settlement) or
need runtime work outside this scope (compose-and-list runtime, agentic-npm /
WASM registries).

## What was built

All on the in-scope `packages/nip90` surface:

- **`packages/nip90/src/lbr-closeout.ts`** — `makeLbrLaborCloseout(...)` composes
  the four accepted LBR lifecycle events into one public-safe, content-addressed
  `LbrLaborCloseout` receipt. It:
  - re-decodes every event through the existing `./lbr` ref/payment-material
    guards (raw prompts, private paths, credentials, invoices, preimages, etc.
    are rejected here too);
  - checks the four events describe **one** job — every lifecycle event's
    `requestId` equals the request event id, the requester is the request
    author across quote/result, and the provider is the quote author across
    acceptance/result;
  - enforces the **quote ≤ request budget** bound;
  - hashes a deterministic canonical projection of the public-safe refs
    (`canonicalLbrLaborCloseout`) into a SHA-256 `digest`, and binds a stable
    ref-only `receiptRef` of `lbr-closeout:<requestId>:<digest>`.
  - `verifyLbrLaborCloseoutDigest(...)` re-derives the digest and `receiptRef`
    from the receipt's own fields — the **dereference check** that lets any
    reader confirm the receipt binds the exact lifecycle that produced it, and
    fails closed on any tamper.
- **`packages/nip90/src/lbr-closeout.test.ts`** — 9 tests covering composition,
  determinism, canonical-order independence, tamper detection (ref + amount +
  receiptRef), over-budget rejection, one-job consistency, party mismatch,
  wrong-kind rejection, and the inherited ref-only safety guard.
- **`packages/nip90/scripts/lbr-closeout-proof.ts`** — the labor-market analogue
  of `nip-ds.ts`. Offline by default: it **signs** a real LBR lifecycle with
  deterministic keys, composes the closeout, and prints the receipt with
  `closeoutDereferenced: true`. With `--relay <url>` it additionally publishes
  the four events to a scoped market relay and reads them back, proving the
  lifecycle is dippable on the relay before the closeout binds it.
- **`packages/nip90/src/index.ts`** — exports the new closeout surface.
- **`packages/nip90/README.md`** — documents the closeout contract and proof.
- **`packages/nip90/tsconfig.json`** — typecheck now covers `scripts/**`.

It is protocol-only and ref-only throughout: it moves no sats, opens no escrow,
creates no invoice, publishes nothing private, and grants no settlement
authority. Settlement authority stays in the platform receipt systems, per
`docs/nips/LBR.md`.

## The dereferenceable receipt (the proof)

`bun packages/nip90/scripts/lbr-closeout-proof.ts` (offline) produces a real
signed lifecycle and a content-addressed closeout, e.g.:

- four real signed Nostr events, kinds `[5934, 7000, 7000, 6934]`, each with a
  real 32-byte hex event id;
- `receiptRef: lbr-closeout:<requestId>:<digest>` where `<requestId>` is the
  signed request event id and `<digest>` is the SHA-256 over the canonical
  ref-only projection;
- `closeoutDereferenced: true` — `verifyLbrLaborCloseoutDigest` re-derived the
  digest and receiptRef from the receipt's own public-safe fields and they
  match.

This is the labor-market shape of DE-6's "dippable market + dereferenceable
receipt" rung, at the protocol/receipt layer. The relay accepts kinds
5934/7000/6934 under `apps/nostr-relay/src/market-policy.ts`
(`marketKindBucket`), so `--relay` makes the same lifecycle dippable on a live
scoped relay.

## State after this change

- `markets.open_protocol_markets.v1` — **STAYS `planned`.** The labor market now
  has a composed, content-addressed, dereferenceable closeout receipt and a
  signed proof, matching the receipt shape the data (NIP-DS) market already has.
  No blocker is cleared: liquidity and risk remain skeleton-only, and
  compute/data are not broadly live paid markets. Green still requires real
  participant transactions plus settlement receipts across all six markets.
- No other promise changes state. Zero green flips.

## What remains for the green flip (owner-gated / out of this scope)

The single remaining step this scope can flag, not perform:

1. **Owner**: run a real labor job through a live scoped relay with a real
   requester and a real independent provider, escrow real budget through the
   platform ledger, and record the green-flip transition receipt with **owner
   sign-off** per `proof.claim_upgrade_receipts.v1`. Building out the liquidity
   and risk markets and broad live compute/data markets is the larger remaining
   DE-6 work for the *unified six-market* green, tracked by the promise's own
   blockers.

## Verification

From the repo root:

- `bun run --cwd packages/nip90 test` — 17 pass (incl. the 9 new closeout
  tests).
- `bun run --cwd packages/nip90 typecheck` — clean (now incl. `scripts/**`).
- `bun run --cwd apps/nostr-relay test` — 28 pass.
- `bun test apps/pylon/tests/nip90-import.test.ts` — 1 pass (export surface
  intact).
- `bun packages/nip90/scripts/lbr-closeout-proof.ts` — prints a signed
  lifecycle and a closeout with `closeoutDereferenced: true`.

No deploy, no money movement, no registry state flip.
