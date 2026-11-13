# 20. Find the pairs that might touch

> **Status: partly implemented.** [`aethyr/physics/sim/broad.coax`](../physics/sim/broad.coax)
> (589 mutants: 558 killed, 31 marked equivalent with their reason)
> finds candidate pairs on the CPU in f64 by three named methods (all pairs,
> sweep and prune, and a linear BVH) behind one filter, and returns the same
> sorted pair list from each. Its checks compare every method with an
> independent `O(n²)` reference on random, degenerate and empty scenes. Device
> kernels, per-environment batches, wall-clock crossover measurements per
> backend and a recorded method choice in the scene are not built yet.

Exact collision tests are expensive, and in a scene of `n` objects there are
`n(n − 1)/2` pairs. Almost all of them are far apart. The *broad phase*
answers a cheaper question for every pair, "could these two touch this
step?", using bounding boxes, and passes only the survivors to the exact
tests of chapter 21. There is no single best algorithm: which one wins
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
linear BVH sorts object centers by their 30-bit Morton code (interleaved
coordinate bits), builds the tree from the sorted codes in parallel and
traverses it per object
([Karras, HPG 2012](https://research.nvidia.com/publication/2012-06_maximizing-parallelism-construction-bvhs-octrees-and-k-d-trees)).
All-pairs testing does `n²/2` box tests with perfectly regular memory access,
which on a GPU can beat both for a few thousand objects.

## What runs today

[`aethyr/physics/sim/broad.coax`](../physics/sim/broad.coax) holds up to 128
bodies as a `Broad_Set`: one box per body, and the filter fields (a static
flag, the joint parent link, a collision group and a mask). `broad_box`
builds a body's box for one step: the box at the start of the step joined
with the box at the end, widened by a margin on every side. `broad_pairs`
runs one of three methods, each a named profile:

- `broad-all-pairs-v1` tests every pair, `n(n − 1)/2` box tests.
- `broad-sweep-prune-v1` keeps the bodies sorted by the low `x` edge of
  their boxes, with ties broken by index, and walks that order. It sorts by
  insertion sort, starting from the previous step's order, so a scene that
  barely moved costs a few swaps.
- `broad-lbvh-v1` sorts the box centers by 30-bit Morton code, builds the
  tree from the sorted keys as Karras describes (each internal node found
  from its own index, with no dependence on the others), fits the boxes
  bottom-up, and walks the tree once per body.

Each method writes pairs as the key `i * 128 + j` with `i < j` and then
sorts the keys, so every method returns the same list in the same order.
Each call also counts its box tests, the part of its cost that does not
depend on the machine.

The example [`find-candidate-pairs.coax`](../../aethyr/examples/find-candidate-pairs.coax)
runs the three methods on random, clustered and stacked scenes:

<!-- source: aethyr/examples/find-candidate-pairs.coax example -->
```jai
unit :: (seed: int, k: int) -> f64 { return cast(f64) rand_unit(seed, 20, k); }

Layout :: enum u8 { random; clustered; stacked; }
layout_name :: (l: Layout) -> string {
  if l == Layout.random { return "random"; }
  if l == Layout.clustered { return "clustered"; }
  return "stacked";
}

// n boxes with half-size 0.5 in a 20 m cube, in 8 tight clusters, or in
// one column on y with each box touching the next.
scene :: (s: [] Broad_Set, l: Layout, n: int) {
  s[0].count = 0;
  for i: 0..<n {
    c := d3(unit(1, 3 * i) * 20.0, unit(1, 3 * i + 1) * 20.0, unit(1, 3 * i + 2) * 20.0);
    if l == Layout.clustered {
      g := i % 8;
      c = d3_add(d3(unit(2, g) * 20.0, unit(3, g) * 20.0, unit(4, g) * 20.0), d3_scale(d3_sub(c, d3(10.0, 10.0, 10.0)), 0.15));
    }
    if l == Layout.stacked { c = d3(0.0, cast(f64) i, 0.0); }
    _ := broad_add(s, broad_box(c, d3(0.5, 0.5, 0.5), D3_ZERO, 0.0, 0.0));
  }
}

main :: () {
  s: [1] Broad_Set; a: [1] Broad_Pairs; b: [1] Broad_Pairs; c: [1] Broad_Pairs;
  print("scene       bodies  pairs  same  box tests: all-pairs  sweep-prune  lbvh\n");
  for l: 0..<3 {
    for n: [3] int.[32, 64, 128] {
      scene(s[..], cast(Layout) l, n);
      broad_pairs(s[..], Broad_Method.all_pairs, a[..]);
      broad_pairs(s[..], Broad_Method.sweep_prune, b[..]);
      broad_pairs(s[..], Broad_Method.lbvh, c[..]);
      same := a[0].count == b[0].count && a[0].count == c[0].count;
      for k: 0..<a[0].count { same = same && a[0].key[k] == b[0].key[k] && a[0].key[k] == c[0].key[k]; }
      print("%  %  %  %  %  %  %\n", layout_name(cast(Layout) l), n, a[0].count, ifx same then "yes" else "no",
            a[0].tests, b[0].tests, c[0].tests);
    }
  }

  // 128 random bodies drift at up to 1 m/s for ten steps of 1/60 s.
  print("sweep and prune swaps, 128 random bodies drifting:\n");
  scene(s[..], Layout.random, 128);
  for step: 0..<10 {
    broad_pairs(s[..], Broad_Method.sweep_prune, b[..]);
    print("  step %: % swaps, % pairs\n", step, b[0].swaps, b[0].count);
    for i: 0..<s[0].count {
      d := d3_scale(d3(unit(5, i) - 0.5, unit(6, i) - 0.5, unit(7, i) - 0.5), 2.0 / 60.0);
      s[0].box[i].lo = d3_add(s[0].box[i].lo, d); s[0].box[i].hi = d3_add(s[0].box[i].hi, d);
    }
  }
}
```

Run it with:

```shell
coax/bin/coax run aethyr/examples/find-candidate-pairs.coax
```

```text
scene       bodies  pairs  same  box tests: all-pairs  sweep-prune  lbvh
random  32  0  yes  496  49  368
random  64  2  yes  2016  193  924
random  128  10  yes  8128  791  2270
clustered  32  7  yes  496  70  406
clustered  64  36  yes  2016  276  1050
clustered  128  151  yes  8128  1058  3116
stacked  32  31  yes  496  496  456
stacked  64  63  yes  2016  2016  1060
stacked  128  127  yes  8128  8128  2400
sweep and prune swaps, 128 random bodies drifting:
  step 0: 3734 swaps, 10 pairs
  step 1: 7 swaps, 10 pairs
  step 2: 7 swaps, 10 pairs
  step 3: 1 swaps, 10 pairs
  step 4: 4 swaps, 9 pairs
  step 5: 6 swaps, 9 pairs
  step 6: 5 swaps, 9 pairs
  step 7: 5 swaps, 10 pairs
  step 8: 3 swaps, 10 pairs
  step 9: 6 swaps, 12 pairs
```

Every scene gets the same pair list from all three methods. The cost does
not. On sparse random scenes, sweep and prune makes about a tenth of the box
tests of all pairs. The tree's share falls as the scene grows, from
three quarters of all pairs at 32 bodies to under a third at 128. On the stacked column every
box covers the same `x` interval, so sweep and prune tests every pair, the
worst case the design warns about, while the tree still prunes. The drifting
scene shows the temporal coherence: the first sort costs 3,734 swaps, and
each later step costs at most 7.

The checks run with:

```shell
coax/bin/coax test aethyr/physics/sim/broad.coax --library
```

They compare every method with a reference that tests separation on each
axis, on random scenes of every size from 0 to 39 bodies and on full sets of
128, on degenerate scenes (all boxes equal, a row touching face to face, a
column on `y`) and on empty and one-body sets. They check that a bullet that
passes through a thin wall within one step is reported although neither end
of its motion touches the wall, that static pairs, joint links and masked
groups never appear, that a still scene costs sweep and prune no swaps, the
Morton codes and the merge sort, and that both selective methods make fewer
box tests than all pairs on a sparse scene.

## The design in the Metaverse Engine

All three methods produce the same thing: the set of candidate pairs
`(i, j)` with `i < j`, sorted lexicographically. Sorting is part of the
contract, because the contact solver's iteration order (chapter 22) depends
on pair order and therefore so do its bits. A method is a profile choice;
switching methods never changes a result, which the checks prove.

Filters are applied before boxes: a pair is skipped if both bodies are
static, if they share a link-adjacency exclusion (parent and child links of
a joint), or if their collision groups and masks say so. Per-environment
batches (chapter 31) run the broad phase for every environment in one
launch, with pairs never crossing environments.

The choice between methods is measured, not written into code: a small
benchmark at build time on the target, or a recorded choice per scene size
and backend, with both the choice and its measurement kept in the scene
record (chapter 37).

## What it still builds

- Generated CUDA and Metal kernels for the three methods through Psionic
  chapter 37's compute subset, with device sorts and scans, checked against
  the CPU pair lists.
- Per-environment batches (chapter 31) in one launch, with no pair crossing
  environments.
- Wall-clock measurements on each backend, the crossover sizes they give,
  and the chosen method recorded in the scene (chapter 37).

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

Chapter 17's scene and chapter 18's bodies; Psionic chapter 36's batching; Psionic chapter 37's compute subset for device sorts and
scans. From Coax: R15's deterministic schedules for parallel sorts and R21's
batch dimension.
