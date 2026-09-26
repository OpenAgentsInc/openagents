---
id: edge-case.duplicate-gene-symbols
version: 1
kind: edge-case
title: Resolve duplicate gene symbols before creating GSEA inputs
summary: >-
  Multiple protein or assay rows can map to the same gene symbol, but GSEA
  inputs and gene sets need consistent feature identifiers. Choose a
  biologically justified resolution and apply it consistently to differential
  testing, expression data, gene sets, and result interpretation.
tags: [proteomics, gene-symbols, duplicate-identifiers, gsea]
applies_when: >-
  A protein-level table has repeated gene symbols and the required GSEA
  identifier is the gene symbol.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - gsea-proteomics
  cites:
    - Broad Institute, GSEA User Guide, sections “Input Data Formats” and “Gene Set Database Formats”
    - UniProt Consortium, UniProt User Manual, section “Mapping IDs”
evidence: []
---

## Details

Repeated symbols may represent distinct protein accessions, isoforms, or ambiguous mappings; they are not automatically interchangeable duplicate measurements. Arbitrarily retaining the row with the highest intensity can make the selected feature depend on abundance and potentially on the samples being compared. Prefer a documented protein-inference or aggregation rule appropriate to the experiment; if selecting a representative row, define the criterion independently of the contrast where possible. Apply the same resolved symbol mapping to the differential-expression results, expression matrix, and gene sets. GSEA input formats identify features by a consistent gene identifier (Broad Institute, *GSEA User Guide*, sections “Input Data Formats” and “Gene Set Database Formats”); protein-to-gene mapping ambiguity is described in UniProt Consortium, *UniProt User Manual*, section “Mapping IDs”.

Do not silently drop unresolved duplicates: report how many identifiers were affected and how they were handled. If the required feature is a gene symbol, ensure that the expression matrix has at most one row per symbol before running GSEA, and that the symbols in the gene set use that same identifier system.

## How to check

```python
# `resolved` is the table after applying the documented mapping/aggregation rule.
assert resolved["gene_name"].notna().all()
assert not resolved["gene_name"].duplicated().any()

expression_ids = set(resolved["gene_name"])
gene_set_ids = set(gene_set_symbols)
assert gene_set_ids.issubset(expression_ids)
```

Keep a separate audit table mapping each original protein row to its resolved feature so that leading-edge results can be traced back to the protein-level data.
