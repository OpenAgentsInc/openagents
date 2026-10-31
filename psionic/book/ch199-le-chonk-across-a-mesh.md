# 199. Le Chonk across a mesh

> **Status: unimplemented — design draft.** Nothing here runs yet.

Mistral Large 4, called Le Chonk, has about 1T total and 52B active parameters and 1,060.6 GB of FP8 originals. This chapter places it across machines with every feature that Part VIII qualifies.

Serving it to others needs a separate commercial licence from Mistral. Running it for OpenAgents' own agents is internal use.

## What this chapter covers

- **The model.** 72 layers, width 6144, 64 heads, vocabulary 204,800, 128 routed experts plus a shared expert, FP8 E4M3 weights with UE8M0 block scales.
- **Sparse attention across boundaries.** The indexer that selects the top 2,048 keys and the KV and indexer sharing map from [chapter 86](ch086-sparse-attention-and-shared-layers.md) cross stage boundaries. Layer 35, for example, is reused by many later layers.
- **The draft head.** The embedded multi-token-prediction head from [chapter 87](ch087-the-models-own-draft-head.md) runs as a speculator.
- **Records.** Parity, latency and cost are recorded for the placement that ran.
- **Licence.** Offering Large 4 as a service is an owner decision that needs Mistral's commercial licence.

## Dependencies

- [Chapter 82](ch082-original-fp8-weights.md): original FP8 weights.
- [Chapter 86](ch086-sparse-attention-and-shared-layers.md): sparse attention and shared layers.
- [Chapter 87](ch087-the-models-own-draft-head.md): the draft head.
- [Chapter 186](ch186-split-a-model-into-stages.md): the stage boundary contract, which must express the sharing map.
- [Chapter 198](ch198-mistral-large-4-across-a-mesh.md): Mistral Large 3 across a mesh.
- Offering the model to others needs Mistral's commercial licence, an owner decision.

## Done means

- Selected layers and tokens match a whole-model reference under [chapter 17](ch017-qualify-honestly.md)'s rules.
- Shared KV and indexer state that crosses a boundary is listed in the contract, and a missing part is refused.
- The draft head gives identical greedy output, and its measured acceptance is reported.
- Per-hop latency, per-stage memory and cost per token are recorded.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [Mistral target](../docs/targets/mistral.md)
