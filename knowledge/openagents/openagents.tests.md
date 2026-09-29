---
id: openagents.tests
version: 2
kind: product
title: "Tests and test sets"
summary: >-
  A test is one task we give Coder, with checks on how it went; a test set is
  the tests for one capability, run with the capability and without it.
tags: [gym, tests, test-sets, evals]
applies_when: >-
  The user asks what a test or a test set is, how tests are checked, or how a
  capability is tested.
answer: >-
  A test is one task we give Coder, with checks on how it went: its last
  message, the files it made, or the steps it took. A test set is the tests
  for one capability, including at least one where the capability should stay out of the
  way. We run each test with the capability and without it, and the result says
  Better, No clear change, or Worse.
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
---

## Answer

A test is one task we give Coder, with checks on how it went: its last message, the files it made, or the steps it took. A test set is the tests for one capability, including at least one where the capability should stay out of the way. We run each test with the capability and without it, and the result says Better, No clear change, or Worse.

## Details

- Engineering docs call a test a case and a test set a suite.
- A full run repeats each test three times on each side; Try it once runs each test once.
- Nothing about your test set is public until you add it to the Gym.

## Sources

- `docs/extensions/evaluation.md`
- `docs/product/2026-09-28-app-wireframe.md`
