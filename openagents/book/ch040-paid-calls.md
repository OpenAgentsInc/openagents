# 40. Paid calls

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

Members can reach everyone. This chapter lets a member charge: an author sets
a price, and a call to a priced member is paid in Bitcoin at the moment it is
made.

## Draft

"Paid usage" in Episode 289 needs a definition. Here it means a call to a
member that has a price.

An author puts a price on an operation in the release. When a plan includes
that operation, the price is part of the plan, and you see it before the work
starts. The call is paid in sats over Lightning.

The payment sits in the request itself. The server answers that payment is
required and names the amount. The caller pays and retries with proof. The
archive first builds this in [Episodes 62](../../docs/transcripts/062.md) and
[63](../../docs/transcripts/063.md), where an agent pays an endpoint. The Coax
tree has the settle and verify steps of the current form. It works the same
whether the caller is a person's agent or another plugin.

Small prices need small payments. A call may cost a few sats or less than one.
Lightning can carry that, and where a single payment is too small to send,
calls are counted and settled in batches.

Many members will be free, and free members are still worth making. They earn
credit, and they build the reputation that makes a paid one credible.

Episode 290 describes a second way to pay, for people who don't hold Bitcoin
yet: a credits account filled by card, with a fee, and a discount for paying
in Bitcoin. That is a plan with open terms, and it is listed below. If we
offer it, the balance is service credit: small, and not withdrawable. A card
buys credit. It is never a rail an agent pays with.

A price is the author's claim about value. Part V is the buyer's way to check
it.

## What we build

A plan for what this chapter ships once the draft is approved:

- A price field in a release, shown in the plan before a paid call runs.
- Payment in the request: a payment-required answer, a Lightning payment, and
  a retry with proof.
- Batched settlement for calls too small to pay one at a time.

It builds on what exists today:

- [`coax/lib/x402`](../../coax/lib/x402)
- [`coax/lib/lightning/invoice.coax`](../../coax/lib/lightning/invoice.coax)
- [`coax/lib/lightning/pay.coax`](../../coax/lib/lightning/pay.coax)
- [`coax/lib/nostr/mkt`](../../coax/lib/nostr/mkt)
- [`coax/lib/gateway/rates.coax`](../../coax/lib/gateway/rates.coax)

On the protocol ([chapter 49](ch049-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Payment required, payment and settlement | `openagents.x402.v1` `PaymentRequired`, `PaymentPayload`, `SettleResponse`, kept exactly as x402 defines them ([spec 26.4](../../docs/protocol/openagents-protocol-spec.md#264-openagentsx402v1)) | HTTP 402 body; a native record in a kind 3188 envelope (NIP-X402) | Spec sketch; no `.proto` file. `coax/lib/x402` has the settle and verify steps |
| A price | `openagents.market.v1` `Offering` ([spec 26.2](../../docs/protocol/openagents-protocol-spec.md#262-openagentsmarketv1)) | Nostr kinds 3192 and 30192 (NIP-MKT) | Spec sketch; no `.proto` file. `coax/lib/nostr/mkt` handles the NIP's JSON |

Planned paths: `openagents/pay/price.coax`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 2:00, paid usage
  - [Episode 62, *Exploring L402*](../../docs/transcripts/062.md): payment in an HTTP request
  - [Episode 63, *Agent Pays L402 Endpoint*](../../docs/transcripts/063.md): an agent pays an endpoint
  - [Episode 70, *L402 Plugin Deployment*](../../docs/transcripts/070.md): deploying a paid plugin
  - [Episode 97, *User Pays Agent*](../../docs/transcripts/097.md): a user pays an agent
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 9:30, credits and a discount for Bitcoin
- **Old code at `coder-v1.0.0`, crates:** `x402`, `pay-host`, `commercial-accounts`, `commercial-spend`, `retail-service`
- **NIPs:** [NIP-X402](../../nostr/nips/openagents/NIP-X402.md), [NIP-MKT](../../nostr/nips/openagents/NIP-MKT.md), [NIP-EXT](../../nostr/nips/openagents/NIP-EXT.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P11

## Open questions for the owner

1. What can an author price: a call, a unit of work, a subscription?
2. Do we offer card-funded credits as Episode 290 describes, with a fee and a
   Bitcoin discount, and what are the numbers?
3. Is there a free amount of paid calls for new people, and who funds it?
