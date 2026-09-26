---
id: slip.one-fix-when-many-are-broken
version: 1
kind: slip
title: Fixing the visible symptom when the task names several defects
summary: >-
  When a task says several modules or utilities are broken, fixing the one
  that explains the visible symptom leaves the rest. Audit every function the
  task points at against its definition, one at a time, before declaring the
  work done.
tags: [debugging, audit, multiple-bugs, planted-bugs, verification, code-review]
applies_when: >-
  The task says code is broken in more than one place, names several files
  or components, or says "not just" one part.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - "Zeller, Why Programs Fail, 2nd edition (2009), chapter 6, Scientific Debugging"
    - "Myers, Sandler, and Badgett, The Art of Software Testing, 3rd edition (Wiley, 2011), chapter 3, Code Inspections"
evidence: []
---

## Details

A task with several planted defects usually has one that produces the most
visible symptom and others that change results only a little. Once the
visible symptom goes away, it's tempting to stop.

What to do:

1. List every function in the files the task names. Treat each as suspect.
2. For each, write down what it should compute, from its name, the task, and
   a reference definition, and compare line by line: formulas, estimators,
   signs, units, off-by-one bounds, default arguments, sort order, and edge
   cases such as empty input or zero norms.
3. Check each function on a small input whose answer you can compute by
   hand or with a trusted library.
4. Only then check the whole pipeline end to end.

A function the task names but that "looks fine" deserves the same check as
the others; defects in utility code are often a single character.
