# CAD Analytic Ray Intersections Parity

Issue coverage: `VCAD-PARITY-098`

## Goal

Lock deterministic parity contracts for vcad analytic ray-surface intersections across plane/cylinder/sphere/cone/torus paths, including hit ordering and positive-`t` filtering behavior.

## Contracts

- Analytic intersection surfaces covered:
  - `plane`, `cylinder`, `sphere`, `cone`, `torus`
- Plane contract parity:
  - parallel and behind-ray cases return no hit
  - hit case projects UV in plane basis
- Cylinder/sphere contract parity:
  - quadratic roots filtered to `t >= 0`
  - output remains sorted by ascending `t`
  - tangent cases preserve deterministic duplicate roots
- Cone contract parity:
  - linear and quadratic branches preserved
  - opposite nappe filtered by `v >= 0`
- Torus contract parity:
  - quartic solve path returns deterministic sorted roots
  - canonical through-center test yields 4 ordered hits

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/analytic_ray_intersections_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/analytic_ray_intersections_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-analytic-ray-intersections-ci.sh
cargo run -p openagents-cad --bin parity-analytic-ray-intersections
```
