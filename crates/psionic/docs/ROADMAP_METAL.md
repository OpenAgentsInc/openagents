# Psionic Metal Roadmap

> Status: updated 2026-03-10 after verifying the current Metal GPT-OSS issue
> set with `gh issue view` / `gh api`, after confirming the latest same-host
> benchmark receipt attached to [#3262](https://github.com/OpenAgentsInc/openagents/issues/3262)
> on 2026-03-10, and after confirming that `main` now includes the native
> Metal GPT-OSS baseline plus the follow-up docs and perf work through
> `79803bb28`.
>
> This is the live roadmap for the native Apple Silicon Metal GPT-OSS lane in
> `crates/psionic/*`. It is intentionally narrower than
> `crates/psionic/docs/ROADMAP.md`: it is about making Metal truthful,
> correct, and fast enough to stand on its own, not about the full Psionic
> replacement program.

Agent execution instruction: implement this roadmap one issue at a time in the
recommended dependency order listed here, not by raw issue number ordering. For
each completed issue, update this document before moving to the next one so it
reflects the new GitHub state, the landed commit hash, the benchmark or parity
receipt that justified closure, and the current remaining queue.

Reference-first instruction: Metal GPT-OSS work must not be implemented from
memory. Choose the reference that owns the layer being changed:

- start with `~/code/gpt-oss` for GPT-OSS-specific correctness, tokenizer and
  Harmony contract, rope/yarn metadata, special-token behavior, and the local
  Metal reference runtime contract
- start with `~/code/llama.cpp` for same-host Metal architecture, prompt versus
  decode reserve behavior, grouped expert dispatch, backend-side sampling, and
  the throughput oracle
- start with `~/code/mlx-lm` for prompt/prefill shape, cache policy, nearest-
  prefix serving reuse, and later batch-oriented serving policy

Psionic-only execution rule: these reference trees are semantic and performance
oracles only. They must not be used as execution shortcuts. Do not shell out
to, proxy through, FFI-wrap, or otherwise delegate runtime behavior to
`llama.cpp`, `gpt-oss`, or `mlx-lm` when closing roadmap items in this track.

## Objective

Make Psionic's native Apple Silicon Metal GPT-OSS path good enough to be
truthfully used as a real local backend:

- correct against the GPT-OSS model contract
- explicit about backend readiness and validation status
- prompt/prefill and decode shaped as real Metal runtimes
- device-owned across the hot path instead of host-driven
- benchmarked against same-host `llama.cpp` on the exact tracked contract

This is not a plan to claim generic "Metal text generation" progress as GPT-OSS
readiness. The dense Metal text-generation baseline already shipped. This
roadmap is about the real OpenAI-MoE path.

## Ownership Rules

This roadmap must continue to respect `docs/OWNERSHIP.md`:

- `crates/psionic/*` owns model loading, runtime policy, backend lowering,
  execution, serving, conformance, validation, and benchmark truth
- `apps/autopilot-desktop` owns the local inference pane, app-level runtime
  controls, provider UX, and any workbench or product-facing orchestration
- Metal roadmap work must not move app-specific UI behavior into
  `crates/psionic/*`

## Why This Roadmap Exists

`crates/psionic/docs/ROADMAP.md` already tracks the full Psionic program, but
the Metal GPT-OSS lane now has its own concrete issue queue and its own
reference lessons.

As of 2026-03-10, the current issue state is:

- closed:
  - [#3270](https://github.com/OpenAgentsInc/openagents/issues/3270)
  - [#3268](https://github.com/OpenAgentsInc/openagents/issues/3268)
  - [#3272](https://github.com/OpenAgentsInc/openagents/issues/3272)
  - [#3271](https://github.com/OpenAgentsInc/openagents/issues/3271)
  - [#3261](https://github.com/OpenAgentsInc/openagents/issues/3261)
- open:
  - [#3286](https://github.com/OpenAgentsInc/openagents/issues/3286)
  - [#3285](https://github.com/OpenAgentsInc/openagents/issues/3285)
  - [#3269](https://github.com/OpenAgentsInc/openagents/issues/3269)
  - [#3262](https://github.com/OpenAgentsInc/openagents/issues/3262)

That is now a clear enough queue that the Metal lane deserves its own
dependency-ordered roadmap.

## Shipped On Main

`main` already includes the following Metal GPT-OSS baseline work:

- `eafc3c5a1` `Ship Metal GPT-OSS path, Apple benchmark harness, and local inference workbench (#3265)`
  - first native Metal GPT-OSS path on main
  - benchmark harness and validation/doc plumbing
  - initial local inference workbench integration surfaces
- `3a691af24` `Wire Metal GPT-OSS serve path into device KV runtime (#3275)`
  - device KV, shared-prefix, and reserved attention runtime wiring
- `da3ea2e9a` `psionic: strengthen metal gpt-oss refusal coverage`
  - stronger validation and readiness truth so the backend does not overclaim
- `a33e3021a` `psionic: bound metal gpt-oss decode logits (#3280)`
  - bounded logits output modes and backend-side greedy/bounded decode behavior
- `8e57e245e` `psionic: batch metal gpt-oss decode submissions`
  - fewer decode submissions and less obviously fragmented steady-state work
- `f9e5833fd` `psionic: trim metal gpt-oss prompt prefill logits`
  - avoids full prompt-logit materialization on intermediate prompt tokens
  - keeps exact and partial prompt-prefix reuse working with final-prompt-logit
    storage only

Issue-level shipped status on `main`:

- [#3270](https://github.com/OpenAgentsInc/openagents/issues/3270): closed
  - Metal versus proxy execution truth is explicit
- [#3268](https://github.com/OpenAgentsInc/openagents/issues/3268): closed
  - native serve path is wired into device KV and shared-prefix/runtime seams
- [#3272](https://github.com/OpenAgentsInc/openagents/issues/3272): closed
  - decode can use bounded logits modes instead of always materializing raw
    output
- [#3271](https://github.com/OpenAgentsInc/openagents/issues/3271): closed
  - some per-op waits, readbacks, and decode-step fragmentation were removed
- [#3261](https://github.com/OpenAgentsInc/openagents/issues/3261): closed
  - CPU-versus-Metal parity, validation, and benchmark evidence exist for the
    current shipped lane

This is real progress, but it is not close enough to call the Metal GPT-OSS
lane complete.

## Current Reality

The latest same-host benchmark receipt attached to
[#3262](https://github.com/OpenAgentsInc/openagents/issues/3262) on 2026-03-10
shows:

- Psionic native Metal:
  - cold: `0.05 tok/s`
  - warm non-hit: `0.10 tok/s`
  - prompt-cache-hit: `0.15 tok/s`
- same-host `llama.cpp`:
  - cold: `1.90 tok/s`
  - warm non-hit: `4.21 tok/s`
  - prompt-cache-hit: `3.50 tok/s`

That puts Psionic at roughly `2.4%` to `4.3%` of same-moment `llama.cpp` on the
tracked Apple benchmark contract, far below the `85%` acceptance band in
[#3262](https://github.com/OpenAgentsInc/openagents/issues/3262).

There is also still a correctness problem:

- the benchmark receipt reports malformed repetitive native-Metal output
- the issue trail also points to prompt-token-count mismatch on the tracked
  contract

So the remaining work is not "one last kernel tune." The lane is still blocked
on both architecture and correctness.

## Lessons Now Baked Into This Roadmap

This roadmap explicitly adopts the conclusions from:

- `crates/psionic/docs/METAL_GPT_OSS_LLAMA_CPP_LESSONS.md`
- `crates/psionic/docs/METAL_GPT_OSS_MLX_LM_LESSONS.md`
- `crates/psionic/docs/METAL_GPT_OSS_GPT_OSS_LESSONS.md`

### `llama.cpp`: performance and architecture truth

`llama.cpp` sets the architectural bar for the Metal backend:

- GPT-OSS should be treated as one graph-owned contract, not a host-driven step
  engine
- prompt-processing and token-generation must be reserved separately
- backend samplers and output ownership belong in the runtime shape
- the backend needs a broad op surface, not just dense matmul
- grouped expert dispatch and graph-safe concurrency belong below the serving
  loop

Practical roadmap consequence:

- [#3269](https://github.com/OpenAgentsInc/openagents/issues/3269) and
  [#3262](https://github.com/OpenAgentsInc/openagents/issues/3262) are mostly
  about execution shape, not micro-optimizations

### `mlx-lm`: prompt/prefill and serving-policy truth

`mlx-lm` sharpens the prompt and cache story:

- prefill is a distinct runtime and should be chunked
- cache policy should be model-owned, not bolted on later
- prompt-cache reuse should be a serving primitive
- prompt and generation throughput should be measured separately
- richer batch scheduling belongs later, after the single-request path is honest

Practical roadmap consequence:

- [#3285](https://github.com/OpenAgentsInc/openagents/issues/3285) is a real
  standalone blocker, not just a sub-bullet of the broad perf umbrella

### `gpt-oss`: correctness and model-contract truth

The `gpt-oss` repo is the strongest model-specific reference:

- explicit `process` versus `sample` runtime separation
- built-in LCP/KV reuse after reset
- a model-specific offline layout contract
- explicit rope/yarn and tokenizer metadata carriage
- GPT-OSS-specific kernel ownership across routing, SDPA, MoE, and sampling

Practical roadmap consequence:

- [#3286](https://github.com/OpenAgentsInc/openagents/issues/3286) must be
  treated as first-class work, not as a side note inside the perf issue

## What Still Blocks A Real Metal Lane

### Correctness and prompt parity

Tracked by [#3286](https://github.com/OpenAgentsInc/openagents/issues/3286).

Current truth:

- native Metal can produce malformed repetitive output on the tracked real-model
  benchmark
- prompt token counts do not yet line up cleanly with the trusted control

Required outcome:

- prompt rendering, tokenization, rope/yarn metadata, and real-model decode
  behavior are all checked against a trusted control and fixed where needed

### Prompt/prefill runtime

Tracked by [#3285](https://github.com/OpenAgentsInc/openagents/issues/3285).

Current truth:

- uncached prompt ingest still effectively replays decode token-by-token
- exact prompt-cache hits help much more than uncached prompt evaluation

Required outcome:

- prompt/prefill becomes a true Metal runtime with its own reserve/reuse
  behavior and device-owned prompt KV construction

### Decode hot path and MoE ownership

Tracked by [#3269](https://github.com/OpenAgentsInc/openagents/issues/3269).

Current truth:

- too much GPT-OSS decode and MoE work still lives on the host
- the backend still does not own enough of the steady-state path

Required outcome:

- RMSNorm, RoPE, router selection, expert dispatch, expert aggregation, and
  decode-side output ownership stay device-owned across the hot path

### Final same-host benchmark closure

Tracked by [#3262](https://github.com/OpenAgentsInc/openagents/issues/3262).

Current truth:

- benchmark plumbing is real
- the lane is still far outside the throughput target
- correctness is still not clean enough to treat perf closure as honest

Required outcome:

- same-host receipts show at least `85%` of same-moment `llama.cpp` throughput
  on the tracked contract, with `90%` as the stretch target

## GitHub-Backed Roadmap Items

### Phase M0: shipped baseline

Already on `main`:

- [#3270](https://github.com/OpenAgentsInc/openagents/issues/3270)
- [#3268](https://github.com/OpenAgentsInc/openagents/issues/3268)
- [#3272](https://github.com/OpenAgentsInc/openagents/issues/3272)
- [#3271](https://github.com/OpenAgentsInc/openagents/issues/3271)
- [#3261](https://github.com/OpenAgentsInc/openagents/issues/3261)

### Phase M1: correctness and contract fidelity

- [#3286](https://github.com/OpenAgentsInc/openagents/issues/3286)

Scope:

- diff Harmony prompt render and tokenization against trusted control
- add a focused real-model native-Metal parity harness
- determine whether the bad output is prompt-contract drift, numerical drift, or
  both
- fix the responsible path before claiming the lane is benchmark-ready

### Phase M2: true prompt/prefill runtime

- [#3285](https://github.com/OpenAgentsInc/openagents/issues/3285)

Scope:

- reserve prompt-shaped runtime separately from decode
- batch or chunk prompt ingest instead of replaying decode semantics token by
  token
- keep prompt KV and prompt-prefix residency on device
- preserve exact and partial shared-prefix reuse

### Phase M3: decode hot-path device ownership

- [#3269](https://github.com/OpenAgentsInc/openagents/issues/3269)

Scope:

- move remaining steady-state decode math off the host
- keep MoE router, gate/up, activation, down, and aggregation device-owned
- align more literally with `llama.cpp`'s grouped expert and backend-sampler
  shape

### Phase M4: same-host throughput closure

- [#3262](https://github.com/OpenAgentsInc/openagents/issues/3262)

Scope:

- rerun the full same-host benchmark contract after each real architectural win
- treat cold, warm non-hit, and prompt-cache-hit as separate receipt classes
- close only when the `85%` same-host band is actually met with truthful output

## Recommended Order

The correct execution order is:

1. [#3286](https://github.com/OpenAgentsInc/openagents/issues/3286)
2. [#3285](https://github.com/OpenAgentsInc/openagents/issues/3285)
3. [#3269](https://github.com/OpenAgentsInc/openagents/issues/3269)
4. [#3262](https://github.com/OpenAgentsInc/openagents/issues/3262)

Why this order:

- correctness first, because benchmarking the wrong prompt or wrong model path
  is not an honest perf program
- prompt/prefill second, because the current cold and warm non-hit receipts are
  dominated by the missing prompt runtime
- decode hot-path ownership third, because steady-state decode still needs to be
  device-owned, but it should be optimized on top of the correct prompt/runtime
  contract
- umbrella perf closure last, because it is the result of the other three, not
  a separately implementable fix

## 2026-03-10 Grouped Expert-Down Checkpoint

Paused state for the next Metal session:

- the working `main` checkpoint now includes a first ids-driven grouped
  expert-down path for Metal GPT-OSS, mirroring the next honest direction that
  unlocked the NVIDIA lane later in
  `crates/psionic/docs/ROADMAP.md`
- `psionic-backend-metal` now exposes `expert_matvec_f32_ids` for grouped
  per-selected-input expert projection in both `Q8_0` and `MXFP4`
- the planned native-Metal GPT-OSS decode path now uses one grouped expert-down
  submission per layer instead of one submission per selected expert
- Metal perf metrics now explicitly report whether the grouped expert ids path
  was used

Synthetic evidence on the kept path:

- new backend parity tests for grouped `expert_matvec_f32_ids` pass for both
  `Q8_0` and `MXFP4`
- `cargo test -p psionic-serve gpt_oss::tests::metal_gpt_oss_service_ -- --nocapture`
  stays green
- the focused planned-path receipt improved from `16` Metal kernel encodes /
  `12` submissions down to `14` kernel encodes / `10` submissions on the tiny
  GPT-OSS fixture

Direct real-model receipt on the local 20B MXFP4 GGUF after this checkpoint:

- first rerun: prompt-only `2` tokens in `25.834s` (`0.077 tok/s`), exact-hit
  plus-one in `5.615s`
- immediate rerun: prompt-only `2` tokens in `11.694s` (`0.171 tok/s`),
  exact-hit plus-one in `5.630s`
- prompt-only runtime counters on the steadier rerun:
  `expert_projection_s=9.971`, `attention_s=0.008`, `submissions=193`,
  `kernels=289`

Interpretation:

- this is real `#3269` progress because the steady-state planned path now uses
  materially fewer Metal submissions and kernel encodes
- it is **not** an honest closure or even a clear throughput win yet
- compared with the earlier kept direct receipt (`9.744s`, `0.205 tok/s`,
  `337` submissions, `433` kernels), the grouped path reduced launch overhead
  materially but did not yet pull prompt wall time into a meaningfully better
  band
- `#3269`, `#3285`, and `#3262` all stay open

Concrete path forward from this checkpoint:

1. Keep the new grouped expert-down substrate as the base for the next Metal
   MoE pass rather than going back to one submission per expert.
2. Move the remaining selected-expert host work off the hot path:
   grouped gate/up bias handling, SwiGLU activation, and if possible weighted
   down-output accumulation should stop bouncing through host `Vec<f32>` slices.
3. In parallel with that MoE cleanup, return to `#3285` and build the real
   prompt/prefill runtime:
   prompt work still replays decode token-by-token, and the current receipts
   say that architecture problem remains larger than launch-count cleanup alone.
4. Keep using the ignored direct short-text receipt as the first acceptance
   gate, then rerun the full same-host `#3262` benchmark only after prompt-only
   and exact-hit follow-up receipts both move materially.

## Definition Of Done For Metal GPT-OSS

This Metal roadmap is complete only when all of the following are true:

- the tracked benchmark contract produces coherent native-Metal Harmony output
- prompt token counts match the trusted control, or any remaining difference is
  deliberate and documented with evidence
- prompt/prefill runs through a real Metal prompt runtime instead of
  decode-step replay
- exact and partial shared-prefix reuse remain correct and device-owned
- steady-state decode no longer depends on host RMSNorm, RoPE, router, softmax,
  SwiGLU, or expert aggregation
- same-host receipts on the tracked Apple host reach at least `85%` of
  same-moment `llama.cpp` throughput, with `90%` remaining the stretch target
- the hardware-validation and provider/serve truth surfaces describe the Metal
  lane honestly
- no part of the shipped execution path shells out to or proxies through an
  external runtime

## Likely Follow-On After This Roadmap

There is one likely follow-on that is not yet part of the core blocker set:

- batched Metal GPT-OSS serving policy for prompt batching, decode batching, and
  richer nearest-prefix prompt-cache reuse once the single-request path is both
  correct and honest

The lessons from `mlx-lm` make that follow-on look worthwhile, but it should
not be pulled ahead of the current blocker queue.

## Non-Goals

- claiming generic "Metal text generation" progress as equivalent to Metal
  GPT-OSS readiness
- optimizing the old host-driven step engine instead of replacing it with the
  right prompt and decode runtime shapes
- filing a long tail of speculative kernel issues before the current four-issue
  blocker queue is resolved
- moving UI/workbench logic from `apps/autopilot-desktop` into `crates/psionic/*`
- treating `llama.cpp`, `gpt-oss`, or `mlx-lm` as acceptable execution
  shortcuts instead of reference implementations
