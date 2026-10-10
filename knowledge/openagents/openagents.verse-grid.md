---
id: openagents.verse-grid
version: 3
kind: product
title: "Verse and the Grid"
summary: >-
  The Verse is our shared world, and the Grid is its first place: you walk
  around as your avatar and see other players live.
tags: [verse, grid, world, game, physics]
applies_when: >-
  The user asks what Verse or the Grid is, or what there is to do in the
  Verse.
answer: >-
  The Verse is our shared world, and the Grid is its first place: a ground
  grid where you walk around as your avatar and see other players walking
  around live. There's no chat
  in the Grid.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-desktop/README.md
    - crates/openagents-desktop/src/grid.rs
    - docs/verse/mobile.md
    - bins/openagents-ios/README.md
    - docs/roadmap/2026-09-29-launch-roadmap.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-10-02: clarified desktop Grid scope and lifecycle from the desktop README and renderer (#10133, #10071)."
  - "2026-10-09: the ball, blocks, and pillar are off, and the phone opens the Grid from its Coder / Verse switch (#11184)."
---

## Answer

The Verse is our shared world, and the Grid is its first place: a ground grid where you walk around as your avatar and see other players walking around live. There's no chat in the Grid.

## Details

- Other players' positions can lag a little.
- On the Mac, open **Verse** from the sidebar footer beside the Local profile and Settings. The Grid is drawn only on the desktop's Verse page and the deck's title slide, not behind chat or other screens.
- Opening the Verse page loads the world; leaving it closes the relay connection and releases the world's GPU resources. Grid, Watch, Play, and Reduce motion behavior stay on that page.

## Sources

- `crates/openagents-desktop/README.md`
- `crates/openagents-desktop/src/grid.rs`
- `docs/verse/mobile.md`
- `bins/openagents-ios/README.md`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
