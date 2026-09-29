---
id: openagents.earn-xp
version: 2
kind: product
title: "How to earn trainer XP"
summary: >-
  Trainer XP comes from checks and adoptions of your Gym results and tools,
  and from six tutorial quests, 50 XP each; XP is a record, not money.
tags: [xp, quests, tutorial, trainer, gym]
applies_when: >-
  The user asks how to earn XP, level up, or complete quests, or whether XP is
  worth money.
answer: >-
  You earn trainer XP in the Gym, from chat: when another trainer's check
  confirms a result you added, when you check someone else's result, and when
  Coder adopts your tool. Our six tutorial quests also pay 50 XP each, if you
  have a desktop with Docker and the command line. XP is a record, not money.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/verse/tutorial-quests.md
    - docs/verse/agent-trainer-leveling.md
    - docs/coder/guides/xp.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-29: version 2 (#9941) adds the eval-check and eval-adopt XP from the Gym in chat (docs/coder/guides/xp.md, live since #9938 and #9935), and words the tutorial quests in the app's plain words (CHK-02)."
---

## Answer

You earn trainer XP in the Gym, from chat: when another trainer's check confirms a result you added, when you check someone else's result, and when Coder adopts your tool. Our six tutorial quests also pay 50 XP each, if you have a desktop with Docker and the command line. XP is a record, not money.

## Details

- In the first Gym quests, a confirmed check is worth 50 XP to the checker and 25 each to the trainer who ran the result and the test set's author; an adoption is worth 200, 100, and 50.
- Each tutorial quest repeats one of our published coding results on your own desktop and sends a signed record of it; the six together are worth 300 XP, level 3, and a run costs about a cent.
- The tutorial season `tb21-tutorial-s1` closes 2026-12-25, and each quest pays once per person.

## Sources

- `docs/verse/tutorial-quests.md`
- `docs/verse/agent-trainer-leveling.md`
- `docs/coder/guides/xp.md`
