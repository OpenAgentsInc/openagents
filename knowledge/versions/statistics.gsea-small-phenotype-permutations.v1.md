---
id: statistics.gsea-small-phenotype-permutations
version: 1
kind: edge-case
title: Small classes limit phenotype-permutation GSEA
summary: >-
  When phenotype-permutation GSEA has few samples per class, the number of
  distinct label assignments can be far smaller than the requested permutation
  count. A completed run may still produce coarse, unstable enrichment
  statistics; check warnings and interpret results accordingly.
tags: [gsea, permutation, small-sample]
applies_when: >-
  GSEA runs use phenotype permutations with small phenotype classes,
  especially when increasing the requested permutation count appears to
  promise more information than the sample labels allow.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - gsea-proteomics-1790395844
  cites:
    - Broad Institute, GSEA User Guide, section “Permutation Type”
evidence: []
---

## Details

The Broad Institute’s GSEA guidance recommends phenotype permutations when there are enough samples per phenotype, and recommends gene-set permutations for smaller datasets. With few samples, label permutations are limited: requesting many permutations does not create more distinct assignments. If phenotype permutations are required, retain that setting, but record the sample-size limitation and avoid treating a successful run as evidence of a well-resolved null distribution.

## How to check

For two balanced classes of size `n`, the number of distinct assignments is at most `choose(2*n, n)` before accounting for equivalent label swaps. Compare this with the requested permutation count, and inspect the run log for warnings about class sizes or insufficient permutations. Confirm the permutation type in the run parameters rather than inferring it from the output files.
