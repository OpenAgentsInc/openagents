# 58. Continue a passage

> **Status: unimplemented — design draft.** Nothing here runs yet.

This chapter runs the first NVIDIA text generator in the plan,
Llama-3.1-Minitron-4B-Width-Base. It is a base model, so the task is plain
continuation: given a passage, produce the next tokens.

## What this chapter covers

- The pinned model: hidden size 3072, 32 query heads and head dimension 128.
  You read the head dimension from the config; you do not infer it by
  division.
- Its position rules and grouped-query attention, gated FFN and KV state.
- Full-sequence logits compared with cached incremental logits.
- Deterministic continuation (greedy) and seeded continuation through chapter
  53's sampler.
- NVIDIA model terms and Llama ancestry notices kept with the artifact.

## What it builds

- A planned model profile on the shared dense decoder path.

## Dependencies

- [Chapter 7](ch007-reading-original-weights.md): original weights.
- [Chapter 10](ch010-attention-without-training.md): dense attention and
  caches.
- [Chapter 52](ch052-tokenizers-and-chat-templates.md): the tokenizer.
- [Chapter 53](ch053-choosing-the-next-token.md): seeded sampling.

## Done means

- Token, intermediate and logit references match for recorded prompts.
- Full and cached logits agree under the stated profile.
- Greedy continuation reproduces exactly; a seeded continuation reproduces
  exactly with the same seed and counter.

## Sources

- [Chapter plan](PLAN.md), Part VI.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md):
  Llama-3.1-Minitron-4B-Width-Base.
