# 41. Metering and receipts

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

Paid calls happen. This chapter counts them in a way nobody has to take on
trust: every paid call writes a signed receipt, and the totals can be
recomputed by anyone.

## Draft

An author who is paid "for all paid usage" has one question: how do I know the
count is right? On a closed platform you can't know. Here the count is built
from receipts.

A receipt is a signed record of one paid call. It names the release that was
called, by hash. It gives the price, the time, and the split of the price
among the people owed. It carries proof of the payment. It does not carry what
the call was about. Your request and the result stay private.

A meter turns receipts into totals: per release, per author, per day. The
meter is a plain program over public records, so an author can run it and get
the same total we do. [Episode 37](../../docs/transcripts/037.md) asks for
this at the start: a flow of funds with public accounting.

Three cases need rules. A call that fails after payment is refunded or not
charged, and the receipt says which. A plugin that calls another paid plugin
produces a chain of receipts, and each author is owed for their link. And our
own use is marked: calls made by us, by our agents and by tests are excluded
from the numbers that describe real use, though authors are still paid for
them if they were paid calls.

Receipts are also what the reputation of chapter 44 reads. Paid use is
evidence that people found a member worth money, and it is costly to fake.

## What we build

A plan for what this chapter ships once the draft is approved:

- A receipt record for every paid call: release hash, price, split and payment
  proof, with no private content.
- A meter that computes totals from receipts, shipped so that authors can run
  it themselves.
- Rules for failed calls, chained calls and our own use.

It builds on what exists today:

- [`coax/lib/x402`](../../coax/lib/x402)
- [`coax/lib/nostr/event_sign.coax`](../../coax/lib/nostr/event_sign.coax)
- [`coax/lib/quantity.coax`](../../coax/lib/quantity.coax)
- [`coax/lib/gateway/usage-observation-checks.coax`](../../coax/lib/gateway/usage-observation-checks.coax)

Planned paths: `openagents/pay/receipt.coax`, `openagents/pay/meter.coax`.

## Sources

- **Episodes:**
  - [Episode 37, *Flow of Funds*](../../docs/transcripts/037.md): flow of funds with public accounting
  - [Episode 95, *Streaming Money*](../../docs/transcripts/095.md)
  - [Episode 96, *Payments & Payouts*](../../docs/transcripts/096.md)
  - [Episode 171, *Visualizing Agent Payments*](../../docs/transcripts/171.md)
  - [Episode 252, *Observer, Automated QA*](../../docs/transcripts/252.md): exact receipts
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 2:00, the dots going back and forth
- **Old code at `coder-v1.0.0`, crates:** `receipts`, `pay-ledger`, `commercial-spend`, `hook-log`
- **NIPs:** [NIP-X402](../../nostr/nips/openagents/NIP-X402.md), [NIP-XP](../../nostr/nips/openagents/NIP-XP.md), [NIP-MKT](../../nostr/nips/openagents/NIP-MKT.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P11

## Open questions for the owner

1. How public is the ledger: every receipt, or totals per release with
   receipts shown only to the author and the payer?
