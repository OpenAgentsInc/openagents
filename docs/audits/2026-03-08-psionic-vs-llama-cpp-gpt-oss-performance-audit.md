# 2026-03-08 Psionic vs llama.cpp GPT-OSS Performance Audit

> Updated 2026-03-10 after re-running the live repo benchmark script against
> the current Psionic worktree on the local RTX 4080 host, after landing the
> CUDA-side shared-prefix residency follow-up, after trimming the default
> OpenAI-compatible hot path so Harmony debug fields are opt-in instead of
> always serialized, after adding prompt-token reuse on the HTTP lane plus
> safe cross-request CUDA decode-graph reuse keyed to the actual KV allocation
> identities, after the newer exact-prompt shared-prefix fast paths that stop
> cloning full prompt-logit histories and avoid re-recording or host-cloning
> unchanged prompt caches on repeated GPT-OSS HTTP requests, and after the
> latest decode-kernel checkpoints that confirmed several plausible
> llama.cpp-aligned ideas still do not win this workload: q8_0 `f16`
> projection mirrors feeding cuBLAS tensor-op GEMV regressed into the mid-80s
> tok/s, replacing the GPT-OSS `selected_count = 4` custom MoE kernels with
> simpler per-expert MMVQ routing also stayed below the best prior checkpoint,
> a grouped-query decode-attention kernel specialized for the exact GPT-OSS
> geometry on this host (`64` query heads, `8` KV heads, `64` head dim)
> regressed the live HTTP benchmark into the low `70 tok/s` range and was
> removed, and forcing an `f16` mirror onto the final q8_0 output head alone
> also lost at `82.33 tok/s`. The newest iteration adds two more ruled-out
> branches on the exact same HTTP benchmark: quantizing the selected-4 MoE down
> activation from `f32` into shared `Q8_1` blocks inside the down kernel
> regressed to `79.03 tok/s`, and widening the selected-4 gate/down kernels to
> four rows per CUDA block regressed to `88.98 tok/s` and was reverted. The
> shared-quantize branch remains available only behind
> `PSIONIC_GPT_OSS_EXPERIMENTAL_FUSED_SELECTED4_MOE_DOWN=1` for profiling, but
> it is off by default because it loses on this host. This update also records
> two small but real wins:
> folding the greedy output-head argmax into the quantized q8_1 logits
> projection lifted the exact HTTP benchmark to `92.45 tok/s`, and the newest
> direct-attention-output checkpoint fused the f16-KV decode-attention output
> directly into contiguous `Q8_1` blocks for the q8_1 attention-output
> projection path. That moved the exact HTTP benchmark to `101.32 tok/s` and
> cut the warm timed-request kernel-launch count from `9102` to `8214`, but
> left step-wall time effectively flat at about `295.5 ms` for the `37`
> generated tokens, which is strong evidence that the remaining gap is now in
> the heavy projection, MoE, and attention dispatch itself rather than in
> standalone quantize helpers. This file is
> the current audit for the GPT-OSS throughput gap; later product truth still
> lives in `docs/MVP.md`, `docs/OWNERSHIP.md`,
> `crates/psionic/docs/ROADMAP.md`, and the referenced issues. The current
> benchmark script now explicitly unsets `PSIONIC_OPENAI_INCLUDE_DEBUG_FIELDS`
> before launching Psionic so perf receipts and extra JSON serialization cannot
> silently contaminate the benchmark. The newest checkpoint in this file makes
> the official-GPT-OSS-aligned selected4 MoE project-plus-accumulate path the
> default decode path for the tracked NVIDIA workload, moves its down-project
> launch to a six-warp threadgroup that matches the official `192`-thread
> small-token matmul shape more closely, and lifts the repeated
> `prompt_cache_hit` lane into the low `123 tok/s` range on this host while
> also correcting the roadmap direction: the official grouped
> routing-metadata / scatter / gather path is prefill-oriented, while the
> decode-hot-path work on this benchmark tracks the official small-token
> `moe_matmul_swiglu -> moe_matmul -> accumulate` path instead. The newest
> checkpoint after that work is much larger: `#3293` is now landed, and the
> real GPT-OSS CUDA decode path no longer calls the old fused
> `router_topk_softmax_32_kernel` helper directly. Instead it uses a
> transposed-router dense matmul, device bias add, and delayed-softmax top-k
> over precomputed router logits. Two consecutive end-to-end runs of the exact
> benchmark contract on this host now measure Psionic `prompt_cache_hit` at
> `173.19 tok/s` and `171.29 tok/s`, both with the exact visible one-sentence
> response. The newest landed follow-up keeps the benchmark in that same
> throughput class while moving GPT-OSS expert down-projection onto a reusable
> ids-driven backend surface: `#3294` now routes the real decode lane through
> ids-driven expert matvec plus accumulate calls and uses a grouped project
> kernel by default, with fresh full-script runs at `173.05 tok/s` and
> `170.05 tok/s`. The final contract-clean closure run then updated the
> benchmark script itself so the `llama.cpp` control also returned the same
> visible sentence under the exact request contract; on that closing run,
> Psionic measured `172.84 tok/s` versus `160.98 tok/s` for `llama.cpp`, with
> `prompt_cache_hit_visible_output_match=true`. That closes the original
> throughput-parity issue honestly on this host.

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

### Current fused-greedy-output checkpoint

- Psionic:
  - `37` completion tokens in `0.400s`
  - `92.45 tok/s`
- `llama.cpp`:
  - `42` completion tokens in `0.247s`
  - `170.23 tok/s`
- Gap:
  - `1.84x`
- Psionic improvement over the original baseline:
  - `5.52x`
- Psionic improvement over the prior clean checkpoint:
  - `1.04x`
- What changed:
  - the greedy GPT-OSS output head now folds argmax into the `Q8_0/MXFP4 x Q8_1`
    CUDA logits projection, so Psionic no longer materializes and rescans the
    full logits vector on the common greedy decode path
- Newly ruled-out branch:
  - building an `f16` mirror for the final q8_0 output head alone loaded and
    ran correctly on this host, but the exact same benchmark regressed to
    `82.33 tok/s`, so the mirror was removed again

The visible output text matched exactly in the current benchmark:

`HTTPS protects users by encrypting traffic, preventing tampering, and confirming they are connected to the right website.`

### Current restored-default checkpoint

- Psionic:
  - `37` completion tokens in `0.403s`
  - `91.79 tok/s`
- `llama.cpp`:
  - `42` completion tokens in `0.253s`
  - `165.87 tok/s`
- Gap:
  - `1.81x`
- Psionic improvement over the original baseline:
  - `5.48x`
- What changed in this iteration:
  - an experimental selected-4 MoE-down kernel that quantizes the activated
    expert rows from `f32` into shared `Q8_1` storage landed in the CUDA
    backend, but it is guarded off by default because the exact benchmark fell
    to `79.03 tok/s` when enabled
- Newly ruled-out branch:
  - widening the selected-4 gate/up and down kernels from two rows per block
    to four rows per block looked promising as a launch-count reduction, but
    the exact benchmark regressed to `88.98 tok/s`, so that kernel shape was
    reverted

### Current attention-output-q8_1 checkpoint

- Psionic:
  - `37` completion tokens in `0.365s`
  - `101.32 tok/s`
- `llama.cpp`:
  - `42` completion tokens in `0.224s`
  - `187.13 tok/s`
- Gap:
  - `1.85x`
- Psionic improvement over the original baseline:
  - `6.05x`
- Psionic improvement over the prior restored-default checkpoint:
  - `1.10x`
- What changed in this iteration:
  - the f16-KV fused decode-attention kernels now have q8_1 output variants,
    and the GPT-OSS CUDA path uses them whenever the attention-output
    projection can consume a contiguous q8_1 activation buffer directly
  - that removes the standalone `quantize_f32_to_q8_1(attention_buffer ->
    vector_q8_1_buffer)` kernel from the attention-output lane on both the
    regular and decode-graph paths
- Warm timed-request receipt on the exact benchmark:
  - `prefix_tokens_reused = 158`
  - `step_count = 37`
  - `kernel_launches = 8214`
  - `host_to_device_bytes = 426832`
  - `device_to_host_bytes = 296`
  - `stage_timings.step_wall_ns = 295535840`
- Interpretation:
  - the launch count dropped materially from the prior `9102`, but decode-step
    wall time stayed roughly flat, so the next real wins still need to come
    from deeper `llama.cpp`-class dispatch and kernel parity rather than from
    shaving more helper kernels around the edges

## New Findings From The Latest Iteration

- another helper-kernel shave is no longer enough to explain the remaining
  throughput gap.
  - fusing decode-attention output directly into q8_1 storage removed another
    `888` kernel launches from the warm timed request, but step-wall time held
    near `295 ms` and the exact HTTP benchmark only moved into the
    `~101 tok/s` range
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
- the selected-4 MoE lane is still sensitive to local fusion choices that look
  obviously cheaper on paper.
  - both "quantize selected expert activations inside the down kernel" and
    "process four rows per block instead of two" reduced one visible kind of
    overhead, and both still lost end-to-end on the real HTTP benchmark
- the output head did have one small avoidable waste on the greedy path.
  - folding argmax into the quantized q8_1 logits projection recovered a few
    tok/s without changing model semantics, but the gain was incremental rather
    than transformational
- the remaining gap is now concentrated even more tightly in the kernels that
  Psionic still does not match line-for-line with llama.cpp.
  - the next real alignment targets are the ids-enabled `mul_mat_vec_q` / MMVQ
    path for the GPT-OSS MoE decode lane and the flash-attention / dispatch
    behavior in `fattn.cu`; the greedy-logits materialization waste is now
    partially addressed and no longer the biggest obvious local inefficiency
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

## 2026-03-09 Exact Prefix CUDA Alignment Checkpoint

The newest checkpoint moved the tracked HTTP benchmark back above the current
issue baseline while also fixing a real prompt-cache correctness bug.

What changed:

- exact host-side prompt hits now require and prefer an exact CUDA KV-prefix hit
  instead of falling back to the generic "best shared prefix" CUDA lookup
- exact prompt hits now also cache the greedy next-token selection derived from
  the stored prompt logits, so the exact-hit lane does not pay an extra CPU
  argmax scan before decode begins

Why it mattered:

- before this change, the exact host prompt cache could pair with the wrong
  device KV entry when two prompts shared a long prefix
- that was visible on the benchmark contract itself: the third exact `HTTPS ...`
  request could incorrectly return the warm `TLS ...` sentence
- it also meant the benchmarked exact-hit lane was not truly using the best
  exact CUDA prefix reuse available

Measured after the checkpoint, with the same exact one-sentence HTTP request:

- Psionic `cold`: `21.08 tok/s`
- Psionic `warm_non_hit`: `57.38 tok/s`
- Psionic `prompt_cache_hit`: `125.12 tok/s`
- `llama.cpp` `prompt_cache_hit`: `168.53 tok/s`

Interpretation:

- this is a real improvement over the issue baseline of `123.42 tok/s`
- the win is modest, which means exact-prefix correctness and first-token cache
  reuse were worth fixing but are not the main remaining limiter
- the benchmark is now more honest because the exact-hit lane no longer mixes an
  exact host prefix with a merely shared CUDA prefix

What should happen next remains the same:

- treat `125.12 tok/s` as the new floor to beat on `#3276`
- keep pressure on the decode kernels and dispatch policy, not request-path
  bookkeeping
- continue aligning grouped expert execution and attention with
  `llama.cpp`'s `mul_mat_id` / `mmvq` / `fattn` families

## 2026-03-09 CUDA Prefix Detachment Checkpoint

The previous exact-prefix checkpoint fixed only half of the repeated-prompt
bug. The live HTTP contract still exposed a deeper device-cache aliasing
problem.

What was actually wrong:

- `CudaSharedPrefixStore` was storing shallow clones of `CudaKvCacheMirror`
  device buffers
- a later non-exact shared-prefix reuse could take a truncated view of an
  earlier exact prompt entry and then append prompt-tail KV rows into the same
  underlying CUDA allocation
- that meant the stored exact prompt entry could still be silently corrupted
  even though the host-side exact prompt lookup and the CUDA-side exact prompt
  lookup both matched the right prompt tokens
- under the benchmark contract this showed up as:
  `HTTPS -> TLS -> HTTPS` returning the wrong third answer, because the exact
  `HTTPS` device prefix had been overwritten by the warm `TLS` request

What changed:

- non-exact CUDA shared-prefix reuse now detaches into a fresh writable device
  KV allocation before any prompt-tail append can mutate the stored prefix
- exact repeated-prompt hits still reuse the stored exact CUDA prefix directly,
  because they only append generated-token KV after the prompt boundary

Measured after the fix, on the exact same benchmark contract:

- Psionic `cold`: `21.13 tok/s`
- Psionic `warm_non_hit`: `58.00 tok/s`
- Psionic `prompt_cache_hit`: `124.36 tok/s`
- `llama.cpp` `prompt_cache_hit`: `170.70 tok/s`

Interpretation:

- the truthful exact-hit floor is now `124.36 tok/s`, not the previously logged
  `125.12 tok/s`
- this checkpoint is primarily a correctness repair; throughput stayed roughly
  flat once the repeated-prompt lane stopped cheating with corrupted device KV
- the benchmark baseline is now trustworthy again, because the third exact
  `HTTPS ...` request really is running on the exact stored `HTTPS` CUDA prefix

What should happen next:

- treat `124.36 tok/s` as the real floor to beat on `#3276`
- keep the focus on decode-side CUDA execution quality
- bias the next wave toward direct `llama.cpp` parity in ids-enabled
  grouped-expert execution and attention dispatch, not more prefix-cache
  micro-tuning

## 2026-03-09 Router32 CUDA Checkpoint

The next durable win came from aligning the GPT-OSS router hot path more
closely with the exact `32`-expert geometry of the model instead of continuing
to push on cache bookkeeping.

What changed:

- the CUDA router path now dispatches a dedicated one-warp
  `router_topk_softmax_32_kernel` when `expert_count == 32`
- that keeps one lane per expert and avoids paying the generic
  `kBlockSize = 256` router launch on the exact GPT-OSS case
- the split router path remains selected for the GPT-OSS MoE lane because the
  larger fused residual+norm+router kernel is still slower on this host

Measured on the exact same benchmark contract:

- Psionic `cold`: `22.40 tok/s`
- Psionic `warm_non_hit`: `63.37 tok/s`
- Psionic `prompt_cache_hit`: `134.62 tok/s`
- `llama.cpp` `prompt_cache_hit`: `168.81 tok/s`

Interpretation:

- this is the first checkpoint in the current wave that moves Psionic
  materially above the truthful `124.36 tok/s` floor
- the gain is real enough to keep: about `+10.26 tok/s` on the tracked
  `prompt_cache_hit` lane
- the remaining gap to `llama.cpp` is now about `34.19 tok/s` on this host

What was ruled out in the same iteration:

- widening the regular shared-input quantized matvec launch from
  `rows_per_block = 8` to `16` regressed the tracked lane to `132.82 tok/s`
- rewriting the selected4 MoE down-aggregate kernel to handle `4` rows per
  block regressed the tracked lane to `131.79 tok/s`

What should happen next:

- treat `134.62 tok/s` as the new floor to beat on `#3276`
- keep the router32 specialization and the split-router GPT-OSS path
- focus the next work on the remaining `llama.cpp`-parity gaps:
  ids-enabled grouped expert execution and decode attention dispatch

## 2026-03-09 Official GPT-OSS Small-Token MoE Checkpoint

The next kept improvement came from re-reading the official `~/code/gpt-oss`
Metal path more carefully and correcting which part of that repo actually
matches the benchmarked workload.

What changed in the reading and in Psionic:

- the earlier grouped `routing metadata -> scatter -> grouped expert compute ->
  gather/accumulate` interpretation is real in the official repo, but it is the
  dense-prefill path used once token count is large enough
- the exact `prompt_cache_hit` benchmark here is decode-dominated, so the
  closer official reference is the small-token path:
  `gptoss_f32_mf4w_moe_matmul_swiglu -> gptoss_f32_mf4w_moe_matmul ->
  gptoss_f32_accumulate`
- Psionic already had a truthful split `selected4` q8_1
  project-plus-accumulate substrate behind the old experimental path; this
  checkpoint makes that path the default for the tracked GPT-OSS decode case
- the selected4 down-project launcher now uses a dedicated six-warp CUDA
  threadgroup for the MXFP4/Q8_1 project stage, which better matches the
  official `192`-thread small-token matmul shape than the old four-warp launch

Measured on the exact benchmark contract after making that path the default:

- Psionic `prompt_cache_hit` repeated range:
  - `121.85 tok/s`
  - `121.80 tok/s`
  - `124.12 tok/s`
- Psionic repeated average:
  - `122.59 tok/s`
- comparison control from the same build before enabling the default path:
  - `120.69 tok/s`
  - `122.31 tok/s`
  - `121.49 tok/s`
  - average `121.50 tok/s`
- `llama.cpp` `prompt_cache_hit`:
  - `168.45 tok/s`

Interpretation:

- this is a small but real shipped gain on the exact tracked lane:
  about `+1.09 tok/s` versus the same-build control average
- the gain is not large enough to change the broader audit conclusion:
  Psionic is still well behind `llama.cpp`, and the main remaining gap is still
  decode-side CUDA execution quality rather than request-path or metadata
  bookkeeping
- the roadmap direction needed correction:
  the grouped routing-metadata / scatter / gather structure from the official
  `gpt-oss` repo is still relevant for dense-prefill work, but it is not the
  first-order decode bottleneck on this exact benchmark

What was explicitly ruled out in the same investigation wave:

- forcing the official small-token path all the way onto Psionic's existing
  generic float kernels regressed badly into the `~20 tok/s` range, which shows
  that semantic alignment alone is not enough if the CUDA kernel shape is still
  generic
- enabling the fused `add_residual_rms_norm_q8_1_router_topk` path also
  regressed into roughly the `113 tok/s` range on this host
- alternate selected4 down-project launch shapes that grouped multiple rows or
  multiple experts per block stayed flat or regressed; the six-warp one-row
  launcher was the only kept variant from this wave

What should happen next:

- treat `~123 tok/s` as the truthful current floor to beat on `#3276`
- keep the default selected4 project-plus-accumulate path
- update the active GPT-OSS alignment work so it follows the official
  small-token MoE path first, then returns to dense grouped prefill work only
  where that actually matters for the measured contract
- keep the bigger throughput push focused on the remaining heavy CUDA paths:
  ids-enabled grouped expert execution and decode attention dispatch

## 2026-03-10 Delayed-Softmax Router Split Checkpoint

This checkpoint closes the old `#3276` `150+ tok/s` umbrella and the concrete
router-parity issue `#3293`.

What changed:

- Psionic no longer runs the real GPT-OSS decode path through the old fused
  `router_topk_softmax_32_kernel` helper.
- `crates/psionic/psionic-serve/src/gpt_oss.rs` now uploads a transposed CUDA
  copy of each router matrix at model load, allocates a per-layer
  `router_logits_buffer` in the CUDA decode-step plan, and wires decode
  routing as:
  `matmul(ffn_norm, router_weight_t) -> add bias -> delayed-softmax top-k`.
- `crates/psionic/psionic-backend-cuda/src/kernels/quantized_matvec.cu` now
  exposes a dedicated delayed-softmax top-k kernel over precomputed router
  logits, including the exact `32`-expert fast path.
- `crates/psionic/psionic-backend-cuda/src/lib.rs` now exposes that op through
  the CUDA submission surface and includes a backend test proving the split
  route matches the old fused router helper on both selected ids and routing
  weights.

Measured on the exact benchmark contract after landing `#3293`:

- Psionic `prompt_cache_hit`:
  - `173.19 tok/s`
  - `171.29 tok/s`
- Both runs returned the exact visible one-sentence response:
  `HTTPS protects users by encrypting traffic, preventing tampering, and confirming they are connected to the right website.`
- Immediate pre-issue truthful floor on the same host and contract:
  - about `123.48 tok/s`
- Net gain from this issue alone:
  - about `+48 tok/s`

Control status:

- The same benchmark script currently records `llama.cpp` in the
  `167-169 tok/s` class on this host.
- However, the script still lets `llama.cpp` spend completion tokens in
  Harmony `reasoning_content`, so the current `llama.cpp` numbers remain useful
  as a throughput oracle but not as a contract-clean visible-output comparison.
- Psionic is now the cleaner benchmark participant on this exact script
  contract because it returns the exact visible sentence and stops cleanly.

Interpretation:

- The router split was not a neutral architectural cleanup; it was a real hot
  path win on this GPU and model.
- The old belief that Psionic's floor was still in the `122 tok/s` class is no
  longer true after this landing.
- The next remaining performance work should move directly to the expert path
  that still differs most from `llama.cpp`: `#3294` ids-driven grouped expert
  matvec, then `#3295` fused gate/up `+ GLU`, then `#3296` attention dispatch.

## 2026-03-10 Grouped-Expert Matvec Checkpoint

This checkpoint closes `#3294`.

What changed:

- `crates/psionic/psionic-backend-cuda/src/lib.rs` now exposes reusable
  ids-driven expert matvec and expert-output accumulation submission calls,
  rather than leaving the down-project expert path explained only as a
  selected4-specific helper sequence.
- `crates/psionic/psionic-serve/src/gpt_oss.rs` now routes the real GPT-OSS
  decode lane through those ids-driven backend calls.
- `crates/psionic/psionic-backend-cuda/src/kernels/quantized_matvec.cu` now
  launches a grouped project kernel for the selected-expert down path, so the
  selected experts are projected together instead of as four mostly
  independent project launches glued together in the serve layer.

Measured on the exact benchmark contract after landing `#3294`:

- Psionic `prompt_cache_hit`:
  - `173.05 tok/s`
  - `170.05 tok/s`
- Both runs returned the exact visible one-sentence response:
  `HTTPS protects users by encrypting traffic, preventing tampering, and confirming they are connected to the right website.`
- Same-script `llama.cpp` control on those runs:
  - about `166.80 tok/s`
  - about `167.89 tok/s`

Short `ncu` sample on the kept path:

- `moe_gate_up_swiglu_q8_1_selected4_quantized_kernel`:
  - about `60.4 us` average
- `expert_mul_mat_vec_q8_1_project_grouped_kernel`:
  - about `32.7 us` average

Interpretation:

- `#3294` is mostly a structural parity landing, not another giant tok/s jump.
- That is still a valid win: Psionic now has the ids-driven expert-matvec
  substrate the roadmap needed, and it kept the exact benchmark in the same
  low `170 tok/s` class instead of regressing it.
- The short `ncu` sample shows the gate/up stage materially below the older
  `~72.6 us` sample, while the project stage stayed roughly flat against the
  older `~33.3 us` sample. That is consistent with the benchmark staying flat
  overall: the selected-expert path is cleaner and somewhat cheaper, but it is
  no longer the source of the huge gap that existed before `#3293`.
- The next remaining direct-alignment work is now `#3295` fused ids-driven
  gate/up `+ GLU`, and only after that should the queue revisit `#3296`
  attention dispatch.

## 2026-03-10 Fused-Gate-Up Issue Closure

After the `#3294` landing, the tracked parity contract on this host no longer
needed `#3295`.

Why it was closed:

- Psionic stayed in the same low `170 tok/s` class:
  - `173.05 tok/s`
  - `170.05 tok/s`
- the same-script `llama.cpp` control on those runs was:
  - `166.80 tok/s`
  - `167.89 tok/s`

Interpretation:

- `#3295` was still a plausible structural cleanup, but it was no longer the
  next honest blocker to parity on this host.
- For the tracked benchmark contract, a riskier fused gate/up rewrite stopped
  being justified once Psionic was already at or above the current
  `llama.cpp` control.
- That is why `#3295` was closed instead of forcing more CUDA churn into the
  path just to satisfy a previously-planned intermediate issue.

## 2026-03-10 Attention-Dispatch Issue Closure

`#3296` was closed for the same reason: it stopped being the next honest
blocker on the tracked host contract.

Why it was closed:

- the older short decode-hotspot samples still had attention at about
  `4.7 us`, far below the router and expert stages that actually dominated the
  path before `#3293` and `#3294`
- after those landings, Psionic was already in the same or better throughput
  class than the current `llama.cpp` control on the exact benchmark

Interpretation:

- a `fattn.cu`-style attention rewrite may still be good future headroom work
- it is not required to close the current parity contract honestly on this host
- the only remaining open issue in the tracked chain is now `#3248`, which is
  about making the benchmark contract itself fully clean and then closing the
  throughput umbrella

## 2026-03-10 Contract-Clean Parity Closure

`#3248` is now closed.

What changed:

- `crates/psionic/scripts/benchmark-gpt-oss-vs-llama.sh` now uses the explicit
  system/developer/user GPT-OSS request contract from the manual flow instead
  of the earlier stripped-down request.
- The script now sends `reasoning_format: "none"` to the `llama.cpp` control,
  normalizes visible output by stripping reasoning wrappers, records
  `visible_output` in the per-case JSON summaries, and prints
  `prompt_cache_hit_visible_output_match=true` when both servers expose the
  same final sentence.

Final contract-clean run on this host:

- Psionic `prompt_cache_hit`:
  - `172.84 tok/s`
- `llama.cpp prompt_cache_hit`:
  - `160.98 tok/s`
- Visible-output status:
  - `prompt_cache_hit_visible_output_match=true`
  - both sides returned:
    `HTTPS protects users by encrypting traffic, preventing tampering, and confirming they are connected to the right website.`

Interpretation:

- The original NVIDIA parity track is closed honestly on this host.
- The final closeout did not require more CUDA kernel churn; it required making
  the benchmark contract itself truthful and then rerunning it.
- Future work in this area should now be framed as headroom, portability, or
  maintenance work rather than as unfinished closure for the original
  `llama.cpp` parity issue.

## 2026-03-10 GPT-OSS 120B Addendum

The 20B parity queue is closed, but the same host still exposes a separate
headroom problem on the larger hybrid 120B model:
`/home/christopherdavid/models/gpt-oss/gpt-oss-120b-mxfp4.gguf`.

Current truthful 120B floor on the exact cold / warm-non-hit /
prompt-cache-hit contract:

- Psionic:
  - `2.24 tok/s`
  - `6.45-6.47 tok/s`
  - `10.41-10.50 tok/s`

What is already landed on the kept hybrid branch:

- the model now loads truthfully on this RTX 4080 through a hybrid placement
  path instead of crashing on `cudaMalloc`
- decode attention for the hybrid path now uses CUDA with a `CudaKvCacheMirror`
- hybrid selected4 layer caches reuse selected experts across decode steps
- feed-forward prep for host-backed MoE layers now keeps residual add, RMSNorm,
  `Q8_1` prep, and router prep on CUDA before the selected4 path
- selected-expert cache fills no longer do an extra single-expert scratch
  repack before writing into the per-layer CUDA caches
- generation-only decode steps now keep the hidden state on CUDA across dense
  attention/router plus staged selected4 accumulation, instead of reading the
  FFN residual and MoE output back to host before the next CUDA-capable substep

What we re-tested and ruled out after those landings:

- simple cache-shape retunes around the kept branch:
  - `7` expanded slots on the last `4` layers regressed to about `9.89 tok/s`
  - `6` expanded slots on the last `8` layers regressed to about `9.92 tok/s`
- full CUDA-visible duplicated host-weight experiments:
  - either OOM or materially slower than the kept branch
- mapped-host selected-expert caches:
  - slower than the kept branch

What we re-tested and ruled out after the original addendum:

- direct selected4 execution from CUDA-registered host-backed GGUF pages:
  prompt-cache-hit regressed into the `6.4 tok/s` class
- using those registered host pages only as cache-fill copy sources:
  still in the same losing `6.4 tok/s` class
- a follow-up branch that kept the fused hybrid FFN residual on CUDA and
  removed one mid-layer readback:
  effectively flat at `10.07-10.09 tok/s`, so it was not kept

What the newest kept checkpoint proved:

- the hidden-state-residency direction was real, but only modest on its first
  landing: repeated exact-contract runs moved prompt-cache-hit from the
  documented `10.07 tok/s` floor to `10.42` and `10.44 tok/s`
- that means the old host hidden-vector bounce was part of the problem, but it
  was not the whole problem
- the next stateless follow-up was directionally plausible: once the
  no-session hybrid CUDA device-argmax lane stopped materializing generated KV
  entries back onto the host cache, one local run appeared to move the cold
  and warm-non-hit lanes into the same `~10 tok/s` class as the
  prompt-cache-hit lane
- that apparent three-lane lift did not survive a fresh clean rebuild and
  rerun; the kept branch still reproduces the older `2.23 / 6.42-6.46 / 10.44`
  shape, so the earlier `~10 tok/s`-across-all-three-lanes reading should be
  treated as a bad sample, not as the truthful floor
- the prompt-cache-hit lane itself stays around `10.44 tok/s`, which is still
  strong evidence that the remaining limiter is no longer the stateless
  host-KV readback
- adding truthful hybrid selected4 cache metrics showed why the old static
  cache-shape intuition was missing: the prompt-hit lane was still restaging
  about `52 GB` of selected experts per request, with per-layer misses proving
  that the old "expand the last 15 layers" rule was not the best use of the
  limited sixth-slot budget on 120B
- the kept follow-up from that measurement is a profiled 120B-specific
  expanded-slot layer set
  (`10, 12, 18, 21, 22, 23, 25, 26, 28, 29, 31, 32, 33, 34, 35`), which
  nudged the exact-contract floor to `2.24 / 6.47 / 10.50 tok/s`

What the remaining gap now points to:

- the heavy cost is no longer well-described as only "expert staging" in the
  abstract; most dense decode state now stays on CUDA, but the hybrid path
  still has to stage selected experts from host-backed MoE storage into CUDA
  caches on the hot decode lane
- after the hidden-state-residency landing plus the stateless host-KV skip,
  the remaining concrete bottleneck is clearer: surviving host-to-device
  selected4 expert staging traffic inside the host-backed MoE lane
- the next honest direction is therefore still `#3345`, but narrowed:
  keep the hybrid hidden state resident on CUDA across the host-backed
  selected4 lane and now reduce or restructure the remaining selected-expert
  staging that still forces heavy per-token PCIe traffic
- new debug-enabled prompt-cache-hit evidence on that kept branch makes the
  remaining gap sharper:
  `step_wall_ns` was about `8.76 s` for `49` generated tokens, while the
  timed kernel buckets only covered about `0.86 s` total
  (`router_ns ~117 ms`, `attention_ns ~118 ms`,
  `attention_output_projection_ns ~69 ms`,
  `expert_projection_ns ~476 ms`, `logits_projection_ns ~50 ms`).
  That means the surviving 120B wall time is still dominated by work outside
  the timed kernels, and the selected4 cache-fill path remains the most
  credible culprit.
- newly ruled out after that timing check:
  an LFU/LRU mixed selected4 eviction policy regressed prompt-cache-hit to
  about `10.32 tok/s`, a reprofiled sixth-slot layer set built from the newest
  per-layer staged-byte trace regressed to about `10.41 tok/s`, a
  memory-neutral hot/mid/cold `7/6/4` slot rebalance regressed to about
  `10.35 tok/s`, and a pinned-host async region-copy rewrite of the decode-lane
  selected4 cache-fill path cratered the cold and warm lanes while leaving
  prompt-cache-hit essentially flat at about `10.30 tok/s`.
- two more nearby 120B cache-shape ideas are now ruled out too:
  a more concentrated memory-neutral `8/6/5` hot-layer slot skew still stayed
  below the kept branch at about `10.44 tok/s`, and disabling selected4 layer
  caches entirely on the historically low-hit 120B layers cratered the exact
  contract to about `1.64 tok/s` cold, `4.68 tok/s` warm-non-hit, and
  `7.43 tok/s` prompt-cache-hit.
- updated conclusion:
  `#3345` should stay focused on cutting or restructuring selected-expert
  staging itself. The new evidence argues against retrying small cache-geometry
  tweaks or scratch-copy rewrites blindly; those are now ruled-out branches on
  this host.

Relevant `llama.cpp` references for that next step:

- `~/code/llama.cpp/ggml/src/ggml-cuda/ggml-cuda.cu`
  - `ggml_backend_cuda_host_buffer_type_*`
  - `ggml_backend_cuda_register_host_buffer(...)`
  - `ggml_backend_cuda_unregister_host_buffer(...)`
- `~/code/llama.cpp/src/llama-model-loader.cpp`
  - host-buffer / async upload setup around the loader path
- `~/code/llama.cpp/src/llama-graph.cpp`
  - decode graph construction and how intermediate state stays backend-owned
    across substeps instead of round-tripping through host vectors
