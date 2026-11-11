# 19. A wallet

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

The agent can call any command. This chapter adds the member that holds money:
a Bitcoin wallet for every person and every agent, with receive, send, a
balance and a history.

## Draft

In [Episode 290](../../docs/transcripts/290.md): underlying all of this, every
person and every agent gets their own Bitcoin wallet. Everything in Part VI
depends on that. An author can't be paid, and an agent can't buy a call, until
both ends have a wallet.

The wallet does four things. It gives you a way to be paid. It pays an invoice
or an offer. It shows a balance. And it lists what came in and went out.

We use Bitcoin and Lightning only. There are no other chains and no other
tokens.

The archive has built this several times: an agent's balance in [Episode
42](../../docs/transcripts/042.md), Lightning withdrawals in [Episode
64](../../docs/transcripts/064.md), a phone wallet in [Episode
143](../../docs/transcripts/143.md), and a web wallet in [Episode
173](../../docs/transcripts/173.md), which the episode presents as a hot
wallet for small balances. That framing stays. This is a wallet for paying and
being paid in small amounts. It is not a place to keep savings, and the wallet
says so.

[Episode 207](../../docs/transcripts/207.md) gives the identity design: one
seed phrase, with your Nostr key and your wallet key both derived from it.
Chapter 43 builds on that.

The Coax tree has the Lightning primitives from Bitcoin From Scratch:
invoices, offers, onion routing, channels and payment. It does not have a
wallet yet. Building one is a plan, and the first open question below decides
what it is built on.

## What we build

A plan for what this chapter ships once the draft is approved:

- A wallet library: receive, send, balance and history, with amounts as whole
  sats.
- The wallet as a member, and the wallet commands in the command line.
- A test wallet on a test network, so every later chapter can run payments
  without real money.

It builds on what exists today:

- [`coax/lib/lightning/invoice.coax`](../../coax/lib/lightning/invoice.coax)
- [`coax/lib/lightning/offers.coax`](../../coax/lib/lightning/offers.coax)
- [`coax/lib/lightning/pay.coax`](../../coax/lib/lightning/pay.coax)
- [`coax/lib/bitcoin`](../../coax/lib/bitcoin)
- [`docs/lightning/README.md`](../../docs/lightning/README.md)
- [`docs/lightning/breez-spark.md`](../../docs/lightning/breez-spark.md)
- [`bitcoin/book/README.md`](../../bitcoin/book/README.md)

Planned paths: `openagents/wallet/`.

## Sources

- **Episodes:**
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 10:00, a wallet for every person and agent
  - [Episode 42, *Agent Bitcoin Balance*](../../docs/transcripts/042.md): an agent's Bitcoin balance
  - [Episode 64, *Lightning Withdrawals*](../../docs/transcripts/064.md)
  - [Episode 99, *Lightning Addresses*](../../docs/transcripts/099.md)
  - [Episode 143, *Onyx as Bitcoin Wallet*](../../docs/transcripts/143.md): a phone wallet
  - [Episode 169, *Agent Payments API*](../../docs/transcripts/169.md): an agent payments API
  - [Episode 173, *OpenAgents Bitcoin Wallet*](../../docs/transcripts/173.md): the web wallet
  - [Episode 207, *Your Keys, Your Coins, Your Identity*](../../docs/transcripts/207.md): keys, coins and identity
- **Old code at `coder-v1.0.0`, crates:** `wallet`, `spark-wallet`, `bitcoin-amount`, `pay-host`
- **NIPs:** [NIP-KEYS](../../nostr/nips/openagents/NIP-KEYS.md), [NIP-X402](../../nostr/nips/openagents/NIP-X402.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P8, P11, P14

## Open questions for the owner

1. What is the wallet built on: the Spark path the plan in `docs/lightning/`
   describes, our own Lightning node from `coax/lib/lightning`, or Spark first
   and our own later?
2. What do we say about custody? Episode 290 calls the current design
   "self-custodial-ish".
3. Do we offer a way to buy Bitcoin with a card, as Episode 290 shows, and
   through whom?
