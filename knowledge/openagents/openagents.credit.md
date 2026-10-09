---
id: openagents.credit
version: 5
kind: product
title: "Credit for tests and plugins"
summary: >-
  You earn XP when another trainer's check confirms your published result, and
  when Coder adopts your plugin; XP is never money.
tags: [gym, credit, xp, checks, adoption]
applies_when: >-
  The user asks how they earn XP or credit from tests, checks, test sets, or
  plugins, or whether that credit pays money.
answer: >-
  You can make your own plugin and its tests with us and add your results
  to the Gym. You earn XP when your work is used: when another trainer
  checks a result you added to the Gym and gets the same verdict (you, the
  checker, and the test set's author each earn XP), and when Coder adopts
  your plugin for everyone. A run, a publish, or a view earns nothing by
  itself. XP can't be spent, and it isn't money.
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
  - "2026-09-29: version 3 (#9958) says capability, the on-screen word decided in #9957."
  - "2026-10-01: version 4 (#10087) says plugin, the one word for anything a person adds (skills, workflows, knowledge, Wasm, and tests); the note's id and tags stay."
  - "2026-10-09: version 5 drops 'test a plugin in chat': the sample plugins that flow tested are no longer shown, so there is nothing to start from chat."
---

## Answer

You can make your own plugin and its tests with us and add your results to the Gym. You earn XP when your work is used: when another trainer checks a result you added to the Gym and gets the same verdict (you, the checker, and the test set's author each earn XP), and when Coder adopts your plugin for everyone. A run, a publish, or a view earns nothing by itself. XP can't be spent, and it isn't money.

## Details

- In the first quests a confirmed check is worth 50 XP to the checker and 25 each to the trainer who ran the result and the test set's author; an adoption is worth 200, 100, and 50.
- Each role earns at most once per test set version per season.
- The menu's **Profile** shows what you made and what each item earned, read from the XP ledger on your phone.

## Sources

- `docs/extensions/evaluation.md`
- `nips/openagents/NIP-XP.md`
- `crates/openagents-mobile/src/account.rs`
