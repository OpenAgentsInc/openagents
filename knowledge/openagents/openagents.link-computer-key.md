---
id: openagents.link-computer-key
version: 2
kind: product
title: "Linking a computer's key to your trainer"
summary: >-
  Account > Trainer > Link a key plus microcoder xp link on the computer lets
  a computer's awards count toward your level.
tags: [trainer, xp, link, keys, computer, in-app]
applies_when: >-
  The user asks how to earn XP from a computer without moving their trainer
  key, or what Link a key does.
answer: >-
  To keep your trainer key on the phone and still earn XP from a computer, tap
  Account > Trainer > Link a key, enter the computer key's npub, then run
  `microcoder xp link --relay wss://relay.openagents.com --trainer <your
  trainer npub>` on the computer. Awards to the linked key then count toward
  your level, and neither secret key moves.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/verse/tutorial-quests.md
    - bins/openagents-ios/README.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-10-09: tagged in-app: its steps are screens of the OpenAgents app, so the website's chat never shows the answer whole."
---

## Answer

To keep your trainer key on the phone and still earn XP from a computer, tap Account > Trainer > Link a key, enter the computer key's npub, then run `microcoder xp link --relay wss://relay.openagents.com --trainer <your trainer npub>` on the computer. Awards to the linked key then count toward your level, and neither secret key moves.

## Details

- `microcoder xp link` signs with `~/.openagents/nostr/knowledge-key`, or the key `--key` names.

## Sources

- `docs/verse/tutorial-quests.md`
- `bins/openagents-ios/README.md`
