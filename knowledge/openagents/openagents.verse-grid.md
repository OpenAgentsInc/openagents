---
id: openagents.verse-grid
version: 1
kind: product
title: "Verse and the Grid"
summary: >-
  The Verse tab shows the Grid: a shared world with other players, a ball,
  cubes, and dominoes everyone shares, a reset pillar, and the Gym.
tags: [verse, grid, world, game, physics]
applies_when: >-
  The user asks what Verse or the Grid is, what there is to do in the Verse
  tab, or what the ball, blocks, and pillar are.
answer: >-
  Verse is the app's shared world, and the Grid is where the Verse tab puts
  you: a ground grid with your character, other players walking around live, a
  ball, a stack of cubes, and dominoes that everyone shares and finds where
  they were left. Walking into the pillar to the right of the spawn puts them
  all back. Straight ahead stands the Gym. There's no chat in the Grid.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/verse/mobile.md
    - bins/openagents-ios/README.md
    - docs/roadmap/2026-09-29-launch-roadmap.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

Verse is the app's shared world, and the Grid is where the Verse tab puts you: a ground grid with your character, other players walking around live, a ball, a stack of cubes, and dominoes that everyone shares and finds where they were left. Walking into the pillar to the right of the spawn puts them all back. Straight ahead stands the Gym. There's no chat in the Grid.

## Details

- Walk into the ball to push it; it rolls with real physics and comes to rest.
- Shared state can lag between players.

## Sources

- `docs/verse/mobile.md`
- `bins/openagents-ios/README.md`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
