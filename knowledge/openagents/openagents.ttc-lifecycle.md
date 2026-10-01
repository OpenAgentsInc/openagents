---
id: openagents.ttc-lifecycle
version: 1
kind: product
title: "The lifecycle of a capability"
summary: >-
  A candidate runs under evaluation admission, gets a controlled with-and-
  without run, is reproduced, externally validated, operationally admitted,
  adopted, credited, and revalidated.
tags: [essay, test-time-capabilities, lifecycle, admission, revalidation]
applies_when: >-
  The user asks about the lifecycle of a test-time capability, its stages, or
  how a claim can be rejected or revoked.
answer: >-
  The lifecycle: a candidate subject with its strongest identity, evaluation
  admission into a sandbox, a controlled with-and-without run producing an
  evidence record, independent reproduction, external validation on tasks the
  author did not write, operational admission, adoption against the current
  defaults, credit, and monitoring and revalidation. There are three exits and
  one revocation, and none can be bought back by the others.
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

The lifecycle: a candidate subject with its strongest identity, evaluation admission into a sandbox, a controlled with-and-without run producing an evidence record, independent reproduction, external validation on tasks the author did not write, operational admission, adoption against the current defaults, credit, and monitoring and revalidation. There are three exits and one revocation, and none can be bought back by the others.

## Details

- A component the evidence shows unsafe is rejected however useful; one with no favorable effect or a failed non-inferiority bound is rejected however safe; one that regresses the default set when composed is rejected however well it did alone.
- A claim that stops holding after adoption, because something in its scope changed, is quarantined or revoked rather than kept.
- There are two admissions because a candidate has to run somewhere before anyone knows whether it is safe: the first is a sandbox that exists to produce the evidence, and the second is the decision, on that evidence, to let it run with real authority.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md#the-lifecycle-in-one-figure`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
