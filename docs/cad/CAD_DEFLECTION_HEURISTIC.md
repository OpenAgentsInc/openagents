# CAD Deflection Heuristic (Wave 1)

This document defines the deterministic beam-style deflection approximation used
by `crates/cad::analysis::estimate_beam_deflection_heuristic`.

## Purpose

Provide fast stiffness feedback for rack variants without running full FEA.

This model is for comparative design guidance only.

## Inputs

- `span_mm`
- `width_mm`
- `thickness_mm`
- `load_kg`
- `youngs_modulus_gpa`

## Model

Model id: `cad.deflection.wave1.v1`

The heuristic uses a simply-supported beam with center point load:

- `delta = (F * L^3) / (48 * E * I)`
- `F = load_kg * 9.80665`
- `I = width * thickness^3 / 12`

Result is reported as `max_deflection_mm`.

## Confidence Label

Confidence labels:

- `medium`: slenderness ratio (`span/thickness`) in `[10, 60]` and `load_kg <= 20`
- `low`: all other inputs

Confidence is emitted in metadata as `confidence` and propagated into CAD
analysis metadata as `deflection.confidence`.

## Documented Limits

The metadata includes explicit limits:

- assumes simply-supported beam with center point load
- ignores local cutouts/vents and fastener compliance
- uses axis-aligned rectangular section approximation from bounds

## Failure Classification

Stable error codes:

- `CAD-DEFLECTION-INVALID-SPAN`
- `CAD-DEFLECTION-INVALID-WIDTH`
- `CAD-DEFLECTION-INVALID-THICKNESS`
- `CAD-DEFLECTION-INVALID-LOAD`
- `CAD-DEFLECTION-INVALID-MODULUS`

Each error includes a remediation hint for UI/reducer surfacing.
