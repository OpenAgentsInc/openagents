# CAD Engineering Overlay Panel

This document defines the live engineering overlay rendered in the CAD demo pane
(`apps/autopilot-desktop/src/panes/cad.rs`).

## Goal

Show engineering metadata in-viewport while preserving CAD interaction speed:

- material
- volume
- mass
- estimated cost
- max deflection + confidence label
- center of gravity

## Update Semantics

Overlay values are read from `CadDemoPaneState.analysis_snapshot`.

The reducer updates `analysis_snapshot` in the same rebuild commit cycle that
accepts the new mesh payload. Therefore overlay values update in the same cycle
as geometry changes (no extra polling round).

## Rendering Rules

- Overlay is rendered inside viewport bounds.
- Bounds are clamped so the panel never overflows pane/viewport.
- Values use deterministic formatting for stable visual snapshots.
- Missing values show placeholder `--` instead of stale values.

## Data Sources

- Material, volume, mass, CoG: `CadAnalysis`
- Cost assumptions/model: `CadAnalysis.estimator_metadata`
- Deflection confidence: `CadAnalysis.estimator_metadata["deflection.confidence"]`

## Out of Scope

- Full interactive engineering report UI
- Drill-down plots/charts
- FEA validation workflows
