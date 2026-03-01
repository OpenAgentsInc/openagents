# CAD Rack Template Generator

This document defines the deterministic two-bay Mac Studio rack template used by the CAD demo.

## Scope

- In scope: deterministic template graph generation, typed parameter schema, semantic references, wall-mount feature toggles.
- Out of scope: objective variants, vent/rib optimization hooks (tracked by backlog items 66-68).

## API

Module: `openagents_cad::rack`

- `MacStudioRackTemplateParams`
- `generate_mac_studio_rack_template(&MacStudioRackTemplateParams) -> CadResult<MacStudioRackTemplate>`

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

## Baseline Feature Graph

Deterministic nodes:

1. `feature.rack.base` (`primitive.box.v1`)
2. `feature.rack.bay_cut` (`cut.hole.v1`)
3. `feature.rack.bay_pattern` (`linear.pattern.v1`)
4. `feature.rack.corner_break` (`fillet.placeholder.v1`)
5. `feature.rack.wall_mount_bracket` (`transform.v1`)
6. `feature.rack.wall_mount_hole` (`cut.hole.v1`)
7. `feature.rack.mount_hole_pattern` (`linear.pattern.v1`)

## Semantic References

- `rack_outer_face`
- `rack_bay_pattern`
- `wall_mount_bracket`
- `mount_hole_pattern`
- `rack_corner_break`

These references are created through `CadSemanticRefRegistry` and can be persisted through `.apcad` stable IDs.
