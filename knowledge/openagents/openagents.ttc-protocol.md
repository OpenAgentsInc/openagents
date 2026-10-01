---
id: openagents.ttc-protocol
version: 1
kind: product
title: "How the protocol carries test-time capabilities"
summary: >-
  The OpenAgents NIPs carry each term and stage: plugins as NIP-EXT releases,
  evaluation in NIP-EVAL, grants in NIP-CAP, runs in NIP-RUN, credit in NIP-
  XP, and decisions in NIP-CJ and NIP-DEC.
tags: [essay, test-time-capabilities, protocol, nips, nostr]
applies_when: >-
  The user asks which NIPs carry test-time capabilities, or what the essay
  says about the protocol.
answer: >-
  The lexicon is ours; the wire formats that carry it are the OpenAgents NIPs.
  The essay says which NIP is for what, in the order a capability lives
  through them: found, admitted, run, delegated, recorded, measured, checked,
  credited, adopted, and shared. Examples are NIP-EXT for plugin releases,
  NIP-CAP for operation descriptions and grants, NIP-RUN for a run's lock and
  outcome, NIP-EVAL for reports, checks, validations, and adoption, and NIP-XP
  for credit.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/essays/2026-09-29-test-time-capabilities.md
    - nips/openagents/README.md
evidence:
  - "2026-10-01: written from the essay Test-Time Capabilities and checked against its text (#10099); the answer text awaits the owner's copy review."
---

## Answer

The lexicon is ours; the wire formats that carry it are the OpenAgents NIPs. The essay says which NIP is for what, in the order a capability lives through them: found, admitted, run, delegated, recorded, measured, checked, credited, adopted, and shared. Examples are NIP-EXT for plugin releases, NIP-CAP for operation descriptions and grants, NIP-RUN for a run's lock and outcome, NIP-EVAL for reports, checks, validations, and adoption, and NIP-XP for credit.

## Details

- The essay's section on the NIPs one by one covers NIP-EXT, NIP-CAP, NIP-KB, NIP-CJ, NIP-DEC, NIP-PRG, NIP-CTX, NIP-SESS, NIP-WORK, NIP-RUN, NIP-ATIF, NIP-EVAL, NIP-XP, NIP-POL, NIP-OPT, and NIP-MV.
- It cites only what the NIP files define, and where a term has no carrier yet, it says so. Each status is the whole contract's, as the glossary records it.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md#how-the-protocol-carries-test-time-capabilities`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
- `nips/openagents/README.md`
