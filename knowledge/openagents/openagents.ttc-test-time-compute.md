---
id: openagents.ttc-test-time-compute
version: 1
kind: product
title: "Test-time compute and why it matters"
summary: >-
  Test-time compute is computation spent when a model answers, not when it is
  trained; a verifier makes extra compute pay, and compute should be allocated
  per question.
tags: [essay, test-time-capabilities, test-time-compute, verifier]
applies_when: >-
  The user asks what test-time compute is, or why it matters in our Test-Time
  Capabilities essay.
answer: >-
  Test-time compute is the computation spent when a model is asked a question,
  as opposed to training. The literature's families are thinking longer,
  controlling the thinking budget, sampling many times and picking, spending
  compute where it helps, and briefly adapting weights. Two lessons: a
  verifier is what makes extra compute pay, and compute should be allocated
  per question, with the allocation judgment costing far less than the work it
  allocates.
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

Test-time compute is the computation spent when a model is asked a question, as opposed to training. The literature's families are thinking longer, controlling the thinking budget, sampling many times and picking, spending compute where it helps, and briefly adapting weights. Two lessons: a verifier is what makes extra compute pay, and compute should be allocated per question, with the allocation judgment costing far less than the work it allocates.

## Details

- Both lessons generalize past tokens, to the system around a model: what it is allowed to use, who decides what to use, and how anyone knows it helped.
- Without a checker that can tell a right answer from a wrong one, extra samples stop helping.
- The essay's Part One is vendor-neutral: it uses no product names and none of our numbers.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md#what-test-time-compute-is`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
