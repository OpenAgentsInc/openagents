# 32. A wallet

> **Status: partly implemented.** The self-custodial Breez Spark + NIP-06
> Bitcoin wallet runs on iOS ([`apps/shared/host/apple/SparkWallet.swift`](../../apps/shared/host/apple/SparkWallet.swift)),
> Desktop ([`apps/openagents-desktop/page/walletpane.coax`](../../apps/openagents-desktop/page/walletpane.coax),
> [`Window+Wallet.swift`](../../apps/openagents-desktop/Window+Wallet.swift),
> [`Window+WalletRPC.swift`](../../apps/openagents-desktop/Window+WalletRPC.swift)),
> and CLI ([`apps/openagents-cli/wallet.coax`](../../apps/openagents-cli/wallet.coax))
> over `#authority` money rules in [`pay/wallet.coax`](../../pay/wallet.coax),
> [`pay/wallet-policy.coax`](../../pay/wallet-policy.coax),
> [`pay/wallet-backup.coax`](../../pay/wallet-backup.coax), and
> [`pay/wallet-rpc.coax`](../../pay/wallet-rpc.coax) (`cpu` checks pass).
> Remaining: bringing the shared wallet screens to the web surface and
> full cross-host Lightning Address parity.

## Premise

The agent can call any command. This chapter adds the member that holds money:
a Bitcoin wallet for every person and every agent, with receive, send, a
balance and a history.

## Draft

In [Episode 290](../../docs/transcripts/290.md): underlying all of this, every
person and every agent gets their own Bitcoin wallet. Everything in the market chapters (53 to 61)
depends on that. An author can't be paid, and an agent can't buy a call, until
both ends have a wallet.

The wallet does four things. It gives you a way to be paid. It pays an invoice
or an offer. It shows a balance. And it lists what came in and went out.

We use Bitcoin rails only. This book builds on Bitcoin and Lightning. The
protocol's amount type also leaves room for Bitcoin-based stablecoins on
Taproot Assets, and for nothing off Bitcoin: no other chains, and no ecash for
now.

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
The owner narrowed that on 2026-11-11: one seed is an opt-in for people and
the default for agents. Chapter 59 builds on that.

The wallet is self-custodial. Its seed is made on your device. We store an
encrypted backup that we can't open, so a new phone can restore it. The only
balance we hold for you is service credit, which is small and can't be
withdrawn. You can claim `name@openagents.com` as a Lightning Address, and
money sent to it lands in your own wallet, never in ours. These are owner
decisions ([wallets and custody](../../docs/auth/wallets-and-custody.md),
[Lightning Addresses](../../docs/auth/lightning-addresses.md)).

The wallet exists on both Desktop and iOS. The shared Apple Breez Spark host
([`apps/shared/host/apple/SparkWallet.swift`](../../apps/shared/host/apple/SparkWallet.swift))
powers the iPhone wallet and the Desktop sidebar Wallet pane
([`apps/openagents-desktop/page/walletpane.coax`](../../apps/openagents-desktop/page/walletpane.coax),
[`Window+Wallet.swift`](../../apps/openagents-desktop/Window+Wallet.swift),
[`Window+WalletRPC.swift`](../../apps/openagents-desktop/Window+WalletRPC.swift)),
alongside `openagents wallet` in [`apps/openagents-cli/wallet.coax`](../../apps/openagents-cli/wallet.coax):
receive by invoice, on-chain or Spark address, send, a balance, a history,
the encrypted backup and a Lightning Address claimed in one tap. Swift talks
to the Spark SDK and nothing else. Every rule about money is Coax in
[`pay/`](../../pay): [`pay/wallet.coax`](../../pay/wallet.coax) decides
whether a quote may be paid and how a payment's state moves,
[`pay/wallet-rpc.coax`](../../pay/wallet-rpc.coax) gates local RPC calls
with `#authority` checks, and
[`pay/wallet-policy.coax`](../../pay/wallet-policy.coax) holds the daily
limit on the server. `OA_PACK_WALLET` in
[`openagents/core/members.coax`](../../openagents/core/members.coax) mounts
`WalletStatus`, `WalletReceive`, `WalletQuote`, and bypass-immune `WalletPay`
into the unified agent (`#14057`). What remains is bringing the same wallet
screens to the web.

The Coax tree also has the Lightning primitives from Bitcoin From Scratch:
invoices, offers, onion routing, channels and payment. A node of our own
built on them is a later option, not the first wallet.

## What we build

A plan for what this chapter ships once the draft is approved:

- The existing wallet as a member: receive, send, balance and history, with
  amounts as whole sats, over the rules in `pay/`.
- The wallet commands in the command line, and the same wallet screens on
  the web, sharing the iPhone app's actions (chapters 2 and 11).
- A test wallet on a test network, so every later chapter can run payments
  without real money.
It builds on what exists today:

- [`apps/shared/host/apple/SparkWallet.swift`](../../apps/shared/host/apple/SparkWallet.swift)
- [`apps/openagents-desktop/page/walletpane.coax`](../../apps/openagents-desktop/page/walletpane.coax), [`apps/openagents-desktop/Window+Wallet.swift`](../../apps/openagents-desktop/Window+Wallet.swift), and [`apps/openagents-desktop/Window+WalletRPC.swift`](../../apps/openagents-desktop/Window+WalletRPC.swift)
- [`apps/openagents-cli/wallet.coax`](../../apps/openagents-cli/wallet.coax)
- [`openagents/core/members.coax`](../../openagents/core/members.coax) (`OA_PACK_WALLET`, `#14057`)
- [`pay/wallet.coax`](../../pay/wallet.coax)
- [`pay/wallet-policy.coax`](../../pay/wallet-policy.coax)
- [`pay/wallet-backup.coax`](../../pay/wallet-backup.coax)
- [`pay/wallet-rpc.coax`](../../pay/wallet-rpc.coax)

- [`coax/lib/lightning/invoice.coax`](../../coax/lib/lightning/invoice.coax)
- [`coax/lib/lightning/offers.coax`](../../coax/lib/lightning/offers.coax)
- [`coax/lib/lightning/pay.coax`](../../coax/lib/lightning/pay.coax)
- [`coax/lib/bitcoin`](../../coax/lib/bitcoin)
- [`docs/lightning/README.md`](../../docs/lightning/README.md)
- [`docs/lightning/breez-spark.md`](../../docs/lightning/breez-spark.md)
- [`bitcoin/book/README.md`](../../bitcoin/book/README.md)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Balance, invoice and history | `openagents.payment.v1` `WalletView`, `Invoice`, `PaymentReceipt`; amounts are `openagents.common.v1` `Amount` ([spec 26.5](../../docs/protocol/openagents-protocol-spec.md#265-openagentspaymentv1), [spec 9.8](../../docs/protocol/openagents-protocol-spec.md#98-amounts)) | HTTP | Spec sketch; no `.proto` file |
| A Lightning Address | `openagents.payment.v1` `LightningAddress` ([spec 26.5](../../docs/protocol/openagents-protocol-spec.md#265-openagentspaymentv1)); the lookup and callback are LNURL's | HTTP: `/.well-known/lnurlp/NAME` | Built ([Lightning Addresses](../../docs/auth/lightning-addresses.md), #13208): forward mode, and the iPhone wallet's own address, claimed with its Spark key. LNURL is an external format |

### The chapter's check

- **Source:** the wallet member over `pay/wallet.coax` and
  `pay/wallet-policy.coax`, with a fake wallet service in place of the
  Spark SDK. It replays recorded answers on a test network, so no real
  money moves.
- **Trace:** `balance` prints `5000 sats`. The wallet makes a Lightning
  invoice for 1000 sats, and the fake service records it paid. `history`
  lists the payment in, and `balance` prints `6000 sats`. Every amount is
  whole sats.
- **Refusal:** a quote above the daily limit in `pay/wallet-policy.coax` is
  refused with a reason that names the limit, and the fake service records
  no send.
- **Independent check:** the invoice is decoded by
  `coax/tests/lightning/bolt11.py`, a BOLT 11 reader that shares no code
  with Coax. It gives the same amount and payment hash as the trace.

Planned paths: none. The wallet is the iPhone app's Spark wallet and the rules
in `pay/`; this chapter adds no `openagents/wallet/` copy.

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

1. Do we offer a way to buy Bitcoin with a card, as Episode 290 shows, and
   through whom?
