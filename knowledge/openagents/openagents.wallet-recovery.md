---
id: openagents.wallet-recovery
version: 2
kind: product
title: "Backing up and restoring the wallet"
summary: >-
  The wallet's backup is its recovery words, shown behind a warning; a wallet
  restores from 12 or 24 words, and OpenAgents never asks for them.
tags: [wallet, backup, recovery, seed, restore, security, in-app]
applies_when: >-
  The user asks how to back up or restore their wallet, where their recovery
  words or seed phrase are, or whether to share them.
answer: >-
  Your wallet's backup is its recovery words. In the Wallet, Show recovery
  words reveals them behind a warning; write them down and keep them offline.
  You can restore a wallet from 12 or 24 words. We'll never ask for your
  recovery words, and no one from OpenAgents will; anyone who has them has
  your bitcoin.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/roadmap/2026-09-29-launch-roadmap.md
    - docs/game/playtesting.md
    - crates/openagents-mobile/src/wallet.rs
    - INVARIANTS.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-10-09: tagged in-app: its steps are screens of the OpenAgents app, so the website's chat never shows the answer whole."
---

## Answer

Your wallet's backup is its recovery words. In the Wallet, Show recovery words reveals them behind a warning; write them down and keep them offline. You can restore a wallet from 12 or 24 words. We'll never ask for your recovery words, and no one from OpenAgents will; anyone who has them has your bitcoin.

## Details

- The seed is kept in its own this-device-only Keychain item on iPhone and under its own Keystore key on Android.
- The words are shown only after an explicit request and a confirmed warning, never in a log or file.

## Sources

- `docs/roadmap/2026-09-29-launch-roadmap.md`
- `docs/game/playtesting.md`
- `crates/openagents-mobile/src/wallet.rs`
- `INVARIANTS.md`
