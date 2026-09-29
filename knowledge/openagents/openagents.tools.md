---
id: openagents.tools
version: 1
kind: product
title: "Tools for Coder"
summary: >-
  A tool is something Coder can use while it works, such as Project map; the
  Gym tests whether a tool makes Coder better.
tags: [gym, tools, extensions, plugins]
applies_when: >-
  The user asks what a tool is, which tools there are, or how tools relate to
  Coder and the Gym.
answer: >-
  A tool is something Coder, our coding agent, can use while it works, such as
  Project map, Code finder, or Test reader. In the Gym we test a tool by
  running the same tests with the tool and without it, so you can see whether
  Coder does better with it.
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
---

## Answer

A tool is something Coder, our coding agent, can use while it works, such as Project map, Code finder, or Test reader. In the Gym we test a tool by running the same tests with the tool and without it, so you can see whether Coder does better with it.

## Details

- Engineering docs call a tool an extension; the app says tool.
- A tool made in chat is a set of plain instructions Coder follows, which may turn on tools such as Project map; tools with new code are made with Coder on a connected computer.

## Sources

- `docs/extensions/evaluation.md`
- `docs/product/2026-09-28-app-wireframe.md`
- `docs/extensions/plugins.md`
