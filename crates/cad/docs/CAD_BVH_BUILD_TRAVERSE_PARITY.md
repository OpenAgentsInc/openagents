# CAD BVH Build/Traverse Parity

Issue coverage: `VCAD-PARITY-100`

## Goal

Lock deterministic parity contracts for vcad BVH build/traverse behavior used by raytrace: SAH constants, leaf/fallback partitioning, hit ordering, closest-hit traversal order, and flatten-node tuple shape.

## Contracts

- BVH build constants parity:
  - leaf threshold: `<= 4` faces
  - SAH buckets: `12`
  - SAH traversal cost: `0.125`
- Split fallback parity:
  - if partitioning produces an empty side, split falls back to midpoint (`len / 2`)
  - fallback path remains deterministic for identical centroids
- Trace parity:
  - `trace` emits non-negative hits sorted by ascending `t`
  - `trace_closest` traverses nearer child first by child AABB entry `t`
  - traversal early-outs when node entry `t` is not better than current closest
- Flatten parity:
  - node tuple contract preserved as `(aabb, is_leaf, left_or_first, right_or_count)`
  - flat face index stream preserves leaf face-count accounting

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/bvh_build_traverse_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/bvh_build_traverse_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-bvh-build-traverse-ci.sh
cargo run -p openagents-cad --bin parity-bvh-build-traverse
```
