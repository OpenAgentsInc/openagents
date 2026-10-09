# Proposal — Non-Custodial Agent Commerce

A four-document proposal for a payment rail where third-party **businesses integrate
to sell**, and OpenAgents **agents pay autonomously**, with settlement **directly
between agent and merchant** — no central party holding funds.

These are a deliberate *fork* of the custodial
[central-receive-and-splits design](../2026-10-02-central-receive-and-splits.md). The
central theme: almost nothing new is needed at the *protocol* layer —
[NIP-X402](../../../nips/openagents/NIP-X402.md) (payment),
[NIP-SOV](../../../nips/openagents/NIP-SOV.md) (identity/treasury),
[NIP-CAP](../../../nips/openagents/NIP-CAP.md) (spend grants),
[NIP-POL](../../../nips/openagents/NIP-POL.md) (approvals), and
[NIP-MKT](../../../nips/openagents/NIP-MKT.md) (listings) already compose into it. The
work is *deployment, tooling, trust services, and the buyer-facing app*.

## The documents

| # | Document | What it covers |
| --- | --- | --- |
| 1 | [Non-custodial agent-commerce rail](2026-10-09-non-custodial-agent-commerce-rail.md) | The proposal, both ends. Why non-custodial; what is reused vs. changed; **merchant end** (onboarding, listing, direct settlement, liquidity); **agent/owner end** (identity, budget-as-capability, approval gates, wallet); the hard problems (splits, liquidity, escrow); fee model; phased rollout; and the governing custodial-vs-non-custodial decision. |
| 2 | [X402 merchant-receiver deployment profile](2026-10-09-x402-merchant-receiver-profile.md) | The normative deployment spec. Why per-merchant receivers are *required* (a shared custodial key for untrusted tenants is non-compliant per NIP-X402); the eight MUST requirements; three non-custodial topologies (self-hosted, managed facilitator, OpenAgents-as-LSP); the per-merchant CAP discovery descriptor; conformance. |
| 3 | [Merchant-facilitator API sketch (Rust)](2026-10-09-x402-merchant-facilitator-api-sketch.md) | How a merchant assembles `crates/x402` for direct settlement. Grounded in the real crate (sync, generic-on-store, `Arc<dyn>` hooks, `thiserror`). Reuse table; the base case as pure assembly; new pieces (`MerchantSink`, `BuyerAttestation`, `FeePolicy`, `DelegatedReceiver`); request lifecycle; the two small core changes needed. |
| 4 | [Agent ownership & mobile purchase approval](2026-10-09-agent-ownership-and-mobile-approval.md) | The buyer's view: *how do I know an agent is mine* (NIP-SOV authority binding, rooted in host admission — not a login), and *how do I approve purchases from a phone* (NIP-HOST enrollment → CAP/SOV budget → NIP-POL single-use approval → SOV guardian threshold). The to-build list: x402↔POL wiring, verification view, grant management, mobile push. |

## Fixtures

- [`fixtures/buyer-attestation-v1.json`](fixtures/buyer-attestation-v1.json) — test
  vectors for the merchant-side `BuyerAttestation::verify` hook (doc 3, §2): 12
  `(input → verdict)` cases covering the happy path and nine refusal codes, with a
  fail-fast evaluation order. See [`fixtures/README.md`](fixtures/README.md) for the
  verdict table.

## Suggested reading order

1 → 2 → 3 is the *build* path (why → deployment contract → code). Read **4** alongside
1 for the consumer/identity side; it stands on its own if ownership and the approval
UX are your main concern.

## Status

All four are **drafts for discussion**, merged as a proposal (PR #11088).
The governing decision below is made: **coexistence**. The BuyerAttestation
fixtures are the starting test vectors for the verifier.
OpenAgents supports every agent payment protocol; this rail is how third-party
merchants get paid directly. See [Agent payments: pay any way](../agent-payments.md). Nothing here is implemented; NIP-X402 itself
is "Designed, not implemented." The single decision that gates everything is in
proposal #1, §11.1: whether this external, non-custodial rail is a *separate* product
alongside the existing custodial plugin-payout ledger, or a replacement. The
recommended posture is **coexistence** (see profile #2, §10).
