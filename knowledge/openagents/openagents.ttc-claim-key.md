---
id: openagents.ttc-claim-key
version: 1
kind: product
title: "Capability claims: key, evidence record, and policy"
summary: >-
  A capability claim has a key K = (A, B, D, S, E, G, M), evidence records on
  it, and a decision policy P that turns the records into a verdict.
tags: [essay, test-time-capabilities, capability-claim, claim-key, policy]
applies_when: >-
  The user asks what a capability claim is, what its key is, or how a claim
  differs from a report or adoption.
answer: >-
  A capability claim is a versioned empirical statement of the marginal effect
  of admitting one identified subject into a particular agent system, not a
  property the subject declares about itself. It has three layers: a claim key
  K = (A, B, D, S, E, G, M), evidence records on that key (each run's estimate
  of the effect with its uncertainty), and a written policy P that turns
  records into a verdict: Better, No clear change, Worse, or adopt. Reports
  are evidence, claims summarize effects, adoption is policy.
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

A capability claim is a versioned empirical statement of the marginal effect of admitting one identified subject into a particular agent system, not a property the subject declares about itself. It has three layers: a claim key K = (A, B, D, S, E, G, M), evidence records on that key (each run's estimate of the effect with its uncertainty), and a written policy P that turns records into a verdict: Better, No clear change, Worse, or adopt. Reports are evidence, claims summarize effects, adoption is policy.

## Details

- In the key, A is the subject, B the locked baseline agent, D the task distribution, S the suite that samples it, E the environment, G the grant, and M how outcomes are measured.
- Policy P is not part of the key: changing an acceptance threshold reinterprets the evidence that exists and cannot change the experiment that was run.
- A reproduction holds K fixed and changes who runs it; an external validation holds D and changes S; a transfer changes D to some D', and is a new claim rather than a stronger version of the old one.
- The same plugin can add twenty points to one agent, nothing to a second, and three points to the first agent on another domain, and none of those results contradicts another, because each belongs to its baseline and distribution.
- Reproducibility cannot be stronger than identity: a claim about exact bytes can be rerun on those bytes, a claim about a version only on what the provider still calls that version, and a claim about an endpoint only on whatever answers there now.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md#the-thesis-capability-is-something-you-can-acquire-at-test-time`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
