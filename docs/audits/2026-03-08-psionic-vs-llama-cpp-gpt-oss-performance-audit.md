# 2026-03-08 Psionic vs llama.cpp GPT-OSS Performance Audit

> Updated 2026-03-09 after re-running the live repo benchmark script against
> the current Psionic worktree on the local RTX 4080 host, after landing the
> CUDA-side shared-prefix residency follow-up, after trimming the default
> OpenAI-compatible hot path so Harmony debug fields are opt-in instead of
> always serialized, after adding prompt-token reuse on the HTTP lane plus
> safe cross-request CUDA decode-graph reuse keyed to the actual KV allocation
> identities, after the newer exact-prompt shared-prefix fast paths that stop
> cloning full prompt-logit histories and avoid re-recording or host-cloning
> unchanged prompt caches on repeated GPT-OSS HTTP requests, and after the
> latest decode-kernel checkpoint that confirmed two plausible llama.cpp-aligned
> ideas did not improve this workload: q8_0 `f16` projection mirrors feeding
> cuBLAS tensor-op GEMV regressed into the mid-80s tok/s, and replacing the
> GPT-OSS `selected_count = 4` custom MoE kernels with simpler per-expert MMVQ
> routing also stayed below the best prior checkpoint. This file is the current
> audit for the GPT-OSS throughput gap; later product truth still lives in
> `docs/MVP.md`, `docs/OWNERSHIP.md`, `crates/psionic/docs/ROADMAP.md`, and the
> referenced issues. The current benchmark script now explicitly unsets
> `PSIONIC_OPENAI_INCLUDE_DEBUG_FIELDS` before launching Psionic so perf
> receipts and extra JSON serialization cannot silently contaminate the
> benchmark, and this update also records one more ruled-out direct port:
> a grouped-query decode-attention kernel specialized for the exact GPT-OSS
> geometry on this host (`64` query heads, `8` KV heads, `64` head dim)
> regressed the live HTTP benchmark into the low `70 tok/s` range and was
> removed.

## Scope

- Psionic GPT-OSS path:
  - `crates/psionic/psionic-serve/src/gpt_oss.rs`
  - `crates/psionic/psionic-serve/src/lib.rs`
  - `crates/psionic/psionic-backend-cuda/src/lib.rs`
  - `crates/psionic/psionic-backend-cuda/src/kernels/quantized_matvec.cu`
  - `crates/psionic/psionic-backend-cuda/build.rs`
  - `crates/psionic/psionic-models/src/harmony.rs`
- `llama.cpp` reference path:
  - `~/code/llama.cpp/src/models/openai-moe-iswa.cpp`
  - `~/code/llama.cpp/src/llama-graph.cpp`
  - `~/code/llama.cpp/src/llama-context.cpp`
  - `~/code/llama.cpp/ggml/src/ggml-cuda/mmvq.cu`
  - `~/code/llama.cpp/ggml/src/ggml-cuda/vecdotq.cuh`
  - `~/code/llama.cpp/ggml/src/ggml-cuda/fattn.cu`
  - `~/code/llama.cpp/ggml/src/ggml-cuda/mmid.cu`
- Harmony reference:
  - `~/code/harmony/src/chat.rs`
  - `~/code/harmony/src/encoding.rs`

## Benchmark Contract

- Model:
  - `/home/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf`
- Host:
  - local NVIDIA host with `NVIDIA GeForce RTX 4080`
- API flow:
  - `POST /v1/chat/completions`
  - `127.0.0.1:8099`
  - `-c 4096`
  - `-ngl 999`
  - warm once, time the second request
- Deterministic prompt:
  - system and developer messages match the local benchmark script
  - user asks for exactly one sentence:
    `HTTPS protects users by encrypting traffic, preventing tampering, and confirming they are connected to the right website.`

## Measured History

### Original baseline

- Psionic:
  - `37` completion tokens in `2.211s`
  - `16.74 tok/s`
- `llama.cpp`:
  - `42` completion tokens in `0.245s`
  - `171.22 tok/s`
- Gap:
  - `10.23x`

### Current checkpoint

- Psionic:
  - `37` completion tokens in `1.049s`
  - `35.26 tok/s`
- `llama.cpp`:
  - `42` completion tokens in `0.251s`
  - `167.27 tok/s`
- Gap:
  - `4.74x`
- Psionic improvement over original baseline:
  - `2.11x`

### Direct-alignment checkpoint

- Psionic:
  - `37` completion tokens in `1.028s`
  - `35.99 tok/s`
- `llama.cpp`:
  - `42` completion tokens in `0.252s`
  - `166.74 tok/s`
- Gap:
  - `4.63x`
- Psionic improvement over the prior checkpoint:
  - `1.02x`

### Current stable checkpoint

- Psionic:
  - `37` completion tokens in `0.539s`
  - `68.65 tok/s`
- `llama.cpp`:
  - `42` completion tokens in `0.251s`
  - `167.11 tok/s`
- Gap:
  - `2.43x`
- Psionic improvement over the original baseline:
  - `4.10x`
- Psionic improvement over the last audited checkpoint:
  - `1.91x`

### Current hot-path checkpoint

- Psionic:
  - `37` completion tokens in `0.422s`
  - `87.59 tok/s`
- Repeated timed-request range on the same loaded server:
  - `87.53 tok/s`
  - `87.59 tok/s`
  - `87.21 tok/s`
- Comparison status:
  - the clean same-moment `llama.cpp` control could not be re-established on
    this machine because an external `dota2` process was already holding about
    `2.1 GiB` of RTX 4080 memory
  - under that contention, a fresh `llama.cpp` load offloaded `0` repeating
    layers to CUDA and fell to about `16.4 tok/s`, which is not a valid parity
    control and must not be compared against the Psionic run
  - the last clean same-machine `llama.cpp` control remains the stable
    checkpoint above at about `167 tok/s`
- Psionic improvement over the original baseline:
  - `5.20x`
- Psionic improvement over the previous stable checkpoint:
  - `1.28x`

### Current exact-prompt cache checkpoint

- Psionic:
  - repeated timed-request range on the same loaded server:
    - `89.34 tok/s`
    - `91.69 tok/s`
    - `91.49 tok/s`
  - best observed timed request:
    - `37` completion tokens in `0.402s`
    - `92.02 tok/s`
- Comparison status:
  - the last clean same-machine `llama.cpp` control still remains about
    `167 tok/s`
  - the local RTX 4080 host still had an external `dota2` process resident
    during these re-runs, so the new Psionic number should be treated as a
    real product-path improvement but not as a fresh clean parity control
- Psionic improvement over the original baseline:
  - `5.50x`
- Psionic improvement over the previous hot-path checkpoint:
  - `1.05x`

### Current ruled-out-experiment checkpoint

- Psionic:
  - `37` completion tokens in `0.401s`
  - `92.32 tok/s`
- `llama.cpp`:
  - `42` completion tokens in `0.252s`
  - `166.46 tok/s`
- Gap:
  - `1.80x`
- Psionic improvement over the original baseline:
  - `5.52x`
- Psionic improvement over the previous exact-prompt cache checkpoint:
  - effectively flat; `92.32 tok/s` is within noise of the prior `92.02 tok/s`
- Ruled-out changes from this checkpoint:
  - enabling q8_0 `f16` transpose mirrors for GPT-OSS attention projections and
    routing them through cuBLAS `GemmEx` regressed the exact same benchmark into
    the `83-84 tok/s` range on this host
  - replacing the GPT-OSS `selected_count = 4` custom MoE kernels with the
    simpler direct per-expert MMVQ/atomic route also failed to improve the
    benchmark and stayed around `90.74 tok/s`
- Host-state note:
  - the external `dota2` process still holds about `2.1 GiB` of RTX 4080 VRAM
  - even the local `llama.cpp` control therefore remains below the requested
    `190 tok/s` target on this machine state, so Psionic cannot honestly prove
    `>190 tok/s` here until that competing workload is cleared

### Current clean benchmark-hygiene checkpoint

- Psionic:
  - `37` completion tokens in `0.415s`
  - `89.16 tok/s`
- `llama.cpp`:
  - `42` completion tokens in `0.250s`
  - `167.98 tok/s`
- Gap:
  - `1.88x`
- Psionic improvement over the original baseline:
  - `5.33x`
- New benchmark-hygiene truth:
  - the repo benchmark script now launches Psionic with
    `PSIONIC_OPENAI_INCLUDE_DEBUG_FIELDS` explicitly unset so request-debug
    receipts cannot distort perf numbers
- Newly ruled-out direct port:
  - a grouped-query attention kernel specialized for the real GPT-OSS decode
    geometry (`n_head = 64`, `n_head_kv = 8`, `head_dim = 64`) was correct in
    CUDA unit coverage but regressed the exact HTTP benchmark to about
    `73.32 tok/s`, so it was removed rather than left in the runtime hot path

The visible output text matched exactly in the current benchmark:

`HTTPS protects users by encrypting traffic, preventing tampering, and confirming they are connected to the right website.`

## New Findings From The Latest Iteration

- CUDA decode-graph replay is still useful plumbing, but it is no longer the
  dominant limiter on this exact prompt.
  - disabling the graph fast path on warm repeated requests left per-token wall
    time almost unchanged while keeping one CUDA submission and one sync per
    token
- the existing shared-input multi-row q8_1 matvec kernels were not helping the
  GPT-OSS decode lane on this host.
  - the current faster checkpoint keeps the regular single-row MMVQ-style
    kernel for both `Q8_0 x Q8_1` and `MXFP4 x Q8_1` decode projections
- not every llama.cpp-looking shortcut is an actual win for this workload.
  - on this RTX 4080 + `gpt-oss-20b-mxfp4.gguf` combination, the q8_0
    projection `f16` mirror path and the direct per-expert MoE route were both
    plausible from code inspection and both lost to the current path in live
    benchmark runs
- the remaining gap is now concentrated even more tightly in the kernels that
  Psionic still does not match line-for-line with llama.cpp.
  - the next real alignment targets are the ids-enabled `mul_mat_vec_q` / MMVQ
    path for the GPT-OSS MoE decode lane, the final-logits greedy path
    (preferably avoiding a full logits materialization when only argmax is
    needed), and a deeper audit of whether llama.cpp's reported `REPACK = 1`
    state implies a useful backend-side weight layout transform for this exact
    model family on CUDA
- the latest ruled-out branch sharpened that further.
  - even for the exact GPT-OSS grouped-query geometry (`64/8/64`), simply
    reusing K/V across eight query-head warps in a larger shared-memory block
    did not help on this RTX 4080 prompt shape; the occupancy/shared-memory
    tradeoff lost to the prior smaller kernel

## What Landed Between The Two Measurements

The performance gap is smaller than it was, and the reason is concrete.

The current Psionic checkpoint added:

- benchmark JSON summaries and request-level perf receipts
- device-resident CUDA step-plan buffers in `psionic-serve`
- CUDA KV-cache mirrors, CUDA RMSNorm, CUDA RoPE, and CUDA decode attention
- CUDA router top-k softmax and CUDA MoE projection / aggregation surfaces
- a single CUDA submission per generated token instead of one submission per
  layer plus a separate logits submission
- `Q8_1` scratch quantization plus `Q8_0 x Q8_1` fast-path kernels
- a direct MMVQ-style warp-scheduled matvec port for the `Q8_0` and `MXFP4`
  logits/projection path
- a CUDA argmax fast path for the greedy decode lane
- removal of two explicit residual-buffer copies from the steady-state decode
  token path
- an explicit high-level GPT-OSS decode-graph representation in Psionic, with a
  graph-derived CUDA step-plan digest and surfaced decode-graph node counts
- CUDA build tuning in `psionic-backend-cuda/build.rs`:
  - `-O3`
  - `--use_fast_math`
  - architecture detection from `CUDAARCHS`, `PSI_CUDA_ARCH`, or `nvidia-smi`

Three correctness fixes were also necessary before the faster path was usable:

- GGML `Q8_1` block size had to be corrected to `36` bytes, not `34`
- `Q8_1` scratch sizing had to be widened to the maximum actual projection
  width, not just hidden size
- CUDA argmax shared scratch had to be sized for the maximum possible warp count
  per block, not only the original narrow-row case

That work is why Psionic moved from `16.74 tok/s` to `35.26 tok/s` without changing
the benchmark contract or delegating execution to `llama.cpp`.

The direct-alignment checkpoint then added the first explicit ports of
`llama.cpp` CUDA fusion ideas:

- a fused RoPE + KV-write + decode-attention kernel for the GPT-OSS single-token
  path, mirroring the role of `ggml_cuda_should_fuse_rope_set_rows(...)` and the
  backend-owned attention/KV write path in `ggml-cuda.cu`
- a fused residual-add + post-attention RMSNorm kernel that preserves the
  intermediate `ffn_inp` surface instead of only normalizing it
- an `f16` device KV mirror for the Psionic CUDA GPT-OSS path, closer to the
  `llama.cpp` KV cache representation
- backend CUDA tests that check the fused attention path against the separate
  Psionic RoPE + attention + cache-write path before using it in the live model

That work materially reduced kernel count but did not materially change
end-to-end throughput. That is important evidence, not a failure to record.

The newer stable checkpoint on `main` added the changes that actually moved the
live number from the mid-30s into the high-60s:

- fixed `MXFP4` decode/execution correctness in both CPU and CUDA paths
  - the `E8M0` scale decode now matches `llama.cpp`
  - the missing `* 0.5f` factor in the `MXFP4 x Q8_1` dot path is restored
- switched the live GPT-OSS CUDA decode lane to use the `Q8_1` fast path for
  all eligible `Q8_0` and `MXFP4` projections
  - QKV
  - attention output
  - MoE gate/up
  - MoE down
  - final output
- added a real CUDA graph replay path for greedy decode and then fixed the
  request-bound graph lifetime bug
  - the first graph replay attempt reused captures across request-local KV
    allocations and caused illegal memory access on the second HTTP request
  - the current code resets the cached graph exec when the reusable decode-step
    plan is pulled from the per-model cache, so each request recaptures against
    its own KV allocations and then reuses that capture for the rest of the
    decode lane
- re-ran the benchmark through
  `crates/psionic/scripts/benchmark-gpt-oss-vs-llama.sh` to confirm the stable
  HTTP result instead of relying on intermediate local probes
- added CUDA-side shared-prefix residency for the GPT-OSS request path
  - Psionic now records and reuses the prompt-only `CudaKvCacheMirror` for
    compatible prompt prefixes instead of always rebuilding the device mirror
    from host-owned cache state
  - this closes one concrete behavioral gap with `llama.cpp`'s prompt-cache
    residency, but only moved the exact HTTP benchmark marginally on this host

Two later experiments were useful but did not move the real benchmark enough to
keep them as the main story:

- a more `llama.cpp`-shaped grouped-selected MMVQ rewrite for the MoE kernels
  was benchmark-neutral on this host/model
- an `f16` dense-transposed mirror for the final vocabulary projection slowed
  the real HTTP benchmark and was backed back out

The current hot-path checkpoint then moved the real HTTP benchmark from the
high-60s into the mid/high-80s without changing the model math:

- the default OpenAI-compatible response path no longer serializes
  `psionic_metrics`, `psionic_perf`, or parsed Harmony structure unless
  `PSIONIC_OPENAI_INCLUDE_DEBUG_FIELDS=1`
- the default visible content lane now uses a small GPT-OSS final-message
  extractor instead of running the full Harmony parse on every timed request
- the full Harmony parse still remains available and is still the source of
  truth when debug fields are explicitly requested
- regression tests now pin the fast extractor against the same final-channel
  semantics as the full parser so the hot path does not regress visible output
- the HTTP server now also caches the tokenized rendered GPT-OSS prompt for
  repeated identical requests
  - this is a real cleanup and keeps repeated requests from re-running Harmony
    tokenization, but it did not materially move the exact benchmark on this
    workload
- the CUDA decode graph is now reused across requests when the shared prompt KV
  mirror resolves to the same underlying device allocations
  - this is the correct long-term behavior and closes one more concrete gap with
  `llama.cpp`'s graph/cache residency model
  - on this exact benchmark it only moved the steady-state result marginally,
  from the mid-86s to about `87.6 tok/s`

The latest exact-prompt cache checkpoint then tightened the repeated-request
path further without changing the decode math:

- exact repeated-prompt shared-prefix hits no longer clone the full
  per-prompt logit history on the request path
  - the shared-prefix entry now carries the final prompt logits separately so
    the repeated-request path can seed sampling from one logits vector instead
    of cloning the whole prompt-logit ladder
- exact repeated-prompt hits no longer re-record unchanged prompt caches
  - Psionic now skips the host prompt-cache clone and CUDA prompt-cache clone
    when the reused prompt already matches the full incoming prompt
- the GPT-OSS path now has an exact-prompt shared-prefix fast path that can
  skip the host KV-cache clone entirely for sessionless repeated requests
  - token history is rebuilt directly from the prompt tokens on that path,
    while the device-side prompt KV mirror still comes from the CUDA shared
    prefix store

That work moved the real repeated HTTP benchmark from about `87.6 tok/s` to
about `92 tok/s`, but the decode-step wall only moved marginally. The best
debug receipt after these changes still showed roughly:

- `322 ms` of summed decode-step wall time for `37` generated tokens
- `13,468` CUDA kernel launches across those `37` decode steps

That result matters because it rules out "the remaining gap is mainly prompt
cache churn" as the top diagnosis. The remaining gap is still in the
device-side decode path.

The latest exact-flow comparison first exposed, and the new checkpoint only
partially closed, a concrete request-to-request gap that is not just "CUDA
kernels are slower":

- `llama.cpp` serves the timed request from a live prompt cache
  - warmup request:
    - `prompt eval time = 80.41 ms / 160 tokens`
    - `eval time = 266.75 ms / 42 tokens`
  - timed request:
    - `prompt eval time = 0.30 ms / 1 token`
    - `eval time = 235.74 ms / 42 tokens`
- the current Psionic path now reuses a recorded `CudaKvCacheMirror` for
  compatible shared prefixes
  - that closes the most obvious request-local device-mirror rebuild
  - the modest benchmark change is strong evidence that backend-resident prompt
    reuse was necessary but not the dominant remaining bottleneck

That prompt-cache difference does not explain the whole remaining gap by
itself, but it is now proven to be part of the measured benchmark delta.

## Current Hard Evidence From The Psionic Benchmark

The latest Psionic benchmark receipt for the timed request reports:

- `step_count = 37`
- `layer_visit_count = 888`
- `graph_node_count = 266`
- `graph_layer_node_count = 11`
- `host_to_device_bytes = 426536`
- `device_to_host_bytes = 148`
- `submission_count = 37`
- `sync_count = 37`
- `kernel_launches = 13468`

Important interpretation:

- the execution-boundary work helped a lot
  - decode is down to one submission and one sync per generated token
- host-to-device traffic is no longer the main problem
  - about `11.5 KB` per token in the timed lane
- full-logits readback is no longer the dominant problem either
  - the greedy lane now reads back only one token ID per step
- kernel launch count is still very high
  - about `364` launched operations per generated token
- one submission and one sync per generated token is no longer the limiting
  story by itself
  - that boundary cleanup is already in place, yet Psionic is still `2.47x`
    slower than `llama.cpp`
- the largest remaining gap is now clearly inside the kernel family and graph
  scheduler rather than in host/device readback
  - the timed lane reads back only one token id per step
  - the timed lane uploads only about `11.5 KB` per token
- the OpenAI/Harmony compatibility surface was also still paying more than it
  needed to on the steady-state benchmark path
  - always serializing debug receipts and always running the full Harmony parse
    cost enough wall time to move the exact benchmark by about `18 tok/s`
  - that is now fixed on the default path, but it is not the remaining `150+`
    tok/s blocker
- request-local graph recapture is no longer the main remaining issue either
  - once graph reuse was tied to the actual shared CUDA KV allocation identity,
    the exact benchmark only improved marginally
  - the remaining gap is still dominated by the core decode kernels, especially
    the MoE and attention execution families, not by per-request HTTP setup
- `llama.cpp` still has a request-to-request advantage before decode even starts
  - its timed request re-evaluates only one prompt token because prompt cache
    state stays live in the backend
  - Psionic still reconstructs CUDA KV state from host-owned prefix entries, so
    prompt reuse is truthful but not yet backend-resident
- the direct `llama.cpp` alignment work that did matter was the correctness +
  fast-path routing work
  - once the `MXFP4` math bug was fixed and the `Q8_1` fast path was used
    across the actual GPT-OSS projection set, Psionic moved from the mid-30s
    to the high-60s
- the next parity work has to target `llama.cpp`'s bigger wins directly
  - backend-resident prompt-cache reuse
  - graph scheduler/update behavior
  - grouped `mul_mat_id`
  - flash attention
  - exact CUDA kernel family / dispatch policy ports for GPT-OSS

Timing caveat:

- the current stage timing fields are trustworthy for broad direction but not
  for precise per-stage attribution after the single-submission rewrite
- the byte counters, submission count, sync count, kernel-launch count, and
  end-to-end benchmark numbers are the solid evidence to use for optimization
  decisions right now

## What `llama.cpp` Actually Does For GPT-OSS

The most important thing about `llama.cpp` is not one individual CUDA kernel.
It is the way the GPT-OSS path is represented and scheduled end to end.

### 1. The GPT-OSS model path is built as one explicit graph

`~/code/llama.cpp/src/models/openai-moe-iswa.cpp` builds the model in the exact
decode order that matters for throughput:

- input embedding
- RMSNorm
- Q / K / V projection
- RoPE on Q and K
- attention through `build_attn(...)`
- residual add
- post-attention RMSNorm
- MoE through `build_moe_ffn(...)`
- residual add
- output RMSNorm
- final lm-head projection

That graph is not just descriptive. It is the input to the scheduler and the
backend fusion logic.

### 2. Attention and MoE are graph-level constructs, not ad-hoc side paths

`~/code/llama.cpp/src/llama-graph.cpp` does three throughput-critical things
that Psionic still does only partially:

- `build_attn(...)` routes eligible shapes through `ggml_flash_attn_ext(...)`
  instead of keeping attention as a simple hand-authored decode kernel
- `build_moe_ffn(...)` builds the full gating path, top-k expert selection,
  expert weights, grouped expert projections, OAI SWIGLU, and expert
  aggregation as graph nodes that the CUDA backend can reason about
- the grouped-expert path uses `ggml_mul_mat_id(...)` and expert views so the
  backend sees the routed-expert structure directly instead of reconstructing it
  from Rust-side imperative sequencing

### 3. The context and scheduler are designed around graph reserve, reuse, and capture

`~/code/llama.cpp/src/llama-context.cpp` reserves graphs, computes them through
`ggml_backend_sched_graph_compute_async(...)`, reports graph-node and graph-split
counts, and maintains reuse-oriented graph/result state. On this host the live
server reported:

- `graph nodes = 1352`
- `graph splits = 2`
- `USE_GRAPHS = 1`

That is materially different from Psionic's current "encode every token's ops
from Rust into one submission" design.

### 4. CUDA fusion and dispatch are driven by the graph, not by isolated call sites

`~/code/llama.cpp/ggml/src/ggml-cuda/ggml-cuda.cu` is where the bigger
throughput story lives:

- `ggml_cuda_should_fuse_mul_mat_vec_q(...)` decides when MMVQ is the right
  quantized path
- `ggml_cuda_mul_mat_id(...)` chooses grouped-expert fast paths and only falls
  back when the graph shape or hardware disqualifies them
- the backend fuses top-k MoE selection, `mul_mat(+id)` plus bias plus GLU,
  RMSNorm patterns, and RoPE-plus-KV write patterns when the graph layout
  permits it
- CUDA graph execution and concurrent-event scheduling are part of the backend
  machinery, not a product-layer afterthought

That is the architecture Psionic still lacks.

## Exact Reason Psionic Is Still Slower

This is no longer a generic "CPU vs GPU" story. The current gap is now mostly an
execution-architecture story.

### 1. Psionic still encodes decode as Rust-owned imperative work, not as a true backend graph

Psionic now has a reusable CUDA step plan and only one submission per token.
That part is real progress.

But the hot path in `crates/psionic/psionic-serve/src/gpt_oss.rs` still manually
walks every layer and explicitly emits the sequence of kernels from Rust each
token. `llama.cpp` instead builds the OpenAI-MoE path as a graph and lets the
backend decide:

- which nodes can fuse
- which kernels to dispatch
- when CUDA graphs are reusable
- how concurrent regions are scheduled

One submission per token is not the same thing as `llama.cpp`'s graph-driven
decode architecture.

### 2. The current MMVQ port is only a slice of the real `llama.cpp` kernel story

The recent Psionic MMVQ-style port was correct and worthwhile, but it is still
only a partial analogue of `mmvq.cu` plus `vecdotq.cuh`.

What `llama.cpp` has that Psionic still does not:

- full dispatch policy around MMVQ vs MMQ, not just one replacement kernel
- fusion-aware matvec use inside bias and GLU subgraphs
- grouped-expert `mul_mat_id` integration
- broader architecture-aware parameter selection

Psionic improved the row kernel. `llama.cpp` improves the whole graph around that
kernel.

### 3. Psionic does not yet mirror `llama.cpp`'s grouped-expert execution model

`llama.cpp`'s `build_moe_ffn(...)` plus `ggml_mul_mat_id(...)` and
`ggml-cuda/mmid.cu` compact work by expert and keep the grouped-expert structure
visible to the backend.

Psionic's current MoE path is still more imperative and less compact:

- routing and execution are CUDA-backed, but not yet driven by the same grouped
  expert graph shape
- the real-model `MXFP4` fast path is still not trustworthy enough to be the
  universal live path
- the remaining kernel-launch count strongly suggests too much small-grain work
  is still being emitted

This is a major reason Psionic remains far behind on the exact GPT-OSS 20B model,
whose expert lane is where `MXFP4` matters most.

### 4. Psionic attention is CUDA-backed, but not `fattn.cu`-class

The current Psionic attention path is much better than the old CPU fallback, but
it is still a simpler decode kernel than the `llama.cpp` flash-attention path.

`llama.cpp` does not just have "a faster attention kernel." It has:

- graph-level `build_attn(...)` integration
- `ggml_flash_attn_ext(...)` eligibility and dispatch
- architecture-specific kernel families inside `fattn.cu`

Psionic is still missing that full stack.

### 5. CUDA graph capture and fusion reuse are still a `llama.cpp` advantage

The live `llama.cpp` run on this host reported `USE_GRAPHS = 1`, and the backend
code clearly separates capture/update logic from ordinary graph execution.

Psionic currently has:

- reusable buffers
- reusable step-plan structures
- one submission per token

Psionic does not yet have the same class of:

- decode-graph identity
- capture/update/reuse contract
- graph-driven fusion regions
- concurrent stream/event scheduling

That missing layer is why the remaining work should start with a graph/fusion
alignment issue before more micro-kernel tweaking.

### 6. This is not a GGUF or Harmony semantics problem

The current gap is not caused by prompt rendering or GPT-OSS parsing semantics.
Those have already been aligned well enough that:

- the exact visible benchmark output matches
- the model loads and runs through the Psionic-owned GGUF path
- Harmony structure survives the real HTTP flow

That matters because it narrows the refactor target: the remaining gap is in the
CUDA/runtime architecture, not the GPT-OSS format semantics.

## What Should Be Ported Directly

If the goal is to bring Psionic exactly in line with `llama.cpp`, these are the
right direct-port candidates:

- `ggml-cuda/vecdotq.cuh`
  - the quantized vector-dot implementations and VDR choices for GPT-OSS-relevant
    types
- `ggml-cuda/mmvq.cu`
  - warp/row scheduling and MMVQ dispatch shape
- `ggml-cuda/mmid.cu`
  - expert-ID compaction and grouped-expert execution helpers
- `ggml-cuda/fattn.cu`
  - the attention-kernel families and their eligibility rules for the supported
    GPT-OSS dimensions
- relevant subgraph-fusion decisions in `ggml-cuda/ggml-cuda.cu`
  - top-k MoE fusion
  - `mul_mat(+id)` plus bias / GLU fusion
  - RMSNorm fusion
  - RoPE / KV-write fusion

What should remain Psionic-owned in Rust:

- the product-facing HTTP surface
- GGUF model loading and truth metadata
- Harmony prompt/render/parse behavior
- observability, receipts, and benchmark evidence
- a Rust graph/runtime representation that mirrors `llama.cpp` semantically
  without making `llama.cpp` a runtime dependency

## Clear Path To llama.cpp-Class Speed

The remaining sequence should be executed in this order:

1. Land `#3249`
   - mirror `openai-moe-iswa.cpp`, `llama-graph.cpp`, `llama-context.cpp`, and
     the relevant `ggml-cuda.cu` fusion/dispatch rules in the Psionic-owned
     runtime architecture
   - stop rebuilding fresh CUDA KV mirrors from host-owned prefix cache state
     on identical requests
   - add `llama.cpp`-class prompt-cache residency and graph-update behavior to
     the exact HTTP benchmark lane
2. Land `#3247`
   - port the relevant `llama.cpp` CUDA kernels and dispatch policy directly,
     especially `vecdotq.cuh`, `mmvq.cu`, `mmid.cu`, and `fattn.cu`
3. Keep `#3248` open until the benchmark contract is actually met
   - same model
   - same host
   - same HTTP flow
   - same visible output
   - within `20%` of `llama.cpp`

The important change in direction is this:

- the next work should not be "find another isolated hotspot"
- the next work should be "make Psionic's GPT-OSS CUDA/runtime architecture look
  like `llama.cpp` on purpose"

## 2026-03-09 Checkpoint: Pinned Host Staging Did Not Move The Benchmark

This checkpoint tightened one real architectural mismatch with `llama.cpp`:

- Psionic's GPT-OSS decode-step plan now uses pinned host staging buffers for
  the per-step hidden input, decode parameters, and greedy-token readback
- the captured CUDA decode graph now owns the host-to-device and
  device-to-host stream copies for that state instead of relying on repeated
  synchronous `cudaMemcpy(...)` calls outside the graph
- the grouped-selected `moe_down_aggregate_q8_1` dispatch gap was also closed
  so `expert_used_count = 4` no longer falls back to the generic atomic path

That alignment matters because it removes one obvious divergence from
`llama.cpp`'s graph-owned decode path. It just did not move the measured
throughput on this exact benchmark.

Measured after the checkpoint, with the same exact one-sentence HTTP request:

- `run=1` `90.55 tok/s`
- `run=2` `90.25 tok/s`
- `run=3` `88.72 tok/s`

That is statistically the same speed class as the prior `~92 tok/s` checkpoint.
The practical conclusion is that repeated synchronous host copies were not the
primary limiter on this prompt. The remaining gap is still dominated by the
device-side decode kernels themselves:

- attention remains on the Psionic-owned scalar-ish fused kernel, not
  `llama.cpp`'s flash-attention family
- grouped expert execution is still only semantically aligned, not kernel-shape
  aligned, with `ggml_mul_mat_id(...)`
- the remaining work should favor direct ports of `fattn.cu`, `mmvq.cu`, and
  the `mul_mat_id` execution path before more request-path or transfer-path
  cleanup

## 2026-03-09 Host Ceiling Note: The Active GPU Workload Is Also Limiting Control Throughput

After the staging checkpoint, the local `llama.cpp` control was rerun on the
same exact benchmark to establish the live ceiling on this host under the
current workload mix.

Measured `llama.cpp` control throughput on the same one-sentence HTTP request:

- `run=1` `166.31 tok/s`
- `run=2` `169.03 tok/s`
- `run=3` `164.63 tok/s`

At the same time, `nvidia-smi` still reported:

- `steamwebhelper` resident on the GPU
- `dota2` resident on the GPU with roughly `2100 MiB`

That matters for planning. It means the current machine state is not just
holding Psionic below `180 tok/s`; it is holding the local `llama.cpp`
reference below `180 tok/s` as well. So:

- algorithm work in Psionic is still required
- but honest verification of a `>180 tok/s` target on this host now also
  requires an exclusive-or-near-exclusive GPU run

This should not change the code direction. The next code work is still the same:

- direct `fattn.cu` alignment for decode attention
- direct `mmvq.cu` / MMQ alignment for the MXFP4 expert path
- closer `mul_mat_id` scheduling alignment for the grouped expert lane

It does change the benchmark contract for future checkpoints:

- if the control stays under `180 tok/s`, do not claim Psionic cleared the
  target on this machine
- first clear the external GPU contention, then rerun both Psionic and
  `llama.cpp`

## Honest Status

Psionic is no longer "nowhere close." The current checkpoint is a real improvement:

- the exact Psionic-owned GPT-OSS HTTP flow works
- the visible benchmark output matches `llama.cpp`
- Psionic is about `4.09x` faster than it was at the start of the perf track
- eliminating logits readback for the greedy lane proved that readback was no
  longer the primary limiter

But Psionic is still not in the same speed class as `llama.cpp` on this host.
The current measured gap is still large enough that `#3249`, `#3247`, and
`#3248` must remain open.

The correct summary is:

- the format, prompt, and execution-boundary issues are mostly solved
- the remaining gap is now mostly backend-resident prompt-cache reuse,
  graph/fusion architecture, grouped-expert execution quality,
  attention-kernel quality, and direct CUDA kernel parity
