# ARC Public-Eval Hygiene
Status: canonical operator policy and artifact-labeling contract for public-eval work
Date: 2026-03-15

## Purpose

This document turns the ARC public-eval hygiene rule into an explicit operator
policy plus a machine-checkable artifact-manifest contract.

The canonical validator is:

```bash
scripts/lint/arc-public-eval-hygiene-check.sh
```

The typed benchmark-side mirror is now:

- `arc_benchmark::validate_public_eval_artifact_manifest`
- `arc_benchmark::run_static_hygiene_suite`

## Operator Policy

These rules are mandatory for ARC public-eval work:

- no per-task manual solver tuning on public evaluation tasks
- no public-eval artifact may be counted as roadmap acceptance evidence unless
  it is explicitly labeled as both `non-regression` and `non-optimization`
- public evaluation runs must not feed search-guide, repair-model, calibration,
  or other training datasets
- internal hidden holdout must stay disjoint from synthetic tasks derived from
  public evaluation tasks
- public evaluation visibility may be used only for bounded compatibility or
  non-regression checks, not for optimization loops

## Artifact Manifest Contract

Every public-eval artifact manifest MUST include:

- `schema_version`
- `artifact_id`
- `benchmark_family`
- `evaluation_visibility`
- `artifact_labels`
- `per_task_manual_tuning`
- `feeds_training`
- `synthetic_derivation`

For `evaluation_visibility = "public_eval"`, the manifest MUST satisfy:

- `artifact_labels` contains `public-eval`
- `artifact_labels` contains `non-regression`
- `artifact_labels` contains `non-optimization`
- `artifact_labels` does not contain `optimization`
- `per_task_manual_tuning` is `false`
- `feeds_training` is `false`
- `synthetic_derivation` is `none`

## Label Meanings

- `public-eval`
  - the artifact came from public-evaluation-visible tasks
- `non-regression`
  - the artifact is only for bounded regression or compatibility checking
- `non-optimization`
  - the artifact must not be treated as optimization evidence or tuning fuel

If a future workflow needs more labels, it may add them, but it must not weaken
the required labels above for public-eval artifacts.

## Validation Fixtures

The repo-owned example manifests live in:

- `crates/arc/fixtures/policy/public_eval_hygiene/valid_public_eval_non_regression.json`
- `crates/arc/fixtures/policy/public_eval_hygiene/invalid_public_eval_optimization.json`
- `crates/arc/fixtures/policy/public_eval_hygiene/invalid_public_eval_training_feed.json`

The validator script must continue to accept the valid fixture and reject the
invalid fixtures.

The Rust hygiene tests must continue to load those same fixtures and produce the
same pass/fail outcomes, so public-eval policy does not drift between shell-only
 lint gates and benchmark/report code.
