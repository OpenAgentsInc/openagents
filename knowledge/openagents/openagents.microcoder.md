---
id: openagents.microcoder
version: 2
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
  - "2026-10-02: BYOK (#10176): the cloud fallback runs on the person's own keys when they chose them."
---

## Answer

Microcoder is our experimental coding loop: it combines Jev's typed judgments, a model, and a shared knowledge base. It replaced our earlier Microluna loop, and its retained Terminal-Bench passes are what the tutorial quests ask you to reproduce.

## Details

- Its XP commands (`microcoder xp`) publish reproductions and link computer keys.
- When no coding agent signed in on your computer has room, its steps run on the OpenAgents cloud; with your own keys on (Use my keys for everything), they run on `openai/gpt-6.1-sol` through your OpenRouter key, then your Vercel AI Gateway key, never on ours (#10176).

## Sources

- `README.md`
- `AGENTS.md`
- `docs/verse/tutorial-quests.md`
