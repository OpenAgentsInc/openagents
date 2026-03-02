# CAD Feature Ops

This document tracks feature-op contracts built on the CAD kernel adapter path.

## Current Ops (MVP)

- `BoxFeatureOp` (`crates/cad/src/features.rs`)
  - deterministic `feature_id`
  - parameter bindings (`width`, `depth`, `height`) resolved from `ParameterStore`
  - explicit primitive validation through `BoxPrimitive::validate`
  - deterministic geometry hash via FNV-1a64 payload hashing
- `CylinderFeatureOp` (`crates/cad/src/features.rs`)
  - deterministic `feature_id`
  - parameter bindings (`radius`, `height`) resolved from `ParameterStore`
  - explicit primitive validation through `CylinderPrimitive::validate`
  - deterministic geometry hash via FNV-1a64 payload hashing
  - tolerance edge-case rejection (`radius <= base_tolerance`)
- `TransformFeatureOp` (`crates/cad/src/features.rs`)
  - translation / rotation / scale payload with robust finite-value validation
  - rejects invalid scale components (`<= 0`)
  - deterministic matrix composition (`compose_transform_sequence`)
  - deterministic output ordering based on input op order
- `CutHoleFeatureOp` (`crates/cad/src/features.rs`)
  - subtraction-style cylindrical cut using radius/depth parameters
  - resolves cutter primitive through `ParameterStore`
  - returns either:
    - valid `FeatureOpResult` (when boolean backend succeeds), or
    - structured `CadError::EvalFailed` (when boolean backend fails)
- `LinearPatternFeatureOp` (`crates/cad/src/features.rs`)
  - deterministic repeated-instance generation for feature copies
  - parameter bindings (`count` as `unitless`, `spacing` as `mm`)
  - stable `pattern_index` assignment (`start_index + offset`)
  - deterministic per-instance geometry hashes and aggregate pattern hash
  - structured validation failures for zero direction, non-integer count, and invalid spacing
- `SweepFeatureOp` (`crates/cad/src/features.rs`)
  - deterministic sweep path sampling with path/twist/scale controls
  - parameter bindings (`twist_angle_rad`, `scale_start`, `scale_end` as `unitless`)
  - vcad-aligned path segment defaults (`path_segments=0` -> 32 samples baseline)
  - deterministic per-station hashes and aggregate sweep geometry hash
  - structured validation failures for zero-length paths, invalid scales, and invalid segments
- `LoftFeatureOp` (`crates/cad/src/features.rs`)
  - deterministic multi-profile loft contracts (`>=2` profiles, uniform vertex counts)
  - supports open (`closed=false`) and closed tube (`closed=true`) transition semantics
  - deterministic lateral/cap topology count summaries in eval result receipts
  - deterministic geometry hash over source hashes, profile vertices, and `closed` mode
  - structured validation failures for too-few profiles, mismatched profile vertex counts, and malformed IDs
- `FilletPlaceholderFeatureOp` (`crates/cad/src/features.rs`)
  - no-op marker contract for fillet/chamfer graph compatibility
  - serializes to feature graph node with operation key `fillet.placeholder.v1`
  - deterministic rebuild output (`geometry_hash`) with source hash passthrough
  - explicit validation and parsing failures for malformed marker payloads

## Golden Hash Fixtures

- `crates/cad/tests/goldens/feature_box_geometry_hashes.json`
- `crates/cad/tests/goldens/feature_cylinder_geometry_hashes.json`

## Reviewer Verification

- `cargo test -p openagents-cad`
