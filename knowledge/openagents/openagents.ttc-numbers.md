---
id: openagents.ttc-numbers
version: 1
kind: product
title: "Our numbers: judgments before thinking"
summary: >-
  From Send on the phone: a Jev judgment takes 170 ms median, a prepared
  answer shows in 0.62 to 0.70 s, and a full model answer's first words take a
  median of 4.2 s.
tags: [essay, test-time-capabilities, numbers, latency, router, jev]
applies_when: >-
  The user asks about the latency numbers in our Test-Time Capabilities essay,
  or how fast the chat router judges a message.
answer: >-
  The essay's numbers, measured from Send on the phone: a Jev judgment of all
  router questions in one request takes 170 ms median and 235 ms at p95; a
  prepared answer is on screen in 0.62 to 0.70 s; the opener before a model
  answer takes 0.60 to 0.75 s; a full model answer's first words come at a
  median of 4.2 s and the complete answer in 3.2 to 5.2 s. A turn answered by
  a prepared answer costs a Jev call and no generation.
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

The essay's numbers, measured from Send on the phone: a Jev judgment of all router questions in one request takes 170 ms median and 235 ms at p95; a prepared answer is on screen in 0.62 to 0.70 s; the opener before a model answer takes 0.60 to 0.75 s; a full model answer's first words come at a median of 4.2 s and the complete answer in 3.2 to 5.2 s. A turn answered by a prepared answer costs a Jev call and no generation.

## Details

- About 0.3 s of every phone number is the relay setup for a fresh connection.
- In the hosted runs, a Project map run took 10.7 s with the plugin against 24.9 s without it, and passed more tests, which is capability substituting for compute.
- We do not yet price every lane; Coder does not price gateway lanes, so the eval records list cost as unknown.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md#our-numbers-judgments-before-thinking`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
