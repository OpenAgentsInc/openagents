---
id: openagents.app-tabs
version: 2
kind: product
title: "The app's four tabs"
summary: >-
  The OpenAgents app has four tabs: Chat, Verse, Wallet, and Account.
tags: [tabs, navigation, chat, verse, wallet, account]
applies_when: >-
  The user asks what the app's tabs or sections are, where something is in the
  app in general, or what each tab does.
answer: >-
  The app has four tabs. Chat (the message icon) is where you talk with us and
  start work on your computers. Verse (the globe) is the Grid, a shared world
  you walk around in. Wallet is a bitcoin wallet. Account holds your
  computers, the Tailnet screen, identity keys, your trainer card, playtest
  tools, and the changelog.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - bins/openagents-ios/README.md
    - crates/openagents-mobile/src/account.rs
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-29: version 2 (#9941) says the Chat tab opens on the menu from build 21 (crates/openagents-mobile/src/account.rs); the answer is unchanged."
---

## Answer

The app has four tabs. Chat (the message icon) is where you talk with us and start work on your computers. Verse (the globe) is the Grid, a shared world you walk around in. Wallet is a bitcoin wallet. Account holds your computers, the Tailnet screen, identity keys, your trainer card, playtest tools, and the changelog.

## Details

- From build 21, Chat opens on a menu: your trainer name, level, and next step, with **Chat with OpenAgents** first, starter questions, **Profile**, and the Gym in the Verse.
- Verse shows the Grid, Verse's bare world, with other players and shared physics objects.
- Wallet runs on Breez's Spark SDK on Bitcoin mainnet.
- Account holds Computers, Tailnet, Identity keys, Trainer, Playtest, Report a problem, **My reports**, About this device, and Changelog, and links to the source code and to OpenAgents on X.

## Sources

- `bins/openagents-ios/README.md`
- `crates/openagents-mobile/src/account.rs`
