---
id: openagents.ttc-overview
version: 1
kind: product
title: "Our essay Test-Time Capabilities"
summary: >-
  Test-Time Capabilities is our 2026-09-29 essay: an agent can gain or lose
  abilities while it runs, and each such change should be a measured
  capability claim.
tags: [essay, test-time-capabilities, overview, thesis]
applies_when: >-
  The user asks about our essay Test-Time Capabilities, what it argues, or
  what its main ideas are.
answer: >-
  Test-Time Capabilities is our essay of 2026-09-29. Its thesis: an agent can
  also gain or lose abilities while it runs, without retraining, when a
  plugin, skill, knowledge entry, or another agent is admitted into the run. A
  component is only a candidate; evidence makes a capability claim. Part One
  states the concept, Part Two how OpenAgents implements it, and Part Three
  what we will measure next.
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

Test-Time Capabilities is our essay of 2026-09-29. Its thesis: an agent can also gain or lose abilities while it runs, without retraining, when a plugin, skill, knowledge entry, or another agent is admitted into the run. A component is only a candidate; evidence makes a capability claim. Part One states the concept, Part Two how OpenAgents implements it, and Part Three what we will measure next.

## Details

- The essay has three parts: Part One is vendor-neutral and states the concept, Part Two says how OpenAgents implements it with links to code and dated records, and Part Three lists our own open problems.
- Its TL;DR: test-time compute spends more computation when a model answers; test-time capabilities go one step further. Cheap judgments should come before expensive thinking, and claims others have reproduced can be shared so capabilities compound across a network; both are stated as hypotheses to measure.
- Most of the mechanisms are prior art; the chain is the proposal: candidate, admission, controlled delta, reproduction, external validation, adoption, credit, and revalidation.
- One capability, Project map, was externally validated and adopted on 2026-09-29.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
