# CAD Rack Template Generator

This document defines the deterministic two-bay Mac Studio rack template used by the CAD demo.

## Scope

- In scope: deterministic template graph generation, typed parameter schema, semantic references, wall-mount feature toggles.
- Out of scope: objective variants, vent/rib optimization hooks (tracked by backlog items 66-68).

## API

Module: `openagents_cad::rack`

- `MacStudioRackTemplateParams`
- `generate_mac_studio_rack_template(&MacStudioRackTemplateParams) -> CadResult<MacStudioRackTemplate>`
- `RackObjectivePreset`
- `generate_objective_variants(&MacStudioRackTemplateParams, seed) -> CadResult<Vec<RackObjectiveVariant>>`

Returned payload:

- `feature_graph: FeatureGraph`
- `params: ParameterStore`
- `semantic_refs: CadSemanticRefRegistry`
- `metadata`

## Baseline Parameter Schema

- `bay_count` (`unitless`) = `2`
- `frame_width_mm` (`mm`) = `180.0`
- `frame_depth_mm` (`mm`) = `210.0`
- `frame_height_mm` (`mm`) = `95.0`
- `bay_pitch_mm` (`mm`) = `88.0`
- `bay_cut_radius_mm` (`mm`) = `19.0`
- `wall_thickness_mm` (`mm`) = `6.0`
- `corner_radius_mm` (`mm`) = `2.0`

Wall-mount parameters:

- `wall_mount_enabled` (`unitless`) = `1`
- `wall_mount_hole_count` (`unitless`) = `4`
- `wall_mount_hole_spacing_mm` (`mm`) = `32.0`
- `wall_mount_hole_radius_mm` (`mm`) = `2.8`
- `wall_mount_bracket_thickness_mm` (`mm`) = `6.0`

Vent parameters:

- `vent_enabled` (`unitless`) = `1`
- `vent_rows` (`unitless`) = `3`
- `vent_cols` (`unitless`) = `8`
- `vent_spacing_mm` (`mm`) = `12.0`
- `vent_hole_radius_mm` (`mm`) = `2.0`
- `vent_density_scale` (`unitless`) = `1.0`

Optimization hooks:

- `opt_ribs_enabled` (`unitless`) = `1`
- `opt_rib_count` (`unitless`) = `3`
- `opt_rib_spacing_mm` (`mm`) = `42.0`
- `opt_rib_thickness_mm` (`mm`) = `3.0`
- `opt_wall_thickness_scale` (`unitless`) = `1.0`
- `effective_wall_thickness_mm` (`mm`) = `wall_thickness_mm * opt_wall_thickness_scale`

## Baseline Feature Graph

Deterministic nodes:

1. `feature.rack.base` (`primitive.box.v1`)
2. `feature.rack.bay_cut` (`cut.hole.v1`)
3. `feature.rack.bay_pattern` (`linear.pattern.v1`)
4. `feature.rack.corner_break` (`fillet.placeholder.v1`)
5. `feature.rack.wall_mount_bracket` (`transform.v1`)
6. `feature.rack.wall_mount_hole` (`cut.hole.v1`)
7. `feature.rack.mount_hole_pattern` (`linear.pattern.v1`)
8. `feature.rack.vent_hole` (`cut.hole.v1`)
9. `feature.rack.vent_pattern_x` (`linear.pattern.v1`)
10. `feature.rack.vent_face_set` (`linear.pattern.v1`)
11. `feature.rack.rib_seed` (`transform.v1`)
12. `feature.rack.rib_pattern` (`linear.pattern.v1`)

## Semantic References

- `rack_outer_face`
- `rack_bay_pattern`
- `wall_mount_bracket`
- `mount_hole_pattern`
- `vent_face_set`
- `rack_rib_set`
- `rack_corner_break`

These references are created through `CadSemanticRefRegistry` and can be persisted through `.apcad` stable IDs.

## Objective Variants

The objective engine emits four deterministic presets (seeded):

- `variant.lightweight`
- `variant.low-cost`
- `variant.stiffness`
- `variant.airflow`

Each variant includes objective score values for:

- `weight`
- `cost`
- `stiffness`
- `airflow`

## Geometry Goldens

Golden fixture: `crates/cad/tests/goldens/rack_geometry_snapshots.json`

Covered snapshots:

- `variant.baseline`
- `variant.lightweight`
- `variant.low-cost`
- `variant.stiffness`

Each snapshot records semantic geometry signals needed for deterministic diff review:

- `rebuild_hash`
- ordered feature IDs
- per-feature geometry hashes
- feature records (`feature_id`, op key, dependency hashes, parameter fingerprint)
- key rack parameters
- semantic reference mapping

Run:

```bash
cargo test -p openagents-cad --test rack_geometry_snapshots --quiet
```

When fixture updates are intentional:

```bash
CAD_UPDATE_GOLDENS=1 cargo test -p openagents-cad --test rack_geometry_snapshots rack_geometry_snapshots_match_golden_fixture --quiet
```
