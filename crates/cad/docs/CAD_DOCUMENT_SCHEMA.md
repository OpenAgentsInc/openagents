# CAD Document Core Schema (Wave 1)

Related issue: [#2458](https://github.com/OpenAgentsInc/openagents/issues/2458)

## Schema Summary

`CadDocument` is the core versioned in-memory and serialized document shape for Wave 1 CAD.

Fields:

- `schema_version` (`u32`): currently `1`
- `document_id` (`String`): stable document identifier
- `revision` (`u64`): monotonic document revision
- `units` (`CadUnits`): canonical units (`mm`)
- `metadata` (`BTreeMap<String, String>`): deterministic metadata map
- `feature_ids` (`Vec<String>`): ordered feature identifiers
- `partDefs` (`Option<BTreeMap<String, CadPartDef>>`): optional assembly part definitions
- `instances` (`Option<Vec<CadPartInstance>>`): optional assembly part instances
- `joints` (`Option<Vec<CadAssemblyJoint>>`): optional assembly joints
- `groundInstanceId` (`Option<String>`): optional assembly world-grounded instance id
- `drawing` (`Option<CadDrawingState>`): optional persisted 2D drawing-mode state
- `analysis_cache` (`Option<BTreeMap<String, String>>`): optional non-authoritative analysis cache

`drawing` payload fields:

- `viewMode` (`"3d" | "2d"`)
- `viewDirection` (`"front" | "back" | "top" | "bottom" | "left" | "right" | "isometric"`)
- `showHiddenLines` (`bool`)
- `showDimensions` (`bool`)
- `zoom` (`f64`)
- `pan` (`{ x: f64, y: f64 }`)
- `detailViews` (`Vec<{ id, centerX, centerY, scale, width, height, label }>`)
- `nextDetailId` (`u64`)

## Determinism Rules

1. `metadata` and `analysis_cache` are deterministic maps (`BTreeMap`).
2. `feature_ids` preserves explicit order from the authoring path.
3. `partDefs` uses deterministic key ordering (`BTreeMap`) when serialized.
4. Drawing payload keys use vcad-compatible camelCase (`viewMode`, `viewDirection`, `detailViews`, `nextDetailId`, `centerX`, `centerY`).
5. Empty/minimal fixture goldens must remain byte-stable unless schema intentionally changes.
6. Round-trip parse/serialize must preserve content exactly for valid payloads.

## Golden Fixtures

- `crates/cad/tests/goldens/cad_document_empty.json`
- `crates/cad/tests/goldens/cad_document_minimal.json`
- `crates/cad/tests/goldens/apcad_envelope_empty.json`
- `crates/cad/tests/goldens/apcad_envelope_single_feature.json`

## Code References

- `crates/cad/src/document.rs`
