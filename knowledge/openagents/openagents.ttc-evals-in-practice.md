---
id: openagents.ttc-evals-in-practice
version: 1
kind: product
title: "Lessons from running our evals"
summary: >-
  Our v1 gate rated a faster-but-not-more-correct plugin Better and was
  replaced the same day; a grader bug flipped a result; old results stay
  readable under old digests.
tags: [essay, test-time-capabilities, evals, lessons, gate, grader]
applies_when: >-
  The user asks what we learned running our evals, or what went wrong with the
  v1 gate.
answer: >-
  Two lessons from the essay. The v1 gate was wrong, and we replaced it: under
  ext-eval-v1 a plugin that made Coder faster but no more correct read Better,
  which we found in a live run where both arms passed the same tests, and
  under ext-eval-v2 faster or cheaper alone is No clear change. And a grader
  bug flipped a result: it looked for not found and missed a different
  phrasing, so we fixed the pattern and released a new test set version. The
  old versions stay readable.
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

Two lessons from the essay. The v1 gate was wrong, and we replaced it: under ext-eval-v1 a plugin that made Coder faster but no more correct read Better, which we found in a live run where both arms passed the same tests, and under ext-eval-v2 faster or cheaper alone is No clear change. And a grader bug flipped a result: it looked for not found and missed a different phrasing, so we fixed the pattern and released a new test set version. The old versions stay readable.

## Details

- In each case the metric, not the agent, produced the verdict. Versioned gates and test sets are how we keep such fixes from rewriting history.
- A third lesson is one we have not yet paid for: a typed decision grader may be a better instrument for machine-consumed criteria than a prose judge, but we have not established that, and neither grader gets epistemic privilege until both are measured against ground truth.
- On benchmarks, we keep in-sample development wins apart from out-of-sample results; benchmarks check Coder as a whole, and evals decide what goes into it.
- Where it comes from: the essay Test-Time Capabilities, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md#our-evals-in-practice`.

## Sources

- `docs/essays/2026-09-29-test-time-capabilities.md`
