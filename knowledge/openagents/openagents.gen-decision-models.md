---
id: openagents.gen-decision-models
version: 1
kind: product
title: "Why typed decision models make the front feasible"
summary: >-
  A router must be right, fast, and cheap on every turn and answer in a form
  code can act on; typed decision models like Jev do that, where chat models
  trained to please do not.
tags: [essay, general-agent, decision-models, jev, router, typed]
applies_when: >-
  The user asks why typed decision models make routing feasible, what a typed
  decision model is, or why not use a chat model as the router.
answer: >-
  Every composition stands or falls on its front: if deciding is slow,
  expensive, wrong, or answered in prose that something must parse, the
  composition collapses into the generic loop. Chat models trained from human
  preference are optimized to please the reader, which suits assistance and
  not automation, and a router is pure automation. A typed decision model
  takes one state and typed questions and returns probabilities, not prose, in
  a fraction of a second for a fraction of a cent, so a general front can
  decide every turn and be measured like any classifier.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/essays/2026-10-01-the-return-of-the-general-agent.md
evidence:
  - "2026-10-01: written from the essay The Return of the General Agent and checked against its text (#10099); the answer text awaits the owner's copy review."
---

## Answer

Every composition stands or falls on its front: if deciding is slow, expensive, wrong, or answered in prose that something must parse, the composition collapses into the generic loop. Chat models trained from human preference are optimized to please the reader, which suits assistance and not automation, and a router is pure automation. A typed decision model takes one state and typed questions and returns probabilities, not prose, in a fraction of a second for a fraction of a cent, so a general front can decide every turn and be measured like any classifier.

## Details

- Four properties: fast (170 ms median, 235 ms p95), cheap ($0.000014 per judgment in our live door check), typed and calibrated (0.907 held-out route accuracy with prepared-answer precision at 100 % in the latest router measurement), and programmable on the fly (a new route is a new option and labeled rows, not a training run).
- The front itself is extensible at machine speed for the same reason the members are.
- Two cautions: Almeida is a vendor describing his product, so the numbers are ours; and a decision model we do not run is a dependency we do not control. The night before launch the provider account ran out of credits and chat turns lost routing, so the front now fails over across three routes to the same model: TypeSafe directly, the Vercel AI Gateway, and OpenRouter.
- Where it comes from: the essay The Return of the General Agent, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-10-01-the-return-of-the-general-agent.md#why-the-front-is-feasible-now-decision-models`.

## Sources

- `docs/essays/2026-10-01-the-return-of-the-general-agent.md`
