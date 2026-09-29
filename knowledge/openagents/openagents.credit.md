---
id: openagents.credit
version: 1
kind: product
title: "Credit for tests and tools"
summary: >-
  You earn XP when another trainer's check confirms your published result, and
  when Coder adopts your tool; XP is never money.
tags: [gym, credit, xp, checks, adoption]
applies_when: >-
  The user asks how they earn XP or credit from tests, checks, test sets, or
  tools, or whether that credit pays money.
answer: >-
  With tests in chat, which are on their way, you earn XP when your work is
  used: when another trainer checks a result you added to the Gym and gets the
  same verdict (you, the checker, and the test set's author each earn XP), and
  when Coder adopts your tool for everyone. A run, a publish, or a view earns
  nothing by itself. XP can't be spent, and it isn't money.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/extensions/evaluation.md
    - nips/openagents/NIP-XP.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9936); the answer text awaits the owner's copy review."
---

## Answer

With tests in chat, which are on their way, you earn XP when your work is used: when another trainer checks a result you added to the Gym and gets the same verdict (you, the checker, and the test set's author each earn XP), and when Coder adopts your tool for everyone. A run, a publish, or a view earns nothing by itself. XP can't be spent, and it isn't money.

## Details

- In the first quests a confirmed check is worth 50 XP to the checker and 25 each to the evaluator and the test set's author; an adoption is worth 200, 100, and 50.
- Each role earns at most once per test set version per season.

## Sources

- `docs/extensions/evaluation.md`
- `nips/openagents/NIP-XP.md`
