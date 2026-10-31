# 175. Continuous batching and a fair scheduler

> **Status: unimplemented — design draft.** Nothing here runs yet.

One request at a time leaves most of a device idle. This chapter adds token-level continuous batching: the scheduler admits and retires requests at every step, under a budget for state memory.

The gate is batch invariance. Every request's output must be identical to the output of the same request run alone. Psionic's fixed reduction order makes that testable.

## What this chapter covers

- **Step-level admission.** Requests join and leave the batch at each decode step. A request is admitted only when its state fits the memory budget.
- **Chunked prefill interleaved with decode.** A long prompt prefills in chunks between decode steps, so running streams keep their inter-token latency.
- **Preemption and priority classes.** A higher-priority request can preempt a lower one. The preempted request keeps or saves its state and resumes without a changed result.
- **A fair scheduler.** Each priority class has a stated share. The scheduler is a pure Coax procedure with authored checks.
- **Batch invariance.** The kernels use a fixed reduction order per row, so batch size and position do not change a request's arithmetic. Tiled attention from [chapter 41](ch041-attention-in-tiles.md) has its own named profile, and the gate states which profile applies.
- **State is more than KV.** Elephant and Mammoth carry recurrent and convolution state. The memory budget counts that state, not only the KV cache.

## What it builds

- A planned scheduler module in `psionic/lib/serve/`, checked in simulation before it runs on a device.

## Dependencies

- [Chapter 173](ch173-serve-a-model-with-an-honest-capability.md): the server this scheduler runs inside.
- [Chapter 172](ch172-benchmark-a-service-not-a-kernel.md): the load generator and baseline.
- [Chapter 36](ch036-many-questions-at-once.md): batched suffix forks that already prove one batched pass equal to sequential forks.
- [Chapter 41](ch041-attention-in-tiles.md): the tiled-attention profile.
- [Chapter 71](ch071-throughput-is-a-gate.md): bounded continuous batching for gpt-oss and the concurrency 1/4/8 measurements.
- Language jobs and threads ([Coax book chapter 14](../../coax/book/ch14-jobs.md)) or host workers under explicit replay gates.

## Done means

- Every request in a seeded mixed workload produces output identical to its solo run, bit for bit, under the named profile.
- No admitted request exceeds the state-memory budget, and a request that would exceed it waits or gets a typed refusal.
- A preempted request resumes and produces its solo output.
- [chapter 172](ch172-benchmark-a-service-not-a-kernel.md) reports aggregate and per-request tokens per second, time to first token and inter-token latency at several concurrency levels, against the one-at-a-time baseline.
- Each priority class receives its stated share within a stated tolerance in the discrete-event simulation of [chapter 177](ch177-route-queue-and-scale-replicas.md).

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
- [Inference across a mesh](../docs/serving/mesh.md)
