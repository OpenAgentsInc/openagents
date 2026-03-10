# Psionic Metal GPT-OSS Lessons From `gpt-oss`

> Status: working notes, 2026-03-09
>
> Purpose: capture what the `gpt-oss` repository's reference Metal path teaches
> Psionic about GPT-OSS correctness, runtime shape, cache reuse, and Apple-local
> execution policy.
>
> Scope note: this document is about the `gpt-oss` repository's native Metal
> implementation and adjacent serving glue. It is not a claim that Psionic
> should adopt its C API, Python bindings, or all of its serving behavior
> wholesale.

## Why This Doc Exists

Unlike `llama.cpp` and `mlx-lm`, the `gpt-oss` repository is not just another
Apple-local inference stack. It is the model's own reference repository, and it
includes a dedicated Metal implementation that the README explicitly describes
as:

- Apple-Silicon specific
- not production-ready
- accurate to the PyTorch implementation

That makes it valuable in a different way from `llama.cpp`:

- `llama.cpp` is the stronger performance and backend-architecture reference
- `gpt-oss` is the stronger model-specific correctness and layout reference

For Psionic, that means the `gpt-oss` Metal path matters most for:

- `#3286`: real-model correctness, prompt parity, tokenizer/special-token
  behavior, and rope/yarn fidelity
- `#3285`: how a native Metal path should separate prefill from decode
- `#3269`: what a model-owned Metal kernel surface looks like for GPT-OSS
- `#3262`: what performance-relevant runtime structure already exists even in a
  "not production-ready" reference implementation

## Sources Reviewed

### Psionic

- `crates/psionic/docs/METAL_GPT_OSS_UNIFIED_PLAN.md`
- GitHub issues `#3262`, `#3269`, `#3285`, and `#3286`

### `gpt-oss`

- `README.md`
- `gpt_oss/responses_api/serve.py`
- `gpt_oss/responses_api/inference/metal.py`
- `gpt_oss/metal/include/gpt-oss/functions.h`
- `gpt_oss/metal/source/model.c`
- `gpt_oss/metal/source/context.c`
- `gpt_oss/metal/source/metal.m`
- `gpt_oss/metal/source/metal-kernels.c`
- `gpt_oss/metal/source/*.metal`
- `gpt_oss/metal/source/generate.c`
- `gpt_oss/metal/benchmark/end-to-end.cc`
- `gpt_oss/metal/benchmark/end-to-end-threadgroup.cc`
- `gpt_oss/metal/scripts/create-local-model.py`

## Executive Summary

The main lessons from `gpt-oss` are:

1. For GPT-OSS on Metal, correctness starts with an explicit model-specific
   runtime contract, not a generic backend shim.
2. Prefill and decode are distinct operations in the API and in the hot path.
3. KV reuse is a first-class context behavior, including cheap LCP-style reuse
   after reset.
4. The reference runtime preallocates one coherent context working set up front
   instead of allocating ad hoc during generation.
5. GPT-OSS-specific kernel coverage matters: RoPE, SDPA, top-k router softmax,
   expert routing metadata, scatter, dense/prefill MoE, decode MoE, gather, and
   accumulation are all backend-owned.
6. The offline model conversion/layout story is part of the backend contract,
   not an afterthought.

The most important distinction from `llama.cpp` is this:

- `llama.cpp` teaches Psionic how a high-performance general LLM Metal backend
  should look
- `gpt-oss` teaches Psionic what the GPT-OSS-specific model contract must stay
  faithful to

So this repo is especially relevant to `#3286`, but it also reinforces the same
execution-shape work already implied by `#3269` and `#3285`.

## What `gpt-oss` Gets Right

### 1. The Metal path is model-owned, not a generic afterthought

The `gpt-oss` repo does not hide its Metal implementation behind a generic
"device backend" abstraction. It has a dedicated `gpt_oss/metal` subtree with:

- its own C API
- its own Python bindings
- its own offline model conversion format
- its own benchmark programs
- its own Metal shader set

That is important because GPT-OSS is not a generic dense decoder model. The
reference implementation bakes in GPT-OSS-specific assumptions such as:

- MoE routing width
- attention head geometry
- sliding-window attention behavior
- rope/yarn parameters
- Harmony/tokenizer special-token layout

For Psionic, the lesson is not "fork the world into per-model backends." The
lesson is:

- the shared GPT-OSS contract must be explicit enough that backend lowering can
  stay faithful to the real model
- correctness-sensitive details should not be reconstructed from loose serving
  assumptions in outer layers

This is especially relevant to `#3286`.

### 2. Prefill and decode are separate API operations

The native API distinguishes:

- `gptoss_context_append_chars(...)`
- `gptoss_context_append_tokens(...)`
- `gptoss_context_process(...)`
- `gptoss_context_sample(...)`
- `gptoss_context_reset(...)`

That separation is materially useful.

`gptoss_context_process(...)` handles prompt prefill for tokens not yet present
in KV. `gptoss_context_sample(...)` handles next-token generation and continues
generation from the cached state.

This matters because Psionic's current native Metal path has been muddier:

- uncached prompt ingest still falls too close to decode-step replay
- serving code has carried too much prompt/decode policy itself

`gpt-oss` shows a cleaner contract:

- append tokens
- process uncached prompt tokens
- sample from cached state
- reset logical context without necessarily discarding reusable KV

That maps directly to `#3285`.

### 3. The context is explicitly batch-shaped for prefill

`gptoss_context_create(...)` takes `max_batch_tokens`, and the public API
documentation explicitly says larger values may improve prefill performance at
the cost of memory.

Inside `context.c`, the context preallocates activation buffers sized around:

- `max_batch_tokens`
- `context_length`
- model dimensions

Examples include:

- residual activations
- RMSNorm activations
- QKV activations
- SDPA activations
- gate activations
- expert predictions
- routing metadata
- MoE intermediate buffers
- score/probability/argmax buffers
- the full KV cache

That is a useful serving/runtime lesson for Psionic:

- prompt throughput is not just about kernels
- it is also about having a prompt-shaped working set and batching contract

This reinforces `#3285` and also clarifies why our current prompt path performs
so badly when it falls back to token-at-a-time behavior.

### 4. LCP-style KV reuse is a first-class context behavior

The `responses_api` Metal shim says the context handles LCP caching internally,
and the core API makes that true:

- `gptoss_context_reset(...)` resets `num_tokens`
- it intentionally does not clear `num_kv_tokens`
- it intentionally does not clear the token buffer
- subsequent `append` calls verify whether the new logical prefix still matches
  the existing cached tokens
- on mismatch, KV is invalidated starting at the first divergent token

That is a compact, honest serving primitive. The important point is not the
exact API name. The important point is:

- prefix reuse is not bolted on later in a separate cache layer
- the runtime itself knows how much of the logical prompt already matches the KV

For Psionic, that is relevant in two ways:

- `#3285`: prompt reuse should be integrated into the prompt runtime contract
- `#3286`: prompt-parity problems should be debugged against an implementation
  that already treats prompt identity and KV reuse explicitly

### 5. The hot path is kernel-owned across the whole GPT-OSS block

The native Metal path owns far more of the GPT-OSS block than Psionic currently
does. The loaded kernel surface includes:

- embeddings
- RMSNorm
- QKV projection
- dense QKV projection for prefill-sized batches
- RoPE
- SDPA
- top-k + router softmax
- expert routing metadata
- scatter
- dense MoE SwiGLU matmul
- dense MoE output matmul
- decode-style MoE SwiGLU matmul
- decode-style MoE output matmul
- gather-and-accumulate
- accumulate
- unembedding
- softmax
- sample

This is not just "Metal matmul plus some host glue." It is a model-specific
kernel surface intended to keep GPT-OSS structure on the device.

For Psionic, that reinforces `#3269`:

- remaining host-owned GPT-OSS math is still a real architectural gap
- MoE routing and accumulation especially should not stay primarily host-owned

### 6. The runtime already has separate prefill and decode MoE shapes

Inside `process_tokens(...)`, the runtime changes strategy based on batch size:

- for larger token batches it uses dense/prefill-optimized kernels
- for smaller token counts it uses decode-style kernels

That distinction shows up in both dense matmul selection and MoE handling.

For MoE specifically, the reference path distinguishes:

- prefill-oriented routing metadata + scatter + dense MoE matmuls +
  gather-and-accumulate
- decode-oriented expert-conditioned matmuls + accumulate

That is a strong lesson for Psionic:

- prompt/prefill and decode are not just the same graph with a different token
  count
- GPT-OSS MoE wants different execution shapes depending on token count

This is one of the clearest pieces of evidence that `#3285` and `#3269` are
separate but tightly linked issues.

### 7. The backend contract includes an offline Apple-GPU-specific layout

`create-local-model.py` does more than repackage weights. It writes a Metal
oriented local model artifact containing:

- model header metadata
- tokenizer and special-token metadata
- rope/yarn-related values
- Apple GPU layout UUID
- shared weights laid out for the runtime
- per-block MoE weights laid out for the runtime

On load, `model.c` then:

- mmaps tokenizer and weight regions
- advises the kernel on access pattern
- prefetches the mapped ranges
- tries to `mlock` the model weights
- wraps mapped regions directly as Metal buffers

This is a real lesson for Psionic because our Metal serving path still tends to
think in terms of "load generic tensors, then figure it out later."

The `gpt-oss` repo's answer is:

- the backend owns a concrete layout contract
- conversion and loading are part of runtime design, not just packaging

That is especially useful for `#3286`, because model-layout mistakes and prompt
parity mistakes can look very similar at runtime.

### 8. Correctness-sensitive model metadata is carried through explicitly

`create-local-model.py` and `model.c` carry model-specific details explicitly:

- context length
- number of blocks
- number of experts
- active experts
- embedding and MLP dims
- attention window
- rope theta
- interpolation scale
- yarn offset / scale / multiplier
- RMSNorm epsilon
- tokenizer UUID and special-token mapping

That matters because GPT-OSS correctness is not just "the tensors loaded."
Prompt shape and model output can drift if any of these values are wrong or
inconsistently interpreted.

For Psionic, this is another correctness lesson:

- the runtime-visible model contract should carry all GPT-OSS-sensitive metadata
  explicitly
- parity work should compare those values first before assuming the kernel math
  is at fault

### 9. The serving shim is thin, and that is mostly good

The Responses API Metal adapter is intentionally small:

- load `Model`
- create `Context`
- reset on a new request
- append tokens
- rely on internal LCP caching
- sample a bounded output chunk

That is useful because it keeps serving logic from reimplementing model/runtime
policy in Python.

There is one caveat: this shim is clearly reference-quality, not production
serving design. It generates up to `MAX_OUTPUT_TOKENS` in a chunk and then
dribbles them out from an in-memory list. That is fine for a simple example, but
it is not the policy Psionic should copy directly for real server/workbench
serving.

So the transferable lesson is:

- keep the serving shim thin
- but do not blindly inherit the example server's batching and streaming policy

## What Psionic Should Adapt

### A. Use `gpt-oss` as a correctness reference first

This is the main value.

Psionic should adapt:

- explicit checks against GPT-OSS-specific metadata
- tokenizer and special-token parity checks
- Harmony-rendered prompt parity checks
- rope/yarn parameter parity checks
- active-expert and attention-window parity checks

This belongs mainly under `#3286`.

### B. Make prompt processing a true runtime operation

Psionic should adapt:

- explicit prompt/prefill runtime semantics
- configurable prompt batch sizing
- prompt-runtime evidence in benchmark receipts
- prompt reuse integrated with runtime state, not just outer serving logic

This belongs mainly under `#3285`.

### C. Push more GPT-OSS block ownership into the backend

Psionic should adapt:

- backend-owned router top-k + weighting
- backend-owned MoE routing metadata
- backend-owned scatter/gather/accumulate
- backend-owned prompt-sized and decode-sized MoE execution shapes

This belongs mainly under `#3269`.

### D. Treat model conversion/layout as part of the backend contract

Psionic should adapt:

- explicit runtime-facing model metadata contracts
- clearer weight-layout expectations for Metal
- stronger evidence around mapped, staged, or transformed weight ownership

This matters to both correctness and performance. It primarily sharpens
`#3286`, but it also supports `#3262`.

## What Psionic Should Not Copy Blindly

### 1. Do not copy the example server's output buffering policy

The reference Responses API shim is intentionally simple. Psionic should not
assume that pre-generating up to a fixed token chunk is the right long-term
serving UX or protocol behavior.

### 2. Do not mistake "accurate" for "fast enough"

The README explicitly says the Metal path is not production-ready. So this repo
is a stronger correctness reference than a performance target.

`llama.cpp` remains the more important backend-shape performance reference.

### 3. Do not hard-code narrow model assumptions without an explicit contract

The reference backend has several fixed assumptions, such as:

- `num_active_experts == 4`
- specific supported expert counts for router kernels
- `head_dim == 64`
- `num_q_heads == 8 * num_kv_heads`

Psionic can adopt those for GPT-OSS if they are stated as GPT-OSS contract
facts, but should not let them leak in as accidental unexplained constants.

## Direct Comparison Against Psionic's Current Metal Problems

### `#3262`: same-host throughput gap

`gpt-oss` does not solve Psionic's performance gap by itself, but it confirms
that even a reference implementation already has:

- explicit prefill batching
- preallocated working sets
- kernel-owned GPT-OSS routing/attention/MoE paths

Those are all things Psionic still needs to improve materially.

### `#3269`: decode hot-path ownership gap

This issue is directly validated. The `gpt-oss` Metal backend clearly owns more
of the GPT-OSS decode and MoE hot path than Psionic's current host-driven path.

### `#3285`: prompt/prefill runtime gap

This issue is also directly validated. The native API and internals already
separate prompt processing from sampling and tie prompt throughput to
`max_batch_tokens`.

### `#3286`: real-model correctness and prompt-parity gap

This is where `gpt-oss` is most important. Its value is not just "it runs on
Metal." Its value is that it is the nearest model-specific reference for:

- tokenizer layout
- Harmony-adjacent special-token handling
- rope/yarn metadata
- GPT-OSS-specific runtime assumptions
- correctness-oriented local model conversion

## Bottom Line

The `gpt-oss` Metal path is worth adapting primarily as a correctness and model
contract reference, not as the final serving architecture.

The three biggest takeaways for Psionic are:

1. `#3286` needs to treat `gpt-oss` as a first-class parity oracle for tokenizer,
   prompt, metadata, and runtime assumptions.
2. `#3285` needs a real prompt/prefill runtime with explicit batch sizing and
   integrated KV reuse semantics.
3. `#3269` still needs the backend to own much more of GPT-OSS's router, MoE,
   and block hot path.

If `llama.cpp` tells us how a fast Metal GPT-OSS backend should feel, `gpt-oss`
helps tell us what the backend must remain faithful to while we get there.
