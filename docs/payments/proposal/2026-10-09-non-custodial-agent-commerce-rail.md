# Non-Custodial Agent Commerce — a direct-settlement rail for businesses and agents (proposal, 2026-10-09)

**Decision (owner, 2026-10-09):** adopted as the merchant-settlement profile for third-party sellers, coexisting with the custodial receiver for our own sales. OpenAgents supports every agent payment protocol; see [Agent payments: pay any way](../agent-payments.md).

**Status:** Proposal / Draft for discussion. Not implemented.
**Scope:** A payment rail where third-party businesses integrate to sell goods and
services, and OpenAgents agents pay for them autonomously — settled **directly
between agent and merchant**, with no central party holding funds.
**Relationship to existing work:** This is a deliberate *fork* of the custodial
[central-receive-and-splits design](../2026-10-02-central-receive-and-splits.md). It
reuses [NIP-X402](../../../nips/openagents/NIP-X402.md) as the payment handshake and
[NIP-SOV](../../../nips/openagents/NIP-SOV.md) / [NIP-CAP](../../../nips/openagents/NIP-CAP.md)
/ [NIP-POL](../../../nips/openagents/NIP-POL.md) as the authorization layer. It changes
*who receives the sats*, not *how the handshake works*.

---

## 0. TL;DR

- NIP-X402 already describes a complete HTTP/MCP/Nostr payment handshake. Its
  `402` challenge names a `payTo` Lightning node key. **Nothing in the protocol
  requires that node to be OpenAgents'.** The custodial model is Decision P1 of the
  splits doc ("One receiver: ours"), a *deployment* choice.
- A non-custodial rail is therefore mostly a *deployment + onboarding + tooling*
  effort on top of X402, plus an authorization story (which SOV/CAP/POL already
  give us), not a new protocol.
- The genuinely hard, genuinely new problems are three: **(1) split payments**
  without a custodian, **(2) merchant inbound liquidity**, and **(3) dispute/
  escrow** — because final settlement to the merchant is irreversible.
- This contradicts the current documented owner direction. See
  [§9 Open decisions](#9-open-decisions-read-first). Resolve that before building.

---

## 1. Motivation and the custodial tension

OpenAgents already has a payment rail. Today it is **custodial by design**:

> "One receiver: ours. Every paid endpoint … issues its `402` from one
> OpenAgents-run Lightning node. Authors and providers never run a receiver; they
> register a payout destination." — [Splits doc, Decision P1](../2026-10-02-central-receive-and-splits.md)

The receiver is a `crates/wallet` `ldk-node`/MoneyDevKit node on `oa-pay-1`; a
single-writer SQLite ledger computes splits and a payout worker pushes sats to
registered destinations on a schedule. The risk section states plainly that
"OpenAgents receives every payment and holds authors' accrued shares until payout."
The owner's stated direction was: *receive it all through our setup.*

This proposal takes the opposite stance for a specific reason: **a rail that
third-party businesses integrate into is a different product from an internal
plugin-payout ledger.** External merchants are far more likely to integrate a rail
where:

- they are paid **directly and instantly** to their own node/wallet, with no
  counterparty holding their money between settlement and payout;
- there is **no money-transmitter / custody surface** on OpenAgents' side (the
  [later-markets](../later-markets.md) doc already notes production custody needs "a
  separately reviewed legal custodian … insolvency treatment, and an admitted
  resolver" — i.e. it is a regulated-business commitment, not a feature);
- the buyer is a **cryptographically identified, owner-authorized agent with a
  verifiable spend budget** — something card networks cannot offer agent traffic.

That last point is the moat, and it is already built (NIP-SOV identity + NIP-CAP
`spend` grants). The card networks answer "is this a real buyer who will pay?" with
chargebacks and bot-bans, both hostile to agents. We answer it with a signed
identity and a signed budget, and final Lightning settlement.

**Honest caveat:** going non-custodial trades away the thing custody makes easy —
atomic multi-party splits and a single reconciliation ledger (see §6). Keep the
custodial receiver for internal plugin payouts if it is working; this rail is for
*external* commerce.

---

## 2. Design principles

1. **Non-custodial by default.** Sats settle agent → merchant directly. OpenAgents
   provides identity, the checkout standard, discovery, and optional trust
   services — it is not in the money path.
2. **Reuse X402 verbatim.** The handshake, the `exact`/`bolt11` scheme, the
   msat units, the description-hash-binds-request-digest rule, settle-before-execute,
   and the replay store are all kept exactly as specified. Only `payTo` changes.
3. **Identity and authorization are already ours.** Agent identity = NIP-SOV
   durable key. Spend authority = NIP-CAP `spend` effect + NIP-SOV treasury policy.
   Human-in-the-loop = NIP-POL approvals and NIP-SOV guardian gates.
4. **Final settlement is a feature for merchants, a risk for buyers.** We balance
   it with opt-in, protocol-level dispute/refund records (NIP-MKT) and reputation
   (NIP-EVAL) — never by becoming a custodian.
5. **Standard, not silo.** X402 is an open, documented protocol; a merchant can
   speak it without us. Our value-add is identity attestation, discovery, liquidity,
   and trust — not lock-in.

---

## 3. What we reuse vs. what changes

| Layer | Existing primitive | Reuse as-is? | Change for non-custodial |
| --- | --- | --- | --- |
| Checkout handshake | NIP-X402 (`http:1`, `mcp:1`, `nostr:openagents:1`) | ✅ verbatim | `payTo` = merchant node, not OA node |
| Payment proof | BOLT11 preimage, replay key `network:payment_hash` | ✅ verbatim | Replay store lives at the merchant, not centrally |
| Agent identity | NIP-SOV `sovereign-profile.v1` (durable Nostr key) | ✅ | — |
| Spend authorization | NIP-CAP `grant.v1` `spend` effect + NIP-SOV `treasury` policy | ✅ | — |
| Human approval | NIP-POL `approval-request/decision.v1`, NIP-SOV guardian policy | ✅ | — |
| Listing / discovery | NIP-MKT `3192`/`30192` offerings, RFQ, `order`/`order_ack` | ✅ | Offering points at merchant-controlled receiver |
| Dispute / refund | NIP-MKT `dispute`, `refund_offer`/`refund_accept` | ✅ | Becomes load-bearing (no custodial clawback) |
| Settlement / splits | central receiver + SQLite split ledger | ❌ | Replaced — see §6 |
| Payout worker | `pay-ledger` + Spark/LN payout | ❌ not needed | Merchant is already paid directly |

The deletion of the central receiver and payout worker is the whole point: there is
nothing to hold, split, or pay out later because the merchant was paid at settlement.

---

## 4. The core handshake (shared by both ends)

Identical to NIP-X402, with `payTo` naming the merchant's node. Using the
`nostr:openagents:1` binding, which already carries both parties' keys and a
request digest:

```
Agent (buyer)                         Merchant (X402 facilitator at merchant)
  │  request resource (no payment)          │
  │────────────────────────────────────────▶
  │   402 Payment Required                  │   challenge: x402 v2, scheme=exact,
  │   PAYMENT-REQUIRED: <challenge>          │   asset=BTC, method=bolt11,
  │◀────────────────────────────────────────   paymentFlow=upfront
  │                                         │   BOLT11: exact msat, h=sha256(binding)
  │                                         │   payTo: <MERCHANT 33-byte node key>
  │
  │  1. verify binding: buyer/provider keys + requestDigest (NIP-X402 nostr binding)
  │  2. check against spend authority (CAP spend grant + SOV treasury ceilings)
  │  3. if over approval threshold → POL approval / SOV guardian gate (§8)
  │  4. pay the BOLT11 DIRECTLY to the merchant node  ──────────▶  (sats land at merchant)
  │     obtain 32-byte preimage
  │
  │  re-send request + PaymentPayload {invoice, payment: <preimage>}
  │────────────────────────────────────────▶
  │                                         │  verify preimage→payment_hash,
  │                                         │  BOLT11 sig vs payTo, amount,
  │                                         │  description hash vs request digest;
  │                                         │  settle (consume proof); insert
  │                                         │  replay key network:payment_hash;
  │                                         │  THEN execute and return 200
  │◀────────────────────────────────────────
  │
  │  both sides retain private 3188 artifacts (request/challenge/claim/status,
  │  NIP-44 v2 encrypted) as the per-purchase audit trail
```

Everything inside this box is already specified in NIP-X402 (it is marked
*Designed, not implemented*). The only substantive change from the custodial model
is in step 4: the invoice's `payTo` is the merchant's node, so the sats never touch
an OpenAgents wallet. Amounts remain millisatoshi strings; there is no refund or
delivery guarantee at the protocol layer ("A provider can receive payment and then
fail. The client MUST admit that risk before paying." — NIP-X402).

---

## 5. Part A — Merchant integration (the business end)

**Audience:** any business that wants to sell APIs, data, compute, or services to
agents and be paid directly.

### 5.1 The pitch
Agents are a buyer class the card networks reject (chargebacks, bot-bans, KYC
friction). This rail lets you accept them with **instant, final settlement to your
own node** and a **cryptographic guarantee the buyer is a real, owner-authorized
agent with a funded budget** (NIP-SOV identity + NIP-CAP spend grant presented in
the request binding). No custodian sits between you and your money.

### 5.2 Onboarding — three tiers, mirroring what already exists
The repo already supports merchant onboarding; we extend it to point at
merchant-controlled receivers rather than the central one:

1. **Plugin authors** — publish a signed EXT release carrying `fee_msat` and a
   `payout`/receiver field (today the `payout` is a payout *destination*; here it
   becomes the **node that issues and receives the 402** for that plugin).
2. **Hosted-resource merchants** — extend the existing
   `openagents x402 publish --upstream URL --price-sats N` CLI with a
   `--receiver <node>` (own node) or `--receiver managed` (OpenAgents-provided
   liquidity, §5.5) flag, then `openagents x402 advertise --slug NAME` to publish
   the NIP-MKT discovery record (`3192`/`30192`) pointing at that receiver.
3. **Zero-infra merchants** — a hosted facilitator that issues 402s *on behalf of*
   a merchant but pays out to the merchant's Lightning address immediately per
   settlement (a thin, stateless forward — not custody, since nothing is held).

### 5.3 Integration surface
- **Middleware / facilitator library** (Rust first, to match `crates/`): wraps a
  route, issues the X402 challenge with `payTo` = merchant node, verifies the
  preimage on retry, enforces the replay store, settles-before-execute. This is the
  existing `crates/x402` facilitator run *at the merchant* instead of centrally.
- **Listing** via NIP-MKT `3192` (immutable offering: `provider`, `offer`,
  `capability` CAP ref, `payment_profiles`, `price_hint_msat` advisory,
  `valid_until`) and a mutable `30192` head (`active`/`paused`/`withdrawn`).
- **Discovery / RFQ** for negotiated goods: buyer validates the `30192` head →
  resolves `3192` → sends a private `3188` RFQ → merchant returns an immutable
  quote → `order`/`order_ack`. Fixed-price API goods skip straight to the 402.

### 5.4 Settlement (the non-custodial boundary)
You price in msat (the only unit X402 and MKT accept). The agent pays your BOLT11
directly; the preimage is your proof of payment. Converting sats → fiat/stablecoin
happens at *your* exchange/PSP, outside this rail. **OpenAgents never holds, splits,
or reconciles your funds** — which is precisely what removes the regulatory and
counterparty surface.

### 5.5 Merchant inbound liquidity (a real adoption blocker)
A merchant node must be able to *receive* a BOLT11 payment, which needs inbound
Lightning capacity. The repo already solves this for its own receiver with
**MoneyDevKit LSPS4 just-in-time liquidity** (inbound on demand, ~2% channel-open
fee). The natural productization: **OpenAgents offers LSPS4 liquidity to merchants**
as a paid service — we become the LSP, not the custodian. (Note: a Spark/Breez
wallet "can pay x402 sellers but cannot receive x402 payments" — merchants need a
real Lightning receiver, not a Spark wallet.)

---

## 6. The split-payment problem (the hard part — read carefully)

Custody makes splits trivial: one receiver gets everything and the ledger divides it
(the splits doc's `plugin_call` = author `fee` + OpenAgents `rest`; `hosted_resource`
= 90% owner / 10% OA). **Non-custodial settlement breaks this**, because a single
BOLT11 pays exactly one node. Honest options, worst-to-best for a first cut:

- **(A) Provider-forwarded obligation.** Merchant receives the full amount and owes
  the platform/author its share, recorded as a NIP-MKT/LAB obligation. *Reintroduces
  trust/custody at the merchant* — rejected as a default.
- **(B) Multiple payment requirements in one challenge.** X402's challenge can carry
  more than one requirement; the agent pays N BOLT11s to N `payTo` nodes (merchant,
  platform fee, author) and presents N preimages. Settlement is "all-or-nothing" only
  at the application layer (not atomic across invoices) — acceptable when the fee
  payee is OpenAgents (we can be lenient about ordering). **Recommended first cut.**
- **(C) Lightning-native split** (AMP/keysend to multiple recipients, or PTLCs).
  True atomic multi-destination settlement, but not expressible in the `exact`/BOLT11
  profile X402 defines today. A future profile, not v1.
- **(D) Cashu/ecash budgets.** Would be elegant for agent budgets, but NIP-X402
  **explicitly excludes** NIP-60/61 Cashu ("different proofs with mint trust … not
  an implicit fallback"). *Revisited 2026-10-09:* Cashu is accepted as its own
  adapter and challenge (NUT-24 `X-Cashu`), never as an x402 fallback; see
  [agent payments §7](../agent-payments.md#7-how-each-protocol-fits-nip-x402).

**Recommendation:** v1 handles the common case — **agent → one merchant, single
payee** — natively and perfectly. For the author-fee / platform-fee case, use **(B)
multiple requirements**, with the platform fee as a separate small invoice to an
OpenAgents node (§7). Defer true atomic splits to a later Lightning-native profile.
Splits are exactly where the custodial model earns its keep; be explicit that we are
trading atomic splits for non-custody.

---

## 7. Fee model (how the rail earns without touching funds)

A non-custodial rail cannot skim a balance it never holds. Revenue options, in
rough order of cleanliness:

1. **Facilitation fee as an extra X402 payment requirement** (§6B): the agent pays
   the merchant *and* a small platform invoice in the same challenge. Funds are
   never held — just a second direct payment.
2. **LSPS4 liquidity provider** (§5.5): be the LSP that gives merchants inbound
   capacity; earn the channel-open fee OpenAgents already pays today (~2%).
3. **Identity / attestation / trust services:** premium merchant-facing attestation
   ("this agent is owner-verified, treasury-backed, reputation ≥ N"), curated
   discovery registries (NIP-REG), and reputation feeds (NIP-EVAL).
4. **Hosted facilitator subscription** for zero-infra merchants (§5.2 tier 3).

All four avoid custody. (1) and (2) scale with volume; (3) and (4) are SaaS-like.

---

## 8. Part B — Agent / owner payer end

**Audience:** the agent (payer) and its human owner (the authority). This end is
almost entirely *already specified* — the work is composition, not invention.

### 8.1 Identity
The agent is a **NIP-SOV sovereign agent**: a durable Nostr key, governed by an
immutable `sovereign-profile.v1` naming its `authority`, `policy` (NIP-POL),
`custody` (NIP-CAP), and `treasury`. A key change "creates a new identity … existing
contracts and grants do not automatically transfer." Identity is presented to the
merchant inside the X402 `nostr:openagents:1` binding (`buyer` = agent key).

### 8.2 Spend authorization (the budget-as-capability)
Two existing mechanisms compose into exactly the "funded, bounded agent" the merchant
pitch promises:

- **NIP-CAP `grant.v1` with the `spend` effect.** Effects are granted *separately*
  (`reads`, `writes`, `network`, `process`, `delegates`, `spend`); `spend` is its own
  grant with `bounds`, `scope`, `expires_at`. A grant is "Host/operator policy, never
  supplied by the component itself" — the agent cannot widen its own budget.
- **NIP-SOV `treasury` policy.** Names the wallet adapter, authorizing principal,
  permitted payment profiles/destinations, **per-operation ceilings**, **aggregate
  allowance**, accounting period, fees, guardian requirements, and reconciliation
  authority. Child/delegated work "cannot evade the ancestor's budget or guardian
  gates."

Together these are the signed, owner-issued budget a merchant can rely on, and the
blast-radius bound an owner relies on: a compromised agent can spend at most its
ceilings/allowance, never the owner's wallet.

### 8.3 Human-in-the-loop gates
For spends above a threshold, two layered gates already exist:

- **NIP-POL exact-action approval:** the precise operation (op + binding + input +
  context + effects + bounds + nonce) is hashed into an `action.v1`; an
  `approval-request.v1` / `approval-decision.v1` pair authorizes *that exact action*,
  "single-use … consumed atomically," and "a later denial cancels still-unconsumed
  approval."
- **NIP-SOV guardian policy (threshold, multi-party):** `members` (1–32),
  `threshold`, and the `operations` (CAP refs) that require the gate. "Silence is not
  an approval"; any authenticated denial before consumption blocks admission.

Map these to spend tiers: under the treasury per-op ceiling → auto-pay; above it →
POL approval; high-value or sensitive categories → guardian threshold.

### 8.4 Wallet and the pay flow
- **Spending wallet:** Spark via the Breez SDK (`crates/spark-wallet`,
  `crates/openagents-mobile`) — "a Spark wallet can pay x402 sellers." Buyer-side
  wallet transport may also use **NWC (NIP-47)**, which X402 names as "the preferred
  optional Nostr wallet transport."
- **`pay()` client:** on a `402`, the agent parses the challenge, verifies the
  binding, checks the amount against its CAP `spend` grant and SOV treasury ceilings,
  auto-pays under threshold or raises a POL/guardian request over it, then retries
  with the preimage. Fully autonomous under budget; human-gated above it.
- **Audit:** every purchase leaves private `3188` request/challenge/claim/status
  artifacts (NIP-44 encrypted) — a built-in, per-purchase, owner-readable ledger. No
  separate spend-tracking system needed.

### 8.5 Owner console
Issue/revoke CAP spend grants and set SOV treasury ceilings; approve over-threshold
POL requests; manage guardian membership; review merchant reputation before adding a
merchant to permitted destinations. Revoking a grant or narrowing the treasury cuts
the agent off immediately.

---

## 9. Trust, dispute, and escrow

Because settlement is final and non-custodial, recourse must live above the payment:

- **Dispute / refund (NIP-MKT, exists):** `dispute` names the exact delivery /
  acceptance / payment / refund record; `refund_offer` → `refund_accept` creates a
  *new* reversed BOLT11 obligation (buyer and payee swapped). Critically: "This v1
  has no default arbitrator or automatic debit" and "a dispute or refund promise
  alone does not reverse a payment." Good enough for cooperative refunds; not an
  arbitration system.
- **Escrow (gap):** [later-markets](../later-markets.md) is explicit that production
  custody/escrow is unavailable and needs a reviewed legal custodian, deposit/
  release/refund rails, insolvency treatment, and an admitted resolver. **Non-
  custodial escrow** (Lightning HODL invoices or PTLCs releasing on fulfillment, with
  a timeout refund) is the right v2 direction because it keeps us out of custody —
  but it is real engineering and should be scoped separately, not promised in v1.
- **Reputation (partial):** NIP-EVAL verified-outcome records and the
  market-infrastructure ranking inputs ("verified outcomes … recent availability …
  full cost", with failures and refusals kept in the denominator) can seed a merchant
  reputation feed. Note XP "never converts" to money and the ledger never reads it.

**v1 posture:** bare final settlement for low-value, single-payee API/data/compute
purchases (where the buyer can absorb the risk, exactly as X402 already requires);
MKT dispute/refund for cooperative cases; escrow explicitly deferred.

---

## 10. Phased rollout

1. **Phase 0 — spec & decision.** Resolve §9/the custodial question (below). Write a
   short X402 *deployment profile* doc: "merchant-hosted receiver" (no protocol
   change; `payTo` = merchant). No code.
2. **Phase 1 — single-payee, self-hosted merchant.** Ship the `crates/x402`
   facilitator as a merchant-runnable library + the `--receiver <node>` publish flag.
   Agent `pay()` honoring CAP `spend` + SOV treasury. Prove it on an internal
   merchant paying itself directly (mirror the existing end-to-end demo, minus the
   central ledger).
3. **Phase 2 — zero-infra merchants + liquidity.** Hosted stateless facilitator
   (forward-to-LN-address) and OpenAgents-as-LSPS4-LSP for merchant inbound.
4. **Phase 3 — fees & trust.** Facilitation-fee requirement (§7), reputation feed
   (NIP-EVAL), curated discovery (NIP-REG).
5. **Phase 4 — splits & escrow.** Multiple-requirement splits (§6B) productized;
   a non-custodial escrow profile (HODL/PTLC) scoped and specced.

---

## 11. Open decisions (read first)

1. **Custodial vs. non-custodial — the governing conflict.** *Decided
   2026-10-09: coexistence (separate product alongside the custodial ledger).* The documented owner
   direction is custodial ("receive it all through our setup"). This proposal is the
   opposite. Decide explicitly: is the external merchant rail a *separate*
   non-custodial product alongside the internal custodial plugin-payout ledger, or a
   replacement? Everything else depends on this answer.
2. **Splits (§6):** accept single-payee-only in v1 with fee-as-extra-requirement, or
   invest in a Lightning-native atomic split profile sooner?
3. **Merchant liquidity (§5.5):** commit to being the LSPS4 LSP for merchants, or
   require merchants to bring their own inbound capacity?
4. **Volatility:** X402/MKT are msat-only with no USD peg or stablecoin fallback.
   Do external, USD-priced merchants need a quote/hedge layer, or do we stay sats-
   native and push FX to the merchant's PSP? (Recommend: stay sats-native in v1.)
5. **Escrow (§9):** confirm it is out of v1 scope; if not, it pulls a reviewed legal/
   custody commitment forward.

---

## 12. What this proposal deliberately does *not* invent

Worth stating, because the instinct is to design a new NIP. We do **not** need one:

- Payment handshake → **NIP-X402** (exists, designed).
- Agent identity → **NIP-SOV** (exists).
- Spend authorization → **NIP-CAP `spend` + NIP-SOV treasury** (exists).
- Approvals → **NIP-POL + SOV guardian** (exists).
- Listing/discovery/RFQ/orders → **NIP-MKT** (exists).
- Dispute/refund → **NIP-MKT** (exists).

The only new artifact is a **deployment profile** of X402 (merchant-hosted receiver)
plus tooling, liquidity, and trust services around it. That is the whole proposal.
