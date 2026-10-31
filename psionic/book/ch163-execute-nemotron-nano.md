# 163. Execute Nemotron Nano

> **Status: unimplemented — design draft.** Nothing here runs yet.

NVIDIA-Nemotron-3-Nano-4B-BF16 is a hybrid model that mixes Mamba-2 layers
with transformer attention layers. This chapter executes the complete
pinned checkpoint, including all recurrent and cache state.

## What this chapter covers

- The pinned revision and its authenticated files.
- The layer pattern: which layers are Mamba-2 and which are attention.
- Mamba-2 layers on chapter 162's state-space core, with convolution state.
- Attention layers on the shared core, with a KV cache.
- Whole-sequence, chunked and token-step comparisons for the complete model.
- Request isolation: every recurrent and cache state belongs to one request.

## Dependencies

- [Chapter 162](ch162-a-recurrence-you-can-inspect.md): the state-space core.
- [Chapter 10](ch010-attention-without-training.md): attention and caches.
- [Chapter 7](ch007-reading-original-weights.md): authenticated weights.
- [Chapter 17](ch017-qualify-honestly.md): qualification rules.

## Done means

- Token, intermediate and logit references match the recorded reference
  under chapter 17's rules.
- Cached continuation equals the full-sequence result within the named
  profile.
- Measured memory and time are reported on a named host.

## Sources

- [Chapter plan](PLAN.md), Part XVII.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), Nemotron Nano entry.
