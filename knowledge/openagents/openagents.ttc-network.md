---
id: openagents.ttc-network
version: 1
kind: product
title: "How capabilities compound across a network"
summary: >-
  A capability can come from anyone, be tested by anyone, and, once adopted,
  reach every agent using the same defaults; the evaluation system is the
  network's objective function.
tags: [essay, test-time-capabilities, network, compounding, goodhart]
applies_when: >-
  The user asks how capabilities compound across a network, or what the essay
  says about the evaluation system as an objective function.
answer: >-
  A test-time capability can come from anyone, be tested by anyone, and, once
  adopted, reach every agent that uses the same defaults without a training
  run. The unit that compounds is a capability claim with independent
  evidence. A network would add more sources of capability, more verification,
  inheritance, and credit that tracks use. This is a hypothesis: whether
  adding participants makes an agent measurably better has to be shown.
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

A test-time capability can come from anyone, be tested by anyone, and, once adopted, reach every agent that uses the same defaults without a training run. The unit that compounds is a capability claim with independent evidence. A network would add more sources of capability, more verification, inheritance, and credit that tracks use. This is a hypothesis: whether adding participants makes an agent measurably better has to be shown.

## Details

- The evaluation system is the objective function of the network: once adoption and credit depend on evaluations, authors build what gets adopted and the defaults inherit whatever those rules reward, which is Goodhart's problem at the scale of a network.
- Candidate selection is the winner's curse: make forty variants, publish the lucky one, and a faithful rerun reproduces the luck. External validation on a suite the author never saw removes most of it.
- The question becomes whether a decentralized network can construct a trustworthy empirical record of which runtime components software can actually depend on.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md#how-capabilities-compound-across-a-network`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
