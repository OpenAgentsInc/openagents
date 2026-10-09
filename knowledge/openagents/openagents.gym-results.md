---
id: openagents.gym-results
version: 8
kind: product
title: "The Gym and its boards"
summary: >-
  The Gym is where plugins for Coder get tested, with and without each plugin;
  in the Verse, the Gym building shows the results on its boards.
tags: [gym, results, verse, tests, checks]
applies_when: >-
  The user asks what the Gym is, what its boards show, or how to open them;
  not what's new in the Gym or how a plugin did on its tests, which chat
  answers from the Gym's records.
answer: >-
  The Gym is where plugins for Coder, our coding agent, get tested: the same
  tests run with a plugin and without it, to show how many Coder passed each
  way. Trainers add their results to the Gym, and other trainers check them;
  you earn XP when a result holds up. In the Verse, the Gym building straight
  ahead of where you start shows the results on its boards.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/verse/mobile.md
    - docs/verse/gym-leaderboard.md
    - docs/roadmap/2026-09-29-launch-roadmap.md
    - docs/extensions/evaluation.md
    - docs/verse/gym.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-28: version 2 (#9936) adds that tool test results and the Gym's news come to chat, from docs/extensions/evaluation.md; the answer is unchanged."
  - "2026-09-29: version 3 (#9944) words the summary, which the Gym's news card shows as a line, in the app's plain words (no Terminal-Bench or traces, CHK-02); the answer is unchanged."
  - "2026-09-29: version 4 (#9941) answers what the Gym is now, where Coder's tools are tested from chat (docs/extensions/evaluation.md), adds the board of tool results beside the central board (docs/verse/gym.md, #9942), and keeps the app's plain words (CHK-02) in the answer and details."
  - "2026-09-29: version bump (#9958) says capability, the on-screen word decided in #9957; the note's id and tags stay."
  - "2026-10-01: version 6 (#10087) says plugin, the one word for anything a person adds (skills, workflows, knowledge, Wasm, and tests); the note's id and tags stay."
  - "2026-10-09: the answer gives the exact page or the one command to run for each thing it tells the reader to do (the owner's rule of 2026-10-09), checked against the cited sources."
  - "2026-10-09: version 8 drops 'test a plugin here in chat': the sample plugins that flow offered are no longer shown."
---

## Answer

The Gym is where plugins for Coder, our coding agent, get tested: the same tests run with a plugin and without it, to show how many Coder passed each way. Trainers add their results to the Gym, and other trainers check them; you earn XP when a result holds up. In the Verse, the Gym building straight ahead of where you start shows the results on its boards.

## Details

- In the Gym building, the board to the left of the central one shows each plugin's published test results, grouped by test set, with the checks each one got. On Android that board is lettered but doesn't open yet.
- The central board shows our earlier coding results, each attempt step by step, and needs no connection. It reads TAP TO OPEN from within 6 meters.
- Ask us what's new in the Gym, or how a plugin did, and we answer from the Gym's records.

## Sources

- `docs/verse/mobile.md`
- `docs/verse/gym-leaderboard.md`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
- `docs/extensions/evaluation.md`
- `docs/verse/gym.md`
