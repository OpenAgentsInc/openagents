---
id: openagents.ttc-cheap-judgments
version: 1
kind: product
title: "Cheap judgments before expensive thinking"
summary: >-
  Before allocating thinking, decide whether the turn needs a large model at
  all; a ladder of answers at rising cost, with three principles including
  never wrong fast.
tags: [essay, test-time-capabilities, cheap-judgments, routing, answer-ladder]
applies_when: >-
  The user asks why cheap judgments should come before expensive thinking, or
  what never wrong fast means.
answer: >-
  Before allocating thinking, decide whether the turn needs a large model at
  all, and which capability should handle it. A system can offer a ladder of
  answers at rising cost: a prepared answer, one finished by a small model, an
  answer grounded in a knowledge base, the full model, and a hand-off to an
  agent with a computer. Three principles: never wrong fast, spend the big
  model where it adds something, and capability can substitute for compute,
  which is a hypothesis to test.
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

Before allocating thinking, decide whether the turn needs a large model at all, and which capability should handle it. A system can offer a ladder of answers at rising cost: a prepared answer, one finished by a small model, an answer grounded in a knowledge base, the full model, and a hand-off to an agent with a computer. Three principles: never wrong fast, spend the big model where it adds something, and capability can substitute for compute, which is a hypothesis to test.

## Details

- Never wrong fast: a prepared answer is served only when the judgment clears thresholds tuned for precision, because a fast answer to the wrong question is worse than a slow right one.
- Requests for work need a computer, not a model's guess at doing the work in chat.
- A claim may name cost as its primary outcome with correctness held non-inferior, but it may never drop the bound, because a faster wrong answer is not a capability.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md#cheap-judgments-before-expensive-thinking`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
