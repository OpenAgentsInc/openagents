# 84. Attention through a latent

> **Status: unimplemented — design draft.** Nothing here runs yet.

Multi-head latent attention stores a low-rank latent instead of full keys and
values, which makes the cache much smaller. Mistral Small 4, Large 3 and
Large 4 use it, and so does DeepSeek. This chapter builds the math once, in
both expanded and absorbed form.

## What this chapter covers

- Queries and keys split into rotary and non-rotary parts. In Large 4, each
  head has 64 rotary and 64 non-rotary dimensions, and values have 128.
- Keys and values reconstructed from a low-rank latent (KV rank 256 in Small
  4, 512 in Large 3 and Large 4).
- The absorbed projections that let decode attend in latent space without
  expanding keys and values.
- Exact accounting of the compressed cache's size.
- A comparison of the absorbed form with the expanded form.

## Dependencies

- [Chapter 10](ch010-attention-without-training.md): attention and caches.
- [Chapter 41](ch041-attention-in-tiles.md): the tiled attention profile.
- [Chapter 90](ch090-execute-deepseek-v2-lite.md) shares this math for
  DeepSeek V2-Lite.

## Done means

- Absorbed and expanded forms agree within a stated tolerance on tiny fixtures
  and on selected real layers.
- Measured cache bytes equal the accounted size.
- Results match references for selected layers and tokens.

## Sources

- [Chapter plan](PLAN.md), Part VIII.
- [Mistral target survey](../docs/targets/mistral.md).
