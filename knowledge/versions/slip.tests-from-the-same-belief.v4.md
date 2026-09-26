---
id: slip.tests-from-the-same-belief
version: 4
kind: slip
title: Validating inferred behavior only with tests built from the same assumptions
summary: >-
  Self-authored structural and idempotence checks can pass while exact
  behavior remains wrong. Add independent reference comparisons and
  adversarial packet variations, not just tests derived from the
  implementation plan.
tags: [testing, test-oracles, independence, hidden-cases]
applies_when: >-
  An implementation passes locally written acceptance checks but depends on
  inferred semantics, generated outputs, manifests, or optional evidence that
  may vary across inputs.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - risk-scorer-replay-1790394700
  cites:
    - ISTQB, Certified Tester Foundation Level Syllabus, v4.0.1, §4.2.1
evidence: []
---

## Details

Tests written from the same interpretation as the implementation tend to confirm that interpretation rather than challenge it. Structural checks (files, columns, deterministic rebuilds, read-only inputs) are valuable, but they do not establish behavioral parity. Keep those checks and add independent expected results from a trusted reference or separately derived specification.

Exercise variations that challenge assumptions: alternate manifest-selected paths, decoy files, incomplete optional evidence, duplicate records, boundaries, and cases where multiple features interact. Where a reference executable is available for development, compare it directly on both supplied and generated cases; do not make production code depend on that reference unless the contract permits it. The ISTQB, *Certified Tester Foundation Level Syllabus*, v4.0.1, §4.2.1, describes black-box test techniques that derive tests from externally observable behavior rather than implementation structure.

## How to check

Separate contract checks from independent behavioral checks, and make the latter fail when a deliberately incorrect implementation is substituted. For example, compare a candidate against a reference over generated inputs:

```python
for case in generated_cases:
    expected = reference(case)       # independent oracle or trusted fixture
    actual = implementation(case)
    assert actual == expected
```

Also rebuild from a temporary packet whose manifest points to renamed source files and includes irrelevant decoys; verify the selected sources, partial-evidence behavior, and outputs rather than assuming fixed paths or complete traces.
