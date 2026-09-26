---
id: slip.tests-from-the-same-belief
version: 1
kind: slip
title: Tests written from the same belief as the code
summary: >-
  A test whose expected value comes from your own reading of the code, or
  from a comment in it, passes whenever the code matches that reading, right
  or wrong. Derive expected values from an independent source: a reference
  definition, a trusted library, brute force, or a property.
tags: [testing, test-oracle, acceptance-tests, verification, properties, independent-check]
applies_when: >-
  You write tests or checks for code you're fixing, especially numerical or
  statistical code, or all your tests pass but the task may still fail.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - "Barr, Harman, McMinn, Shahbaz, and Yoo, The Oracle Problem in Software Testing: A Survey, IEEE Transactions on Software Engineering 41(5) (2015)"
    - "Myers, Sandler, and Badgett, The Art of Software Testing, 3rd edition (Wiley, 2011)"
evidence: []
---

## Details

The hard part of a test is its oracle: how it knows the right answer. If the
oracle is the same belief that wrote the code, the test can't catch a wrong
belief. That's how a wrong estimator, sign, or default survives a green
test suite.

Independent oracles, strongest first:

- **A trusted implementation**, such as SciPy or NumPy, on the same input.
- **The reference definition**, computed a second, simpler way (a
  double loop instead of vectorized code).
- **Properties the definition implies**: a divergence of a sample with
  itself is 0; a symmetric measure is symmetric; an unbiased estimator
  averages near its true value over many random draws; a distance is 0 only
  for equal inputs.
- **Hand-worked small cases** with the arithmetic shown in a comment.

For each requirement, ask: "If the code used the other common variant, would
this test fail?" If not, the test doesn't check the requirement.
