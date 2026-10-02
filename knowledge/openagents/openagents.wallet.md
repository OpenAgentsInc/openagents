---
id: openagents.wallet
version: 2
kind: product
title: "The OpenAgents Wallet"
summary: >-
  The Wallet is a mainnet bitcoin wallet on Breez's Spark SDK, with keys on
  the phone and any computer linked to it, that receives, sends, and buys
  bitcoin.
tags: [wallet, bitcoin, spark, lightning, breez]
applies_when: >-
  The user asks what the Wallet is, what it can do, which network it uses,
  whether it's real bitcoin, or whether the wallet on their computer is the
  same as the one on their phone.
answer: >-
  The Wallet is a bitcoin wallet on mainnet, built on Breez's Spark SDK, with
  its keys on your phone and on any computer you link to it, so you have one
  balance everywhere. You can receive by Lightning invoice, Spark address,
  or Bitcoin address; send to invoices, Lightning addresses, LNURL codes,
  npubs, and Spark or Bitcoin addresses; and buy bitcoin with dollars through
  MoonPay or Cash App. It's real bitcoin, so use amounts you can afford to
  lose.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - bins/openagents-ios/README.md
    - docs/roadmap/2026-09-29-launch-roadmap.md
    - INVARIANTS.md
    - docs/breez/README.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-10-02: version 2 says the same wallet runs on linked computers, per the owner's decision 8 in docs/breez/README.md (#10202)."
---

## Answer

The Wallet is a bitcoin wallet on mainnet, built on Breez's Spark SDK, with its keys on your phone and on any computer you link to it, so you have one balance everywhere. You can receive by Lightning invoice, Spark address, or Bitcoin address; send to invoices, Lightning addresses, LNURL codes, npubs, and Spark or Bitcoin addresses; and buy bitcoin with dollars through MoonPay or Cash App. It's real bitcoin, so use amounts you can afford to lose.

## Details

- The network is fixed to Bitcoin mainnet in the app's code; nothing switches it.
- The Wallet is on iPhone and Android.
- Its seed is separate from the device and world keys and from the x402 receiver's Lightning node on computers.
- On a computer, `openagents wallet` is the same wallet: `balance`, `address`, `receive`, `send`, and `history`. `openagents wallet link` brings the wallet from the phone after you approve it there and check a code; `openagents wallet restore` takes the recovery words instead.

## Sources

- `bins/openagents-ios/README.md`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
- `INVARIANTS.md`
- `docs/breez/README.md`
