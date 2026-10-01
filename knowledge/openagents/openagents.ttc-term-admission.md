---
id: openagents.ttc-term-admission
version: 1
kind: product
title: "Lexicon term 2: capability admission"
summary: >-
  Capability admission is the host's decision that a specific, locked version
  of a component may take part in a run; there are two admissions, evaluation
  and operational.
tags: [essay, test-time-capabilities, admission, lexicon, grant, safety]
applies_when: >-
  The user asks what capability admission is, or what evaluation admission and
  operational admission are.
answer: >-
  Capability admission is the host's decision that a specific, locked version
  of a component may take part in a run. Discovering, installing, enabling,
  granting access to, and admitting are separate decisions; installing should
  grant nothing. Evaluation admission lets a candidate run in a constrained
  sandbox with just enough authority to measure it. Operational admission is
  the later decision, on that evidence, to let it run with the authority real
  use would give it.
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

Capability admission is the host's decision that a specific, locked version of a component may take part in a run. Discovering, installing, enabling, granting access to, and admitting are separate decisions; installing should grant nothing. Evaluation admission lets a candidate run in a constrained sandbox with just enough authority to measure it. Operational admission is the later decision, on that evidence, to let it run with the authority real use would give it.

## Details

- Grant is not authority: a grant records the operations directly made available, while effective authority is the set of effects the component can cause through those operations and every other component it can reach.
- If the operational authority differs materially from the grant under which the claim was established, operational admission requires safety evidence under the operational grant, and if the changed grant can change task behavior it creates a new claim.
- Safety is a constraint, not a score: utility establishes the claim, safety decides operational admissibility, and a dangerous component cannot make up for the danger by being useful enough. Capability evidence is never permission to run.
- A component that reads untrusted data is a path for prompt injection, and a component's own description of itself is not evidence.
- To measure it: record a digest of the locked component set in every result, so a result names exactly what it measured.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md#2-capability-admission`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
