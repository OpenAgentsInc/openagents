# 177. Route, queue and scale replicas

> **Status: unimplemented — design draft.** Nothing here runs yet.

One replica serves a few users. More traffic needs several replicas, a router that sends each request to the right one and an autoscaler that changes the replica count.

In this chapter the routing, queueing and scaling policies are pure Coax procedures. You check them against a deterministic discrete-event simulation of recorded and synthetic traffic before any machine runs them.

## What this chapter covers

- **A cache-aware and adapter-aware router.** The router prefers a replica that holds the request's prefix pages from [chapter 176](ch176-paged-state-and-a-tiered-prefix-cache.md) or its adapter from [chapter 140](ch140-post-train-a-decision-model.md).
- **A bounded priority queue.** The queue has a stated maximum. A full queue refuses rather than grows.
- **A replica autoscaler.** Settings follow section 7.2 of *Inference Engineering*: minimum and maximum replicas, window, scale-down delay, concurrency target and scale to zero.
- **A prefill cost model.** Cost comes from uncached tokens and the prefix they attend to. Requests fall into length buckets, and engines pull work.
- **Decode balancing.** Decode balances both request count and total cache length per replica, following DeepSeek's published load balancers.
- **Simulation first.** A discrete-event simulator replays recorded and synthetic arrivals against the policies. It reports latency percentiles, queue depth, replica count and cost.

## What it builds

- Planned router, queue and autoscaler policies in `#authority` Coax files.
- A discrete-event traffic simulator used as the policies' test bed.

## Dependencies

- [Chapter 172](ch172-benchmark-a-service-not-a-kernel.md): recorded traffic and the load generator.
- [Chapter 175](ch175-continuous-batching-and-a-fair-scheduler.md): the per-replica scheduler.
- [Chapter 176](ch176-paged-state-and-a-tiered-prefix-cache.md): the prefix cache whose contents the router reads.
- [Chapter 178](ch178-start-fast.md): cold-start cost, which the autoscaler must account for.
- Hosted machines need a qualified Linux target and an owner-approved budget, recorded in `NEEDS_OWNER.md`.

## Done means

- The policies pass their authored checks and zero-survivor mutation, because they decide where work and money go.
- In simulation, the router's realized cache-hit rate and latency beat a round-robin baseline on recorded traffic by a stated margin, or the chapter reports that they do not.
- The queue never exceeds its bound, and refusals are counted.
- The autoscaler holds the concurrency target within a stated band and scales to zero after the stated delay.
- A live run on owner-approved machines reproduces the simulation's percentiles within a stated tolerance.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
- [DeepSeek target](../docs/targets/deepseek.md)
