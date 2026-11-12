# 23. Particles and their neighbors

> **Status: partly implemented.** [`aethyr/physics/sim/particles.coax`](../physics/sim/particles.coax)
> (344 mutants: 331 killed, 13 marked equivalent with their reason)
> holds the shared particle store, sorts particles into cells by counting
> sort and finds fixed-radius neighbors on the CPU in f64, with overflow
> reported. It samples boxes and spheres into lattices, plain or jittered.
> Its checks compare neighbor sets with brute force. Device kernels, hashed
> unbounded grids, Poisson-disk sampling, sampling inside meshes and a
> measured choice between stored and recomputed lists are not built yet.

Liquids, cloth, sand and snow are all simulated, in this part, as particles
that interact with nearby particles. Whatever the material, every step has
to answer one question for every particle: which others are within radius
`h`? Answering it in `O(n)` with good memory locality, the same way on every
backend, is most of the cost of a particle simulator.

## What this chapter teaches

- Uniform spatial hashing and cell lists.
- Sorting particles by cell (counting sort, prefix sums) so neighbors are
  close in memory.
- Fixed-radius neighbor search and its determinism.
- Sampling shapes into particles at build time.

## The method

With cell size `h`, particle `i` at `x_i` lies in cell `c_i = ⌊x_i / h⌋`.
All neighbors within `h` lie in the 27 cells around `c_i`. A step computes
`c_i`, a key `k_i = hash(c_i)` (or a linear index in a bounded domain),
counts particles per key, takes an exclusive prefix sum to get each cell's
start, scatters particles into sorted order, and then each particle scans
its 27 cells:

```jai
count[k]  = |{ i : k_i = k }|
start     = exclusive_scan(count)
order     = stable scatter by k_i            // stable: ties keep index order
N(i)      = { j in cells around c_i : |x_i − x_j| < h }
```

Sorting by cell, ideally along a space-filling curve, keeps neighbor reads
coalesced on GPUs ([Green, *Particle Simulation using CUDA*, 2010](https://developer.download.nvidia.com/assets/cuda/files/particles.pdf);
[Ihmsen et al., 2014 SPH survey](https://doi.org/10.2312/egst.20141034)).

## What runs today

[`aethyr/physics/sim/particles.coax`](../physics/sim/particles.coax) keeps up to
512 particles in a `Pstore`: position, velocity, mass, a material ID and an
entity ID. A `Pgrid` is bounded: an origin, a cell size `h` and at most
4,096 cells, so a cell is a linear index rather than a hash. A grid too
fine for that many cells, and a particle outside the grid, are refusals,
never clamps.

`pgrid_build` is the counting sort from the method above: it counts
particles per cell, takes the exclusive scan for each cell's start, and
scatters particles in index order through one cursor per cell, so within a
cell particles keep index order. `pgrid_neighbors` then scans the 27 cells
around each particle in a fixed order (`z`, then `y`, then `x`) and keeps
the first 48 neighbors. It records every particle's true neighbor count
whether the list held them or not, and counts the particles whose lists
overflowed.

`psample_box` and `psample_sphere` fill a shape with a cell-centred lattice
of spacing `a`, each particle carrying `density × a³`. With jitter, each
point moves by up to `jitter × a / 2` per axis, drawn from a counter stream
of [`aethyr/core/rand.coax`](../core/rand.coax), so the same seed gives the
same points.

The example [`particle-neighbors.coax`](../../aethyr/examples/particle-neighbors.coax)
samples a block of water and a ball and searches each:

<!-- source: aethyr/examples/particle-neighbors.coax example -->
```jai
report :: (name: string, p: [] Pstore, h: f64) {
  g: [1] Pgrid; n: [1] Pneighbors;
  if pgrid_new(g[..], D3_ZERO, d3(1.0, 1.0, 1.0), h) != Pgrid_Refusal.none { print("%: too many cells\n", name); return; }
  if pgrid_build(p, g[..]) != Pgrid_Refusal.none { print("%: a particle is outside the grid\n", name); return; }
  pgrid_neighbors(p, g[..], n[..]);
  low := PSTORE; high := 0; sum := 0; mass := 0.0;
  for i: 0..<p[0].count {
    c := n[0].count[i]; sum += c; mass += p[0].mass[i];
    if c < low { low = c; }
    if c > high { high = c; }
  }
  print("%: % particles, % kg, h = % m\n", name, p[0].count, mass, h);
  print("  neighbors: min %, max %, mean %\n", low, high, floor(cast(f64) sum / cast(f64) p[0].count * 100.0 + 0.5) / 100.0);
  print("  distance tests: % (all pairs: %), overflowed lists: %\n", n[0].tests, p[0].count * (p[0].count - 1), n[0].overflow);
}

main :: () {
  p: [1] Pstore;
  // 0.4 x 0.2 x 0.2 m of water at 1000 kg/m^3, one particle per 0.05 m.
  print("block: % added\n", psample_box(p[..], d3(0.1, 0.1, 0.1), d3(0.5, 0.3, 0.3), 0.05, 1000.0, 0.0, 1, 1, 1));
  // A grid of 0.06 m cells over the unit cube needs 17^3 cells: refused.
  report("block, lattice", p[..], 0.06);
  report("block, lattice", p[..], 0.075);

  // The same block jittered by up to a quarter spacing per axis.
  p[0].count = 0;
  print("block: % added\n", psample_box(p[..], d3(0.1, 0.1, 0.1), d3(0.5, 0.3, 0.3), 0.05, 1000.0, 0.5, 7, 1, 1));
  report("block, jittered", p[..], 0.075);

  // A ball of radius 0.3 m at spacing 0.075 m: 4/3 pi r^3 / a^3 = 268.08.
  p[0].count = 0;
  print("ball: % added\n", psample_sphere(p[..], d3(0.5, 0.5, 0.5), 0.3, 0.075, 1000.0, 0.0, 1, 2, 2));
  report("ball", p[..], 0.11);
  report("ball", p[..], 0.2);
}
```

Run it with:

```shell
coax/bin/coax run aethyr/examples/particle-neighbors.coax
```

```text
block: 128 added
block, lattice: too many cells
block, lattice: 128 particles, 16.0 kg, h = 0.075 m
  neighbors: min 6, max 18, mean 12.25
  distance tests: 4192 (all pairs: 16256), overflowed lists: 0
block: 128 added
block, jittered: 128 particles, 16.0 kg, h = 0.075 m
  neighbors: min 4, max 17, mean 9.69
  distance tests: 4994 (all pairs: 16256), overflowed lists: 0
ball: 280 added
ball: 280 particles, 118.125 kg, h = 0.11 m
  neighbors: min 8, max 18, mean 13.8
  distance tests: 13072 (all pairs: 78120), overflowed lists: 0
ball: 280 particles, 118.125 kg, h = 0.2 m
  neighbors: min 34, max 80, mean 51.09
  distance tests: 39376 (all pairs: 78120), overflowed lists: 136
```

The block's mass is its volume times the density exactly, 16 kg. On the
lattice with `h = 0.075 m`, an interior particle has 18 neighbors: 6 along
the axes at 0.05 m and 12 along face diagonals at 0.0707 m. The cells cut
the distance tests to about a quarter of all pairs for the block, and to a
sixth for the ball at `h = 0.11 m`. The ball holds 280 particles, against
268.08 for its volume divided by `a³`; the difference is the lattice's
surface error. At `h = 0.2 m`, 136 particles have more than 48 neighbors,
and the report says so instead of dropping them silently.

The checks run with:

```shell
coax/bin/coax test aethyr/physics/sim/particles.coax --library
```

They compare neighbor sets with brute force on random and clustered clouds
and on a lattice whose spacing equals `h` exactly, where the axis
neighbors, at distance exactly `h`, must not count. They check that the
sort is stable, that every list follows the fixed order, that a full list
keeps its true count, the refusals, and that sampled counts and masses
match each shape's volume and density.

## The design in Aethyr

One particle store per scene: position, velocity, mass, a material ID and an
entity ID, plus solver-specific fields owned by each solver. The sort and
scan are the deterministic primitives the tensor IR (Psionic chapter 38)
already needs; here the stable scatter is required, not optional, because neighbor
order sets the order of every later sum. Neighbor lists are either stored
(bounded capacity, with overflow reported) or recomputed on the fly, a
measured profile choice.

Sampling turns shapes into particles at build: regular lattice, jittered
lattice from the counter-based RNG, or Poisson-disk, inside a primitive or
a watertight mesh (chapter 35).

## What it still builds

- Generated CUDA and Metal kernels for the build and the search through
  Psionic chapter 37's compute subset, with device scans, checked against
  the CPU lists.
- Hashed cells for unbounded domains, and cells ordered along a
  space-filling curve.
- Poisson-disk sampling, and sampling inside watertight meshes
  (chapter 35).
- A measured choice between stored and recomputed neighbor lists.

## Acceptance checks and oracles

- Neighbor sets equal brute-force `O(n²)` sets on random, clustered and
  boundary cases; order identical across worker counts and devices.
- Overflowing a bounded neighbor list is reported, never truncated quietly.
- Sampling: particle count and total mass match the shape's volume and
  density within the lattice's stated error.

## How Genesis does it

Genesis's particle solvers (SPH, PBD, MPM's particles) share spatial hashing
and sorting in Quadrants kernels
([solvers](https://github.com/Genesis-Embodied-AI/genesis-world/tree/v1.4.5/genesis/engine/solvers)).
We follow the usual cell-list design; our additions are stable ordering as a
contract and reported overflow.

## Dependencies

Chapter 17's scene; Psionic chapter 38's scans and sorts; Psionic chapter 37's compute
subset. From Coax: R15 deterministic schedules, R21 layouts.
