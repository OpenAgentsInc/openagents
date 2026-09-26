---
id: slip.placeholder-checks-prove-nothing
version: 1
kind: slip
title: Placeholder checks, a clean compile, or an empty test run prove nothing
summary: >-
  Creating self-authored tests that merely fail or restate assumptions does
  not validate an implementation and can consume the run budget. Read the full
  task constraints and verifier metrics, then run meaningful differential and
  resource checks.
tags: [acceptance-tests, verification, workflow, differential-testing]
applies_when: >-
  An implementation task provides an authoritative verifier or explicit
  correctness and resource budgets, especially when scalability is part of
  grading.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - distributed-dedup-1790444805309
  cites:
    - Myers, Sandler, Badgett, The Art of Software Testing, 3rd ed. (Wiley, 2011), chapter 2, The Psychology and Economics of Software Testing
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details
Do not spend steps creating placeholder scripts that always fail, copying them to a purported frozen location, or treating compilation and an empty test suite as validation. Such checks provide no evidence about output correctness or resource behavior. Instead extract every constraint up front, implement against the actual contract, and validate with small differential cases against a simple oracle plus scale/resource measurements that match the verifier's dimensions (for example join output, shuffle read/write, latency, and memory). Compilation is necessary but does not establish either correctness or scalability.

For a general testing framework, see *The Art of Software Testing*, Glenford J. Myers, Corey Sandler, and Tom Badgett, 3rd ed., chapter 2, “The Psychology and Economics of Software Testing” (a test that cannot find an error has no value).

## How to check
Before declaring completion, map each stated requirement to a test or measurement. Ensure each test can pass and exercises the implementation rather than printing a placeholder. Run the actual project/verifier commands and inspect failures by metric; avoid relying on `compile` or a no-tests `test` result as acceptance.
