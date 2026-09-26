---
id: slip.frozen-checks-miss-graded-requirements
version: 1
kind: slip
title: Passing your own acceptance checks is not meeting every graded requirement
summary: >-
  A self-authored acceptance suite that goes green proves only that the code
  satisfies the checks that were written, not that every stated requirement is
  met. Enumerate each requirement and deliverable from the instruction, map one
  check to each, and treat an unchecked requirement as unfinished work rather
  than trusting a passing subset.
tags: [acceptance-tests, requirements, coverage, verification, self-grading]
applies_when: >-
  You freeze or write your own checks and then decide the task is done because
  they pass, especially when a separate grader or hidden suite will score more
  conditions than you enumerated.
status: admitted
author: claflampernton (hand-written)
provenance:
  written_from:
    - reference
    - batched-eval-parity-1790405023
    - fin-saccr-rwa-1790405023
    - gsea-proteomics-1790402816
  cites:
    - "Ian Sommerville, Software Engineering, 10th edition (2015), chapter 4, Requirements Engineering, requirements traceability"
    - "Barr, Harman, McMinn, Shahbaz, and Yoo, The Oracle Problem in Software Testing: A Survey, IEEE Transactions on Software Engineering 41(5) (2015)"
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

A check suite you author yourself measures the requirements you thought of.
When a hidden or independent grader scores the work, it can test conditions you
never enumerated: values compared against a reference, invariants that must hold
across inputs, deliverables in a second file or format, edge inputs, and
prohibitions. A green run on your own subset is consistent with all of those
being wrong or absent.

Build a requirement inventory before deciding the work is finished:

- Read every provided instruction, spec, and data file. List each distinct
  requirement, deliverable, output path, format, and "must"/"must not" as a
  separate line.
- Map exactly one check to each line. A requirement with no check is not done;
  a check that covers several requirements at once hides which one failed.
- Derive each checks expected value from the tasks definition or an
  independent oracle, not from the output your own code happens to produce.
- When several requirements are stated ("also", "in addition", "as well as"),
  confirm each independently; fixing the most visible one leaves the rest.

Completion is coverage of the stated requirements, not a passing count of the
checks you chose to write. Absence of a check is not evidence of success.

## How to check

Write the requirement inventory as an explicit list and tick each item against
a check that fails when the requirement is unmet. Before finishing, confirm
every listed requirement has a mapped, passing check, and that no deliverable,
output artifact, or stated constraint is missing from the list. If a check
would still pass when the corresponding requirement is deleted from the code,
it does not cover that requirement.
