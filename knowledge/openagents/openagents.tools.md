---
id: openagents.tools
version: 2
kind: product
title: "Capabilities for Coder"
summary: >-
  A capability is something Coder can use while it works, such as Project
  map; the Gym tests whether a capability makes Coder better.
tags: [gym, tools, extensions, plugins]
applies_when: >-
  The user asks what a capability (or a tool) is, which capabilities there
  are, or how capabilities relate to Coder and the Gym.
answer: >-
  A capability is something Coder, our coding agent, can use while it works,
  such as Project map, Code finder, or Test reader. In the Gym we test a
  capability by running the same tests with it and without it, so you can
  see whether Coder does better with it.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/extensions/evaluation.md
    - docs/product/2026-09-28-app-wireframe.md
    - docs/extensions/plugins.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9936); the answer text awaits the owner's copy review."
  - "2026-09-29: version 2 (#9958) says capability, the on-screen word decided in #9957; the note's id and tags stay."
---

## Answer

A capability is something Coder, our coding agent, can use while it works, such as Project map, Code finder, or Test reader. In the Gym we test a capability by running the same tests with it and without it, so you can see whether Coder does better with it.

## Details

- Engineering docs call a capability an extension, shipped as a program, a plugin, a skill, or a knowledge entry; the app says capability. "Tool" is retired as the umbrella word on screen, though a model's own tool call in a transcript keeps its name.
- A capability made in chat is a set of plain instructions Coder follows, which may turn on capabilities such as Project map; capabilities with new code are made with Coder on a connected computer.

## Sources

- `docs/extensions/evaluation.md`
- `docs/product/2026-09-28-app-wireframe.md`
- `docs/extensions/plugins.md`
