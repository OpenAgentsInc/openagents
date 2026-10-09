# X402 Merchant-Hosted Receiver — deployment profile (draft, 2026-10-09)

**Decision (owner, 2026-10-09):** adopted as the merchant-settlement profile for third-party sellers, coexisting with the custodial receiver for our own sales. OpenAgents supports every agent payment protocol; see [Agent payments: pay any way](../agent-payments.md).

**Status:** Draft deployment profile for discussion. No protocol change; no new
event kind. Not implemented.
**Defines:** How a third-party merchant runs its *own* X402 receiver and
facilitator so that agents settle **directly to the merchant**, with no central
party holding funds.
**Normative base:** [NIP-X402](../../../nips/openagents/NIP-X402.md) (the spec is
normative; this profile only constrains *deployment*). Companion to the
[non-custodial agent-commerce rail proposal](2026-10-09-non-custodial-agent-commerce-rail.md).
Contrasts with the custodial [central-receive-and-splits design](../2026-10-02-central-receive-and-splits.md).

Key words MUST / SHOULD / MAY are used as in the shared contracts.

---

## 1. Why this profile exists (it is what the spec already requires)

NIP-X402 is explicit that a receiver key is per-merchant-scope and that a shared
custodial key for untrusted tenants is **non-compliant**:

> "The receiver key MUST have exclusive invoice-issuance authority for the resource
> server's admitted merchant scope. A shared custodial key under which untrusted
> tenants can issue invoices is not compatible with this method. Nostr signatures
> cannot fix that attack. Hosts MUST pin the receiver-key association and its
> operational assurance; discovery alone is insufficient. All facilitators serving
> that receiver MUST share one authoritative replay store, including across native,
> HTTP, and MCP entrances."
> — NIP-X402, *Roles and identities*

The existing [central receiver](../2026-10-02-central-receive-and-splits.md) ("One
receiver: ours") is compliant **only because OpenAgents is the single, trusted
resource server** for every endpoint it fronts. The moment the rail admits
*third-party merchants* as tenants, a single shared receiver key becomes exactly
the attack the spec forbids: one tenant could issue invoices in another's scope,
and the central party holds everyone's sats.

Therefore a multi-merchant, non-custodial rail does not merely *prefer* a
per-merchant receiver — **the X402 security model requires one.** This document
specifies that deployment.

---

## 2. What does and does not change vs. the custodial model

| Concern | Central-receive (existing) | Merchant-hosted receiver (this profile) |
| --- | --- | --- |
| `payTo` in the challenge | OpenAgents node key | **The merchant's** node key |
| Invoice-issuance authority | One OA key for all endpoints | **One key per merchant scope** (spec-mandated) |
| Replay store | One central store | **One store per receiver** (at/for the merchant) |
| Custody of funds | OA holds, splits, pays out later | **None — sats land at the merchant at settlement** |
| Split ledger + payout worker | `pay-ledger` + Spark/LN payout | Not used; merchant already paid (see §7) |
| Handshake, wire rules, units | NIP-X402 | **Identical — unchanged** |
| Discovery | CAP `oa-x402-v1` | **Identical — CAP, with merchant's own `receivers`** |

Nothing in the *handshake* changes. This is a change of *who holds the receiver key
and the replay store*, which the spec already treats as per-merchant.

---

## 3. Normative requirements for a merchant deployment

A conforming merchant deployment (the "merchant resource server") MUST:

1. **Own its receiver key.** Control a Lightning node whose 33-byte compressed key
   is the `payTo` for its own merchant scope, with exclusive invoice-issuance
   authority for that scope (NIP-X402 *Roles and identities*). It MUST NOT issue
   invoices under another merchant's scope, and the rail MUST NOT let it.
2. **Run (or delegate) a facilitator with one authoritative replay store** for that
   receiver, shared across every entrance it exposes (`http:1`, `mcp:1`,
   `nostr:openagents:1`). The facilitator MAY run inside the provider process
   (NIP-X402 permits this).
3. **Settle before execute.** On the paid retry it MUST verify the preimage hashes
   to the invoice payment hash, verify the BOLT11 signature against its own `payTo`,
   verify amount, currency, creation time, expiry vs `maxTimeoutSeconds`, and that
   the single signed description hash equals the locally reconstructed request
   digest; then **call settle before executing** and MUST NOT call facilitator
   `/verify` (upfront flow).
4. **Insert the replay key exactly once** as `network + ":" + payment_hash`, and
   retain keys at least `invoice_end + skew + 3600` seconds (longer for unresolved
   attempts). A relay event log alone is NOT a replay store.
5. **Advertise via CAP, with its own receiver.** Publish a CAP `adapter` definition
   carrying the feature ID `oa-x402-v1` and exactly one `x402` object whose
   `receivers` list contains only `{network, pay_to}` pairs it controls, and a
   provider-scoped `merchant` identifier (≤128 ASCII bytes, a discovery identity,
   not a wallet credential). See §5.
6. **Keep fees out of the invoice amount.** "Fees are extra, never hidden in the
   invoice amount" (NIP-X402 *Payment, settlement, and execution state*). Platform/
   facilitation fees are a separate payment requirement (§7), never a markup folded
   into `payTo`'s invoice.
7. **Guard every entrance equally.** "The same paid operation must not be callable
   through an unguarded CJ/HTTP/MCP alias." Payment does not bypass ordinary
   resource permissions.
8. **Omit the Lightning payer field.** `SettlementResponse.payer` MUST remain
   absent; buyer identity comes from the separately verified Nostr key, never a
   fabricated Lightning payer.

A merchant deployment MUST NOT be advertised until it passes the NIP-X402
conformance cases (§8).

---

## 4. Deployment topologies

Three ways a merchant satisfies §3, from most to least self-operated. All three are
non-custodial — OpenAgents never holds the merchant's sats in any of them.

### 4A. Self-hosted receiver (reference)
The merchant runs its own Lightning node (`crates/wallet` with `--lsp mdk`, or any
node that can issue exact-amount, caller-supplied-description-hash invoices and
return preimages) and its own facilitator + replay store. The merchant's CAP
definition advertises its node key as `payTo`. This is the cleanest mapping to the
spec: exclusive invoice authority and a dedicated replay store, both at the
merchant.

> Reuse: the existing `crates/x402` facilitator (`Receiver` trait,
> `crates/x402/src/server.rs`) and `crates/wallet`'s `receive_exact`
> (`crates/wallet/src/ldk.rs`) run **at the merchant** instead of centrally.
> The splits doc already notes self-hosting "stays possible … the protocol is open."

### 4B. Managed facilitator, direct-to-merchant payout (zero-infra)
OpenAgents (or any operator) runs a **stateless** facilitator that issues the `402`
and verifies the proof, but the `payTo` resolves to the **merchant's** Lightning
address / node, so settlement lands at the merchant. The operator never holds a
balance; it only brokers the handshake.

Caveat to stay compliant: per §3.1 the receiver key must be the merchant's. A
managed facilitator MUST NOT issue invoices under a shared operator key for
untrusted merchants. It MAY issue under the merchant's delegated key / LN address
*only* where the merchant has granted exclusive scope and the association is pinned
(NIP-X402: "Hosts MUST pin the receiver-key association"). If that delegation cannot
be pinned, use 4A.

### 4C. OpenAgents-as-LSP (liquidity, not custody)
Orthogonal to 4A/4B. A merchant node needs inbound capacity to *receive* a BOLT11.
The repo already uses MoneyDevKit **LSPS4 just-in-time liquidity** (inbound on
demand, ~2% channel-open fee). OpenAgents offers that LSPS4 service to merchants.
This provides liquidity, not custody — sats still settle to the merchant's node.
(Note: a Spark/Breez wallet "can pay x402 sellers but cannot receive x402 payments";
a merchant receiver MUST be a Lightning node, not a Spark wallet.)

---

## 5. Discovery descriptor (per-merchant)

No new event kind. The merchant publishes a CAP `adapter` definition whose
`requires` includes `oa-x402-v1` and whose `binding_contract` has exactly one
`x402` object (NIP-X402 *Discovery*):

```jsonc
{
  "v": "openagents.x402-discovery.v1",
  "protocol": "x402-v2",
  "scheme": "exact", "asset": "BTC", "method": "bolt11", "flow": "upfront",
  "bindings": ["http:1"],                       // and/or mcp:1, nostr:openagents:1
  "receivers": [
    { "network": "lnbtc:000000000019d6689c085ae165831e93",  // mainnet
      "pay_to": "<MERCHANT 33-byte compressed node key>" }
  ],
  "merchant": "<provider-scoped opaque id, <=128 ASCII bytes>",
  "recovery": "none",                           // or provider-contract-v1 / native-record-v1
  "recovery_contract": null
}
```

Rules that matter for the rail (all from NIP-X402 *Discovery*):

- `receivers` MUST list only `{network, pay_to}` pairs the merchant controls; the
  challenge's chosen network + `payTo` MUST match an admitted descriptor pair.
- No invoice, preimage, purchase ID, account, request digest, or fee grant may
  appear in public tags or catalog metadata.
- A descriptor is "not a live challenge, price commitment, balance, spend approval,
  or proof that the receiver/facilitator is correctly operated." Buyers MUST still
  verify the provider-signed CAP definition and operation, and a new key/endpoint
  requires a new reviewed binding — "not automatic trust in an invoice or redirect."

For marketplace listing/RFQ of negotiated goods, layer NIP-MKT `3192`/`30192` on top
(the offering points at this same merchant receiver). Fixed-price API/data/compute
goods can skip straight to the `402`.

---

## 6. The flow (unchanged handshake, merchant `payTo`)

```
Agent (buyer, Nostr x-only key)        Merchant resource server (owns payTo + replay store)
  │  request operation (no payment)           │
  │───────────────────────────────────────────▶
  │  402 + challenge: x402 v2, exact/BTC/bolt11,│
  │     flow=upfront, payTo=<MERCHANT node key>,│   (HTTP: PAYMENT-REQUIRED header;
  │     BOLT11 exact msat, h = request digest   │    MCP: _meta["x402/payment"];
  │◀───────────────────────────────────────────    native: 3188 `challenge` record)
  │
  │  reconstruct + verify binding; check amount vs CAP `spend` grant + SOV treasury
  │  ceilings; if over threshold → POL approval / SOV guardian gate
  │  pay BOLT11 directly to MERCHANT ───────────▶  (sats settle at the merchant)
  │  obtain 32-byte preimage
  │
  │  retry request + PaymentPayload{invoice, payment:<preimage>}
  │───────────────────────────────────────────▶
  │                                             │  verify preimage→hash, sig vs payTo,
  │                                             │  amount, description-hash vs digest;
  │                                             │  settle (consume proof); insert
  │                                             │  replay key once; THEN execute
  │◀───────────────────────────────────────────  200 + result
```

Buyer-side authorization (CAP `spend` grant, SOV treasury ceilings, POL/guardian
gates) is specified in the companion proposal, §8. The merchant does not implement
it; it only presents its identity-bearing challenge and trusts the signed buyer
identity in the binding.

---

## 7. Fees and splits in this profile

- **Platform/facilitation fee:** a *separate* payment requirement in the same
  challenge (the agent pays the merchant and a small fee invoice to an OpenAgents
  node). This respects "fees are extra, never hidden in the invoice amount" and
  keeps OpenAgents out of custody. Settlement across the two invoices is
  all-or-nothing only at the application layer, not atomic at the Lightning layer.
- **Multi-party splits** (e.g. plugin author + resource owner): NOT solved
  atomically here. Options and the recommended v1 stance (single-payee native;
  fee-as-extra-requirement; defer Lightning-native atomic splits) are in the
  companion proposal §6. This profile deliberately scopes to **single-payee
  settlement to the merchant** as the compliant, custody-free base case.

---

## 8. Conformance

Before advertising any binding, a merchant deployment MUST pass the NIP-X402
conformance cases (*Bounds, receipts, and conformance*), notably:

- Upstream HTTP/MCP digest and signed-invoice vectors; malformed invoices; invalid
  signer / network / amount; missing binding; unknown profile; boundary expiry/skew;
  **actual-request recomputation** (reconstruct the operation server-side, never
  trust the client's echo); body/credential mutation and same-price cross-route
  proofs refuse before execution.
- If native: binding vectors, wrong signer/recipient/account, missing signed input,
  changed request under the same purchase (`idempotency_conflict`), fresh envelope
  with stable artifact, same-request concurrent challenges, distinct-purchase
  isolation.
- Cross-process proof races; crash before/after each atomic write; restore and
  facilitator failover; `duplicate_settlement`; response loss; paid-result recovery.
  **Payment/intent commit must not permit duplicate execution.**
- Expired-but-paid refusal (a late paid claim may remain spent but unadmitted —
  reconciliation, not an automatic new payment); two paid invoices for one native
  purchase (`purchase_already_admitted`, consume at most one); no bearer proof in
  tags, logs, exported traces, zaps, or model context.

The shared [binding vectors](../../protocol/fixtures/x402-lightning-bindings-v1.json)
check canonical digest inputs only — they are not invoice-signature, wallet,
replay-store, or paid-service conformance. Each merchant receiver + replay store
needs its own operational assurance (NIP-X402: "Hosts MUST pin the receiver-key
association and its operational assurance").

---

## 9. Explicitly out of scope / excluded

- **Escrow, credit, streaming, postpaid, automatic refund, delivery guarantee.**
  NIP-X402 supplies none; "A provider can receive payment and then fail. The client
  MUST admit that risk before paying." Non-custodial escrow (HODL/PTLC) is a future
  profile — see companion proposal §9.
- **Cashu / nutzaps (NIP-60/61)** and **L402/LSAT** — NIP-X402 explicitly excludes
  them as implicit fallbacks. Not usable in this profile without a separate reviewed
  profile. *2026-10-09:* OpenAgents now accepts both as their own adapters on the
  payment router (L402 on the same invoice and replay key; Cashu with an accepted-mint
  list); see [agent payments](../agent-payments.md). A merchant may add them the same way.
- **Shared custodial receiver key for untrusted tenants** — non-compliant by §1; the
  whole reason this profile exists.
- **MKT/LAB post-acceptance labor payment** — unchanged; this profile is for a single
  exact operation purchased before execution.

---

## 10. Relationship to the existing central receiver

This profile does not delete the central receiver. The recommended posture
(companion proposal §11, decision 1) is **coexistence**:

- Keep the custodial `central-receive-and-splits` path for **internal** plugin
  payouts and first-party endpoints, where OpenAgents *is* the trusted resource
  server and a single receiver key is compliant and operationally simpler.
- Use this **merchant-hosted receiver** profile for **external third-party**
  merchants, where the spec requires per-merchant receiver keys and where
  non-custody is the product requirement.

The two share the same NIP-X402 handshake, the same CAP discovery mechanism, and the
same buyer-side authorization (SOV/CAP/POL). They differ only in who holds the
receiver key and the replay store, and therefore in whether any party holds funds.

---

## 11. Open items

1. **Receiver-key delegation for 4B.** Can a managed facilitator issue under a
   merchant's delegated key with a *pinnable* association, or must zero-infra
   merchants always use an LN-address forward? Needs a concrete delegation record.
2. **LSPS4 as a merchant service (4C).** Commit OpenAgents to running the LSP for
   merchant inbound, or document a bring-your-own-liquidity requirement?
3. **Per-merchant operational assurance.** The spec demands pinned receiver-key
   association and operational assurance; define the minimum attestation a merchant
   must present before its binding is advertised in discovery.
4. **Fee-requirement ergonomics (§7).** Standardize how the platform-fee invoice is
   carried alongside the merchant invoice in each of the three transport bindings.
