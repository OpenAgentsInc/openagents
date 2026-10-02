---
id: openagents.wallet-trust
version: 2
kind: product
title: "Who you rely on with the Spark wallet"
summary: >-
  The Wallet runs on Spark, whose operators are run by Lightspark, Breez, and
  Flashnet; the i button shows the trust note.
tags: [wallet, spark, trust, custody, risk]
applies_when: >-
  The user asks whether the Wallet is custodial or self-custodial, who holds
  their bitcoin, what Spark is, or what happens if a company stops.
answer: >-
  The Wallet runs on Spark, not on a Lightning node of your own, and your keys
  stay on your phone and any computer you link to it. Three companies run Spark's operators: Lightspark,
  Breez, and Flashnet. Two must cooperate for payments off the chain, and your
  safety depends on at least one having deleted old keys, which no one can
  check. If the operators stop, you can still withdraw on-chain yourself,
  though it can take days. The Wallet's info button shows this note.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-mobile/src/wallet.rs
    - docs/breez/wallet-design.md
    - bins/openagents-ios/README.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-10-02: version 2 says a linked computer holds the keys too (#10202)."
---

## Answer

The Wallet runs on Spark, not on a Lightning node of your own, and your keys stay on your phone and any computer you link to it. Three companies run Spark's operators: Lightspark, Breez, and Flashnet. Two must cooperate for payments off the chain, and your safety depends on at least one having deleted old keys, which no one can check. If the operators stop, you can still withdraw on-chain yourself, though it can take days. The Wallet's info button shows this note.

## Details

- An on-chain exit needs a separate on-chain payment for fees.
- Lightning payments go through Lightspark.
- Keep amounts you'd be comfortable carrying in a phone wallet.

## Sources

- `crates/openagents-mobile/src/wallet.rs`
- `docs/breez/wallet-design.md`
- `bins/openagents-ios/README.md`
