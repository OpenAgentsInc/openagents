---
id: openagents.playtesting
version: 5
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
  Coder, our coding agent for your terminal, at
  https://openagents.com/download for macOS, Linux, and Windows. Get the
  iPhone app on TestFlight at https://testflight.apple.com/join/dvQdns5B, and
  build the Android and desktop apps from source at
  https://github.com/OpenAgentsInc/openagents. Season 1 runs from 2026-09-29
  to 2026-10-26. Joining earns nothing by itself; XP and titles come only from
  contributions we accept, such as reproducible bug reports. Nothing pays
  money.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-web/src/pages/download.rs
    - crates/openagents-web/src/pages/connect.rs
    - docs/game/playtesting.md
    - docs/roadmap/2026-09-29-launch-roadmap.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-10-01: We checked the download page and corrected the installation guidance."
  - "2026-10-01: The page offers Mac and Terminal release candidates. All other apps require source builds."
  - "2026-10-09: v3 (chat goldens): the download page offers Coder and the OpenAgents command-line program, not a Mac .dmg, so the answer says so."
  - "2026-10-09: the answer gives the exact page or the one command to run for each thing it tells the reader to do (the owner's rule of 2026-10-09), checked against the cited sources."
  - "2026-10-09: v5: the iPhone app ships on TestFlight (https://testflight.apple.com/join/dvQdns5B, the link the download page and /connect give), not as a source build; checked against the cited sources."
---

## Answer

Our playtest is open to anyone: chat with us on openagents.com, and get Coder, our coding agent for your terminal, at https://openagents.com/download for macOS, Linux, and Windows. Get the iPhone app on TestFlight at https://testflight.apple.com/join/dvQdns5B, and build the Android and desktop apps from source at https://github.com/OpenAgentsInc/openagents. Season 1 runs from 2026-09-29 to 2026-10-26. Joining earns nothing by itself; XP and titles come only from contributions we accept, such as reproducible bug reports. Nothing pays money.

## Details

- Source builds of the Android and desktop apps start from
  https://github.com/OpenAgentsInc/openagents.
- We test the app, not you; there are no wrong answers.
- The repository is open source, so there's no confidentiality.

## Sources

- `crates/openagents-web/src/pages/download.rs`
- `crates/openagents-web/src/pages/connect.rs`
- `docs/game/playtesting.md`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
