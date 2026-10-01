---
id: openagents.ttc-thesis
version: 1
kind: product
title: "The thesis: capability is something you can acquire at test time"
summary: >-
  The essay asks what an agent can become able to do at the moment it runs,
  without retraining, and proposes treating every admitted component as a
  candidate whose effect is a measured claim.
tags: [essay, test-time-capabilities, thesis, candidate, claim]
applies_when: >-
  The user asks for the thesis of our Test-Time Capabilities essay, or what a
  test-time capability is at the top level.
answer: >-
  The question is what an agent can become able to do at the moment it runs,
  without anyone retraining it. A coding agent that cannot see a repository
  cannot say what its largest file is however long it thinks; give it a plugin
  that maps the repository and it can. A test-time capability is an ability
  gained or lost at inference time, without a weight update, because something
  was admitted into the run. The chain is candidate, admission, controlled
  delta, reproduction, external validation, adoption, credit, revalidation.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/essays/2026-09-29-test-time-capabilities.md
evidence:
  - "2026-10-01: written from the essay Test-Time Capabilities and checked against its text (#10099); the answer text awaits the owner's copy review."
---

## Answer

The question is what an agent can become able to do at the moment it runs, without anyone retraining it. A coding agent that cannot see a repository cannot say what its largest file is however long it thinks; give it a plugin that maps the repository and it can. A test-time capability is an ability gained or lost at inference time, without a weight update, because something was admitted into the run. The chain is candidate, admission, controlled delta, reproduction, external validation, adoption, credit, revalidation.

## Details

- A test-time capability changes the achievable frontier: going from 60 % to 80 % success at the same budget is one, and reaching the same 80 % for a fifth of the cost is another.
- The five general sources of what gets admitted: plugins and programs, skills, knowledge, delegation to another agent, and typed judgment.
- Having a component installed, described, or demonstrated makes no claim, and admitting something can destroy capability as easily as create it: on SkillsBench, 13 of 87 tasks show negative skill deltas, and skills agents wrote for themselves landed below the no-skills baseline.
- Restraint, not admitting or not invoking a harmful thing, is as much a capability as reach.
- As far as the essay's search went, no prior work puts the whole chain together; that is a claim about the search, not about the literature's limits.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md#the-thesis-capability-is-something-you-can-acquire-at-test-time`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
