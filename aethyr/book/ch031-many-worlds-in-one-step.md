# 31. Many worlds in one step

> **Status: unimplemented — design draft.** Nothing here runs yet.

Training a policy needs millions of steps of experience. One world on a GPU
uses a fraction of the device, so engines step thousands of copies at once,
each slightly different, and reset each one independently when its episode
ends. The engineering is in keeping every copy's results identical to what
it would have been alone, while the copies share every kernel launch.

## What this chapter teaches

- A leading batch dimension over every solver array.
- Per-environment parameters and domain randomization, and what may vary
  (masses, friction, gains, poses) versus what may not (topology).
- Independent resets without disturbing other environments.
- Seeding: one counter-based stream per environment.
- Throughput accounting: aggregate steps per second versus per-environment.

## The model

Every state array gains a leading dimension `B`. A step is the same
transition applied to each slice:

```jai
x_b[k+1] = Φ(x_b[k], u_b[k]; θ_b),   b = 0 … B−1
θ_b      = θ̄ + sample(seed, b, episode_b)        // counter-based, per env
```

The contract is *batch invariance*: slice `b` of a batched run equals the
same environment run alone with the same seed and parameters, bit for bit,
in the deterministic profile. This rules out reductions that mix
environments and makes the per-environment RNG stream a function of
`(seed, b, episode)` only.

## The design in Aethyr

Coax R21's logical tensors are what make this cheap to write: solvers are
written for one world and gain the batch dimension through the layout, not
through rewritten indexing. Until R21 lands, batched kernels index an
explicit `env` dimension. Resets copy a snapshot slice (chapter 36's format)
into the slots marked done, inside the same step. Randomization specs are
data with provenance (R20): each randomized parameter is named, with its
distribution and range. The step report gives aggregate steps per second,
per-environment steps per second and the batch size, always together.

## What it builds

- Batch dimension through every solver; `aethyr/sim/batch.coax` for
  resets, randomization and seeding.
- A benchmark of a rigid quadruped and a Panda arm at B = 1 to 16,384.

## Acceptance checks and oracles

- Batch invariance: random slices of a B = 4,096 run equal solo runs bit for
  bit.
- Reset isolation: resetting env `b` leaves every other slice's digest
  unchanged.
- Randomization: sampled parameters match their declared distributions
  (Kolmogorov–Smirnov at stated sample size); reproducible from the seed.
- Throughput reported with both rates; a regression check on per-env rate.

## How Genesis does it

Genesis builds scenes with `n_envs`, requiring every environment to share
topology, supports per-environment parameters and resets, and reports
throughput such as 43 million aggregate frames per second over 30,000
environments, about 1,433 per environment
([parallel simulation](https://genesis-world.readthedocs.io/en/latest/user_guide/getting_started/parallel_simulation.html),
[technical report](../../docs/research/genesis/technical-report-2026-10.md)).
We adopt the same model and require batch invariance as a checked property,
and always report per-environment rates next to aggregate ones.

## Dependencies

Chapters 17–22 supply the first rigid batched profile and its Panda and
quadruped fixtures. The batch/reset calculation can be qualified before
particle solvers or material couplers exist.

Chapters 23–30 must each pass batch invariance and reset-isolation checks
before their batched matrix cells are admitted. They remain part of the
goal of batching every solver, not prerequisites for batching the rigid
profile. From Coax: R20 provenance, R21 batch dimension and layouts, R23
device graphs.
