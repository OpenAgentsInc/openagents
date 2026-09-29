---
id: openagents.wallet-amounts
version: 1
kind: product
title: "Why amounts show as whole ₿ numbers"
summary: >-
  Amounts use BIP 177: one bitcoin (₿1) is the smallest unit, so 0.00010000
  BTC shows as ₿10,000; Show amounts as switches to legacy BTC.
tags: [wallet, amounts, bip177, units, btc]
applies_when: >-
  The user asks why the Wallet shows amounts like ₿10,000, what the ₿ numbers
  mean, how they relate to BTC, or how to change how amounts are shown.
answer: >-
  We show bitcoin amounts in BIP 177 form: ₿1 is the smallest unit, the one
  once called a satoshi, so 0.00010000 BTC shows as ₿10,000. The legacy BTC
  form keeps its meaning, 100,000,000 units to 1 BTC. Show amounts as in the
  Wallet switches between the two, and the balance always shows both.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/breez/amounts.md
    - INVARIANTS.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

We show bitcoin amounts in BIP 177 form: ₿1 is the smallest unit, the one once called a satoshi, so 0.00010000 BTC shows as ₿10,000. The legacy BTC form keeps its meaning, 100,000,000 units to 1 BTC. Show amounts as in the Wallet switches between the two, and the balance always shows both.

## Details

- In BIP 177 mode, typed amounts are whole units; in legacy mode, decimal BTC with at most eight decimals.
- The choice applies to every screen at once.

## Sources

- `docs/breez/amounts.md`
- `INVARIANTS.md`
