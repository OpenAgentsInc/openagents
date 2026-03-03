# CAD Raytrace Face-Pick Parity

Issue coverage: `VCAD-PARITY-102`

## Goal

Lock deterministic parity contracts for vcad raytrace face picking via the current wasm `pick` entrypoint: input validation, scene-upload guard, pixel-to-ray mapping, and current stub return behavior.

## Contracts

- Input validation parity (`pick`):
  - `camera`, `target`, and `up` must each contain exactly 3 components
  - invalid vector lengths return a deterministic validation error
- Scene guard parity:
  - when no solid is uploaded, `pick` returns deterministic guard error:
    - `No solid uploaded. Call uploadSolid() first.`
- Pixel mapping parity:
  - NDC uses pixel-center coordinates: `(pixel + 0.5)`
  - Y axis is top-left origin (`ndc_y = 1 - ...`)
- Ray direction parity:
  - basis is `forward`, `right = normalize(forward × up)`, `up_normalized = right × forward`
  - direction remains normalized
- Stub baseline parity:
  - current vcad wasm baseline returns `-1` for picks (background sentinel) while CPU face-pick path remains TODO

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/raytrace_face_pick_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/raytrace_face_pick_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-raytrace-face-pick-ci.sh
cargo run -p openagents-cad --bin parity-raytrace-face-pick
```
