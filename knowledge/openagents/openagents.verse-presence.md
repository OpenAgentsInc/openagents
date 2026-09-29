---
id: openagents.verse-presence
version: 1
kind: product
title: "Who sees you in the Grid"
summary: >-
  While the Verse tab shows, the app shares your avatar's position and the
  shared objects on the public relay; other app players see a name tag with
  your key prefix.
tags: [verse, presence, players, privacy, multiplayer]
applies_when: >-
  The user asks who can see them in the Grid, whether other players are real,
  what their name tag shows, or what the Verse tab shares.
answer: >-
  While the Verse tab is showing, the app joins the Grid on our public relay
  and shares only your avatar's position and the shared objects. Other
  OpenAgents app players see you there, with a name tag of your key's first
  eight characters, and your level if you chose to show it. Switching tabs or
  leaving the app closes the connection.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/verse/mobile.md
    - docs/roadmap/2026-09-29-launch-roadmap.md
    - bins/openagents-ios/README.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

While the Verse tab is showing, the app joins the Grid on our public relay and shares only your avatar's position and the shared objects. Other OpenAgents app players see you there, with a name tag of your key's first eight characters, and your level if you chose to show it. Switching tabs or leaving the app closes the connection.

## Details

- The tab publishes no chat, rooms, private messages, or profile; it signs with the world key, never the device key.
- Other avatars are drawn a few seconds in the past so they move smoothly.

## Sources

- `docs/verse/mobile.md`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
- `bins/openagents-ios/README.md`
