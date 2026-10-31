# 191. Experts across machines and from disk

> **Status: unimplemented — design draft.** Nothing here runs yet.

MoE models put most of their weights in experts that each token uses rarely. This chapter spreads experts across machines and streams experts from local disk when a model is larger than memory.

Both techniques change where an expert runs. Neither may change its result.

## What this chapter covers

- **Expert parallelism over a mesh.** Routed tokens travel to the machine that holds their expert, and the outputs are gathered back.
- **Expert streaming from disk.** Experts that do not fit in memory load from local disk on demand. Mesh-LLM's Flash-MoE plugin streams experts from local NVMe on one node; this chapter builds the idea in Coax.
- **The cache from [chapter 96](ch096-hot-experts-in-fast-memory.md).** Residency follows a recorded routing profile and decayed-frequency heat.
- **Comparison.** Results are measured against [chapter 85](ch085-execute-mistral-small-7.md)'s single-machine Mistral Small 4 results.

## Dependencies

- [Chapter 61](ch061-a-few-experts-then-a-router.md): the router.
- [Chapter 85](ch085-execute-mistral-small-7.md): Mistral Small 4 on one machine: 128 routed and 1 shared expert, top 4.
- [Chapter 96](ch096-hot-experts-in-fast-memory.md): the exact expert cache.
- [Chapter 183](ch183-split-a-model-across-devices.md): expert parallelism inside one fast domain.
- [Chapter 188](ch188-peers-transport-and-discovery.md): peer transport.

## Done means

- Routed tokens and gathered outputs produce the same results as [chapter 85](ch085-execute-mistral-small-7.md)'s single-machine run under the same profile.
- Disk streaming reports bytes read per token, hit rate and tokens per second.
- Per-hop latency for expert traffic is measured.
- A missing expert on a peer produces a typed refusal, never a silent substitute.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [Mistral target](../docs/targets/mistral.md)
