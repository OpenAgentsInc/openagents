---
id: openagents.wallet-buy
version: 2
kind: product
title: "Buying bitcoin with dollars"
summary: >-
  Buy pays with dollars through MoonPay or Cash App on their own pages;
  on-chain deposits are claimed automatically.
tags: [wallet, buy, moonpay, cash-app, dollars, in-app]
applies_when: >-
  The user asks how to buy bitcoin, add funds, or use a card, dollars,
  MoonPay, or Cash App.
answer: >-
  In the Wallet, choose Buy and enter how much bitcoin you want. You pay with
  dollars through MoonPay or Cash App, which opens their page. On-chain
  deposits are claimed into your balance automatically when they mature.
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
  - "2026-10-09: tagged in-app: its steps are screens of the OpenAgents app, so the website's chat never shows the answer whole."
---

## Answer

In the Wallet, choose Buy and enter how much bitcoin you want. You pay with dollars through MoonPay or Cash App, which opens their page. On-chain deposits are claimed into your balance automatically when they mature.

## Details

- A purchase page opens only over https, and only once.
- A MoonPay purchase is paid to the wallet's own deposit address.

## Sources

- `bins/openagents-ios/README.md`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
- `INVARIANTS.md`
