---
id: openagents.ttc-meaning-of-capability
version: 1
kind: product
title: "What the word capability means in the essay"
summary: >-
  The essay uses capability for a measured ability of an agent, not the older
  security sense of an unforgeable token; the two meet only at admission.
tags: [essay, test-time-capabilities, capability, object-capability, grant]
applies_when: >-
  The user asks what the word capability means in our essay, or how it relates
  to capability security.
answer: >-
  In computer security a capability is an unforgeable token that names an
  object and carries the rights to use it, from Dennis and Van Horn in 1966.
  In our essay, capability means a measured ability of an agent, established
  by a claim. The two meet at admission: admitting a component usually grants
  it some authority, and a claim's scope names that grant as G. But a grant
  says nothing about whether the component helps, and a favorable delta grants
  nothing.
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

In computer security a capability is an unforgeable token that names an object and carries the rights to use it, from Dennis and Van Horn in 1966. In our essay, capability means a measured ability of an agent, established by a claim. The two meet at admission: admitting a component usually grants it some authority, and a claim's scope names that grant as G. But a grant says nothing about whether the component helps, and a favorable delta grants nothing.

## Details

- Throughout the essay, grant or authority is the security sense and capability is the measured sense.
- The essay draws four ideas from the object-capability tradition: permission is not authority, an arena with terms of entry, the reliance set, and letting designation carry authority.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md#what-the-word-capability-means-here`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
