# Psionic Metal GPT-OSS Lessons From `llama.cpp`

> Status: working notes, 2026-03-09
>
> Purpose: capture the concrete Metal and GPT-OSS lessons Psionic should adapt
> from the current `llama.cpp` codebase, beyond the higher-level conclusions
> already summarized in `METAL_GPT_OSS_UNIFIED_PLAN.md`.
>
> Scope note: this document is about architecture, runtime reserve/reuse,
> backend op surface, and serving-relevant execution shape. It is not a claim
> that Psionic should clone `ggml` or mechanically port `llama.cpp`.

## Why This Doc Exists

For native Metal GPT-OSS, `llama.cpp` is still the main reference implementation
that Psionic is trying to approach on the tracked Apple host.

That is not just because `llama.cpp` is "faster." It is because its current
Apple-local path already embodies a set of choices that Psionic's native Metal
lane still lacks:

- one model-owned OpenAI-MoE graph shape
- separate prompt-processing and token-generation reserve behavior
- graph reserve and reuse policy tied to backend samplers and outputs
- a broad Metal backend primitive surface
- grouped expert dispatch that stays backend-owned
- graph reordering and concurrency rules in the Metal backend itself
- specialized pipeline selection for attention, routing, and output ops

`METAL_GPT_OSS_UNIFIED_PLAN.md` already points at these themes. This doc exists
to make them more concrete by tying them back to the actual `llama.cpp` source
paths.

## Sources Reviewed

### Psionic

- `crates/psionic/docs/METAL_GPT_OSS_UNIFIED_PLAN.md`
- `crates/psionic/psionic-serve/src/gpt_oss.rs`
- GitHub issues `#3262`, `#3269`, `#3285`, and `#3286`

### `llama.cpp`

- `src/models/openai-moe-iswa.cpp`
- `src/llama-context.cpp`
- `ggml/src/ggml-metal/ggml-metal-common.cpp`
- `ggml/src/ggml-metal/ggml-metal-ops.cpp`
- `ggml/src/ggml-metal/ggml-metal-device.cpp`
- `ggml/src/ggml-metal/ggml-metal.metal`

## Executive Summary

The main lessons from `llama.cpp` are:

1. GPT-OSS is expressed as one backend-agnostic graph contract first, not as a
   backend-specific host step engine.
2. Prompt processing and token generation are both first-class runtime shapes,
   and the context reserves both explicitly.
3. Backend samplers and output ownership are part of graph shape and reserve
   policy, not an afterthought bolted onto the serving loop later.
4. Metal performance comes from a wide backend op surface plus aggressive
   pipeline specialization, not from one generic matvec shader.
5. Metal concurrency is guarded by backend-owned graph reorder logic and
   memory-range safety, not by a host loop issuing many tiny conservative
   barriers.
6. GPT-OSS MoE dispatch stays backend-owned through grouped-id execution rather
   than bouncing through per-expert host loops.

For Psionic, this means the remaining native Metal gap is still primarily about
execution shape, not one missing micro-optimization.

## What `llama.cpp` Gets Right

### 1. GPT-OSS is one model-owned graph shape

`src/models/openai-moe-iswa.cpp` builds GPT-OSS as one graph with stable node
ordering and explicit graph callbacks:

- `attn_norm`
- Q / K / V projection
- RoPE on Q and K
- attention output
- residual add into `ffn_inp`
- `attn_post_norm`
- MoE feed-forward via `build_moe_ffn(...)`
- output norm
- output projection

This matters because the host is not manually coordinating an ad hoc sequence of
"compute this vector, read it back, decide what to do next" substeps. The model
graph owns the shape first.

For Psionic, this reinforces the same core rule as the unified plan:

- the shared GPT-OSS contract should be graph-first
- backend lowering should decide how to execute that graph
- the serving layer should not remain the place where most of the block-level
  policy lives

Relevant reference:

- `src/models/openai-moe-iswa.cpp`

### 2. Prompt-processing and token-generation are reserved separately

`src/llama-context.cpp` is explicit that prompt processing and decode are not
the same runtime shape.

During reserve it:

- builds a worst-case reserve context
- resolves feature gates such as Flash Attention support
- reserves the prompt-processing graph first
- reserves the token-generation graph separately
- reserves prompt-processing again to avoid later allocator reallocations

That reserve flow is one of the clearest practical lessons for Psionic because
native Metal GPT-OSS currently still ingests uncached prompts by replaying the
decode-step runtime token-by-token.

Key ideas to adapt:

- reserve a prompt-shaped runtime explicitly
- reserve a decode-shaped runtime explicitly
- keep prompt and decode shapes honest in the runtime
- bias reserve policy toward avoiding allocator churn later

Relevant references:

- `src/llama-context.cpp`: `sched_reserve()`
- `src/llama-context.cpp`: `graph_reserve(...)`

### 3. Backend samplers are part of the reserved graph shape

`llama_context` initializes backend samplers before the reserve passes run.

That matters because it means:

- sampler-owned output nodes are present in the graph before reserve
- later graph reuse is less likely to be invalidated by output-path changes
- backend-side output selection is treated as part of the compute graph, not
  just host-side postprocessing

Psionic has already moved toward backend-owned bounded logits and greedy-token
selection, but `llama.cpp` shows the stronger rule:

- output ownership and graph reserve policy should be coupled

This is one reason `llama.cpp` can be more honest about when raw logits are or
are not actually needed.

Relevant reference:

- `src/llama-context.cpp`: backend sampler initialization before reserve

### 4. Metal backend owns a broad GPT-OSS-relevant op surface

`ggml-metal-ops.cpp` and `ggml-metal-device.cpp` expose a far broader Metal op
surface than Psionic's current native Metal GPT-OSS lane.

Relevant ops include:

- `MUL_MAT_ID`
- `NORM` / `RMS_NORM`
- `ROPE`
- `TOP_K`
- `FLASH_ATTN_EXT`
- `ARGMAX`

This matters because GPT-OSS parity on Metal is not about "do matmul on GPU and
do the rest on CPU." The backend needs enough primitives to keep the block hot
path on the device.

For Psionic, that maps directly to the remaining problem:

- current Metal GPT-OSS still performs too much block math and MoE control on
  the host

Relevant references:

- `ggml/src/ggml-metal/ggml-metal-ops.cpp`
- `ggml/src/ggml-metal/ggml-metal-device.cpp`

### 5. Metal graph reorder and concurrency live in the backend

`ggml-metal-common.cpp` contains graph-reorder logic that reasons about memory
range safety and only reorders across safe op classes.

Important features of that approach:

- it tracks source and destination memory ranges
- it only reorders when reads and writes do not conflict
- it limits reorder attempts to ops that are known-safe for concurrency
- it lets the Metal backend exploit more concurrency without violating graph
  correctness

This is important because it shows a better answer than a host loop that inserts
conservative waits after every meaningful substep.

For Psionic, the lesson is:

- concurrency policy should move down into backend/runtime planning
- graph reorder and memory-range awareness belong below the serving loop

Relevant reference:

- `ggml/src/ggml-metal/ggml-metal-common.cpp`

### 6. Flash attention is a family of specialized kernels, not one generic op

The `FLASH_ATTN_EXT` path in `llama.cpp` is not a single generic kernel.

What it does instead:

- chooses vector or non-vector path based on shape
- reserves extra padding, block, and temp buffers up front to avoid graph
  reallocations
- compiles pipeline variants keyed by:
  - input type
  - `dk` / `dv`
  - mask presence
  - sink presence
  - bias usage
  - logit softcap usage
  - KV padding
  - simdgroups per threadgroup

That design is a concrete example of the deeper rule:

- backend specialization should be driven by the real graph shape and real data
  layout, not hidden behind one abstract "attention kernel" label

For Psionic, the lesson is not necessarily to copy every variant name. The
lesson is:

- the backend should own shape-aware specialization explicitly
- reserve should pre-budget the extra buffers the specialized path requires
- graph/runtime identity should reflect those decisions so reuse stays truthful

Relevant references:

- `ggml/src/ggml-metal/ggml-metal-ops.cpp`: `FLASH_ATTN_EXT` extra buffer logic
- `ggml/src/ggml-metal/ggml-metal-device.cpp`: flash attention pipeline lookup
- `ggml/src/ggml-metal/ggml-metal.metal`: flash attention kernel family

### 7. Grouped expert dispatch stays backend-owned

`MUL_MAT_ID` in `llama.cpp` is a serious GPT-OSS-relevant path, not a placeholder.

The Metal path:

- supports `MUL_MAT_ID` directly
- computes intermediate id mappings on the device
- chooses matrix-matrix-id or matrix-vector-id style execution based on device
  capability and a break-even threshold
- uses dedicated kernels for grouped-id execution rather than falling back to
  nested host loops over experts

This is one of the most important lessons for Psionic because GPT-OSS MoE is
the part most likely to tempt a host-driven workaround.

For Psionic, the takeaway is direct:

- grouped expert dispatch should remain backend-owned
- per-expert host loops are the wrong shape
- device capability should influence grouped expert execution policy

Relevant references:

- `ggml/src/ggml-metal/ggml-metal-ops.cpp`: `ggml_metal_op_mul_mat_id(...)`
- `ggml/src/ggml-metal/ggml-metal-device.cpp`: `get_pipeline_mul_mv_id(...)`
- `ggml/src/ggml-metal/ggml-metal.metal`: `kernel_mul_mv_id_*`

### 8. The Metal shader surface is broad and quantization-aware

`ggml-metal.metal` contains a large family of instantiated kernels across:

- dense types like `f32`, `f16`, and `bf16`
- quantized types such as `q4_*`, `q5_*`, `q8_0`, `mxfp4`, and multiple `iq*`
  variants
- multiple head dimensions and value dimensions for attention
- grouped-id kernels for expert dispatch

The important lesson is not "copy every kernel." The lesson is:

- `llama.cpp`'s Metal path is fast because it has enough backend-native kernel
  coverage for the real model shapes it serves
- generic fallback kernels are not enough for GPT-OSS parity

This reinforces that Psionic's native Metal backend needs a wider and more
model-aware primitive surface if it wants to stop depending on host staging in
the hot path.

Relevant reference:

- `ggml/src/ggml-metal/ggml-metal.metal`

### 9. Reserve also acts as capability detection and fallback control

`llama.cpp` uses reserve passes not just for allocation sizing, but also to
resolve backend capability truth.

Examples include:

- auto-enabling or auto-disabling Flash Attention based on actual assigned
  device support
- checking fused Gated Delta Net support
- falling back when pipeline parallel reserve fails

That matters because it keeps runtime feature claims tied to actual backend
assignment reality.

For Psionic, this suggests a stronger discipline:

- backend capability claims should be validated against actual reserve/runtime
  success, not just compile-time support flags
- receipts should expose those decisions clearly

Relevant reference:

- `src/llama-context.cpp`: Flash Attention and fused GDN checks in reserve

## What Psionic Should Adapt

### A. Keep the shared GPT-OSS contract graph-first

Psionic should adapt:

- one reusable GPT-OSS graph/runtime contract
- explicit graph-owned node ordering for attention, post-attention norm, MoE,
  and output
- backend lowering that owns dispatch policy

Psionic should avoid:

- keeping the Rust serving loop as the real owner of block-level execution
  policy

Mapped issues:

- `#3269`
- `#3262`

### B. Introduce a real prompt/decode reserve split

Psionic should adapt:

- explicit prompt/prefill reserve path
- explicit decode reserve path
- reserve passes that budget worst-case buffers once
- runtime evidence for prompt reuse versus rebuild

This is the clearest direct lesson for the current Metal backlog.

Mapped issue:

- `#3285`

### C. Treat backend-side output selection as part of graph shape

Psionic should adapt:

- backend sampler or backend output ownership before reserve
- graph reserve that already includes the output mode needed for the request
  class
- truthful graph reuse metrics when output-path requirements change

Mapped issues:

- `#3262`
- partially `#3269`

### D. Expand Metal op coverage where it keeps GPT-OSS off the host

Psionic should adapt:

- more backend-owned norm, rope, routing, and output behavior
- grouped expert dispatch semantics that stay on device
- attention specialization that reflects real shapes

This does not mean "implement every `ggml` op." It means:

- prioritize the ops that remove host ownership from the GPT-OSS hot path

Mapped issue:

- `#3269`

### E. Move concurrency and reorder policy into backend planning

Psionic should adapt:

- memory-range-aware scheduling and reorder logic where appropriate
- fewer host-side conservative waits
- backend/runtime-owned barriers only where dependency truth requires them

Mapped issue:

- partly `#3269`
- partly `#3262`

### F. Use reserve as a truth surface for capability and fallback

Psionic should adapt:

- reserve-time feature validation
- explicit runtime fallback evidence
- receipts that reflect the real reserved path instead of only static intent

Mapped issues:

- `#3262`
- `#3286`

## What Psionic Should Not Copy Blindly

### 1. Do not cargo-cult the entire `ggml` stack

`llama.cpp` owns its own graph IR, scheduler, backend registry, kernel naming,
and allocator strategy.

Psionic should learn from the architecture, but not assume that direct source
translation is the fastest route to correctness or maintainability.

The right adaptation is:

- preserve the architecture lesson
- implement only the Psionic-native pieces that close the real gap

### 2. Do not confuse "many kernels" with "random kernel sprawl"

`llama.cpp` has a wide kernel family because its graph and backend have already
committed to serving many real shapes and quantizations.

Psionic should not respond by opening a generic "add every Metal kernel"
backlog. Kernel work should stay tied to:

- actual GPT-OSS graph hot spots
- actual same-host benchmark bottlenecks
- actual missing backend ownership in the Metal path

### 3. Do not copy thresholds or heuristics without measuring

Examples:

- break-even thresholds for grouped expert execution
- vec versus non-vec attention cutoffs
- simdgroup counts and shared-memory tuning

Those are useful clues, not Psionic acceptance criteria. Psionic should borrow
the idea of shape-aware policy, then measure on the tracked Apple host.

### 4. `llama.cpp` does not solve Psionic's prompt/render correctness problem

`llama.cpp` is the throughput and architecture reference, but Psionic's current
real-model malformed-output problem also involves prompt/render/token parity.

So while `llama.cpp` teaches important runtime lessons, it does not remove the
need for a dedicated correctness track in Psionic.

Mapped issue:

- `#3286`

## Direct Comparison Against Psionic's Current Metal Problems

### Problem 1: prompt replay instead of prompt runtime

Psionic today:

- uncached prompt ingest still behaves like decode-step replay

`llama.cpp` lesson:

- reserve prompt-processing and token-generation separately
- budget prompt buffers explicitly

Primary tracker:

- `#3285`

### Problem 2: host-owned GPT-OSS hot-path math

Psionic today:

- too much norm, rope, routing, activation, and aggregation work is still
  host-owned

`llama.cpp` lesson:

- the backend needs enough op coverage and lowering ownership to keep the block
  path on device

Primary tracker:

- `#3269`

### Problem 3: grouped expert dispatch still lacks `llama.cpp`-class shape

Psionic today:

- grouped dispatch exists in pieces, but the overall MoE path is still too
  host-driven and too synchronization-heavy

`llama.cpp` lesson:

- grouped-id execution should remain backend-owned and capability-aware

Primary tracker:

- `#3269`

### Problem 4: reserve/reuse is not yet as honest as the reference

Psionic today:

- runtime reuse exists, but prompt/decode reserve honesty and fallback evidence
  are still behind the reference behavior

`llama.cpp` lesson:

- reserve is where feature truth, buffer sizing, graph identity, and fallback
  decisions get nailed down

Primary trackers:

- `#3262`
- `#3285`

### Problem 5: correctness and output parity still need dedicated work

Psionic today:

- the real same-host benchmark shows malformed repetitive native-Metal output

`llama.cpp` lesson:

- a production-grade runtime keeps graph shape, output ownership, and reserve
  policy tighter than Psionic currently does
- but Psionic still needs a dedicated prompt/output parity track to isolate the
  exact failure

Primary tracker:

- `#3286`

## Issue Mapping

### Already covered well enough by existing issues

- `#3262`
  - overall same-host throughput target and receipts
- `#3269`
  - move remaining GPT-OSS decode math and MoE control off host
- `#3285`
  - add a true Metal GPT-OSS prompt/prefill graph
- `#3286`
  - fix real-model native Metal Harmony output correctness and prompt parity

### Likely future issue, but not yet the next move

There is still one likely follow-up issue that this `llama.cpp` scan also
supports:

- native Metal GPT-OSS serving/runtime policy after the single-request path is
  honest
  - richer graph reuse policy
  - batched prompt/decode scheduling
  - deeper reserve-time fallback evidence
  - richer prefix-cache reuse policy

That should probably wait until the existing prompt, decode, and correctness
issues have materially progressed.

## Recommended Psionic Next Steps

In order:

1. Finish `#3285` so prompt processing stops being decode replay.
2. Keep pushing `#3269` until GPT-OSS block math and MoE control stop depending
   on the host in the steady-state path.
3. Use `#3286` to isolate prompt/render parity versus native Metal numerical
   drift.
4. Tighten receipts in `#3262` so prompt reserve, decode reserve, fallback, and
   output-path evidence are more explicit.
5. Only after the single-request path is honest, open a dedicated issue for
   batch-oriented native Metal serving policy if it is still warranted.

## Bottom Line

`llama.cpp` keeps reinforcing the same answer:

- graph-first model contract
- prompt/decode reserve split
- backend-owned grouped expert dispatch
- backend-owned output selection
- wide Metal op coverage
- backend-owned concurrency and specialization

Psionic's native Metal GPT-OSS gap is still mostly the gap between that shape
and the current host-driven serving path.
