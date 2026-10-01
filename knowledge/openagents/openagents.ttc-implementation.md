---
id: openagents.ttc-implementation
version: 1
kind: product
title: "How OpenAgents implements test-time capabilities"
summary: >-
  In OpenAgents a claim decides three things: the Gym's gate rates it Better,
  other trainers' checks reproduce it and XP is paid, and it can be adopted
  into Coder's defaults.
tags: [essay, test-time-capabilities, implementation, adoption, project-map]
applies_when: >-
  The user asks how OpenAgents implements test-time capabilities, what a claim
  decides in our system, or how the Gym rates a plugin Better.
answer: >-
  Part Two of the essay says a claim decides three concrete things in
  OpenAgents. The Gym's ext-eval-v2 gate rates a plugin Better only when more
  tests pass with it, the score gain clears the spread between repeats, and
  cost and time stay within bounds. A different trainer's check reproduces it
  and earns XP whether it confirms or disputes. And an operator can adopt it
  into every Coder's defaults once three distinct trainers' checks confirmed
  it and a suite by someone else externally validated it.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/essays/2026-09-29-test-time-capabilities.md
    - docs/extensions/measurements/2026-09-29-first-adoption.md
evidence:
  - "2026-10-01: written from the essay Test-Time Capabilities and checked against its text (#10099); the answer text awaits the owner's copy review."
---

## Answer

Part Two of the essay says a claim decides three concrete things in OpenAgents. The Gym's ext-eval-v2 gate rates a plugin Better only when more tests pass with it, the score gain clears the spread between repeats, and cost and time stay within bounds. A different trainer's check reproduces it and earns XP whether it confirms or disputes. And an operator can adopt it into every Coder's defaults once three distinct trainers' checks confirmed it and a suite by someone else externally validated it.

## Details

- A component with no such report is a candidate: the gate has not rated it, no check can confirm it, and it cannot be adopted.
- One capability, Project map, went the whole way on 2026-09-29: it passed 5 of 6 tests with the plugin against 2 of 6 without, three trainer keys reproduced it, a second test set validated it at 4 of 6 against 2 of 6, and it was adopted into coder-defaults.
- The essay's Part Two maps each of the eleven terms to a component and a NIP, with the status as the glossary records it.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md#what-is-a-capability-means-in-our-system`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
- `docs/extensions/measurements/2026-09-29-first-adoption.md`
