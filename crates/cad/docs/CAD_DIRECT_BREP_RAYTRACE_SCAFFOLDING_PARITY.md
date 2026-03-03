# CAD Direct BRep Raytrace Scaffolding Parity

Issue coverage: `VCAD-PARITY-097`

## Goal

Lock deterministic parity contracts for vcad's direct BRep raytrace crate scaffolding (`vcad-kernel-raytrace`) so OpenAgents preserves module/API topology and baseline non-tessellated rendering entrypoints.

## Contracts

- Crate module graph parity:
  - public modules: `bvh`, `cpu`, `intersect`, `trim`
  - internal module: `ray`
  - feature-gated module: `gpu` behind feature `gpu`
- Public API parity at crate root:
  - `Bvh`, `CpuRenderer`, `render_scene`, `Ray`, `RayHit`
- Intersection registry scaffolding parity:
  - `plane`, `cylinder`, `sphere`, `cone`, `torus`, `bilinear`, `bspline`
- CPU renderer baseline contract parity:
  - RGBA output (`4` channels)
  - default background `[30, 32, 40, 255]`
  - default material color `[0.6, 0.7, 0.8]`
- Ray/AABB contract parity:
  - slab entry/exit pair
  - entry clamp to `0.0`
  - miss/behind rejection
  - axis-aligned infinite reciprocal safety
- Direct BRep path contract remains non-tessellated.

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/direct_brep_raytrace_scaffolding_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/direct_brep_raytrace_scaffolding_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-direct-brep-raytrace-scaffolding-ci.sh
cargo run -p openagents-cad --bin parity-direct-brep-raytrace-scaffolding
```
