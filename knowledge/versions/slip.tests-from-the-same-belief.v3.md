---
id: slip.tests-from-the-same-belief
version: 3
kind: slip
title: Tests written from the same belief as the code
summary: >-
  A passing test on one complete fixture can leave manifest selection, partial
  reference data, and state-machine edge cases untested. Add small independent
  fixtures that vary one contract dimension at a time.
tags: [testing, fixtures, manifests, partial-data, hidden-cases]
applies_when: >-
  A program reads manifest-selected inputs or compares generated results
  against reference rows that may cover only part of the input.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - risk-scorer-replay-1790394263
  cites:
    - pytest contributors, “How to parametrize fixtures and test functions,” “Parametrizing test functions.”
evidence: []
---

## Details

Tests derived only from one visible packet can accidentally share the implementation’s assumptions: default paths may appear equivalent to manifest paths, and complete reference tables may hide behavior for missing reference rows. Build fixtures that deliberately include decoy files beside manifest-selected files, nested paths, and incomplete reference coverage. Treat missing reference rows as missing comparison data—not as evidence that the corresponding input or generated output should be omitted.

Keep separate assertions for input selection, output completeness, and comparisons on rows for which an oracle exists. Parameterized independent cases make it easier to vary one dimension without changing the rest. (pytest contributors, “How to parametrize fixtures and test functions,” “Parametrizing test functions.”)

## How to check

Add a compact fixture with a manifest pointing to non-default files, decoys at conventional paths, and a reference table containing only a subset of requests. Assert that selected inputs control the rebuild, every required output row is still produced, and parity is checked only where reference data exists.

```python
assert set(generated_by_id) == set(all_input_request_ids)
for request_id, reference in reference_by_id.items():
    assert generated_by_id[request_id]["score"] == reference["score"]
```
