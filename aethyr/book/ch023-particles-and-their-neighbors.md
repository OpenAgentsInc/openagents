# 23. Particles and their neighbors

> **Status: unimplemented — design draft.** Aethyr's particles are visual
> only (GPU sprites with no interaction). This chapter plans the shared
> particle substrate that chapters 24 to 26 build on. Nothing here runs
> yet.

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

## What it builds

- `aethyr/sim/particles.coax`: store, hashing, counting sort, scan,
  neighbor iteration, sampling.

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
