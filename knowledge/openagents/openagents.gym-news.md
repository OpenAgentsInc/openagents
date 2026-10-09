---
id: openagents.gym-news
version: 5
kind: product
title: "What's new in the Gym"
summary: >-
  The Gym is in chat: ask what's new, make a plugin and its tests with us,
  check results, and see your credit.
tags: [gym, news, roadmap, evals]
applies_when: >-
  The user asks what is new or coming in the Gym, what OpenAgents is working
  on, or what the latest advancements are.
answer: >-
  The Gym is in this chat. Ask what's new, make a plugin and its tests with
  us, add a result to the Gym, check other trainers' results, and see the XP
  your work earns. The Gym's news here comes from published results and
  checks, our changelog, and our notes.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/extensions/evaluation.md
    - docs/product/2026-09-28-app-wireframe.md
    - docs/roadmap/2026-09-29-launch-roadmap.md
    - docs/verse/gym.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9936); the answer text awaits the owner's copy review."
  - "2026-09-29: version 2 (#9941) says the Gym is in chat now (build 21, crates/openagents-mobile/src/account.rs), with the hosted runner's first live results (docs/extensions/measurements/2026-09-29-hosted-runner-live.md) and the EVALS board (docs/verse/gym.md)."
  - "2026-09-29: version bump (#9958) says capability, the on-screen word decided in #9957; the note's id and tags stay."
  - "2026-10-01: version 4 (#10087) says plugin, the one word for anything a person adds (skills, workflows, knowledge, Wasm, and tests); the note's id and tags stay."
  - "2026-10-09: version 5 stops offering the sample plugins (Project map and the rest) for testing in chat; they are test fixtures for the hosted runner now, not shown to people."
---

## Answer

The Gym is in this chat. Ask what's new, make a plugin and its tests with us, add a result to the Gym, check other trainers' results, and see the XP your work earns. The Gym's news here comes from published results and checks, our changelog, and our notes.

## Details

- Published results and checks appear in the Gym's news with their source.
- In the Verse, the Gym has a board that shows published results by test set, with the checks on each one.

## Sources

- `docs/extensions/evaluation.md`
- `docs/product/2026-09-28-app-wireframe.md`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
- `docs/verse/gym.md`
