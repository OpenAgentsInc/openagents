---
id: openagents.credit
version: 2
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
  You can test a tool in chat, make your own tool and its tests with us, and
  add your results to the Gym. You earn XP when your work is used: when
  another trainer checks a result you added to the Gym and gets the same
  verdict (you, the checker, and the test set's author each earn XP), and
  when Coder adopts your tool for everyone. A run, a publish, or a view earns
  nothing by itself. XP can't be spent, and it isn't money.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/extensions/evaluation.md
    - nips/openagents/NIP-XP.md
    - crates/openagents-mobile/src/account.rs
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9936); the answer text awaits the owner's copy review."
  - "2026-09-29: version 2 (#9941) says tests in chat are live in build 21 (crates/openagents-mobile/src/account.rs) instead of on their way; the referee's first live awards are in docs/extensions/measurements/2026-09-29-hosted-runner-live.md."
---

## Answer

You can test a tool in chat, make your own tool and its tests with us, and add your results to the Gym. You earn XP when your work is used: when another trainer checks a result you added to the Gym and gets the same verdict (you, the checker, and the test set's author each earn XP), and when Coder adopts your tool for everyone. A run, a publish, or a view earns nothing by itself. XP can't be spent, and it isn't money.

## Details

- In the first quests a confirmed check is worth 50 XP to the checker and 25 each to the evaluator and the test set's author; an adoption is worth 200, 100, and 50.
- Each role earns at most once per test set version per season.
- The menu's **Profile** shows what you made and what each item earned, read from the XP ledger on your phone.

## Sources

- `docs/extensions/evaluation.md`
- `nips/openagents/NIP-XP.md`
- `crates/openagents-mobile/src/account.rs`
