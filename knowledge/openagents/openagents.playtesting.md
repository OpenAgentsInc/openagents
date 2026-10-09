---
id: openagents.playtesting
version: 3
kind: product
title: "The playtesting program"
summary: >-
  The playtest is open to anyone; season 1 runs 2026-09-29 to 2026-10-26, and
  only accepted contributions earn XP, never money.
tags: [playtest, testing, season, beta]
applies_when: >-
  The user asks about the playtest or beta, how to join as a tester, what
  season 1 is, or whether testers get rewarded.
answer: >-
  Our playtest is open to anyone: chat with us on openagents.com, and get
  Coder, our coding agent for your terminal, at openagents.com/download
  for macOS, Linux, and Windows. Build the iPhone, Android, and desktop
  apps from source. Season 1 runs from 2026-09-29 to 2026-10-26. Joining
  earns nothing by itself; XP and titles come only from contributions we
  accept, such as reproducible bug reports. Nothing pays money.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-web/src/pages/download.rs
    - docs/game/playtesting.md
    - docs/roadmap/2026-09-29-launch-roadmap.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-10-01: We checked the download page and corrected the installation guidance."
  - "2026-10-01: The page offers Mac and Terminal release candidates. All other apps require source builds."
  - "2026-10-09: v3 (chat goldens): the download page offers Coder and the OpenAgents command-line program, not a Mac .dmg, so the answer says so."
---

## Answer

Our playtest is open to anyone: chat with us on openagents.com, and get Coder, our coding agent for your terminal, at openagents.com/download for macOS, Linux, and Windows. Build the iPhone, Android, and desktop apps from source. Season 1 runs from 2026-09-29 to 2026-10-26. Joining earns nothing by itself; XP and titles come only from contributions we accept, such as reproducible bug reports. Nothing pays money.

## Details

- Source builds of the iPhone, Android, and desktop apps start from
  https://github.com/OpenAgentsInc/openagents.
- We test the app, not you; there are no wrong answers.
- The repository is open source, so there's no confidentiality.

## Sources

- `crates/openagents-web/src/pages/download.rs`
- `docs/game/playtesting.md`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
