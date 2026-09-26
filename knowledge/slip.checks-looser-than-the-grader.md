---
id: slip.checks-looser-than-the-grader
version: 1
kind: slip
title: Checks that normalize or tolerate more than the grader pass work the grader rejects
summary: >-
  A check that strips whitespace, sorts, lowercases, rounds, parses loosely,
  or uses a wide tolerance accepts output that an exact grader rejects. Make
  each check at least as strict as the stated format and precision: compare
  bytes or parsed values exactly where the task is exact, and use the task's
  tolerance, not a convenient one.
tags: [acceptance-tests, output-format, tolerance, precision, exact-match, verification]
applies_when: >-
  Your checks compare output to expected values or formats, and the task
  states an exact format, ordering, precision, or tolerance.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "Python Software Foundation, math.isclose and unittest assertAlmostEqual (relative versus absolute tolerance)"
    - "NumPy documentation, numpy.allclose / testing.assert_allclose (rtol, atol semantics)"
    - "RFC 8259, The JavaScript Object Notation (JSON) Data Interchange Format; RFC 4180, Common Format for CSV Files"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

Loose checks are convenient while iterating and hide exactly the faults an
exact grader catches:

- `strip()`, whitespace collapsing, or case folding before comparison hides
  trailing spaces, missing final newlines, CRLF line endings, and wrong case
  in keys or enum values.
- Sorting both sides hides an ordering requirement; set comparison hides
  duplicates.
- Comparing parsed JSON hides invalid JSON (NaN, trailing commas, single
  quotes) in the file itself; comparing parsed numbers hides required
  formatting (fixed decimals, no exponent, no thousands separator).
- `assert abs(a - b) < 1e-3` when the task says six decimals or "exactly",
  or a relative tolerance applied to values near zero, accepts wrong values;
  `np.allclose` defaults (`rtol=1e-5`, `atol=1e-8`) may be looser or stricter
  than the task.
- Checking a column exists, a file is non-empty, or a count is positive
  accepts wrong contents.

## How to check

For every check, write down the grader's likely comparison from the task
text: exact bytes, exact parsed values, a stated tolerance, or a stated
ordering. Remove any normalization the task does not allow, parse output with
a strict parser, and use the task's tolerance with the right mix of relative
and absolute terms. Then run the check against a deliberately off output (one
extra space, one swapped row, one value off in the last required digit) and
confirm it fails.
