---
id: openagents.world-key
version: 2
kind: product
title: "The world key and the trainer key"
summary: >-
  The Grid signs with a separate world key, which is also the trainer key XP
  belongs to, kept apart from the device key.
tags: [keys, verse, trainer, world-key, privacy, in-app]
applies_when: >-
  The user asks what key the Grid or Verse uses, what the trainer key is, what
  the code over their head means, or why they have more than one key.
answer: >-
  The Grid uses a separate key, the world key, which signs your presence
  there; its first characters are the tag over your head. It's also your
  trainer key, which your XP and level belong to. It's kept apart from the
  device key that holds access to your computers, so other players can't link
  the two. You can reveal it in Account > Trainer, behind a warning.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/verse/mobile.md
    - bins/openagents-ios/README.md
    - INVARIANTS.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-10-09: tagged in-app: its steps are screens of the OpenAgents app, so the website's chat never shows the answer whole."
---

## Answer

The Grid uses a separate key, the world key, which signs your presence there; its first characters are the tag over your head. It's also your trainer key, which your XP and level belong to. It's kept apart from the device key that holds access to your computers, so other players can't link the two. You can reveal it in Account > Trainer, behind a warning.

## Details

- The world key is its own Keychain item, this device only; if Keychain can't provide it, the world stays offline.
- Reports sent with Report a problem are signed by the world key too, so XP they earn lands on the key over your head.

## Sources

- `docs/verse/mobile.md`
- `bins/openagents-ios/README.md`
- `INVARIANTS.md`
