# Next Step: Forfeitable Provider Bonds for Agent-to-Agent Labor

Date: 2026-06-30

Status: design + spec for the next step toward agents transacting naturally with
each other (Lightning / Nostr / forfeitable funds). This doc flips no promise
state, changes no runtime authority, and broadens no public copy. It defines the
bounded unit to build and resolves a standing policy contradiction between two
owned NIPs before any code lands.

## Why this is the next step

We already have two working agent-to-agent (A2A) payment paths:

- **NIP-90 / LBR labor market over Nostr** — ref-only negotiation, sats move
  out-of-band. Protocol in `nostr-effect/src/core/Nip90.ts`; OpenAgents wrapper
  in `packages/nip90/src/lbr.ts` and `lbr-closeout.ts`; provider invoice mint in
  `apps/pylon/src/provider-nip90.ts`; buyer dispatch in
  `apps/openagents.com/workers/api/src/buy-mode-dispatcher.ts`.
- **L402 / MPP HTTP-402 paywall** — an agent pays for Khala inference
  (`apps/openagents.com/workers/api/src/inference/mpp/*`).

Sats actually move on **Spark (primary, offline receive)** and **MDK
(checkouts/treasury + Lightning fallback)**; BOLT12 is a payout-target kind only.

The labor-market escrow lifecycle is built and proven on a no-spend credit
ledger: **reserve → release → refund** (`apps/openagents.com/INVARIANTS.md`,
"Labor Escrow Credit Ledger"; `apps/pylon/src/labor-market.ts`). 1-sat jobs have
settled end-to-end on the ledger (P6/P7).

**The one axis the design genuinely lacks is forfeitability.** Today escrow can
only go back to the payer (refund) or forward to the worker on accepted work
(release). There is no terminal state where staked funds are *forfeited* on
validated non-performance. That is exactly the primitive "agents transact
naturally, with forfeitable funds" requires — a counterparty can trust an agent
because the agent has *skin in the game* that it loses if it defects.

The transport for a bond amount already exists but is inert:
`nostr-effect/src/wrappers/nip69.ts` (NIP-69 kind-38383 carries a `bond` tag).
There is no escrow/forfeit logic behind it.

### Why not jump straight to Lightning / Ark forfeiture

- **Lightning hold-invoice forfeiture is blocked on rail support.** Real
  HODL/hold-invoice forfeit paths exist only in read-only reference code
  (`projects/ldk/repos/rust-lightning` manual `claim_funds`/`fail_htlc_backwards`;
  `projects/mutiny/repos/fedimint` `forfeit_message()`/`verify_forfeit_signature()`).
  Whether Spark/MDK expose hold invoices is unverified. `mutiny-node` only
  *detects/avoids* hold invoices.
- **Ark/VTXO forfeit transactions are a much larger lift** gated on a whole new
  `projects/ark` integration (`bark/lib/src/forfeit.rs`), already sketched in
  `docs/2026-06-09-ark-mdk-agent-payments-audit.md` as a later target.

So the minimal, groundable unit that makes funds genuinely forfeitable *today* is
the **credit-ledger `forfeit` state plus a typed bond contract**, with a clean
settlement-adapter seam toward Lightning and Ark later.

## The policy contradiction this must resolve first

`docs/nips/AC.md` ("Agent Credit") **deliberately rejects slashing**: "failure is
handled via reputation decay and limit reductions, not token slashing," and it
defines a soft-escrow Cancel Window (kind 39246). A forfeitable bond is, by
definition, a (soft) slashing primitive. Shipping it without scoping would put
two owned NIPs in direct contradiction.

**Resolution (the decision this doc makes):** the two primitives govern
*different stakes on different sides of the trade*, and do not overlap.

| Primitive | Whose money | Failure handling | Governs |
| --- | --- | --- | --- |
| **Agent Credit (AC.md)** | buyer-side credit / spending limits | reputation decay + limit reduction, **never slashed** | a requester's standing and ability to commission work |
| **LBR Provider Bond (this doc)** | provider-side performance stake, posted voluntarily to win a job | **forfeitable** on validator-confirmed non-performance | a provider's skin-in-the-game for a specific accepted job |

A provider *opts in* by posting a bond to make its quote more credible; it is not
a tax on credit. AC's reputation-only model is untouched. This scoping is the
load-bearing reason the bond is safe to add.

## What to build (the bounded unit)

Build all four parts as one coherent step. Parts 1–2 are the core; 3 keeps it
future-proof; 4 makes a posted bond actually reachable.

### 1. Typed contract — `packages/nip90/src/lbr-bond.ts`

A sibling to `lbr.ts`, same discipline: **ref-only, decode-time rejection of all
payment material** (`lnbc`/`lno1`/`preimage`/`payment_hash`), Effect Schema,
kind-7000 (`KIND_JOB_FEEDBACK`) feedback variants carrying only refs and amounts.
Mirror the existing `LbrQuote`/`LbrUnsignedEventDraft` shapes.

```
LbrProviderBond            // posted alongside a quote; provider stakes a bond
  requestId, providerRef
  bondMsats                // amount (no invoice/preimage)
  bondReceiptRef           // ref to the off-event escrow/receipt that holds it
  forfeitDestination       // "refund_payer" | "counterparty" | "burn"
  forfeitConditionRef      // ref to the typed condition (e.g. verifier failure)
  expiresAt?

LbrBondRelease             // terminal: provider performed -> bond returns to provider
  requestId, bondReceiptRef, releaseReceiptRef, authorityRef

LbrBondForfeit             // terminal: validated non-performance -> bond forfeited
  requestId, bondReceiptRef, forfeitReceiptRef, forfeitDestination,
  forfeitConditionRef, authorityRef
```

Extend `lbr-closeout.ts` so the bond outcome (`released` | `forfeited`) is bound
into the content-addressed closeout digest. Add `lbr-bond.test.ts` covering:
decode rejection of payment material, ref-only round-trip, terminal-state
exclusivity (a bond is released XOR forfeited), and digest binding.

### 2. Ledger — add a `forfeit` terminal state

Extend the worker escrow lifecycle behind `apps/openagents.com/INVARIANTS.md`
"Labor Escrow Credit Ledger" from `reserve → release → refund` to
`reserve → release → {refund | forfeit}`:

- `forfeit` moves the held `bond` `held_msat` to the `forfeitDestination`
  (counterparty or a burn sink) instead of back to the payer.
- **Authority invariants (non-negotiable):** only validator non-acceptance can
  trigger `forfeit`; the worker can neither self-release nor self-forfeit;
  transitions are fail-closed and idempotent; the "not settled bitcoin until a
  payout receipt records settlement evidence" invariant still holds — `forfeit`
  on the credit ledger is *not* an on-chain/Lightning movement.

### 3. Settlement-adapter seam

Define the bond settle/forfeit destination as a pluggable adapter:

```
BondSettlementAdapter {
  hold(bondMsats, ...): BondReceiptRef
  release(receiptRef, ...): ReleaseReceiptRef
  forfeit(receiptRef, destination, ...): ForfeitReceiptRef
}
```

Today's only implementation is the credit ledger. Future implementations target
Spark/MDK Lightning hold invoices, then the Ark forfeit-transaction path from
`docs/2026-06-09-ark-mdk-agent-payments-audit.md`. The contract and ledger never
import a rail directly; they speak to this interface, so the rail decision stays
deferred and reversible.

### 4. Couple to the P1 relay → DB offer bridge

A posted bond is only meaningful once relay quotes become API offers. The
negotiation chain currently breaks at `relay quote → (no bridge) → API offer`:
`recordForumWorkRequestOffer` has zero production callers and nothing subscribes
to the relay for kind-7000 quotes
(`docs/labor/2026-06-14-p1-live-provider-quote-and-offer-bridge-gap.md`). Land the
minimal relay→DB ingestion wire as part of this step so a live posted bond is
actually ingestible. Without it, the bond is a schema with no path to the ledger.

## Phased plan

| Phase | Deliverable | Acceptance |
| --- | --- | --- |
| FB-1 | `packages/nip90/src/lbr-bond.ts` + tests | **landed 2026-06-30 in package contract**: ref-only round-trip; payment-material rejected at decode; release XOR forfeit; closeout digest binds bond outcome; `bun run --cwd packages/nip90 test` and `bun run --cwd packages/nip90 typecheck` green |
| FB-2 | Ledger `forfeit` terminal state + invariants | **landed 2026-06-30 in the Worker credit ledger**: validator-only forfeit; worker/provider/requester cannot self-trigger; fail-closed, idempotent; refund/release after forfeit cannot move balances; counterparty vs burn destination covered; INVARIANTS.md updated with regression coverage |
| FB-3 | `BondSettlementAdapter` seam + credit-ledger impl | adapter interface; ledger adapter passes; no rail imported; no-spend invariant preserved |
| FB-4 | Minimal relay→DB quote/offer ingestion (P1) | `recordForumWorkRequestOffer` has a live caller; a relay kind-7000 quote+bond becomes an API offer; existing labor-market tests stay green |

Each phase lands with its package/worker tests **and** `check:deploy` green, and
updates `apps/openagents.com/INVARIANTS.md` where it touches escrow authority. No
public copy or promise state changes until a deployed, live-smoke-proven claim
goes through the normal promise-evidence gate. FB-1 and FB-4 are independent and
can run in parallel; FB-2 depends on FB-1's contract; FB-3 wraps FB-2.

## Non-goals (for this step)

- No real Lightning hold-invoice creation/forfeiture (blocked on rail support;
  reference-only impls exist).
- No Ark/VTXO forfeit transactions (larger lift; later target).
- No change to Agent Credit's reputation-only model (AC.md stays as-is).
- No on-chain or Lightning settlement of the bond — credit-ledger `forfeit` only,
  behind the adapter seam.
- No vendoring of `projects/*` reference code (fedimint, rust-lightning, bark).

## References

Owned, extensible:

- `packages/nip90/src/lbr.ts`, `lbr-closeout.ts` — LBR contract patterns to mirror.
- `nostr-effect/src/core/Nip90.ts` — NIP-90 job/feedback kinds and schemas.
- `nostr-effect/src/wrappers/nip69.ts` — existing `bond` tag transport (NIP-69).
- `apps/pylon/src/labor-market.ts` — escrow reserve/release/refund today.
- `apps/openagents.com/INVARIANTS.md` — "Labor Escrow Credit Ledger", "Open Labor
  Market Pylon Gates".
- `docs/nips/LBR.md`, `docs/nips/AC.md` — the two NIPs reconciled above.
- `docs/labor/2026-06-10-open-agent-labor-market-roadmap.md` — market roadmap.
- `docs/labor/2026-06-14-p1-live-provider-quote-and-offer-bridge-gap.md` — the P1
  bridge gap this step closes.
- `docs/2026-06-09-ark-mdk-agent-payments-audit.md` — Ark/MDK + forfeit-path
  future target.

Read-only reference (study, do not vendor):

- `projects/mutiny/repos/fedimint/modules/fedimint-lnv2-common/src/contracts.rs`
  — real forfeit-signature cancel path.
- `projects/ldk/repos/rust-lightning/lightning/src/ln/channelmanager.rs` —
  hold-invoice-equivalent via manual claim/fail + HTLC interception.
- `projects/ark/repos/bark/lib/src/forfeit.rs` — canonical Ark forfeit-tx + VTXO.

## Status

| Phase | Status |
| --- | --- |
| FB-1 — `lbr-bond.ts` contract + tests | implemented in `packages/nip90/src/lbr-bond.ts`; exported by `packages/nip90/src/index.ts`; closeout digest binding implemented in `packages/nip90/src/lbr-closeout.ts`; verified by `bun run --cwd packages/nip90 test` and `bun run --cwd packages/nip90 typecheck` |
| FB-2 — ledger `forfeit` terminal state | implemented in `apps/openagents.com/workers/api/src/labor-escrow.ts`; migration `0261_labor_escrow_forfeit.sql` widens the D1 CHECK state; invariant ledger updated; verified by `bun --cwd apps/openagents.com/workers/api test src/labor-escrow.test.ts src/labor-live-rehearsal.test.ts` and `bun run --cwd apps/openagents.com/workers/api typecheck` |
| FB-3 — `BondSettlementAdapter` seam | not started |
| FB-4 — relay→DB quote/offer bridge (P1) | not started |

Implementation is intended to run through the Khala→Pylon→Codex labor fleet (the
same no-spend lane this workspace uses), each phase running the package/worker
verifier before merge. The 2026-06-30 FB-1 pass was implemented directly in the
owner checkout after confirming Pylon fleet status was online and idle; it adds
no ledger state, no rail adapter, and no public promise upgrade.

The 2026-06-30 FB-2 pass is still credit-ledger-only. It adds no Lightning hold
invoice, no Ark/VTXO forfeit transaction, no payout settlement, and no public
promise upgrade. It gives the platform a durable, receipt-backed terminal
`forfeited` state so the later settlement adapter can target a real invariant
instead of inventing slashing semantics at the rail boundary.
