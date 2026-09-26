---
id: statistics.gsea-small-phenotype-permutations
version: 2
kind: edge-case
title: Small classes limit phenotype-permutation GSEA
summary: >-
  With few samples per phenotype, phenotype-label permutations provide a
  limited null distribution even when Broad GSEA completes successfully. Check
  class sizes and heed the tool’s warning before interpreting nominal
  p-values or NES as robust.
tags: [gsea, permutation, small-sample]
applies_when: >-
  Running phenotype-permutation GSEA, especially when either phenotype has
  fewer than seven samples.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - gsea-proteomics-1790395844
    - gsea-proteomics-1790396068
  cites:
    - "Subramanian et al., “Gene set enrichment analysis: A knowledge-based approach for interpreting genome-wide expression profiles,” PNAS, Methods."
    - "Broad Institute, GSEA User Guide, “GSEA Parameters: Permutation type.”"
evidence: []
---

## Details

Phenotype permutation estimates a null by shuffling sample labels. Small phenotype classes sharply limit the available distinct labelings, so the resulting significance estimates have limited resolution and may be unstable. Broad GSEA warns when a class has fewer than seven samples and notes that gene-set randomization may be preferable for small datasets. That alternative changes the null being tested; do not silently switch permutation type if the requested interpretation depends on phenotype permutations. Report the limitation and preserve the requested method, or explicitly justify and document a different null.

A successful CLI exit is not evidence that the requested permutation scheme had adequate resolution. Inspect the run log and result metadata as well as the class counts.

## How to check

Count samples before launching the run and flag small classes for review:

```python
from collections import Counter

counts = Counter(labels)
small = {group: n for group, n in counts.items() if n < 7}
if small:
    print("Review phenotype-permutation resolution:", small)
```

Confirm the report records the intended permutation type and that any warning about class size is considered in interpretation.
