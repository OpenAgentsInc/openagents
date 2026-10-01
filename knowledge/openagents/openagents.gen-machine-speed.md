---
id: openagents.gen-machine-speed
version: 1
kind: product
title: "Extensible at machine speed, and why speed needs a brake"
summary: >-
  In this composition no step waits on a training run or a reviewer's queue;
  the one human gate is adoption into the defaults, and measurement is the
  brake that keeps speed safe.
tags: [essay, general-agent, machine-speed, brake, adoption, skillsbench]
applies_when: >-
  The user asks what extensible at machine speed means, or why speed needs a
  brake.
answer: >-
  Machine speed is a claim about cycle time: contribute by signing and
  publishing a release, get evaluated when anyone runs the with-and-without
  test, get trusted through independent reproductions and a validation on
  someone else's test set, and reach every user through an adoption decision
  and a defaults release. The one human gate we keep is adoption, a decision
  about evidence already on the record. Machine-speed extension without
  measurement is machine-speed regression, so plugins are cheap to propose and
  expensive to adopt, and the expense is evidence, not permission.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/essays/2026-10-01-the-return-of-the-general-agent.md
evidence:
  - "2026-10-01: written from the essay The Return of the General Agent and checked against its text (#10099); the answer text awaits the owner's copy review."
---

## Answer

Machine speed is a claim about cycle time: contribute by signing and publishing a release, get evaluated when anyone runs the with-and-without test, get trusted through independent reproductions and a validation on someone else's test set, and reach every user through an adoption decision and a defaults release. The one human gate we keep is adoption, a decision about evidence already on the record. Machine-speed extension without measurement is machine-speed regression, so plugins are cheap to propose and expensive to adopt, and the expense is evidence, not permission.

## Details

- The Project map record shows the whole path, from result through three reproductions, an external validation, adoption, and the next run admitting it, completed in one afternoon. It is demonstrated once end to end, not yet a measured rate of improvement across many contributors.
- On SkillsBench, 13 of 87 tasks got worse with a skill added, and self-written skills landed below the no-skill baseline on every configuration tested, while curated skills added 18 to 25 points.
- Our own first gate rated a plugin Better for making Coder faster without making it more correct; we replaced it the same day.
- The latest router revision reads 0.907 held-out route accuracy with prepared-answer precision at 100 %, and named the right engine 22 of 22 times when one was asked for.
- Where it comes from: the essay The Return of the General Agent, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-10-01-the-return-of-the-general-agent.md#at-machine-speed`.

## Sources

- `docs/essays/2026-10-01-the-return-of-the-general-agent.md`
