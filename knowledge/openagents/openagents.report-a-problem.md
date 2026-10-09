---
id: openagents.report-a-problem
version: 2
kind: product
title: "Reporting a problem"
summary: >-
  Report a problem is in Account and on a long press of the tab bar; reports
  go privately to the triage key, and **My reports** lists them.
tags: [report, bug, feedback, playtest, in-app]
applies_when: >-
  The user asks how to report a bug, crash, or problem, send feedback, or find
  reports they sent.
answer: >-
  Open Account > Report a problem, or long-press the tab bar on any screen, so
  the report knows where you were. Pick a kind (bug, confusing, idea, or felt
  good), say what happened and what you expected, and attach a screenshot only
  if you want; you see it first, and the Wallet and key screens never attach
  one. Reports go privately to our triage key, and **My reports** lists what
  you sent. TestFlight feedback and the Playtest report issue template on
  GitHub work too.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/game/playtesting.md
    - bins/openagents-ios/README.md
    - docs/roadmap/2026-09-29-launch-roadmap.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-10-09: tagged in-app: its steps are screens of the OpenAgents app, so the website's chat never shows the answer whole."
---

## Answer

Open Account > Report a problem, or long-press the tab bar on any screen, so the report knows where you were. Pick a kind (bug, confusing, idea, or felt good), say what happened and what you expected, and attach a screenshot only if you want; you see it first, and the Wallet and key screens never attach one. Reports go privately to our triage key, and **My reports** lists what you sent. TestFlight feedback and the Playtest report issue template on GitHub work too.

## Details

- A report travels as a NIP-17 private message sealed with NIP-44, signed by your world key.
- Until a build carries the triage key, reports wait on the phone.
- When a report is sent, the app also publishes a public, content-free record of it (NIP-XP kind 3197) that a playtest award can cite.

## Sources

- `docs/game/playtesting.md`
- `bins/openagents-ios/README.md`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
