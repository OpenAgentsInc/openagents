# 267. A looped MoE transformer

> **Status: unimplemented — design draft.** This chapter specifies the
> recurrent middle-layer MoE transformer used by Inkling 2 Mini (3 passes per
> token) and `TML-d24-varloop-research` (variable passes). No looped-layer
> execution is implemented here yet.

A standard decoder runs each layer once per token. Inkling 2 Mini runs its
middle layers **three times per token**, reusing the same weights across passes
so the model spends more serial computation per generated token without growing
its parameter footprint.

Building on [chapter 61](ch061-a-few-experts-then-a-router.md) (MoE routing) and
[chapter 86](ch086-sparse-attention-and-shared-layers.md) (cross-layer state
sharing), this chapter defines the exact recurrence, KV-cache layout, and
variable-loop contract on a small hand-checkable model.

## What the chapter will build

1. **Three-stage schedule.** Execute prelude layers once, loop the middle
   block `L` times (`L = 3` for Inkling 2 Mini; `L >= 1` configurable for
   `TML-d24-varloop-research`), and execute postlude layers once.
2. **Per-pass KV cache.** Index cached keys and values by
   `(logical_layer, pass_index, position)` (or the checkpoint's declared
   sharing map) so pass 2 and pass 3 never clobber pass 1's attention state.
3. **Per-pass MoE routing.** Re-evaluate the MoE router on each pass through
   the looped block so a token can select different experts on pass 1, pass 2
   and pass 3.
4. **Prefill / decode equivalence.** Verify that prefilling a prompt in one
   batch produces the same logits and KV state as stepping token by token
   through all `L` passes.

## Acceptance checks

- **Hand calculation:** trace a 2-token sequence through a tiny 1-prelude,
  1-looped-MoE (`L = 3`), 1-postlude network by hand and check router choices
  and hidden states at every pass.
- **Independent oracle:** compare single-step decode and multi-token prefill
  outputs for `L = 1, 2, 3, 5` against an independent reference implementation.
- **Refused inputs:** reject `L = 0`, out-of-bounds pass indices, and KV-cache
  shapes that omit the pass dimension.
