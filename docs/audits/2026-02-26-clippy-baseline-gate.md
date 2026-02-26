# Clippy Warning Baseline Gate

Date: 2026-02-26

## Purpose

This repo now enforces a "no net-new clippy warnings" rule by lane. Existing warning debt is tracked as a baseline and can only move down unless intentionally re-baselined.

## Baseline File

- `scripts/lint/clippy-baseline.toml`

Current baseline:

- `LIB_WARNINGS=240`
- `TEST_WARNINGS=434`
- `EXAMPLE_WARNINGS=286`

## Lanes

- `lib`: `cargo clippy --workspace --lib -- -W clippy::all`
- `tests`: `cargo clippy --workspace --tests -- -W clippy::all`
- `examples`: `cargo clippy -p wgpui --examples --features desktop -- -W clippy::all`

## Scripts

- Capture or refresh baseline:
  - `scripts/lint/clippy-baseline.sh`
- Check for regressions against baseline:
  - `scripts/lint/clippy-regression-check.sh`

## Baseline Update Policy

Only refresh the baseline when one of these is true:

1. Warning count decreased and you want to record the improvement.
2. A deliberate, reviewed tradeoff requires a temporary increase.

When baseline changes, include in the PR/commit message:

1. Lane(s) changed.
2. Old vs new count.
3. Why the delta is acceptable.
