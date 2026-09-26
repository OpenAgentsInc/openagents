---
id: method.black-box-compatibility-cloning
version: 2
kind: method
title: Clone black-box behavior with structured probes and differential tests
summary: >-
  Reconstruct an unavailable or changing component by separating its behavior
  into dimensions, probing boundaries and interactions, and comparing an
  ordinary source implementation against a reference. Use this when examples
  or stale documentation do not fully specify the contract; probes are
  evidence, not a substitute for testing generalization.
tags: [black-box-testing, differential-testing, compatibility, reverse-engineering]
applies_when: >-
  A component must be reimplemented from observable behavior and a reference
  executable or service is available during investigation but must not be a
  runtime dependency.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - risk-scorer-replay
  cites:
    - McKeeman, William M., “Differential Testing,” section “Differential Testing,” 1998.
    - Free Software Foundation, GNU Binutils, “objdump,” sections “objdump” and “Overview.”
evidence: []
---

## Details

Build a behavior map before implementing: identify inputs, outputs, routing or version boundaries, defaults, parsing rules, categorical cases, piecewise regions, and stateful behavior. Change one factor at a time to isolate effects, then probe interactions and boundary values; include missing, malformed, extreme, and reordered inputs when they are in scope. Prefer a compact probe matrix that distinguishes competing hypotheses over a large collection of arbitrary examples.

When black-box probes leave ambiguity, inspect available executable metadata or disassembly as supporting evidence, not as a replacement for behavioral validation. Keep the final implementation as ordinary source code and separate the reference adapter from production code so the reference cannot accidentally become a runtime dependency.

Differential testing compares implementations on the same inputs. Start with known traces, then generate varied inputs and compare normalized outputs; retain mismatches as regression cases. Randomized agreement is useful evidence but does not prove correctness: target untested branches and boundaries, and check invariants or metamorphic properties where possible. This follows the differential-testing approach described by McKeeman and GNU Binutils' documentation of executable disassembly tools.

## How to check

Use an investigation-only reference adapter and compare parsed outputs, not incidental formatting, for a deterministic generated corpus:

```python
for case in cases:
    expected = reference(case)       # investigation/test harness only
    actual = candidate(case)          # standalone implementation
    assert normalize(actual) == normalize(expected), case
```

Include explicit cases on both sides of each discovered threshold, absent and malformed fields, and combinations of factors. Finally, search the production source for reference-executable invocations and run the same regression corpus with the reference unavailable.
