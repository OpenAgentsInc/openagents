# Psionic Metal GPT-OSS Lessons From `mlx-lm`

> Status: working notes, 2026-03-09
>
> Purpose: capture what `mlx-lm` gets right for Apple-local LLM execution and
> identify the parts Psionic should adapt for the native Metal GPT-OSS lane.
>
> Scope note: this document is about execution shape, prompt/cache policy, and
> serving strategy. It is not a claim that Psionic should port itself to MLX or
> copy Python implementation details directly.

## Why This Doc Exists

The native Metal GPT-OSS benchmark in `#3262` now has real same-host receipts,
and they are bad:

- Psionic native Metal: `0.05 tok/s` cold, `0.10 tok/s` warm non-hit,
  `0.15 tok/s` prompt-cache-hit
- same-host `llama.cpp`: `1.90 tok/s` cold, `4.21 tok/s` warm non-hit,
  `3.50 tok/s` prompt-cache-hit

That leaves Psionic at roughly `2.4%` to `4.3%` of the same-host reference on
the tracked Apple machine, while also producing malformed repetitive output on
the benchmark contract.

`llama.cpp` remains the main architecture reference for the long-term Metal
backend shape, and `crates/psionic/docs/METAL_GPT_OSS_UNIFIED_PLAN.md` already
captures that. However, `mlx-lm` is also worth studying because it is another
Apple-local inference stack that gets a number of important serving choices
right:

- explicit prompt/prefill processing
- first-class prompt cache reuse
- model-owned cache policy
- graph-shaped GPT-OSS model code instead of a host-driven token step engine
- batch-aware prompt and decode scheduling

The main question for Psionic is not "should we become MLX?" It is:

> Which `mlx-lm` design choices reinforce the real work Psionic still has to do
> on native Metal?

## Sources Reviewed

### Psionic

- `crates/psionic/docs/METAL_GPT_OSS_UNIFIED_PLAN.md`
- `crates/psionic/psionic-serve/src/gpt_oss.rs`
- GitHub issues `#3262`, `#3269`, `#3285`, and `#3286`

### `mlx-lm`

- `mlx_lm/models/gpt_oss.py`
- `mlx_lm/models/switch_layers.py`
- `mlx_lm/models/cache.py`
- `mlx_lm/generate.py`
- `mlx_lm/server.py`
- `mlx_lm/BENCHMARKS.md`
- `README.md`

## Executive Summary

`mlx-lm` reinforces four broad conclusions:

1. The missing Psionic work is mostly architectural, not one more kernel tweak.
2. Prompt/prefill needs to be treated as a first-class runtime, not as decode
   replay with a different loop condition.
3. GPT-OSS MoE work should stay graph-owned and device-owned for as long as
   possible, rather than bouncing through host vectors and per-expert waits.
4. Prompt-cache reuse should be a serving primitive with exact, shorter-prefix,
   and longer-prefix behavior, not just a narrow exact-hit optimization.

The main value from `mlx-lm` is not "use MLX's compiler." The value is that its
model code, prompt processing, and server cache policy all push in the same
direction:

- keep the hot path in the graph
- treat prefill separately from decode
- make KV reuse explicit and cheap
- batch prompt work when possible

That direction is consistent with Psionic's current open Metal backlog:

- `#3262`: same-host throughput target
- `#3269`: move remaining decode math and MoE control off host
- `#3285`: add a true prompt/prefill graph
- `#3286`: fix real-model native Metal prompt/output correctness

## What `mlx-lm` Gets Right

### 1. GPT-OSS model math lives in the model graph

In `mlx_lm/models/gpt_oss.py`, the GPT-OSS model is expressed as one coherent
model path:

- RMSNorm is part of the block definition
- RoPE is applied inside attention
- KV updates happen through the cache object
- router top-k, expert weighting, and MoE aggregation all live in tensor code

Relevant examples:

- `mlx_lm/models/gpt_oss.py`: `AttentionBlock.__call__`
- `mlx_lm/models/gpt_oss.py`: `MLPBlock.__call__`
- `mlx_lm/models/switch_layers.py`: `SwitchGLU.__call__`

This matters because Psionic's current native Metal path still does too much of
that work as host-side control and host-side vector math inside
`GptOssMetalModelInner::forward_step_with_device_attention_plan(...)`.

Current Psionic Metal hot-path problems include:

- CPU RMSNorm
- CPU RoPE
- CPU router top-k and softmax
- CPU SwiGLU
- CPU expert aggregation
- repeated host staging and readback within a single token

This is exactly the gap tracked by `#3269`.

### 2. Prompt/prefill is explicit and chunked

`mlx_lm.generate.generate_step(...)` does not pretend that prompt ingest and
decode are the same thing. It processes the prompt in chunks:

- `prefill_step_size` controls prompt chunking
- prompt tokens are consumed in blocks until only the final decode boundary
  remains
- the KV cache is updated during prefill
- only then does it switch into token-at-a-time generation

Relevant references:

- `mlx_lm/generate.py`: `generate_step(...)`
- `mlx_lm/generate.py`: prefill loop guarded by `prefill_step_size`

That is materially better than Psionic's current native Metal behavior, where
uncached prompt tokens still go through the decode-step runtime token-by-token.

For Psionic, this directly validates `#3285`. The lesson is not "use the same
Python loop." The lesson is:

- reserve prompt-shaped runtime separately from decode-shaped runtime
- keep prompt KV construction on the device
- amortize prompt work across chunks instead of replaying a decode step for
  every uncached prompt token

### 3. Cache policy is model-owned, not bolted on afterward

`mlx_lm.models.gpt_oss.Model.make_cache()` chooses cache type per layer:

- full-attention layers get `KVCache`
- sliding-attention layers get `RotatingKVCache`

Relevant references:

- `mlx_lm/models/gpt_oss.py`: `Model.make_cache()`
- `mlx_lm/models/cache.py`

That is a useful shape because the model owns its cache semantics directly,
instead of forcing the serving layer to infer all cache policy from the outside.

For Psionic, the lesson is:

- the shared GPT-OSS graph/runtime contract should own cache topology and cache
  compatibility more directly
- prompt/decode runtimes should consume that contract, rather than embedding
  too much per-backend cache policy in the outer serving loop

### 4. Prompt-cache reuse is a real server feature

`mlx_lm.server.LRUPromptCache` supports:

- exact prompt-cache hits
- shorter-prefix reuse
- longer-prefix reuse via trimming
- LRU accounting by bytes and entries

Relevant references:

- `mlx_lm/server.py`: `_search(...)`
- `mlx_lm/server.py`: `fetch_nearest_cache(...)`
- `mlx_lm/server.py`: `insert_cache(...)`

This is a stronger serving story than "cache only exact prompt hits and hope
that is enough."

Psionic already has shared-prefix machinery, but the `mlx-lm` scan sharpens two
lessons:

- prefix reuse should be thought of as a serving strategy, not just a benchmark
  trick
- exact-hit behavior alone is not enough if we want long-context local serving
  to feel fast and stable across real requests

This does not mean Psionic should literally copy the trie structure, but it
does suggest that richer nearest-prefix reuse belongs on the Metal serving
roadmap once the single-request path is honest.

### 5. Batch prefill and batch decode are distinct scheduling problems

The batched path in `mlx_lm/generate.py` and the request scheduler in
`mlx_lm/server.py` explicitly distinguish:

- prompt batching
- prompt-cache extraction and merge
- prompt checkpoints
- decode batching
- prompt cache reinsertion after completion

Relevant references:

- `mlx_lm/generate.py`: `BatchGenerator`
- `mlx_lm/server.py`: request insertion, cache fetch, batch drain, cache
  reinsertion

This matters because it avoids treating online serving as "just run the same
single-request decode loop many times."

Psionic is not ready to copy this immediately into the Metal GPT-OSS lane,
because the single-request path is still not correct or fast enough. But once
`#3269`, `#3285`, and `#3286` are further along, batched prompt/decode
scheduling becomes an obvious next serving-level optimization.

### 6. `mlx-lm` measures prompt and generation separately

`mlx_lm/BENCHMARKS.md` reports separate:

- prompt tok/s
- generation tok/s

That is the right framing for Apple-local inference. It also matches what the
same-host Psionic receipts are already telling us:

- uncached prompt ingest is one major bottleneck
- steady-state decode is another

For Psionic, this means benchmark receipts and performance audits should keep
splitting:

- prompt-side throughput
- decode-side throughput
- prompt-cache-hit behavior

That is more actionable than one aggregate "tok/s" number.

## What Psionic Should Adapt

### A. Build a real Metal prompt/prefill runtime

This is the clearest lesson from `mlx-lm`.

Psionic should adapt:

- explicit prompt runtime reserve separate from decode
- prompt chunking policy
- device-owned prompt KV construction
- prompt runtime reuse evidence in receipts

Psionic should not adapt:

- Python-level loop structure as-is
- MLX-specific stream or array APIs

Mapped issue:

- `#3285`

### B. Move more GPT-OSS block math into backend-owned lowering

`mlx-lm` reinforces that the block should behave like one graph, not like a
host coordinator dispatching many tiny subproblems.

Psionic should adapt:

- backend-owned RMSNorm and RoPE in the hot path
- graph-owned router selection and MoE aggregation semantics
- device-owned expert activation and down projection completion
- fewer hard synchronization points inside a token

Mapped issue:

- `#3269`

### C. Treat prompt-cache reuse as a serving product, not just an optimization

Psionic should adapt:

- exact / shorter / longer prefix thinking
- trimmable cache semantics where safe
- cache accounting in terms of both bytes and request value
- checkpoint-style prompt cache insertion at useful boundaries

This is not yet the first priority, but it is a real medium-term serving
lesson from `mlx-lm`.

Mapped issues:

- partially `#3285`
- partially `#3262`

### D. Split prompt and decode metrics more aggressively

Psionic should adapt:

- explicit prompt-side throughput evidence
- explicit decode-side throughput evidence
- clearer cache-hit / non-hit breakdown

This belongs in benchmarks, audits, and issue updates.

Mapped issue:

- `#3262`

## What Psionic Should Not Copy Blindly

### 1. `mx.compile` is not the lesson

`mlx-lm` uses `mx.compile` selectively, for example around its GPT-OSS SwiGLU
helper. That does not mean Psionic's answer is "find the equivalent Rust macro"
or "wrap every helper in a compiler shim."

The important lesson is:

- the useful work is expressed in a graph-friendly way
- the runtime sees stable shapes and stable cache semantics

If Psionic keeps a host-driven step engine with many intermediate readbacks,
there is no magic compiler wrapper that will make it competitive.

### 2. Do not mistake MLX's dense array graph for a direct backend design

MLX can express the GPT-OSS path in high-level array code because the MLX stack
owns that execution environment end to end.

Psionic still has to solve:

- quantized GGUF-native backend storage
- backend-specific grouped expert dispatch
- runtime reserve and command-buffer policy
- receipt-quality performance evidence across backends

So the right adaptation is conceptual, not literal.

### 3. Do not over-read their benchmark numbers

`mlx-lm` benchmark tables are useful mainly because they separate prompt and
generation performance and demonstrate that Apple-local serving can be fast when
the execution path is shaped correctly.

Psionic should not use those published numbers as a direct acceptance band for
GPT-OSS parity work. The real near-term acceptance target for native Metal is
still the same-host `llama.cpp` comparison in `#3262`.

## Direct Comparison Against Psionic's Current Metal Problems

The `mlx-lm` scan makes Psionic's current Metal problems easier to classify.

### Problem 1: prompt replay instead of prompt runtime

Psionic today:

- uncached prompt ingest still reuses a decode-step runtime token-by-token

`mlx-lm` lesson:

- prompt ingest must be a real prefill phase with chunking and dedicated cache
  update behavior

Primary tracker:

- `#3285`

### Problem 2: host-driven MoE and block math

Psionic today:

- too much RMSNorm, RoPE, router, activation, and aggregation work is still
  driven by the host

`mlx-lm` lesson:

- keep that path graph-owned as long as possible

Primary tracker:

- `#3269`

### Problem 3: correctness and prompt parity are not isolated enough

Psionic today:

- the real-model benchmark shows malformed repetitive output
- the same contract produced different prompt token counts than the
  `llama.cpp` control

`mlx-lm` lesson:

- serving correctness depends on prompt rendering, tokenizer behavior, cache
  boundaries, and runtime parity all lining up

Primary tracker:

- `#3286`

### Problem 4: serving strategy is too narrow

Psionic today:

- current focus is mostly single-request path integrity

`mlx-lm` lesson:

- exact-hit caching alone is not the full local-serving story
- batch prefill/decode scheduling matters
- richer nearest-prefix reuse matters

Primary tracker:

- no dedicated issue yet for batched Metal GPT-OSS serving policy

This should stay secondary until the single-request native Metal path is both
correct and materially faster.

## Issue Mapping

### Already covered well enough by existing issues

- `#3262`
  - same-host throughput target and benchmark receipts
- `#3269`
  - move remaining decode math and MoE control off host
- `#3285`
  - add a true Metal prompt/prefill graph
- `#3286`
  - fix real-model Harmony output correctness and prompt parity

### Not yet tracked as a dedicated issue

One possible future issue is still missing:

- batched native Metal GPT-OSS serving policy
  - request coalescing
  - batch prefill
  - batch decode
  - richer prompt-cache reuse and reinsertion policy

That should probably wait until the single-request path is no longer failing the
basic throughput and correctness bar. Opening it too early risks creating a
serving-level optimization issue before the underlying native path is honest.

## Recommended Psionic Next Steps

In order:

1. Finish the prompt/prefill architecture work in `#3285`.
2. Keep pushing `#3269` until the Metal hot path stops depending on host-owned
   block math and per-expert waits.
3. Use `#3286` to isolate whether the malformed benchmark output is caused by
   prompt serialization mismatch, native Metal numerical drift, or both.
4. Update receipts and audits so prompt throughput and decode throughput are
   reported separately.
5. Only after the single-request path is honest, add a new issue for batched
   Metal GPT-OSS serving strategy and richer nearest-prefix prompt-cache reuse.

## Bottom Line

`mlx-lm` does not change Psionic's long-term Metal reference away from
`llama.cpp`. What it does is strengthen confidence in the current diagnosis.

The same broad themes appear again:

- real prefill matters
- cache policy matters
- graph ownership matters
- serving policy matters

Psionic's biggest native Metal losses are still coming from execution shape, not
from the absence of one last micro-optimization.
