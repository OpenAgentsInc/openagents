---
id: openagents.ttc-assistance-automation
version: 1
kind: product
title: "Assistance and automation set different bars"
summary: >-
  In assistance a person reads and can repair the output; in automation
  software acts on it, so a capability meant for automation needs a second,
  higher bar.
tags: [essay, test-time-capabilities, assistance, automation, typed-judgment]
applies_when: >-
  The user asks about the difference between assistance and automation in our
  Test-Time Capabilities essay.
answer: >-
  In assistance a human reads the output and can inspect, repair, or reject
  it. In automation software consumes the output and acts on it, often with
  nobody watching, so the output has to be something a machine can depend on.
  A capability meant for assistance needs a favorable effect on its declared
  primary outcome; one meant for automation also needs a machine-readable
  output contract, bounded authority, calibrated uncertainty where it decides
  probabilistically, and repeatable reliability.
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

In assistance a human reads the output and can inspect, repair, or reject it. In automation software consumes the output and acts on it, often with nobody watching, so the output has to be something a machine can depend on. A capability meant for assistance needs a favorable effect on its declared primary outcome; one meant for automation also needs a machine-readable output contract, bounded authority, calibrated uncertainty where it decides probabilistically, and repeatable reliability.

## Details

- The essay's corollary: code generation changes the cost of constructing a program; a test-time capability changes the set of behaviors the running program can express.
- Typed probabilistic judgment is one proposed example: ordinary code keeps the state machine, the effects, and the invariants, and a learned decision supplies judgment where deterministic rules were too brittle or too expensive to write.
- The essay cites Almeida's argument that human-preference training produced the assistance bias, and says that causal claim is his argument, not a premise of the essay.
- A capability claim tells you that something helps; automation asks whether software can safely depend on it, and the first answer does not give the second.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md#assistance-and-automation-set-different-bars`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
