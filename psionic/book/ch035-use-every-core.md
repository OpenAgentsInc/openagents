# 35. Use every core

> **Status: unimplemented — design draft.** Nothing here runs yet.

A decision on the CPU uses one core today. This chapter spreads the work over
several cores without changing a single output byte. Parallel work counts only
when it reproduces the single-worker result exactly under the named CPU
profile.

## What this chapter covers

- Fixed work partitioning: each worker receives a disjoint range that depends
  only on the shape and the worker count.
- Tree-ordered merges: partial sums combine in a fixed tree order, so the
  floating-point result does not depend on which worker finishes first.
- Byte-identical output at 1, 2, 4 and 8 workers.
- A host thread pool that calls Coax kernels on disjoint ranges. This is the
  interim form until Coax has language jobs.
- The same replay gate on Coax jobs once they exist.
- Measured speedup per worker count, against chapter 33's baseline.

## What it builds

- A planned host worker pool and partition plan, and a replay gate that
  compares every multi-worker output with the single-worker output.

## Dependencies

- [Chapter 33](ch033-measure-the-work-and-plan-the-memory.md): the baseline
  workload and timing.
- [Chapter 14](ch014-execute-elephant.md): the decision path you parallelize.
- Coax language jobs, threads and a memory model
  ([Coax chapter 14](../../coax/book/ch14-jobs.md)) for the second half of the
  chapter. They remain planned.

## Done means

- For every admitted shape, outputs at 1, 2, 4 and 8 workers are byte-equal to
  the single-worker output.
- A worker that fails or is cancelled publishes no output, and the caller sees
  a typed refusal.
- The chapter reports measured wall time per worker count with repetitions and
  spread, on a named host.
- The same gate passes again after the host pool is replaced by Coax jobs.

## Sources

- [Chapter plan](PLAN.md), Part V and the prerequisite map ("Decision
  throughput").
- [Coax roadmap](../../coax/ROADMAP.md).
