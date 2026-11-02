# 37. Measure the engine honestly

> **Status: unimplemented — design draft.** Nothing here runs yet.

A physics engine is easy to make fast by measuring the wrong thing. Report
aggregate steps per second over a large batch and the number is enormous;
report it for one robot and it is ordinary. Leave contacts out of the
benchmark scene and it is faster still. This chapter closes the part by
defining what we measure, against what, and what a claim about the engine
must include.

## What this chapter teaches

- The compatibility matrix as published data: what combinations exist.
- An acceptance suite: every chapter's oracles, run per backend and profile.
- Benchmark methodology: scenes, warm-up, compile and startup time,
  per-environment and aggregate throughput, and variance.
- Comparison with other engines without overclaiming.

## The measures

For a scene with `B` environments stepped `K` times in wall time `T`:

```jai
aggregate rate     = B K / T          // steps per second, all environments
per-env rate       = K / T            // what one environment experiences
real-time factor   = K Δt / T
```

Every claim reports all three, the scene (bodies, contacts, solver
iterations), the profile, backend and precision, and separately the five
Coax R22 iteration cases: cold compile, warm load, scene rebuild, parameter
change and structural change. Compile and startup time count as part of the
engine, not excluded setup.

## The design in Aethyr

**Matrix.** One machine-readable file lists, for each solver, backend,
coupler, gradient mode, renderer and precision, whether the combination is
supported, refused (with reason) or untested. Chapter 17's admission code
reads it; the book's tables are generated from it; a cell may be marked
supported only if the acceptance suite has a passing case for it.

**Suite.** The acceptance checks of chapters 17 to 36 run on each backend
in each profile, with their tolerances. Results are retained as
verification evidence, like Psionic's.

**Benchmarks.** Fixed scenes: falling boxes, a 20-box stack, a Panda
grasp, a quadruped on rough terrain, a dam break, a sand pour, a cloth
drape. Each reports the measures above with run-to-run variance, on named
hardware. Comparisons with Genesis or MuJoCo use the same scene definition
on the same hardware, or are not made.

## What it builds

- `aethyr/sim/matrix.json` (generated tables), the suite runner, the
  benchmark scenes and a results ledger.

## Acceptance checks and oracles

- Every supported matrix cell has a passing suite case; every refused cell
  a refusal fixture.
- Benchmarks repeat within their stated variance.
- Generated book tables match the matrix file.

## How Genesis does it

Genesis publishes a compatibility matrix and performance figures such as
43 million aggregate frames per second over 30,000 environments, which is
about 1,433 per environment
([technical report](../../docs/research/genesis/technical-report-2026-10.md),
[Genesis README](https://github.com/Genesis-Embodied-AI/genesis-world)).
We follow the practice of publishing both, and require per-environment
rates, compile time and the scene's contact load alongside every aggregate
figure.

## Dependencies

The measurement method and compatibility ledger can start with the first
scene profile. Every supported matrix cell and every benchmark must then
have evidence from the relevant chapters
[17](ch017-a-scene-owns-the-state.md)–[36](ch036-record-and-replay-a-world.md).
Completing the full matrix therefore depends on all of Part IV, even though
starting the ledger does not. An empty ledger or a rigid-only benchmark
does not complete the whole-engine measurement goal. From Coax: R0
measurements, R22 budgets.
