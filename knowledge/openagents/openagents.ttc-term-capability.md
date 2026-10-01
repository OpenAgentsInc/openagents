---
id: openagents.ttc-term-capability
version: 1
kind: product
title: "Lexicon term 1: test-time capability (TTCap)"
summary: >-
  A test-time capability is a change in what outcomes an agent can achieve
  under stated resource constraints, at inference time, because a component
  was admitted.
tags: [essay, test-time-capabilities, ttcap, lexicon, capability-claim]
applies_when: >-
  The user asks what a test-time capability or TTCap is.
answer: >-
  A test-time capability is a change in what outcomes an agent can achieve
  under stated resource constraints, at inference time, without updating
  weights, because a component was admitted to the run. It is stated only as a
  capability claim with a key, evidence records, and a policy. It separates
  what a component is from what it does for a particular agent on particular
  work; a component with no claim yet is a candidate capability.
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

A test-time capability is a change in what outcomes an agent can achieve under stated resource constraints, at inference time, without updating weights, because a component was admitted to the run. It is stated only as a capability claim with a key, evidence records, and a policy. It separates what a component is from what it does for a particular agent on particular work; a component with no claim yet is a candidate capability.

## Details

- B, the baseline, is the whole executable agent, not the agent minus the component: model and version, system instructions, router, the default set, sampling settings, runtime, and provider endpoint.
- Whatever in B cannot be pinned, such as the weights behind a hosted endpoint, the claim should name as unpinned.
- To measure one: a with-and-without evaluation on tests written for what the component claims to help with, reported with every element of the scope.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md#1-test-time-capability-ttcap`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
