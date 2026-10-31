# 193. Compress activations exactly

> **Status: unimplemented — design draft.** Nothing here runs yet.

Activations that cross a stage boundary can dominate the link. This chapter negotiates a compression codec for them with a mandatory raw fallback.

The old DeepSeek work sent F32 residuals at 261 KB per token; BF16 would have halved that. That number came from the pre-reset Rust stage and is not a result of this chapter.

## What this chapter covers

- **Codec negotiation.** Both stages agree on a codec. Raw transfer is always available.
- **Exact round trips.** An exact codec must reproduce each frame bit for bit after decoding.
- **Full cost.** Encode, decode and copy time are all measured, not only the bytes saved.
- **Lossy formats are separate.** A lossy activation format is its own profile with its own quality evaluation.

## Dependencies

- [Chapter 186](ch186-split-a-model-into-stages.md): the boundary contract and its frames.
- [Chapter 188](ch188-peers-transport-and-discovery.md): link measurements.
- [Chapter 17](ch017-qualify-honestly.md): comparison rules for a lossy profile.

## Done means

- Every frame in a recorded run round-trips exactly through each exact codec.
- A peer that offers no common codec falls back to raw transfer.
- Bytes per token and encode, decode and copy time are reported for each codec, and end-to-end latency is compared with raw transfer.
- No lossy codec shares a profile name with an exact one.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [DeepSeek target](../docs/targets/deepseek.md)
