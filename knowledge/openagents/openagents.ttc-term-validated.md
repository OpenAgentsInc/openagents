---
id: openagents.ttc-term-validated
version: 1
kind: product
title: "Lexicon term 8: externally validated capability claim"
summary: >-
  An externally validated claim is a reproduced claim whose improvement
  persists on new tasks from the same distribution, written independently of
  the author.
tags: [essay, test-time-capabilities, external-validation, lexicon, test-set]
applies_when: >-
  The user asks what an externally validated capability claim is, or how
  validation differs from reproduction or transfer.
answer: >-
  An externally validated capability claim is a reproduced claim whose
  improvement persists on new tasks from the same intended distribution,
  written independently of the artifact's author and not available to the
  author before the artifact was locked. Reproduction proves reproducibility,
  not external validity: three people rerunning the same six tests says
  nothing about whether the plugin was fitted to those six. Validation belongs
  before adoption, not after it.
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

An externally validated capability claim is a reproduced claim whose improvement persists on new tasks from the same intended distribution, written independently of the artifact's author and not available to the author before the artifact was locked. Reproduction proves reproducibility, not external validity: three people rerunning the same six tests says nothing about whether the plugin was fitted to those six. Validation belongs before adoption, not after it.

## Details

- Reproduction holds K fixed and changes who runs it; external validation keeps D and changes S; transfer deliberately changes D to a different distribution and is a new claim.
- A different author is provenance, not independence. Independence needs chronology and information flow: the plugin was locked before the suite was revealed to its author, or the suite was hidden.
- A protocol can check distinct authorship keys and subject-before-suite chronology; it cannot establish social independence, which the essay treats as unsolved.
- The externally validated delta is usually smaller than the original; how much smaller is the finding.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md#8-externally-validated-capability-claim`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
