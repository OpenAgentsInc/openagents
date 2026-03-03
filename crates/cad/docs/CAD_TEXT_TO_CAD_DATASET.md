# CAD Text-to-CAD Dataset Tooling

This document defines deterministic dataset generation + annotation utilities for text-to-cad workflows.

## Module

- `crates/cad/src/text_to_cad_dataset.rs`

## Entry Points

- `generate_text_to_cad_dataset(TextToCadDatasetConfig) -> CadResult<TextToCadDataset>`
- `dataset_to_ndjson(&TextToCadDataset) -> CadResult<String>`
- `summarize_annotations(&TextToCadDataset) -> TextToCadAnnotationSummary`

## Dataset Contract

- Deterministic part-family prompt corpus (`bracket`, `stand`, `enclosure`).
- Deterministic profile coverage (`cad0`, optionally `cad0-mini`).
- Prompt->compact IR samples generated via adapter module (`text_to_cad`).
- Dataset-level stable hash for replay verification.

## Annotation Contract

Each sample includes:

- part family label
- operation/root counts
- prompt token and numeric-token counts
- compact IR hash
- deterministic tags (`family:*`, `model:*`, source tag)

## Determinism

- Same config produces byte-stable sample ordering and dataset hash.
- NDJSON export order is stable and hashable.
- Summary indexes (`by_family`, `by_model`) are deterministic.
