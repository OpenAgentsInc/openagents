---
id: edge-case.gsea-zero-permutation-pvalue
version: 1
kind: edge-case
title: Interpret zero-valued permutation GSEA p-values as resolution-limited
summary: >-
  A permutation-based GSEA report may display a nominal p-value of zero when
  no sampled null scores are at least as extreme as the observed score. This
  is a finite-permutation resolution limit, not evidence that the true p-value
  is exactly zero.
tags: [gsea, permutation, p-value, interpretation]
applies_when: >-
  Reading nominal p-values from GSEA reports generated using a finite number
  of phenotype or gene-set permutations.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - gsea-proteomics
  cites:
    - "Subramanian et al., “Gene set enrichment analysis: a knowledge-based approach for interpreting genome-wide expression profiles,” PNAS (2005), Methods, “Significance assessment.”"
evidence: []
---

## Details

Permutation-based significance estimates compare an observed enrichment score with scores generated under a permutation null. If none of the sampled null scores is as extreme as the observed score, software may print `0` or a rounded zero. Interpret this as “no exceedance observed at the resolution of this run,” not as a mathematically exact zero probability. The number of effective permutations limits how finely the tail probability can be resolved; permutation constraints can reduce the effective null sample further.

When reporting such results, preserve the tool's output if required, but explain its finite-permutation meaning. Increase the permutation count when finer tail resolution is scientifically important and feasible; do not treat a printed zero as more precise than the permutation design supports.

**Source:** Subramanian et al., “Gene set enrichment analysis: a knowledge-based approach for interpreting genome-wide expression profiles,” *PNAS* (2005), Methods, “Significance assessment.”
