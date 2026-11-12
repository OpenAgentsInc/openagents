# 42. Payouts to authors

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

Receipts say who is owed. This chapter pays them: a payout runner sends each
author their share over Lightning, on a schedule, and publishes what it sent.

## Draft

[Episode 289](../../docs/transcripts/289.md) shows dots moving across the
screen and names them: Lightning micropayments, where you get paid for all
paid usage of plugins that you create. This is promise P11.

A payout runner reads the meter's totals. For each author it computes what is
owed since the last payout and takes out what was already sent. If the amount
is above a minimum, it pays the author's own Lightning Address or offer. An
author can use an address they already have or claim `name@openagents.com`.
Either way the money lands in the author's own wallet (chapter 19). The runner
records each payout as a signed record that cites the receipts it covers.

The runner has to be boring. A payout has an ID, and running the same payout
twice sends nothing the second time. A payment that fails is retried later and
is never dropped. An author with no way to be paid keeps a balance until they
add one. The sum of payouts and balances always equals the sum of author
shares in the receipts, and anyone can check that.

We have run payouts before and learned from each attempt. [Episode
93](../../docs/transcripts/093.md) pays four builders by a daily process that
was partly manual. [Episode 98](../../docs/transcripts/098.md) moves to
minute-by-minute balances. [Episode 223](../../docs/transcripts/223.md)
reports payouts as the bottleneck once many people were owed. The lesson is to
automate it fully and keep each step checkable.

The share itself is not ours to draft. The split between the author, any
authors of plugins theirs depends on, and us is an owner decision. This
chapter builds the machinery so that the split is a number in one signed
policy record.

## What we build

A plan for what this chapter ships once the draft is approved:

- A payout runner as a Coax binary: totals in, Lightning payments out, a
  signed record of each.
- Repeat-safe payouts, retries, and held balances for authors with no address
  yet.
- A public check that payouts plus balances equal the author shares in the
  receipts.

It builds on what exists today:

- [`coax/lib/lightning/offers.coax`](../../coax/lib/lightning/offers.coax)
- [`coax/lib/lightning/pay.coax`](../../coax/lib/lightning/pay.coax)
- [`coax/lib/lightning/invoice.coax`](../../coax/lib/lightning/invoice.coax)
- [`coax/lib/bitcoin/treasury.coax`](../../coax/lib/bitcoin/treasury.coax)
- [`coax/lib/nostr/nip57.coax`](../../coax/lib/nostr/nip57.coax)

On the protocol ([chapter 49](ch049-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| What an author is owed and was paid | `openagents.ledger.v1` `Earnings`, `Statement` ([spec 26.6](../../docs/protocol/openagents-protocol-spec.md#266-openagentsledgerv1-and-openagentsbillingv1)) | HTTP; a signed payout record on Nostr | Spec sketch; no `.proto` file |
| Where to pay an author | `openagents.payment.v1` `LightningAddress` ([spec 26.5](../../docs/protocol/openagents-protocol-spec.md#265-openagentspaymentv1)) | HTTP: LNURL | Built in forward mode ([Lightning Addresses](../../docs/auth/lightning-addresses.md), #13208). LNURL is an external format |

Planned paths: `openagents/pay/payout.coax`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 2:00, Lightning micropayments to authors
  - [Episode 93, *The Sats Must Flow*](../../docs/transcripts/093.md)
  - [Episode 96, *Payments & Payouts*](../../docs/transcripts/096.md)
  - [Episode 98, *Agent Revenue Sharing*](../../docs/transcripts/098.md): revenue sharing
  - [Episode 99, *Lightning Addresses*](../../docs/transcripts/099.md)
  - [Episode 223, *Pay the People*](../../docs/transcripts/223.md)
  - [Episode 235, *Agents Earn Bitcoin Tips*](../../docs/transcripts/235.md): agents earn tips
- **Old code at `coder-v1.0.0`, crates:** `pay-ledger`, `pay-host`, `wallet`, `spark-wallet`, `receipts`
- **NIPs:** [NIP-X402](../../nostr/nips/openagents/NIP-X402.md), [NIP-XP](../../nostr/nips/openagents/NIP-XP.md), [NIP-KEYS](../../nostr/nips/openagents/NIP-KEYS.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P11

## Open questions for the owner

1. What is the author's share of each paid call? This is the audit's decision
   3.
2. What is the payout schedule and the minimum: for example, daily above a set
   number of sats?
3. When a plugin depends on another paid plugin, how is the price split
   between the two authors?
4. A tip goes straight from the payer to the maker's own wallet (owner
   decision D6 in [docs/auth](../../docs/auth/decisions.md), built in #13210). A paid call
   with a split can't always do that. Who holds the money between such a call
   and the payout, and under what custody?
