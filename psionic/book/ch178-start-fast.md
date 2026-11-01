# 178. Start fast

> **Status: unimplemented — design draft.** Nothing here runs yet.

Scale to zero and autoscaling only work if a new replica starts quickly. This chapter splits cold start into its parts, measures each part and shortens the slow ones.

The weights and the generated kernels both get content-addressed delivery, so a new replica fetches only what it does not already have.

## What this chapter covers

- **Four measured phases.** Machine start, process start, weight loading and kernel startup are timed separately with [chapter 18](ch018-time-a-decision.md)'s monotonic phases.
- **Content-addressed weights.** Delivery uses [chapter 7](ch007-reading-original-weights.md)'s file identities. A replica that already holds a file with the right digest does not fetch it again.
- **Mapped loading.** Weights load by memory mapping where the backend allows it, with observed cold and warm page counts.
- **Generated-kernel caches.** Compiled kernels are cached under a key of device, compiler identity and source identity. A change to any part of the key invalidates the entry.
- **Readiness after warm-up.** A replica reports ready only after the golden warm-up of [chapter 22](ch022-run-it-live.md) passes.

## What it builds

- A planned kernel cache keyed by device, compiler and source identity.
- A planned content-addressed weight fetcher.

## Dependencies

- [Chapter 7](ch007-reading-original-weights.md): authenticated file identities.
- [Chapter 18](ch018-time-a-decision.md): monotonic phase timing and cold/warm page counts.
- [Chapter 19](ch019-keep-the-model-resident.md): the resident session and its verified files.
- [Chapter 22](ch022-run-it-live.md): golden warm-up before readiness.

[Chapter 177](ch177-route-queue-and-scale-replicas.md)'s autoscaler uses the
cold-start cost this chapter measures.

## Done means

- Each of the four phases has a recorded time on a named host, for a cold machine and for a warm one.
- A replica with cached weights and kernels reaches readiness faster than a cold replica, by a measured amount.
- A kernel cache entry with a stale compiler or source identity is never used. A test changes each part of the key.
- A corrupted weight file fails its digest check, and the replica does not report ready.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
