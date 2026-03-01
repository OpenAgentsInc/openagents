# CAD Core Analysis (Wave 1)

This document defines deterministic Wave 1 analysis behavior for body-level
physical properties in `crates/cad::analysis`.

## Scope

Implemented now:

- Volume (`mm^3`)
- Mass (`kg`) from selected material density
- Center of gravity (`mm`)
- Surface area (`mm^2`) and bounding box metadata (used by inspect UI)

Deferred to follow-up issues:

- Cost heuristics
- Deflection heuristics
- Objective scoring

## Deterministic Contract

Given the same mesh payload and density input:

- Analysis output values are deterministic.
- Error code classification is deterministic.
- CoG source strategy is deterministic.

No random values are used in the analysis path.

## Computation Notes

- Volume uses signed tetrahedron accumulation over triangle faces.
- `mass_kg = density_kg_m3 * volume_mm3 * 1e-9`.
- CoG uses signed volume centroid when non-degenerate.
- If signed volume magnitude is effectively zero, CoG falls back to bounds center.

CoG source labels:

- `mesh_volume`
- `bounds_center_fallback`

## Failure Classification

Core analysis returns structured failures with stable codes and remediation hints.

- `CAD-ANALYSIS-EMPTY-VERTICES`
- `CAD-ANALYSIS-EMPTY-TRIANGLES`
- `CAD-ANALYSIS-MALFORMED-TRIANGLES`
- `CAD-ANALYSIS-INVALID-DENSITY`
- `CAD-ANALYSIS-MISSING-VERTEX`
- `CAD-ANALYSIS-NONFINITE-VERTEX`

UI/reducers must surface these failures explicitly and avoid silent corruption.
