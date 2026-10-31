# 182. Disaggregate prefill and decode

> **Status: unimplemented — design draft.** Nothing here runs yet.

Prefill is compute-bound and decode is memory-bound. At high volume, running both on the same workers makes each slow the other. This chapter separates them into prefill workers and decode workers that exchange a complete state snapshot.

The separation is conditional. You build it only after [chapter 172](ch172-benchmark-a-service-not-a-kernel.md) and [chapter 175](ch175-continuous-batching-and-a-fair-scheduler.md) measure that prefill interference is the bottleneck.

## What this chapter covers

- **A state-transfer message.** A prefill worker sends a complete, versioned state snapshot to a decode worker. The snapshot extends [chapter 12](ch012-decisions-have-a-different-mask.md)'s identity-bound claim for transfer and includes recurrent state, not only KV blocks.
- **A prefill queue and a local-prefill threshold.** Short prompts prefill locally on the decode worker. Longer prompts go to the prefill queue.
- **An adaptive xPyD ratio.** The ratio of prefill to decode workers adjusts to traffic.
- **The reference design.** DeepSeek's published layout: small prefill groups, large decode groups, a separate vision encoder role, prefill scaled on unserved tokens and decode scaled on requests in flight. DeepSeek reports a prefill-to-decode ratio near 1:1.

## What it builds

- A planned snapshot transfer message in `proto/` and worker roles in the scheduler.

## Dependencies

- [Chapter 172](ch172-benchmark-a-service-not-a-kernel.md): the measurement that must show the need first.
- [Chapter 175](ch175-continuous-batching-and-a-fair-scheduler.md): continuous batching and the interference it measures.
- [Chapter 176](ch176-paged-state-and-a-tiered-prefix-cache.md): pages and snapshots.
- [Chapter 12](ch012-decisions-have-a-different-mask.md): the identity-bound snapshot.
- [Chapter 177](ch177-route-queue-and-scale-replicas.md): the router and autoscaler that scale each role.

## Done means

- A request served through a transferred snapshot produces output identical to the same request served on one worker.
- A snapshot with a wrong model identity or version is refused.
- On a workload where [chapter 172](ch172-benchmark-a-service-not-a-kernel.md) shows prefill interference, disaggregation lowers inter-token latency P99 by a measured amount, or the chapter reports that it does not.
- The transfer cost per request is measured and reported.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
- [DeepSeek target](../docs/targets/deepseek.md)
