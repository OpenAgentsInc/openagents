---
id: slip.validate-planner-artifacts-against-exact-evaluator
version: 1
kind: slip
title: Validate generated plans with the exact evaluator and input identity
summary: >-
  A plausible optimization or internal objective is not evidence that a
  generated plan satisfies the evaluator's constraints. Re-read saved
  artifacts, check exact coverage and structural invariants, recompute every
  metric, and verify input hashes.
tags: [validation, optimization, artifacts, reproducibility]
applies_when: >-
  A script produces plans, schedules, assignments, or other artifacts scored
  by a provided cost model or strict schema.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - llm-inference-batching-scheduler
  cites:
    - National Institute of Standards and Technology, Secure Hash Standard (SHS), FIPS PUB 180-4, §6
    - Python Software Foundation, hashlib — Secure hashes and message digests, documentation section “Hash algorithms”
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

Do not stop after the optimizer reports a good objective. Validate the serialized outputs independently with the authoritative evaluator, including every hard threshold and the evaluator's actual percentile, padding, rounding, shared-resource, and sequential-time semantics. Check that every input entity appears exactly as required, with no duplicates or omissions; every group has internally consistent configuration; and global limits such as the number of distinct configurations hold across all outputs, not separately per output.

Hash inputs before or during planning and verify their hashes during final validation when input preservation matters. Re-read the written artifacts rather than trusting in-memory objects, and run the generator twice or compare output hashes to establish deterministic regeneration. Keep feasibility margins: candidates near a threshold can fail due to rounding or aggregation details.

## How to check

A generic validation pattern:

```python
from collections import Counter
assert Counter(row[key] for row in output_rows) == Counter(row[key] for row in input_rows)
assert all(row["configuration"] == configuration_by_group[row["group"]]
           for row in output_rows)
metrics = authoritative_evaluator(input_rows, output_rows)
for name, limit in limits.items():
    assert metrics[name] < limit  # use <= only if the specification permits equality
assert len({tuple(shape.items()) for shape in all_shapes}) <= global_shape_cap
```

Also compare input SHA-256 hashes before and after, and run validation on JSON/CSV data parsed back from disk. Definitions: NIST, *Secure Hash Standard (SHS)*, FIPS PUB 180-4, §6 (hash computation); Python Software Foundation, *hashlib — Secure hashes and message digests*, documentation section “Hash algorithms.”
