---
id: openagents.microcoder
version: 4
kind: product
title: "Microcoder"
summary: >-
  Microcoder is the coding engine built into Coder, combining Jev,
  generation, and a shared knowledge base; its recorded passes back the
  tutorial quests.
tags: [microcoder, coder, agent, terminal-bench]
applies_when: >-
  The user asks what Microcoder is, or how it relates to Coder, Microluna, or
  the tutorial quests.
answer: >-
  Microcoder is the coding engine built into Coder: it combines Jev's typed
  judgments, a model, and a shared knowledge base. It installs as part of
  Coder, with nothing separate to download. It replaced our earlier Microluna
  loop, and its recorded Terminal-Bench passes are what the tutorial quests
  ask you to reproduce.
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
  - "2026-10-09: version 3 (#11031) says it in plain words, without internal terms."
  - "2026-10-09: version 4: it is part of Coder, installed with it, not a separate product or download (scripts/install/coder.sh, crates/openagents-web/src/pages/download.rs)."
---

## Answer

Microcoder is the coding engine built into Coder: it combines Jev's typed judgments, a model, and a shared knowledge base. It installs as part of Coder, with nothing separate to download. It replaced our earlier Microluna loop, and its recorded Terminal-Bench passes are what the tutorial quests ask you to reproduce.

## Details

- Its XP commands (`microcoder xp`) publish reproductions and link computer keys.
- When no coding agent signed in on your computer has room, its steps run on the OpenAgents cloud; with your own keys on (**Use my keys for everything**), they run on `openai/gpt-6.1-sol` through your OpenRouter key, then your Vercel AI Gateway key, never on ours (#10176).

## Sources

- `README.md`
- `AGENTS.md`
- `docs/verse/tutorial-quests.md`
