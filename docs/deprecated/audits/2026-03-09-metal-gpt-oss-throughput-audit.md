# 2026-03-09 Metal GPT-OSS Throughput Audit

## Executive Summary

The current Apple Silicon GPT-OSS result is bad because we are measuring two different problems at once:

1. The current `main` benchmark on macOS is not a native Psionic Metal benchmark. It intentionally runs Psionic in a `llama.cpp` proxy mode.
2. The native Psionic Metal path that exists in `crates/psionic/psionic-serve/src/gpt_oss.rs` is still architected as a CPU-owned decode loop that calls tiny Metal kernels, waits after each one, and round-trips intermediate activations through host `Vec<f32>` values.

Those two facts explain almost all of the ugly `~1 tok/s` result.

The proxy path is slow because it is really constrained `llama.cpp` running with Apple-safe settings on a 16 GB M2 Pro:

- `-c 1024`
- `-ngl 4`
- `-b 64`
- `-ub 64`
- `--cpu-moe`

The native Metal path is slow because the implementation is still fundamentally not llama.cpp-shaped:

- no real Metal decode graph
- no device-resident Metal KV cache in the hot path
- CPU RMSNorm / CPU RoPE / CPU attention / CPU router / CPU top-k / CPU softmax / CPU SwiGLU / CPU expert aggregation
- per-op command buffer submission with immediate wait
- repeated buffer allocation, synchronization, and `Vec<f32>` materialization between kernels

If we want real throughput, the answer is not "tune the benchmark script harder." The answer is to finish the Metal runtime architecture so decode stays on device and is replayed as a reusable graph, then measure that natively.

## Fresh Main-Branch Receipt

Command run on `main` on this host:

```bash
./crates/psionic/scripts/benchmark-gpt-oss-vs-llama.sh --json-out /tmp/psionic-metal-audit-bench
```

Host:

- Apple M2 Pro
- model: `/Users/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf`

Observed results:

| Server | Case | Completion Tokens | Seconds | tok/s |
| --- | --- | ---: | ---: | ---: |
| `psionic` | cold | 39 | 46.610 | 0.84 |
| `psionic` | warm non-hit | 39 | 29.174 | 1.34 |
| `psionic` | prompt-cache-hit | 39 | 30.654 | 1.27 |
| `llama.cpp` | cold | 39 | 42.924 | 0.91 |
| `llama.cpp` | warm non-hit | 39 | 28.383 | 1.37 |
| `llama.cpp` | prompt-cache-hit | 39 | 33.171 | 1.18 |

The important truth is that `psionic` and direct `llama.cpp` are basically tied on this host. That is the first sign that the current macOS `psionic` benchmark is mostly benchmarking the same upstream engine with nearly the same low-memory settings.

The `llama.cpp` logs emitted during the run make the hardware bottleneck explicit:

- only `4/25` repeating layers are offloaded to GPU
- `CPU_Mapped model buffer size = 11536.17 MiB`
- `Metal_Mapped model buffer size = 1727.10 MiB`
- warm request prompt processing is about `3.62 tok/s`
- warm request generation is about `1.82 tok/s`

That is already enough to explain why the current benchmark is nowhere near a useful GPT-OSS throughput target.

## Critical Clarification: The Current macOS Benchmark Is A Proxy Benchmark

The current benchmark script explicitly does this on Darwin:

- sets `PSI_BACKEND=metal`
- sets `CTX=1024`
- sets `NGL=4`
- wraps the `psionic` command with:
  - `PSIONIC_METAL_PROXY_LLAMA_CPP=1`
  - `PSIONIC_LLAMA_SERVER_BIN=.../llama-server`
  - `PSIONIC_LLAMA_BATCH_SIZE=64`
  - `PSIONIC_LLAMA_UBATCH_SIZE=64`

That behavior lives in `crates/psionic/scripts/benchmark-gpt-oss-vs-llama.sh`.

Then `crates/psionic/psionic-serve/src/openai_http.rs` turns that env var into a `LlamaCppProxyState` instead of a native `GptOssWorker`. On macOS it also adds `--cpu-moe` by default when it spawns the upstream `llama-server`.

So the current benchmark truth on macOS is:

- `psionic` is currently acting as an OpenAI-compatible wrapper around `llama.cpp`
- the wrapper overhead is relatively small
- the bad throughput number is mostly the throughput of `llama.cpp` under a heavily memory-constrained Apple configuration

That matters because it means the current `~1 tok/s` result is not evidence that the Psionic native Metal kernels are only worth `~1 tok/s`. It is evidence that:

- the current benchmark is not measuring native Psionic Metal
- the Apple fallback configuration is too constrained to be a respectable performance target

## Why The Proxy Path Is So Slow

The proxy path is slow for boring reasons:

1. The machine is memory-constrained for this model.
2. Only `4` repeating layers are offloaded.
3. Most of the model stays CPU-mapped.
4. `--cpu-moe` pushes MoE work off the GPU.
5. The benchmark runs with `ctx=1024`, which is a fit-safe setting, not an aggressive performance-optimized setting.

In other words, the current Apple benchmark is effectively:

- partial Metal offload
- CPU-heavy MoE
- CPU-heavy prompt processing
- modest batch settings chosen to fit and stay stable

That setup is useful for "does it serve at all on this laptop?" but it is not a meaningful proof of Psionic Metal architecture quality.

## Native Metal Root Cause Audit

The native Metal implementation is still slow for structural reasons, not because one more micro-kernel is missing.

### 1. The Metal Runtime Still Has No Real Decode Graph

`psionic-runtime` now defines a reusable GPT-OSS decode graph in `crates/psionic/psionic-runtime/src/gpt_oss.rs`.

The CUDA model uses that graph shape. The Metal model does not.

`GptOssMetalModelInner` in `crates/psionic/psionic-serve/src/gpt_oss.rs` has:

- no `decode_graph`
- no `decode_step_plan`
- no graph reuse contract

Its perf receipts explicitly set:

- `graph_node_count: 0`
- `graph_layer_node_count: 0`

That is the clearest possible signal that the Metal path is not yet executing a reusable GPT-OSS device graph. It is still just a Rust function that happens to call some Metal kernels.

### 2. The Metal Forward Pass Is Still CPU-Owned

`GptOssMetalModelInner::forward_step(...)` is the native hot path. Its shape is:

- CPU token embedding decode
- CPU RMSNorm
- GPU query matvec -> host `Vec<f32>`
- GPU key matvec -> host `Vec<f32>`
- GPU value matvec -> host `Vec<f32>`
- CPU bias application
- CPU RoPE
- CPU copy into host KV buffers
- CPU attention over `InMemoryKvCache`
- GPU attention output matvec -> host `Vec<f32>`
- CPU residual add
- CPU feed-forward RMSNorm
- CPU dense router matvec
- CPU top-k selection
- CPU softmax over selected experts
- GPU grouped gate/up expert matvec -> host nested vectors
- CPU SwiGLU
- GPU down expert matvec per selected expert -> host `Vec<f32>`
- CPU expert aggregation
- CPU final RMSNorm
- GPU output matvec -> host logits

That is not a GPU-first decode step. It is a CPU step that keeps borrowing the GPU for a few projections.

### 3. Attention Is Entirely On CPU

The `attend_impl(...)` helper in `crates/psionic/psionic-serve/src/gpt_oss.rs` is a pure host implementation:

- iterates heads on CPU
- iterates cached entries on CPU
- computes dot products on CPU
- computes `exp` / normalization on CPU
- accumulates output with CPU `axpy`

This means the native Metal path currently pays GPU launch and synchronization overhead for Q, K, V, then does the actual decode attention math on the CPU anyway.

That is catastrophic for throughput on a decode-heavy workload.

### 4. The KV Cache Is Host-Resident In The Native Metal Path

The native Metal forward step reads from `&super::InMemoryKvCache`.

It also constructs:

- `cache_key: Vec<f32>`
- `cache_value: Vec<f32>`

and copies each layer's K/V slices into those host buffers.

There is no Metal equivalent today of the CUDA-side mirror/shared-prefix flow. The hot path is still logically built around host KV ownership and host cache traversal.

That means:

- attention reads happen from host memory
- any future device attention will still be blocked until we add a true `MetalKvCacheMirror` or equivalent
- shared-prefix residency is not yet a real native Metal fast path

### 5. The Router And MoE Control Logic Are Still On CPU

The router path still does this on CPU:

- dense router `matvec`
- `top_k_indices(...)`
- `softmax_selected(...)`

Then the selected expert flow still returns host-owned nested vectors before the CPU applies:

- bias
- `oai_swiglu(...)`
- route-weighted aggregation

For GPT-OSS, MoE is not optional side work. It is central decode work. Leaving router and expert activation/aggregation on CPU guarantees that Metal throughput stays bad even if the projection kernels themselves improve.

### 6. The Metal Backend Submission Model Forces Tiny Synchronous Transactions

The strongest backend-level evidence is in `crates/psionic/psionic-backend-metal/src/lib.rs`.

`run_quantized_matvec(...)` currently:

- allocates a fresh input buffer
- writes the input
- allocates a fresh output buffer
- begins a submission
- encodes one quantized matvec
- synchronizes the output
- commits with `MetalCommandWait::Completed`
- reads `f32` values back to host

`run_grouped_quantized_matvec(...)` does the same pattern:

- fresh output buffer
- one submission
- synchronize output
- `commit(MetalCommandWait::Completed)`
- read host `Vec<f32>`

This is the opposite of a high-throughput decode architecture. It means:

- one tiny command buffer per logical op
- explicit wait after each op
- mandatory host readback between ops
- no steady-state scratch reuse
- no opportunity for device-side chaining across the layer

Even if the individual Metal kernels are good, this submission model will murder throughput.

### 7. We Still Allocate And Copy Too Much Inside The Inner Loop

Examples in the current native path:

- `hidden.clone()`
- `Vec::new()` for every projection output
- `input.to_vec()` in `MetalQuantizedExpertProjectionGroup::selected_matvec(...)`
- repeated `backend.input_buffer(...)`
- repeated `output.read_f32()`

These are not the biggest problem individually, but together they confirm the architecture is still prototype-grade:

- host temporaries everywhere
- buffer lifetime tied to single ops
- no scratch arena reused across the decode step

### 8. The Native Metal Path Is Missing Backend-Side Sampling And Output Reduction

The current Metal path still returns full logits to host and samples on CPU. CUDA already has more mature output handling.

This is not the biggest current bottleneck, but it still matters once the bigger graph issues are fixed. It also shows that Metal is not yet feature-parity with the faster backend architecture.

## Root Cause Table

| Problem | Evidence In Repo | Why It Hurts | Severity |
| --- | --- | --- | --- |
| macOS benchmark silently proxies to `llama.cpp` | `benchmark-gpt-oss-vs-llama.sh`, `openai_http.rs` | current `psionic` number is not native Metal truth | Critical |
| Apple fallback runs with `-ngl 4` and `--cpu-moe` | benchmark script + proxy launcher | most work stays on CPU | Critical |
| no Metal decode graph | `GptOssMetalModelInner` has no graph/plan and reports zero graph nodes | no reusable steady-state execution contract | Critical |
| CPU attention | `attend_impl(...)` is host-only | decode attention dominates long-running requests | Critical |
| host KV cache | native Metal uses `InMemoryKvCache` directly | blocks device-side decode attention and reuse | Critical |
| CPU router / top-k / softmax / SwiGLU / aggregation | native Metal `forward_step(...)` | MoE path stays host-bound | Critical |
| per-op synchronous command buffers | `run_quantized_matvec`, `run_grouped_quantized_matvec` | command overhead and host waits dominate | Critical |
| repeated allocations and host `Vec` materialization | multiple helpers in `gpt_oss.rs` and backend | turns decode into alloc/copy churn | High |
| no backend-side sampling/output reductions | Metal returns host logits | extra sync and readback at end of step | Medium |

## What Will Actually Make It Faster

### P0: Split Proxy Benchmarking From Native Benchmarking

The benchmark harness should stop presenting the proxied Apple path as the default proof of Metal throughput.

Required changes:

- add explicit benchmark modes:
  - `--psionic-metal-mode proxy`
  - `--psionic-metal-mode native`
- print the mode in the benchmark receipt
- fail loudly if the user thinks they ran native but the script enabled proxy mode

Why this is first:

- right now we are mixing "serves on this laptop" and "native backend throughput"
- that makes performance work harder because the benchmark number is ambiguous

### P0: Build A Real Metal Decode Step Graph

The Metal path should adopt the same high-level graph contract already defined in `psionic-runtime`.

Required changes:

- add `decode_graph` and `decode_step_plan` ownership to `GptOssMetalModelInner`
- make graph identity and validity inputs explicit
- reserve all device buffers needed for one decode step
- encode the full decode step as a stable replayable graph or graph-like submission plan

Acceptance criteria:

- non-zero graph node counts in Metal perf receipts
- no per-token re-construction of the logical op sequence
- observable graph reserve/reuse hits in logs or perf metrics

### P0: Add A Device-Resident Metal KV Cache

The native Metal path needs a real device KV mirror, not host-only `InMemoryKvCache` traversal.

Required changes:

- introduce `MetalKvCacheMirror`
- keep K/V in device buffers during prompt and decode
- make shared-prefix storage able to preserve the device-resident form
- only materialize host KV when required for session persistence or debugging

Acceptance criteria:

- decode attention reads from Metal buffers, not host slices
- no per-token host copy of full K/V payloads in the hot path

### P0: Move Decode Attention To Metal

`attend_impl(...)` must stop being the native runtime implementation for Metal.

Required changes:

- implement native Metal decode attention over the device KV cache
- add a flash-attention style path when geometry allows
- keep attention output on device for the next projection

Acceptance criteria:

- no CPU head loop in the native Metal hot path
- no host logits/weights vector built inside attention

### P0: Keep The Entire MoE Subgraph On Device

For GPT-OSS, the MoE lane must stay on Metal:

- router projection
- top-k selection
- route normalization
- grouped gate/up projection
- SwiGLU
- down projection
- route-weighted accumulation

Required changes:

- move router dense matvec off CPU
- add Metal top-k and selected softmax
- keep expert outputs in device scratch buffers
- fuse or at least chain gate/up/activation/down without returning nested host vectors

Acceptance criteria:

- no CPU `top_k_indices`, `softmax_selected`, or `oai_swiglu` in the Metal decode loop

### P1: Stop Waiting After Every Tiny Metal Submission

Current `run_quantized_matvec(...)` and `run_grouped_quantized_matvec(...)` are too synchronous.

Required changes:

- reuse scratch buffers across the full step
- encode many ops into one command buffer per layer or per step
- only synchronize at real graph boundaries
- stop calling `read_f32()` between internal ops

Acceptance criteria:

- dramatic drop in command buffer count per generated token
- dramatic drop in host wait time

### P1: Remove Host `Vec<f32>` As The Internal Data Model

The native Metal inner loop should not model every intermediate as a host `Vec<f32>`.

Required changes:

- introduce device-native intermediate tensors for:
  - normalized hidden state
  - q/k/v
  - attention output
  - router logits
  - selected expert activations
  - final logits or reduced token outputs
- use host reads only for external API boundaries

Acceptance criteria:

- internal step code passes Metal buffers between phases instead of vectors

### P1: Bring Metal To Output/Sampling Parity

Once the step graph is mostly device-resident:

- add backend-side argmax for greedy decode
- add backend-side top-k/top-p if we care about non-greedy speed
- bound readback to the selected token or compact output state

This is not the first fix, but it is required if we want the Metal path to stop lagging CUDA architecture quality.

## Immediate Tuning Ideas For The Proxy Path

These are not the long-term fix, but they are the only honest way to move the current Apple proxy benchmark upward before native Metal is ready:

1. Sweep `-ngl` upward until the model actually hits the memory wall.
2. Re-test whether `--cpu-moe` is still required on this exact host and model.
3. Sweep `-b` / `-ub` instead of freezing at `64`.
4. Separate "fit-safe laptop mode" from "throughput mode" in the script.

These changes may improve the current benchmark number, but they do not improve Psionic Metal architecture. They only improve the fallback wrapper path.

## What Not To Waste Time On

These are low-yield until the architecture changes above land:

- tweaking JSON response formatting
- micro-optimizing host-side `Vec` math while attention and MoE stay on CPU
- adding more standalone quantized projection kernels without changing the command-buffer model
- celebrating proxy parity with `llama.cpp` under the same constrained Apple settings

Those may move the number a little. They will not turn `~1 tok/s` into a respectable Metal GPT-OSS implementation.

## Recommended Execution Order

### Phase 1: Make The Benchmark Honest

- split proxy/native modes in the benchmark script
- add a native Metal benchmark mode that never enables `PSIONIC_METAL_PROXY_LLAMA_CPP`
- record startup mode explicitly in the receipt

### Phase 2: Instrument Native Metal Properly

- add perf counters for:
  - command buffers per token
  - host waits per token
  - host bytes read/written per token
  - graph reserve/reuse hits
  - per-stage timings for attention, router, MoE, output

### Phase 3: Finish The Metal Runtime Architecture

- device KV cache
- device attention
- device MoE control path
- reusable decode graph
- buffer reuse and no per-op waits

### Phase 4: Re-Benchmark Native Metal

Only after Phase 3 should we treat Apple tok/s as the scoreboard for Psionic Metal itself.

## Bottom Line

The current `~1 tok/s` result is bad, but it is bad for a very specific and fixable reason:

- today’s macOS benchmark is largely measuring a constrained `llama.cpp` fallback
- today’s native Metal path is still architected like a CPU implementation with a few Metal projections bolted on

The path to real speed is straightforward even if it is a lot of work:

- make the benchmark honest
- make KV/attention/MoE device-resident
- make the decode step a reusable Metal graph
- stop synchronizing and reading back after every tiny op

Until those land, the number will stay bad no matter how many isolated kernels we add.
