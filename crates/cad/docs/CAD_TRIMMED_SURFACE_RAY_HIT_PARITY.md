# CAD Trimmed-Surface Ray Hit Parity

Issue coverage: `VCAD-PARITY-099`

## Goal

Lock deterministic parity contracts for trimmed-surface ray hit handling in vcad: hit acceptance inside outer trim loops, hole rejection for inner loops, and closest-hit selection after trim filtering.

## Contracts

- `point_in_polygon` uses winding-number semantics (`winding != 0`).
- `point_in_face` behavior:
  - accept points inside outer loop
  - reject points inside any inner loop (holes)
  - reject points outside outer loop
- Concave loop handling parity:
  - interior points accepted
  - notch point in concave cutout rejected
- Ray-hit filtering parity (BVH face test semantics):
  - trim filter applied before constructing accepted hit list
  - closest hit computed from accepted trimmed hits only

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/trimmed_surface_ray_hit_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/trimmed_surface_ray_hit_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-trimmed-surface-ray-hit-ci.sh
cargo run -p openagents-cad --bin parity-trimmed-surface-ray-hit
```
