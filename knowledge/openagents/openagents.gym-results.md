---
id: openagents.gym-results
version: 4
kind: product
title: "The Gym and its RESULTS board"
summary: >-
  The Gym stands straight ahead of where you start in the Grid; its RESULTS
  board shows our published coding results, each attempt step by step, with
  no connection needed.
tags: [gym, results, leaderboard, terminal-bench, verse]
applies_when: >-
  The user asks what the Gym is, what the RESULTS board shows, how to see
  benchmark results or traces, or how to open the Gym board; not what's new in
  the Gym or how a tool did on its tests, which chat answers from the Gym's
  records.
answer: >-
  The Gym stands straight ahead of where you start in the Grid. Inside, the
  RESULTS board shows our published Terminal-Bench results: tap it to open the
  boards, an attempt, and its trace, which you can play, pause, and step
  through. It needs no connection. The central Gym board opens only with a Gym
  connection code granted to your world key.
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
  - "2026-09-29: version 4 (#9941) adds the EVALS board beside the central board (docs/verse/gym.md, #9942); the answer is unchanged."
---

## Answer

The Gym stands straight ahead of where you start in the Grid. Inside, the RESULTS board shows our published Terminal-Bench results: tap it to open the boards, an attempt, and its trace, which you can play, pause, and step through. It needs no connection. The central Gym board opens only with a Gym connection code granted to your world key.

## Details

- The board reads TAP TO OPEN from within 6 meters.
- Results publications are signed with NIP-EVAL kind 3195.
- The results aren't shown as signed by OpenAgents until a publisher key is created and pinned.
- Results of tools tested on Coder, and the Gym's news, come to chat: ask us what's new in the Gym, or how a tool did.
- On iPhone, the EVALS board to the left of the central board shows tools' published test results, grouped by test set, with the checks each one got; in chat, **See the board** under a tool's result walks you there. On Android the board is lettered but doesn't open yet.

## Sources

- `docs/verse/mobile.md`
- `docs/verse/gym-leaderboard.md`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
- `docs/extensions/evaluation.md`
- `docs/verse/gym.md`
