---
id: openagents.ttc-term-reach
version: 1
kind: product
title: "Lexicon term 4: reach and restraint"
summary: >-
  Reach is how often a candidate is used where it helps; restraint is how
  often it is withheld where it hurts. Both are properties of the agent,
  selector, and component together.
tags: [essay, test-time-capabilities, reach, restraint, lexicon]
applies_when: >-
  The user asks what reach and restraint mean for capabilities.
answer: >-
  Reach is how often the host exposes and the agent uses a candidate on tests
  where its conditional delta is positive; restraint is how often the host
  withholds it, or the agent leaves it alone, where the delta is negative or
  zero. An installed capability the agent never invokes changes no outcome, so
  it measures as no capability at all. Because admission is symmetric, on
  tasks where a component hurts, withholding it is the capability, and it
  belongs to the router or policy that withheld it.
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

Reach is how often the host exposes and the agent uses a candidate on tests where its conditional delta is positive; restraint is how often the host withholds it, or the agent leaves it alone, where the delta is negative or zero. An installed capability the agent never invokes changes no outcome, so it measures as no capability at all. Because admission is symmetric, on tasks where a component hurts, withholding it is the capability, and it belongs to the router or policy that withheld it.

## Details

- Keep three things apart: the declared label on a test (the author's hypothesis), the exposure and invocation record (an instrumentation fact), and the observed conditional outcome (the causal evidence).
- A should-fire test the baseline already passes proves no router failure, and a component that fired and changed nothing reached nothing.
- The agent's own account of its use is not evidence: what an agent says it used and what changed its decision come apart.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md#4-reach-and-restraint`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
