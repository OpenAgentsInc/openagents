---
id: openagents.ttc-term-delta
version: 1
kind: product
title: "Lexicon term 3: capability delta"
summary: >-
  The capability delta is the estimated effect of admitting a component: the
  with arm against the without arm on the same tests, with its uncertainty.
tags: [essay, test-time-capabilities, delta, with-and-without, lexicon]
applies_when: >-
  The user asks what a capability delta is, or how a with-and-without
  evaluation is designed.
answer: >-
  The capability delta is the estimated treatment effect of admitting the
  component: the difference in outcome between the with arm and the without
  arm, on the same tests, with its uncertainty, from enough repeats to
  estimate it. It is the size and sign of the claim, and it holds only for the
  baseline it was measured against. A negative delta is a finding about the
  component, not a failure of the evaluation.
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

The capability delta is the estimated treatment effect of admitting the component: the difference in outcome between the with arm and the without arm, on the same tests, with its uncertainty, from enough repeats to estimate it. It is the size and sign of the claim, and it holds only for the baseline it was measured against. A negative delta is a finding about the component, not a failure of the evaluation.

## Details

- A delta has more than one outcome; the policy says which is primary. The usual claim names correctness as primary and requires cost and time not to be materially worse; the same correctness at a fifth of the cost is also a legitimate claim. What can never be a claim is faster and wrong.
- Both arms start from isolated, equivalent state, arm order is randomized or interleaved, the same task instance is paired across arms, and neither arm may leave caches or files the other benefits from; otherwise with minus without is a comparison, not a treatment effect.
- Repeats matter because agents are inconsistent; tau-bench's pass^k metric asks whether an agent succeeds on all of k trials.
- A written, versioned rule turns the estimate into a verdict, and that rule is an engineering gate that should say what it cannot see.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md#3-capability-delta`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
