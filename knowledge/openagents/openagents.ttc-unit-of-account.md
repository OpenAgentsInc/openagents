---
id: openagents.ttc-unit-of-account
version: 2
kind: product
title: "Capability claims as the unit of account"
summary: >-
  A per-component with-and-without evaluation produces a claim, which is more
  useful than a leaderboard score; a claim answers only the first of three
  operator questions.
tags: [essay, test-time-capabilities, unit-of-account, evals, leaderboard]
applies_when: >-
  The user asks why capability claims are the unit of account, or how evals
  differ from benchmarks.
answer: >-
  A capability claim is the unit of account, produced by a per-component with-
  and-without evaluation. Two arms, not one score: a with-and-without result
  says what one component changed. A written, versioned rule gives the
  verdict, and every result names the exact version of the rule that judged it.
  Others can rerun it from the published test set, result, and exact component
  versions. Benchmarks ask how capable an agent is; a claim says what caused
  it to become more capable.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/essays/2026-09-29-test-time-capabilities.md
evidence:
  - "2026-10-01: written from the essay Test-Time Capabilities and checked against its text (#10099); the answer text awaits the owner's copy review."
  - "2026-10-09: version 2 (#11031) says it in plain words, without internal terms."
---

## Answer

A capability claim is the unit of account, produced by a per-component with-and-without evaluation. Two arms, not one score: a with-and-without result says what one component changed. A written, versioned rule gives the verdict, and every result names the exact version of the rule that judged it. Others can rerun it from the published test set, result, and exact component versions. Benchmarks ask how capable an agent is; a claim says what caused it to become more capable.

## Details

- A claim answers only the first of three questions an operator asks: does it work (the evaluation), may it run (operational admission), and should everyone get it (adoption). They should never be folded into one score.
- Evals also have failure modes: a grader is software and can be wrong. ToolBench's original pass rate counted unsolvable queries as passes, and with randomly chosen APIs it reached 99.0 %.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md#capability-claims-as-the-unit-of-account`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
