---
id: slip.comments-in-broken-code
version: 1
kind: slip
title: Trusting a comment in code the task calls broken
summary: >-
  In code the task says is broken, a comment or docstring that declares a
  choice deliberate ("uses the biased estimator", "intentionally", "by
  design") is part of what's under suspicion. Check the choice against the
  standard definition instead of trusting the comment.
tags: [debugging, comments, docstrings, code-review, planted-bugs, statistics, verification]
applies_when: >-
  The task says code is broken, buggy, or has defects, and a comment,
  docstring, or name in that code states which variant, formula, unit, or
  behavior it uses.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - "Kernighan and Pike, The Practice of Programming (1999), section 1.6, Comments"
    - "Zeller, Why Programs Fail, 2nd edition (2009), chapter 6, Scientific Debugging"
evidence: []
---

## Details

A comment says what its author believed, not what's right. When a task
states that code is broken, the comments came with the broken code: a
defect is often a deliberate choice of the wrong variant, and the comment
describing that choice reads as reassurance. Typical forms:

- "Uses the biased estimator", "population variance", "one-sided test".
- "Intentionally", "by design", "this is correct", "do not change", "as
  specified".
- A name that claims a property: `safe_divide`, `normalized`, `_ms`.

The mistake is to read such a line as a requirement. Models and people both
tend to preserve what a comment defends, and tests written after reading it
encode the same belief, so they pass.

## What to do

1. List each claim the comments and names make about a method, a formula, a
   unit, or a threshold.
2. For each, find the standard definition in a reference (a textbook, the
   paper that defines it, or a library's documentation) and ask which
   variant the task's requirements need. For example, a statistic that should
   read near 0 on stable data needs an unbiased estimator.
3. If the code or comment disagrees with what the task needs, fix the code
   and rewrite the comment. Say in your rationale that the comment was wrong.
4. Write acceptance tests from the definition or an independent
   computation, never from the comment.

A comment is evidence only when the task itself states the same choice.
