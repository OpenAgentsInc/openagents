---
id: openagents.ttc-term-reproduced
version: 1
kind: product
title: "Lexicon term 7: reproduced capability claim"
summary: >-
  A reproduced capability claim has a second evidence record on the same key
  from someone other than the author, with a compatible effect estimate.
tags: [essay, test-time-capabilities, reproduction, reliance-set, lexicon]
applies_when: >-
  The user asks what a reproduced capability claim is, or what makes a
  reproduction independent.
answer: >-
  A reproduced capability claim is a claim with a second evidence record from
  someone other than the person who made the first: a different evaluator, the
  key held fixed, and an effect estimate compatible with the original's.
  Confidence should come from reproduction by someone else, not from the
  author's report. Reproduction answers one question only: can someone else
  get this result? It does not say whether the result was fitted to the tests.
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

A reproduced capability claim is a claim with a second evidence record from someone other than the person who made the first: a different evaluator, the key held fixed, and an effect estimate compatible with the original's. Confidence should come from reproduction by someone else, not from the author's report. Reproduction answers one question only: can someone else get this result? It does not say whether the result was fitted to the tests.

## Details

- Compatibility is a property of two estimates, not of two labels: two reruns can both read Better while estimating +2 and +25. Confirmations and disputes both stay visible.
- Every claim has a reliance set: the runner, the model provider, the agent build, the selector, the grader, and the host. Three reruns on one platform are not three independent replications, and a claim should say what its reruns shared.
- A reproduced claim needs a rerun by someone else and a grader that has itself been checked.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md#7-reproduced-capability-claim`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
