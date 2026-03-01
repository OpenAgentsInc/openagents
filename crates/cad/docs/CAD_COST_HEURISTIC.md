# CAD Cost Heuristic (Wave 1)

This document defines the deterministic CNC/material cost estimator used by
`crates/cad::materials::estimate_cnc_cost_heuristic_usd`.

## Purpose

Provide a first-pass engineering estimate for:

- material assignment previews
- variant comparison overlays
- activity/event receipts

This is an estimate model, not a quote engine.

## Inputs

- `mass_kg`
- `volume_mm3`
- `surface_area_mm2`
- `triangle_count`
- selected material preset (`cnc_cost_usd_per_kg`, `cnc_setup_usd`)

## Output

`CadCostHeuristicEstimate` returns:

- `total_cost_usd`
- `material_cost_usd`
- `setup_cost_usd`
- `machining_cost_usd`
- `machining_minutes`
- `complexity_factor`
- `metadata` (`BTreeMap<String, String>`) with assumptions + derived values

## Model

Model id: `cad.cost.wave1.v1`

- Material cost = `mass_kg * cnc_cost_usd_per_kg`
- Setup cost = `cnc_setup_usd`
- Machining minutes =
  `(base_programming_minutes + surface_area_cm2 * surface_minutes_per_cm2)`
  `* thin_wall_factor * triangle_complexity_factor`
- Machining cost = `machining_minutes * machine_rate_usd_per_min`
- Total cost = material + setup + machining

## Metadata Assumptions

The estimator metadata always includes model assumptions and derived terms, e.g.:

- `model_id`
- `assumption.machine_rate_usd_per_min`
- `assumption.base_programming_minutes`
- `assumption.surface_minutes_per_cm2`
- `assumption.triangle_complexity_weight`
- `assumption.thin_wall_ratio_weight`
- `derived.complexity_factor`
- `derived.machining_minutes`
- `component.material_cost_usd`
- `component.setup_cost_usd`
- `component.machining_cost_usd`
- `result.total_cost_usd`

## Failure Classification

Stable error codes:

- `CAD-COST-INVALID-MASS`
- `CAD-COST-INVALID-VOLUME`
- `CAD-COST-INVALID-SURFACE-AREA`
- `CAD-COST-INVALID-MATERIAL`

Each error carries a remediation hint for UI/reducer surfacing.
