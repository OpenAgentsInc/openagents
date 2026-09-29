---
id: openagents.playtest-logging
version: 1
kind: product
title: "What playtest logging records"
summary: >-
  Playtest logging keeps a local log of tabs, screens, error codes, and times,
  never content, and sends it only inside a report you preview.
tags: [playtest, logging, privacy, analytics, telemetry]
applies_when: >-
  The user asks whether the app tracks them, sends analytics or telemetry, or
  what playtest logging records.
answer: >-
  Playtest logging keeps a short local log of structural events: which tab and
  screen, which error code, and when. It never records message text,
  transcripts, keys, recovery words, addresses, amounts, or balances. It stays
  on your phone and leaves only inside a report whose preview showed it and
  where you ticked it. Account > Playtest shows whether it's on and has Delete
  the log. The app sends no analytics.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/game/playtesting.md
    - INVARIANTS.md
    - bins/openagents-ios/README.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

Playtest logging keeps a short local log of structural events: which tab and screen, which error code, and when. It never records message text, transcripts, keys, recovery words, addresses, amounts, or balances. It stays on your phone and leaves only inside a report whose preview showed it and where you ticked it. Account > Playtest shows whether it's on and has Delete the log. The app sends no analytics.

## Details

- It keeps at most 200 events, oldest dropped first, in the app's encrypted store.
- It's on in every playtest build; a release build can turn it off.

## Sources

- `docs/game/playtesting.md`
- `INVARIANTS.md`
- `bins/openagents-ios/README.md`
