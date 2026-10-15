# Batched environments and an honest env-steps/s benchmark

**Status:** Landed 2026-10-14 for issue
[#11659](https://github.com/OpenAgentsInc/openagents/issues/11659), port plan
item 4 of the [Genesis repo study](../research/genesis/repo.md). The idea
comes from Genesis's `scene.build(n_envs=...)` (genesis-world `df30b61a`,
Apache-2.0); the code is ours.

## What exists

- **MuJoCo:** `robot_sim::mujoco::batch` (feature `mujoco`).
  - `Batch::new(&model, envs, seed, Settings)` makes N `mjData` on one
    loaded `mjModel`.
  - When `Settings::randomize` is set, each environment instead gets its own
    `mj_copyModel` copy. Its body masses and inertias, geom friction and
    servo gains are drawn from `1 ± width` (MuJoCo keeps these on the
    model, not the data).
  - Environment `i` is seeded with `env_seed(seed, i)` (SplitMix64), so any
    environment can be rebuilt alone with `Batch::with_seeds`.
  - `reset(i)` restores one environment; `reseed(i, seed)` draws new
    parameters and resets it.
  - Actions are flat `envs × joints` position-target rows (name order), and
    observations flat `envs × (nq + nv)` rows. Servos follow the same
    speed-limited law as the single-environment `Model::step`.
  - `Batch::run(steps, control)`: a central policy on the caller's thread
    sees every environment's last observations and contact count, then
    persistent workers step every environment once, synchronised by a spin
    barrier. This is the learning-loop shape.
  - `Batch::rollout(steps, policy)`: a policy per environment. Each worker
    carries its environments through every step, with no hand-over between
    steps.
- **Native physics:** `physics::batch::BatchWorld`.
  - N independent `World`s built from per-environment seeds, with `step`,
    `step_each` (per-environment actions) and `rollout`.
  - A world's own island threads are switched off on batch workers
    (`parallel::serial_here`), because the batch already fills the cores.
  - `physics::batch::pile(seed, 50)`: 50 boxes, spheres and capsules
    dropped into a walled bin, with sleep off.
- **Determinism.** Each environment of a threaded batch of 64 matches the
  same environment run alone, bit for bit, step for step:
  - `robot-sim/tests/batch.rs`: SO-101 table grasp scene, randomised mass,
    friction and gains, 8 threads; the trace hash over `qpos` and `qvel`
    every step is compared;
  - `physics::batch::tests`: the 50-body pile, 8 threads; every body's
    position, velocity, spin and attitude are compared at every step.

## The benchmark

```sh
cargo run --release -p robot-sim --features mujoco --example batch_bench -- \
  LIBRARY FRANKA_ROOT [ENVS,ENVS,...] [THREADS]
```

- `LIBRARY` is the pinned MuJoCo 3.3.7 library (`~/work/film-out/mujoco` on
  the owner's Mac).
- `FRANKA_ROOT` is the Menagerie Panda, fetched with
  `crates/robot-sim/tests/fixtures/franka/fetch.py`.
- It prints the CPU, its core types, memory and the load average before and
  after.
- Each row shows the scene, the mode, the environment count, threads, `dt`,
  steps, wall time, **env-steps/s** (the sum over the batch), **steps/s per
  environment**, and **real time per environment** (simulated seconds per
  wall second for one environment).
- The `sync` rows also show proof that the scene is active: mean joint
  speed or body speed, and contacts per step.

Every timed step is real work, and no scene is idle:

| Scene | What moves | dt |
| --- | --- | --- |
| `franka-reach` | Menagerie Panda, 7 servos chasing a new random joint target every 0.5 s | 2 ms |
| `so101-grasp` | SO-101 on a table with a 5 cm cube, a 2 s reach, close, lift and open cycle (about 3.6 contacts per step) | 1/240 s |
| `pile-50` | 50 mixed bodies dropped into a bin, timed from the drop with sleep off (about 130 contact points per step) | 1/240 s |

## Results on the M5 Max

Apple M5 Max (6 performance + 12 efficiency cores, 128 GB), macOS, 18 worker
threads, 2 s simulated per environment, `f64`, recorded 2026-10-14.

**The machine was not idle.** Other agents' builds held the load average at
40 to 140 on 18 cores during every run (each run prints it). Treat these as
lower bounds. The `sync` mode suffers most: one descheduled worker stalls
the whole step. Re-run on an idle machine before quoting `sync` numbers.

`rollout` (per-environment policy), load 45 to 140:

| Scene | Envs | env-steps/s (total) | steps/s per env | real time per env |
| --- | ---: | ---: | ---: | ---: |
| franka-reach | 1 | 73,912 | 73,912 | 148x |
| franka-reach | 16 | 1,003,735 | 62,733 | 125x |
| franka-reach | 64 | 1,100,117 | 17,189 | 34x |
| franka-reach | 256 | 1,203,820 | 4,702 | 9.4x |
| franka-reach | 1024 | 1,307,195 | 1,277 | 2.6x |
| so101-grasp | 1 | 39,862 | 39,862 | 166x |
| so101-grasp | 16 | 413,857 | 25,866 | 108x |
| so101-grasp | 64 | 518,159 | 8,096 | 34x |
| so101-grasp | 256 | 428,764 | 1,675 | 7.0x |
| so101-grasp | 1024 | 496,735 | 485 | 2.0x |
| pile-50 | 1 | 2,299 | 2,299 | 9.6x |
| pile-50 | 16 | 19,200 | 1,200 | 5.0x |
| pile-50 | 64 | 16,040 | 251 | 1.0x |
| pile-50 | 256 | 19,258 | 75 | 0.31x |
| pile-50 | 1024 | 16,330 | 16 | 0.07x |

Single-environment `sync` rows from the same run, for scale:

- franka-reach: 142,855 steps/s (286x real time);
- so101-grasp: 78,835 steps/s (328x);
- pile-50: 2,373 steps/s (9.9x).

The single-environment rows of both modes vary by up to 2x between runs at
this load.

`sync` (central policy, a barrier every step) at 64 environments, same run:

- franka-reach: 8,438 env-steps/s;
- so101-grasp: 31,074;
- pile-50: 10,899.

That is below the single-environment rate for the arms: at this load the
barrier waits on descheduled workers. This is the number to re-measure idle.

## Reading the numbers

- A batch does not make any one environment faster. Real time per
  environment falls as the batch grows; only the total rises.
- The total saturates at roughly 1.3 M env-steps/s for the Franka (17x one
  environment) and 0.5 M for the contact-rich SO-101 (12x).
- The 50-body pile saturates near 16 to 19 k env-steps/s: about 0.8 M body
  steps/s, with roughly 130 contact points solved in every step.
- Genesis's headline (`num_steps × n_envs / time`) is our `env-steps/s`
  column. We do not quote it without real time per environment, the scene,
  the environment count and the hardware.
