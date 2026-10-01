---
id: openagents.checks
version: 3
kind: product
title: "Checking a result"
summary: >-
  A check reruns another trainer's published test set on the same plugin; it
  confirms the result when the verdict matches.
tags: [gym, checks, results]
applies_when: >-
  The user asks what checking a result means, why checks matter, or how a
  result gets confirmed.
answer: >-
  Checking a result means running another trainer's published test set on the
  same plugin again. If you get the same verdict, your check confirms their
  result; if not, it disputes it, and both show beside the result. A plugin
  whose Better result three different trainers confirmed is a candidate for
  Coder to use for everyone.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/extensions/evaluation.md
    - docs/product/2026-09-28-app-wireframe.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9936); the answer text awaits the owner's copy review."
  - "2026-09-29: version bump (#9958) says capability, the on-screen word decided in #9957; the note's id and tags stay."
  - "2026-10-01: version 3 (#10087) says plugin, the one word for anything a person adds (skills, workflows, knowledge, Wasm, and tests); the note's id and tags stay."
---

## Answer

Checking a result means running another trainer's published test set on the same plugin again. If you get the same verdict, your check confirms their result; if not, it disputes it, and both show beside the result. A plugin whose Better result three different trainers confirmed is a candidate for Coder to use for everyone.

## Details

- You can't check your own result.
- Adoption into Coder's defaults is an OpenAgents decision, never automatic.

## Sources

- `docs/extensions/evaluation.md`
- `docs/product/2026-09-28-app-wireframe.md`
