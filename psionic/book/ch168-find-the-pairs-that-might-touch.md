# 168. Find the pairs that might touch

> **Status: unimplemented — design draft.** Verse tests contacts by
> enumerating the pairs a game names; nothing yet finds candidate pairs in a
> general scene. Nothing here runs yet.

Exact collision tests are expensive, and in a scene of `n` objects there are
`n(n − 1)/2` pairs. Almost all of them are far apart. The *broad phase*
answers a cheaper question for every pair, "could these two touch this
step?", using bounding boxes, and passes only the survivors to the exact
tests of chapter 169. There is no single best algorithm: which one wins
depends on the number of objects, how much they move per step, and the
hardware. This chapter builds three and chooses between them by
measurement.

## What this chapter teaches

- Axis-aligned bounding boxes (AABBs), fattened by motion and a margin.
- Sweep and prune: sort box endpoints on an axis, report overlapping
  intervals, and exploit the fact that the order barely changes per step.
- Bounding volume hierarchies built in parallel from Morton codes.
- Brute-force all-pairs testing, and why it can win on a GPU.
- Collision filters, and a canonical pair order so results are
  deterministic.

## The geometry

Two AABBs overlap exactly when their intervals overlap on all three axes:

```jai
overlap(A, B) = ∀ axis: A.min[axis] ≤ B.max[axis] ∧ B.min[axis] ≤ A.max[axis]
```

A box for a moving body covers its swept motion over the step plus a margin
`δ`, so a pair that starts touching mid-step is still found. Sweep and prune
sorts the `2n` endpoints on one axis and walks them; with insertion sort and
temporal coherence the per-step cost is close to `O(n + k)` for `k`
overlaps, but it degrades when many objects line up on the chosen axis. A
linear BVH sorts object centres by their 30-bit Morton code (interleaved
coordinate bits), builds the tree from the sorted codes in parallel and
traverses it per object
([Karras, HPG 2012](https://research.nvidia.com/publication/2012-06_maximizing-parallelism-construction-bvhs-octrees-and-k-d-trees)).
All-pairs testing does `n²/2` box tests with perfectly regular memory access,
which on a GPU can beat both for a few thousand objects.

## The design in Psionic

All three methods produce the same thing: the set of candidate pairs
`(i, j)` with `i < j`, sorted lexicographically. Sorting is part of the
contract, because the contact solver's iteration order (chapter 170) depends
on pair order and therefore so do its bits. A method is a profile choice;
switching methods never changes a result, which the checks prove.

Filters are applied before boxes: a pair is skipped if both bodies are
static, if they share a link-adjacency exclusion (parent and child links of
a joint), or if their collision groups and masks say so. Per-environment
batches (chapter 179) run the broad phase for every environment in one
launch, with pairs never crossing environments.

The choice between methods is measured, not written into code: a small
benchmark at build time on the target, or a recorded choice per scene size
and backend, with both the choice and its measurement kept in the scene
record (chapter 185).

## What it builds

- `psionic/lib/sim/broad.coax`: AABB computation and fattening, filters,
  sweep and prune with coherent insertion sort, an LBVH, all-pairs, and the
  canonical pair sort.
- A benchmark over random, clustered and stacked scenes at several sizes, on
  CPU, CUDA and Metal.

## Acceptance checks and oracles

- Every method returns exactly the pair set of an `O(n²)` reference written
  independently, on random scenes, degenerate scenes (all boxes equal, all
  on one line) and empty ones.
- Pair order is identical across methods, worker counts and devices.
- A fast body crossing another within one step is still reported (the swept
  box test).
- Filters: adjacent links, static pairs and masked groups never appear.
- The benchmark reports the crossover sizes per backend; no method is
  chosen without a recorded measurement.

## How Genesis does it

Genesis filters with bounding boxes and collision masks, then selects sweep
and prune on CPU or with heterogeneous entities and parallel all-pairs on GPU
otherwise; contact islands and hibernation skip further work
([collision detection](https://genesis-world.readthedocs.io/en/latest/user_guide/theory/rigid_solver/collision_detection.html),
[tagged options](https://genesis-world.readthedocs.io/en/v1.4.5/_modules/genesis/options/solvers.html)).
The lesson we take is the same: regular parallel work can beat a more
selective algorithm. We differ by requiring all methods to produce identical
sorted output, so the choice is purely a cost decision with a recorded
measurement, never a change in behaviour.

## Dependencies

Chapter 165's scene and chapter 166's bodies; chapter 36's batching; chapter 37's compute subset for device sorts and
scans. From Coax: R15's deterministic schedules for parallel sorts and R21's
batch dimension.
