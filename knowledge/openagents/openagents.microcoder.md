---
id: openagents.microcoder
version: 1
kind: product
title: "Microcoder"
summary: >-
  Microcoder is the experimental coding loop that combines Jev, generation,
  and a shared knowledge base; its retained passes back the tutorial quests.
tags: [microcoder, coder, agent, terminal-bench]
applies_when: >-
  The user asks what Microcoder is, or how it relates to Coder, Microluna, or
  the tutorial quests.
answer: >-
  Microcoder is our experimental coding loop: it combines Jev's typed
  judgments, a model, and a shared knowledge base. It replaced our earlier
  Microluna loop, and its retained Terminal-Bench passes are what the tutorial
  quests ask you to reproduce.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - README.md
    - AGENTS.md
    - docs/verse/tutorial-quests.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

Microcoder is our experimental coding loop: it combines Jev's typed judgments, a model, and a shared knowledge base. It replaced our earlier Microluna loop, and its retained Terminal-Bench passes are what the tutorial quests ask you to reproduce.

## Details

- Its XP commands (`microcoder xp`) publish reproductions and link computer keys.

## Sources

- `README.md`
- `AGENTS.md`
- `docs/verse/tutorial-quests.md`
