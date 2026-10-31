# 176. Paged state and a tiered prefix cache

> **Status: unimplemented — design draft.** Nothing here runs yet.

Requests that share a prefix repeat the same prefill. This chapter stores model state in fixed-size pages and keeps a prefix cache across requests. The cache spills from device memory to host memory to local disk.

DeepSeek's published serving system uses a shared prefix cache of 128-token pages. This chapter builds the same idea in Coax, with exact identities for every page.

## What this chapter covers

- **Fixed-size pages.** KV cache pages and hybrid recurrent snapshots both use fixed sizes. A page table maps a sequence to its pages.
- **Content-hashed chains.** A prefix is found through a hash chain over 128-token pages. A page's identity includes the model identity and the exact token prefix, as in [chapter 36](ch036-many-questions-at-once.md).
- **Three tiers.** Pages move from device memory to host memory to local disk. Each tier has a measured capacity and eviction rule.
- **Storage independent of compute.** Pages never depend on one compute machine staying alive.
- **Layer-by-layer restore.** Restoring a cached prefix overlaps with compute, one layer at a time.
- **Retention per traffic type.** Each traffic type has its own retention setting.
- **Approximate reconstruction is separate.** Any approximate rebuild, such as bounded sliding-window replay, is its own profile and never enters an exact cache identity. DeepSeek's own description makes that replay approximate.

## What it builds

- A planned page allocator and prefix cache in `psionic/lib/serve/`.

## Dependencies

- [Chapter 175](ch175-continuous-batching-and-a-fair-scheduler.md): the scheduler and its state-memory budget.
- [Chapter 12](ch012-decisions-have-a-different-mask.md): the identity-bound prefix snapshot.
- [Chapter 23](ch023-make-the-numbers-move.md): exact per-request prefix snapshots and their parity checks.
- [Chapter 36](ch036-many-questions-at-once.md): prefix reuse keyed by model identity and exact token prefix.
- [Chapter 98](ch098-pack-the-cache-exactly.md): exact packed cache formats where the reference already rounds.

## Done means

- A request served from a cached prefix produces output identical to a cold run of the same request.
- A page restored from each tier has the same bytes it had when it was stored.
- [chapter 172](ch172-benchmark-a-service-not-a-kernel.md) reports hit rate, eviction counts and time to first token on hits versus misses for a recorded long-prefix workload.
- A change to the model identity or to one prefix token produces a cache miss.
- No approximate profile shares an identity with an exact page.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
- [DeepSeek target](../docs/targets/deepseek.md)
