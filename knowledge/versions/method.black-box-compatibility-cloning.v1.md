---
id: method.black-box-compatibility-cloning
version: 1
kind: method
title: Clone executable behavior with structured probes and differential tests
summary: >-
  When replacing an executable whose implementation is unavailable or stale,
  infer behavior through targeted probes and, when possible, binary
  inspection; then validate an ordinary standalone implementation against the
  reference with boundary cases and randomized differential tests.
tags: [reverse-engineering, differential-testing, compatibility]
applies_when: >-
  A replacement must reproduce a reference program's behavior for the same
  input schema, including branches, defaults, parsing quirks, and output
  formatting, without depending on the reference at runtime.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - risk-scorer-replay
  cites:
    - McKeeman, “Differential Testing for Software,” “Differential Testing” section.
    - GNU Project, GNU Binutils Documentation, “objdump” section.
evidence: []
---

## Details

Treat observed outputs as evidence for a behavioral specification, not as examples to memorize. First inspect available documentation and traces. If the executable exposes symbols or is inspectable, use disassembly and constant/string inspection to form hypotheses; confirm those hypotheses with probes before implementing them. Probe one factor at a time, especially around branch boundaries, missing values, malformed inputs, and equivalent representations such as timestamps with different offsets.

Implement the inferred rules as normal source code. Keep the reference executable out of the runtime path. During development, compare the candidate and reference on hand-picked probes and generated inputs; investigate every mismatch rather than loosening comparisons indiscriminately. Use tolerances only where the output contract is genuinely approximate, and separately check textual formatting and deterministic ordering when those are contractual.

This follows differential testing: execute equivalent inputs against multiple implementations and use discrepancies to find behavioral differences (McKeeman, “Differential Testing for Software,” “Differential Testing” section). For ELF inspection, consult the GNU Binutils documentation, “objdump” section.

## How to check

A minimal development harness can compare a candidate against a reference without embedding the reference in production code:

```python
def differential_check(cases, reference, candidate, normalize=lambda x: x):
    mismatches = []
    for case in cases:
        expected = normalize(reference(case))
        actual = normalize(candidate(case))
        if actual != expected:
            mismatches.append((case, expected, actual))
    return mismatches

assert not differential_check(probes, reference, candidate)
assert not differential_check(generated_cases, reference, candidate)
```

Include values immediately before, at, and after each inferred boundary, plus missing, empty, malformed, and out-of-range inputs. Run the same checks after removing the reference executable from the candidate's runtime environment.
