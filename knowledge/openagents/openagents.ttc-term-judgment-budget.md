---
id: openagents.ttc-term-judgment-budget
version: 1
kind: product
title: "Lexicon term 5: judgment budget"
summary: >-
  The judgment budget is the time and money spent deciding how to answer
  before spending anything on answering; the judgment must be much cheaper
  than the work it can avoid.
tags: [essay, test-time-capabilities, judgment-budget, calibration, lexicon]
applies_when: >-
  The user asks what a judgment budget is, or why calibration matters for
  routing.
answer: >-
  The judgment budget is the time and money a system spends deciding how to
  answer before it spends anything on answering. It is the per-question
  allocation lesson of test-time compute applied one level up: before
  allocating thinking, decide whether the turn needs a large model at all, and
  which capability should handle it. The proposal is one cheap, typed judgment
  as a general allocation mechanism over every capability source, with routing
  as one use of it.
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

The judgment budget is the time and money a system spends deciding how to answer before it spends anything on answering. It is the per-question allocation lesson of test-time compute applied one level up: before allocating thinking, decide whether the turn needs a large model at all, and which capability should handle it. The proposal is one cheap, typed judgment as a general allocation mechanism over every capability source, with routing as one use of it.

## Details

- The judgment's probabilities are the interface, so calibration is part of a probabilistic claim: accuracy asks whether the judge chose correctly, calibration asks whether a 0.97 should be treated as a 97 % event.
- A router acts at thresholds, so the measurement that matters is the operating point: precision and coverage above each serving threshold, the abstention or escalation rate, and the risk-coverage curve.
- Close prior art is model routing and cascades such as FrugalGPT and RouteLLM.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md#5-judgment-budget`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
