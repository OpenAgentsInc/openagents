# 86. Sparse attention and shared layers

> **Status: unimplemented — design draft.** Nothing here runs yet.

Mistral Large 4 adds several attention features that no smaller model in the
ladder has. You cannot run the whole 1,060.6 GB model on one machine, so this
chapter qualifies each feature on tiny fixtures and on selected real layers.

## What this chapter covers

- A lightning indexer (32 heads of 128, in FP8) that picks the top 2,048 keys
  for each query.
- Indexer and KV sharing across layers, from the pinned `kv_sharing` source
  map. For example, later layers reuse layer 35's state.
- No rotary positions on every fourth layer (`nope` repeats "rope, rope,
  rope, nope").
- Tanh soft-capping of logits at 30.
- YaRN scaling with factor 2048 over 512 original positions, up to 1,048,576
  positions.

## Dependencies

- [Chapter 84](ch084-attention-through-a-latent.md): latent attention.
- [Chapter 82](ch082-original-fp8-weights.md): FP8 decode, including UE8M0
  scales.
- [Chapter 62](ch062-attention-with-sinks-and-a-window.md): the YaRN rotary
  pattern.
- Whole-model execution comes later, in
  [chapter 199](ch199-le-chonk-across-a-mesh.md), which builds on this chapter.

## Done means

- Indexer selections match references for selected layers and tokens.
- A shared-layer read returns the source layer's state exactly.
- NoPE layers, soft-capping and YaRN pass their own tiny fixtures.

## Sources

- [Chapter plan](PLAN.md), Part VIII.
- [Mistral target survey](../docs/targets/mistral.md): Mistral Large 4 as
  configured.
