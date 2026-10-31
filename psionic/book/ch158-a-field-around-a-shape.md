# 158. A field around a shape

> **Status: unimplemented — design draft.** Nothing here runs yet.

Models that predict flow around a car or an aircraft read the shape as
geometry, not as an image. They use signed distances, point clouds and local
neighborhoods. This chapter builds those geometry operations and checks each
one against a small geometric oracle.

## What this chapter covers

- Signed distance functions for simple shapes (sphere, box) and for a
  triangle mesh, with the sign convention stated.
- Point clouds: sampling points on a surface and in a volume.
- Spatial neighborhood queries: radius search and k-nearest neighbors, with
  a uniform grid or a tree, compared with brute-force search.
- Point convolutions and local aggregation over neighborhoods.
- Bounded sizes: maximum point counts and neighbor counts, with typed
  refusals.

## Dependencies

- [Chapter 5](ch005-a-standard-library-of-tensor-operators.md): gather,
  reductions and indexed updates.
- [Chapter 106](ch106-differentiate-any-tensor-program.md): gradients through
  the aggregation layers.
- The Aethyr engine's spatial queries
  ([Aethyr chapter 7](../../aethyr/book/ch007-ask-the-world-where-things-are.md))
  as a related design.

## Done means

- Signed distances for simple shapes match closed-form values.
- Neighborhood queries return exactly the brute-force result on fixed point
  sets.
- Point convolution outputs match a direct loop on small fixtures, and their
  gradients match finite differences.

## Sources

- [Chapter plan](PLAN.md), Part XVI.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), DoMINO entry.
