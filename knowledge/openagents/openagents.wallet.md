---
id: openagents.wallet
version: 1
kind: product
title: "The OpenAgents Wallet"
summary: >-
  The Wallet is a mainnet bitcoin wallet on Breez's Spark SDK, with keys on
  the phone, that receives, sends, and buys bitcoin.
tags: [wallet, bitcoin, spark, lightning, breez]
applies_when: >-
  The user asks what the Wallet is, what it can do, which network it uses, or
  whether it's real bitcoin.
answer: >-
  The Wallet is a bitcoin wallet on mainnet, built on Breez's Spark SDK, with
  its keys on your phone. You can receive by Lightning invoice, Spark address,
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
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

The Wallet is a bitcoin wallet on mainnet, built on Breez's Spark SDK, with its keys on your phone. You can receive by Lightning invoice, Spark address, or Bitcoin address; send to invoices, Lightning addresses, LNURL codes, npubs, and Spark or Bitcoin addresses; and buy bitcoin with dollars through MoonPay or Cash App. It's real bitcoin, so use amounts you can afford to lose.

## Details

- The network is fixed to Bitcoin mainnet in the app's code; nothing switches it.
- The Wallet is on iPhone and Android.
- Its seed is separate from the device, world, and computer wallet keys.

## Sources

- `bins/openagents-ios/README.md`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
- `INVARIANTS.md`
