# CAD Text-to-CAD Training/Eval Hooks

This document defines gated training/eval split hooks built on top of deterministic text-to-cad datasets.

## Module

- `crates/cad/src/text_to_cad_training_eval.rs`

## Entry Points

- `build_text_to_cad_training_eval_hooks(&TextToCadDataset, TextToCadTrainingHookConfig)`
- `training_hook_records_ndjson(&TextToCadDataset, &TextToCadTrainingEvalHooks)`

## Gate Contract

- Hooks are disabled by default.
- Gate environment contract:
  - `OPENAGENTS_CAD_ENABLE_TRAINING_HOOKS`
- Disabled path returns explicit gate metadata (`CAD0-TRAINING-HOOKS-GATED`).

## Enabled Contract

- Deterministic train/eval split over dataset sample IDs.
- Bounded eval ratio (`1..=50` percent).
- Stable hashes for:
  - annotation summary
  - train payload
  - eval payload
- Deterministic NDJSON record export for downstream training/eval pipelines.
