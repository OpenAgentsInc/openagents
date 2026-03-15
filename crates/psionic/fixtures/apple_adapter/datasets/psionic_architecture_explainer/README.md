# Psionic Architecture Explainer Corpus

This directory is the first reviewed corpus for the real Apple adapter target
`Psionic architecture explainer`.

## Goal

Teach the adapter stable Psionic architecture, ownership-boundary discipline,
and operator workflow truth.

This corpus is intentionally not trying to memorize the whole repo. Volatile
repo state still belongs to retrieval.

## Included Splits

- `train.jsonl`
- `held_out.jsonl`
- `benchmark.jsonl`
- `corpus_manifest.json`

## Source Inventory

The reviewed source inventory lives in `corpus_manifest.json` and currently
anchors the corpus to:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `crates/psionic/docs/TRAIN_SYSTEM.md`
- `crates/psionic/docs/ARCHITECTURE.md`
- `docs/headless-compute.md`
- `docs/kernel/compute-training-authority.md`
- `apps/autopilot-desktop/src/apple_adapter_training_control.rs`
- `crates/openagents-provider-substrate/src/lib.rs`

## Split Policy

- benchmark rows must stay stable-digest disjoint from `train` and `held_out`
- held-out rows must stay stable-digest disjoint from `train`
- every row must carry reviewed task-family and source-provenance tags in the
  corpus manifest
- explicit negative, correction, and refusal rows must remain present in
  `train`, `held_out`, and `benchmark`

## Dataset Identity

- dataset ref:
  `dataset://openagents/apple_adapter/psionic_architecture_explainer`
- version:
  `2026.03.15.2`
