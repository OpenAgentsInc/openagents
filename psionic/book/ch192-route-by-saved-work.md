# 192. Route by saved work

> **Status: unimplemented — design draft.** Nothing here runs yet.

A mesh can save prefill by sending a request to a peer that already holds its prefix. This chapter routes by that saved work and records whether the saving was real.

Cache advertisements are hints, not facts. A claimed hit can be gone by the time the request arrives.

## What this chapter covers

- **The comparison.** Queue delay plus restore and suffix-prefill cost on a peer with a cached prefix, against cold prefill elsewhere.
- **Bounded advertisements.** Advertisements are bounded, short-lived and salted, and never contain raw tokens.
- **Fallback.** A claimed hit that turns out to be missing falls back to cold prefill.
- **Recorded savings.** Each routed request records its realized saving.

## Dependencies

- [Chapter 176](ch176-paged-state-and-a-tiered-prefix-cache.md): paged state and the prefix cache.
- [Chapter 177](ch177-route-queue-and-scale-replicas.md): cache-aware routing on one cluster.
- [Chapter 188](ch188-peers-transport-and-discovery.md): beacons and peers.
- [Chapter 189](ch189-place-the-model.md): placement.

## Done means

- No advertisement contains raw tokens, checked by a test that searches advertisements for fixture token sequences.
- A request routed to a missing hit completes with the same output as a cold run.
- On a recorded workload, realized savings are reported against a routing baseline that ignores caches.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
