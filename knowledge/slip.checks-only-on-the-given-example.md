---
id: slip.checks-only-on-the-given-example
version: 1
kind: slip
title: Passing on the provided example says little about the inputs a grader will use
summary: >-
  Sample inputs in a task are usually small and typical. Graders often run
  other inputs: edge values, empty or malformed records, larger sizes, other
  seeds, or a hidden dataset of the same shape. Derive cases from each stated
  rule and input constraint, generate varied and adversarial inputs, and test
  at the stated scale and time limit.
tags: [acceptance-tests, generalization, edge-cases, property-based-testing, overfitting]
applies_when: >-
  Your checks run the solution only on the example files or the single case
  shown in the instructions, and the grader will run inputs you have not seen.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "Claessen and Hughes, QuickCheck: A Lightweight Tool for Random Testing of Haskell Programs (ICFP 2000)"
    - "Hypothesis documentation (property-based testing for Python): strategies and shrinking"
    - "Glenford Myers et al., The Art of Software Testing, 3rd ed. (Wiley, 2011), chapter 4: equivalence partitioning and boundary-value analysis"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

Code tuned until the example passes can encode the example: constants read
off the sample, assumptions that hold only for it (sorted input, unique keys,
a fixed number of rows, no missing values), or behavior that happens to be
right at that size. A grader with fresh inputs finds each of these.

Build cases from the specification, not from the sample:

- **Partitions and boundaries.** For each rule and input constraint, pick a
  value just inside, on, and just outside each boundary, plus empty input,
  one element, duplicates, unsorted order, missing or null fields, Unicode,
  and the extremes of stated ranges.
- **Properties.** State what must hold for every valid input (round trip,
  conservation of totals, idempotence, ordering, agreement with a slow
  reference implementation) and check it on many generated inputs with a
  property-based tool; keep the seeds of failures.
- **Scale and time.** Generate an input at the largest stated size and time
  the solution under the stated limit; quadratic code often passes the
  sample and times out on the real set.
- **Variation.** If inputs come from a generator or seed, run several seeds;
  if a hidden dataset shares a schema, perturb the sample (shuffle rows,
  rename ids, scale values) and confirm outputs change as they should.

## How to check

Search the solution for literals that appear in the sample data. Then run the
generated cases, including the largest one, and confirm every property holds
and the run fits the time limit.
