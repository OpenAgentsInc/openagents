# CAD Feature Ops

This document tracks feature-op contracts built on the CAD kernel adapter path.

## Current Ops (MVP)

- `BoxFeatureOp` (`crates/cad/src/features.rs`)
  - deterministic `feature_id`
  - parameter bindings (`width`, `depth`, `height`) resolved from `ParameterStore`
  - explicit primitive validation through `BoxPrimitive::validate`
  - deterministic geometry hash via FNV-1a64 payload hashing

## Golden Hash Fixtures

- `crates/cad/tests/goldens/feature_box_geometry_hashes.json`

## Reviewer Verification

- `cargo test -p openagents-cad`
