# Native Clef decision serving plan

> Moved from the psionic repo on 2026-10-09: the standalone psionic repo is reference only. All Clef engine work and its issues live in this monorepo (`crates/psionic`, issues #11194–#11197).


Status: M1 `implemented_early` on the CPU (2026-10-09); M2 done on CUDA (2026-10-10, gate met, chunk bound restated); M3 in progress on Metal ([status](#m3-status-2026-10-10-in-progress)); M4 implementation and validation in progress ([current controls](#m4-status-2026-10-11-validation-in-progress)). See [M1 status](#m1-status-2026-10-09) and [M2 status](#m2-status-2026-10-10). File-relevance calibration map (X1): [below](#file-relevance-calibration-x1-2026-10-10).

This plan covers serving Cloudflare's open-weight Clef decision models
natively in Psionic, at `POST /v1/systemone`, with no Ollama, llama.cpp,
MLX-Python or SGLang process at run time. Those engines are used only as
reference comparators when building correctness fixtures.

Tracking issues:

| Milestone | Repo | Issue |
| --- | --- | --- |
| M1 Clef-Flash lane: admission, exact encoder, joint head, CPU route | psionic | [#11194](https://github.com/OpenAgentsInc/openagents/issues/11194) |
| M2 CUDA chunked prefill and device head (RTX 4080) | psionic | [#11195](https://github.com/OpenAgentsInc/openagents/issues/11195) |
| M3 Metal chunked prefill and device head (M5 Max) | psionic | [#11196](https://github.com/OpenAgentsInc/openagents/issues/11196) |
| M4 Clef 27B, images, prefix reuse, batching | psionic | [#11197](https://github.com/OpenAgentsInc/openagents/issues/11197) |
| I1 Judge failover door on psionic `/v1/systemone` | openagents | [#11191](https://github.com/OpenAgentsInc/openagents/issues/11191) (follows [#11189](https://github.com/OpenAgentsInc/openagents/issues/11189)) |
| I2 Pylon sells decisions (`pylon/decision`) | openagents | [#11192](https://github.com/OpenAgentsInc/openagents/issues/11192) |
| I3 Router decision split so Clef can take it | openagents | [#11193](https://github.com/OpenAgentsInc/openagents/issues/11193) |

The measurements this plan builds on are in openagents
`docs/inference/clef-self-host.md` (commit `c33281c2d1`).

## M1 status (2026-10-09)

The Clef lane is `implemented_early` on the CPU in
`crates/psionic/crates/psionic-serve/src/clef/` ([#11194](https://github.com/OpenAgentsInc/openagents/issues/11194)).

**What runs.**

- `psionic-openai-server -m Clef-Flash-Q4_K_M.gguf` serves
  `POST /v1/systemone`, `GET /v1/models` (`capabilities: ["decision"]`) and
  `/health`. A Clef GGUF given with `-m` becomes a decision model, and
  `--decision-model` names one explicitly. Next to other `-m` models, the
  route is added to the generic server.
- **Admission.** `general.architecture = clef` GGUFs are admitted with their
  backbone read as `qwen35`, plus the embedded head (Q8_0/BF16/F32,
  dequantized to f32). `--clef-head <dir|joint_head.safetensors>` uses
  Cloudflare's HF head instead, beside a Clef or `qwen35` GGUF.
  - A missing head, wrong shapes, `hidden_size` drift, unknown head tensors,
    or a tied LM head are refused at load.
- **Prompt.** The encoder is a byte-exact port of `encode_record`. It has its
  own JSON reader and writer that keep Python's semantics: key order,
  repeated keys, exact big integers, `repr(float)`, and `ensure_ascii=False`
  escapes.
- **Backbone.** The CPU backbone runs a chunked prefill (default 256 tokens,
  `--decision-chunk`).
  - Each weight row is decoded once per chunk and multiplied with every row
    of the chunk, using an AVX2+FMA kernel when the CPU has it.
  - The recurrent and attention updates run token by token.
  - It streams each final hidden row into the head. The head keeps only the
    1024-wide memory rows, the span sums and the last row, and the LM head
    is never multiplied.
- **Limits and refusals.**
  - Limits: up to 255 options, 64 questions and a 16,384-token budget
    (`--decision-max-{tokens,questions,options}`), with an 8 MiB body.
  - Over the budget, too many questions or options, images or videos, or an
    oversized body are refused with `not_admitted`. Truncation happens only
    when the request sends `truncation: "state_tail"`.
  - A malformed request gets `invalid_request` (400). A full queue gets
    `busy` (503).
- **Answers.** Confidence is the reference's (the top probability), and
  probabilities are not rounded. Every answer carries
  `psionic.{artifact, artifact_digest, head_source, head_digest, backend,
  prefill_chunk, prompt_tokens, truncated_state_tokens, trained_length,
  latency_ms}`.
- **Judge chain.** `crates/jev/src/doors.rs` now fails over on these door
  refusals:
  - a 413;
  - a `not_admitted` or size/unsupported code;
  - a bare 400 that names a known door limit (Ollama's 2–26 options and
    64 KiB cap, llama.cpp's "input is too large").

  A malformed question still does not fail over.

**Measured** on coderos-4080 (CPU), against `Clef-Flash-Q4_K_M.gguf`
(revision `4a192915`, sha256 `fd3e9060…638c`; on 2026-10-11 upstream
re-uploaded the file, so `main` is now `3626116623ca…6868a2`). The fixtures
and tools are in `crates/psionic/fixtures/clef/`.

| Gate | Result |
| --- | --- |
| Encoder vs Cloudflare `encode_record` + HF tokenizer, 200 synthetic records | **0 differences** (ids, question spans, option spans, option ids) |
| Head vs torch f32 `JointSchemaHead`, the same hidden rows (Psionic's backbone, 155-token record) | **max \|Δlogit\| 2.3e-6** (HF head). The GGUF Q8_0 head vs torch f32 is 1.2e-3 logit and 6e-5 in p; torch bf16 vs f32 is 1.3e-2 logit. A tiny-head fixture test is 1e-4. |
| Chunked prefill vs token at a time | Same rows: the test requires min row cosine > 0.9999 at chunks 7, 64 and 512; the same answer to 7 digits |
| Backbone hidden rows vs HF f32 reference (155 tokens) | mean cosine 0.989, min 0.855 (Q4_K_M vs f32 weights) |
| End to end vs llama.cpp b11538 (CPU, same GGUF), 37 requests ≤ 1.1k tokens, 95 questions | top answer 90.5 %, median \|Δp\| 0.006, p99 0.115, max 0.249 |
| Same, only the 16 requests where llama.cpp builds the reference prompt (36 questions) | top answer 91.7 % (3 near-ties: top two within 0.03), median \|Δp\| 0.004, p99 0.034, max 0.077 |
| End to end vs the Cloudflare reference in f32 (HF weights), 40 requests, 104 questions | Psionic: top answer 94.2 %, max \|Δp\| 0.177. llama.cpp on the same gate: 92.6 %, max 0.30 |
| `crates/jev` quickstart (`TYPESAFE_BASE_URL` = the Psionic route) | answers `billing` in 7.1 s (143 tokens, CPU) |

**Not met: the M1 end-to-end gate as written** (100 % top answer and
\|Δp\| ≤ 0.02 against llama.cpp). The measurements show llama.cpp is not a
tight comparator here:

- llama.cpp b11538's own prompt differs from the reference encoder on 21 of
  37 requests, by up to 41 tokens. Psionic's prompt is exact.
- llama.cpp disagrees with the f32 reference more than Psionic does (92.6 %
  against 94.2 %).
- Where both build the same prompt, every top-answer difference is a near
  tie on synthetic questions.
- llama.cpp refuses `1e400` in a request (3 of 40).

Before M1 closes, the gate should be restated as **Psionic vs the reference
encoder plus the f32 reference model**, with near ties excluded, or measured
on decisive product requests.

**Speed.** On the CPU this is development speed, not serving speed: about
7–13 s per 150-token request, depending on load. M2 (#11195) owns CUDA
speed.

## M2 status (2026-10-10)

The Clef lane runs on CUDA ([#11195](https://github.com/OpenAgentsInc/openagents/issues/11195)):
`crates/psionic/crates/psionic-serve/src/qwen35/clef_cuda.rs`, kernels in
`crates/psionic-backend-cuda/src/kernels/clef_prefill.cu` (with
`clef_prefill.rs`). `psionic-openai-server --decision-device auto|cpu|cuda`
picks the device (`auto`, the default, uses CUDA when it loads and says so
when it falls back to the CPU).

### Per-layer check: is the 0.989 cosine quantization noise?

**Yes.** Every layer's residual rows and the final normalized rows were
compared on encoder records 7 (155 tokens) and 10 (970 tokens). The four
sources were:

- llama.cpp b11538 on the 4080 (CUDA), dumped through libllama's `cb_eval` hook
  with `tools/lldump.cpp`;
- Psionic's CPU lane;
- Psionic's new CUDA lane;
- the Hugging Face f32 reference.

Report: `crates/psionic/fixtures/clef/reports/layer-parity-2026-10-10.json`.
Each cell is mean cosine / min row cosine / mean normalized RMSE.

| Pair (record 7 / record 10) | Layer 0 | Layer 15 | Layer 30 | Final rows |
| --- | --- | --- | --- | --- |
| Psionic CPU vs Psionic CUDA | 1.00000 / 1.0000 / 0.000 | 1.00000 / 1.0000 / 0.001 | 1.00000 / 1.0000 / 0.001 | 1.00000 / 0.9999 / 0.001 |
| llama.cpp (CUDA) vs Psionic | 0.99997 / 0.9999 / 0.008 | 0.99934 / 0.994 / 0.034 | 0.99809 / 0.963 / 0.051 | **0.9986** / 0.972 / 0.045 (r7), 0.9986 / 0.930 / 0.044 (r10) |
| HF f32 vs llama.cpp | 0.99978 / 0.9985 / 0.021 | 0.99410 / 0.973 / 0.103 | 0.98533 / 0.742 / 0.150 | **0.9873** / 0.770 / 0.142 (r7), 0.9887 (r10) |
| HF f32 vs Psionic | 0.99981 / 0.9987 / 0.019 | 0.99476 / 0.971 / 0.098 | 0.98696 / 0.826 / 0.143 | **0.9893** / 0.855 / 0.132 (r7), 0.9900 (r10) |

What this shows:

- On the same GGUF, Psionic and llama.cpp agree to a final mean cosine of
  0.9986. The drift grows smoothly with depth (normalized RMSE 0.008 at layer
  0, 0.05 at layer 30). No layer jumps, which is what different activation
  rounding looks like (llama.cpp quantizes activations to Q8_1; Psionic uses
  f16/f32), not what a wrong kernel looks like.
- Against f32 weights, llama.cpp is *further* from the reference than
  Psionic (0.987 vs 0.989). The M1 figure of 0.989 is therefore Q4_K_M
  quantization noise and is shared by every Q4_K_M runtime.
- The CUDA lane reproduces the CPU lane row for row.

### What runs

- **Projections.**
  - Weights stay on the device in their GGUF layout (Q8_0 attention and SSM
    projections, Q4_K FFN, about 5 GB).
  - Each projection is dequantized to f16, on a side stream into one of two
    scratch slots, then multiplied by the chunk's f16 activations with
    cuBLAS.
  - The output projections accumulate into the f32 residual inside the GEMM.
- **Gated DeltaNet.**
  - A parallel causal conv1d carries the last three inputs between chunks.
  - The delta rule runs as a sequential scan: one warp per two value rows,
    with the state in registers.
  - Each token needs one fused reduction, because
    `o = S'q + delta (k.q)` and `k.q` comes from the prep kernel.
- **Full attention.**
  - One kernel does q/k RMSNorm, rotary (the CPU lane's cos/sin table), the
    q scale, and appends to the f16 K/V caches.
  - Scores and `P V` run as strided-batched cuBLAS GEMMs per KV group, with a
    causal softmax between them.
  - The score buffer is capped at 512 MB, so long prompts run fewer heads at
    a time instead of failing.
- **Head inputs on the device.**
  - Output RMSNorm, then the head's `hidden_norm`, then `W_mem`, so the
    1024-wide memory rows stay on the GPU.
  - Span sums of `LN(H)` accumulate on the GPU; only the span sums and the
    last row come back.
  - `token_embd` and `output` stay on the host; only rows are gathered.
- **Head.**
  - Memory attention uses the query-side form. MHA has no mask, so a head's
    score against a memory row is `(W_k,h^T q_h) . m`, and its output is
    `W_v,h (sum p m) + b_v`. Nothing is projected per memory row.
  - The softmax-weighted sums run as GEMMs over the device memory rows.
  - Every dense head product goes through the device too (head matrices
    resident, f32), batched over options and questions.
  - The head takes about 10–15 ms. On the CPU it took 55 ms.
  - The CPU lane uses the same head code, so it is faster than in M1 as well.
- **Flags.**
  - `--decision-chunk` (default 2048 on CUDA, 256 on the CPU).
  - `--decision-accumulate f16|f32` (default f16; see the precision rows).
  - `PSIONIC_CLEF_PROFILE=1` prints a per-request split.
  - The receipt adds `backend: "cuda"`, `device` and `accumulate`.

### Measured (RTX 4080, coderos-4080, idle except the resident pylon-psionic and desktop apps)

Psionic and llama.cpp b11538 were measured back to back in one session, on
localhost, with a fresh nonce per request and the median of 9 runs after
warmup. llama.cpp ran as `-ngl 99 -fa on -c 17408 -b 17408 -ub 17408`.
Details and command lines:
`crates/psionic/fixtures/clef/reports/cuda-latency-2026-10-10.json`.

| Prompt | Gate | llama.cpp b11538 | **Psionic CUDA, f16 accumulate** | Psionic CUDA, f32 accumulate |
| --- | --- | --- | --- | --- |
| 1,082 tokens | ≤ 0.20 s | 0.180 s | **0.215 s** (not met) | 0.263 s |
| 3,917 tokens | ≤ 0.65 s | 0.676 s | **0.673 s** (not met; ties llama.cpp) | 0.883 s |
| 15,511 tokens | ≤ 3.25 s | 3.415 s | **2.931 s** (met; 1.17× llama.cpp) | 3.768 s |
| 30,659 / 60,955 tokens | admitted | refused (one physical batch) | 6.5 s / 17.7 s (`--decision-max-tokens 65536`) | — |
| Process VRAM (nvidia-smi, includes the CUDA context) | ≤ 7.5 GB at 16k | 10.3 GB at a 17k batch | 7.5 GB at 16k, 9.8 GB at 61k | same |

- **Per file.** For the Coder per-file relevance request (about 1.5k
  tokens), Psionic takes about 0.28 s locally. llama.cpp takes about 0.26 s
  (its 0.35 s p50 in [clef-jev-relevance-bench.md](clef-jev-relevance-bench.md)
  includes about 0.09 s of ssh tunnel).
- **Where 16k goes** (f16):
  - about 1.15 s of GEMMs;
  - about 0.2 s of weight dequantization;
  - about 0.7 s of Gated DeltaNet scan, conv and norms;
  - about 0.45 s of attention;
  - about 15 ms of head.
- **Where 1k goes.** The dequantization, about 30–40 ms, and the
  sequential delta scan, about 25 ms, are what keep 1k above llama.cpp. Its
  int8 MMQ kernels read the quantized weights directly.

**Precision** (`clef::tests::cuda_chunks_and_cpu_agree` and the e2e fixtures):

| Check | f32 accumulate | f16 accumulate (default) |
| --- | --- | --- |
| CUDA vs CPU lane, 155-token record | max \|Δlogit\| 3.8e-4, max \|Δp\| 1.3e-5 | — |
| CUDA vs CPU lane, 40 e2e requests (104 questions) | top answer 100 %, max \|Δp\| 0.0009 | top answer 99.0 % (one near tie), median \|Δp\| 0.0006, max 0.018 |
| CUDA vs the HF f32 reference, same 40 requests | top answer 94.2 %, max \|Δp\| 0.178 | top answer 95.2 %, max \|Δp\| 0.173 (CPU lane: 94.2 %; llama.cpp: 92.6 %) |
| Chunk equivalence {whole, 2048, 512, 64}, 155 tokens | max \|Δlogit\| 6.4e-4, same argmax | 2.3e-2, same argmax |
| Chunk equivalence, 7,274 tokens | 1.8e-3, same argmax | 2.0e-2, same argmax |
| A repeat on the same chunk | bitwise identical | bitwise identical |

**Gate status.**

- **Met:**
  - 16k latency, 1.17× llama.cpp;
  - 16k memory, 7.5 GB including the context;
  - 32k and 64k admitted with no physical-batch limit;
  - bitwise repeats;
  - agreement with the CPU lane.
- **Not met:**
  - **1k latency.** 0.215 s against the 0.20 s gate (llama.cpp 0.180 s).
  - **4k latency.** 0.673 s against 0.65 s; this is a tie with llama.cpp's
    0.676 s in the same session.
  - **The 1e-3 chunk-equivalence bound as written.**
    - With f32 accumulation the bound holds on short prompts; long prompts
      reach 1.8e-3 because of the f16 K/V and per-shape GEMM rounding.
    - f16 accumulation, the speed default, gives about 2e-2 logit (about
      0.005 in p). That is an order of magnitude under the
      Q4_K_M-vs-f32 noise above, but over the bound.
    - `--decision-accumulate f32` is the strict mode.

### Round 2 (2026-10-10): staged delta scan kept, fused GEMM opt-in

Same box, same session, back to back. Each row is the median of 9 runs
with a fresh nonce, at `--decision-chunk 2048`, f16 accumulate. The two
pairs ran in the order new, old, new, old. The router set is the
production split router's three requests (main 9,535 tokens, `answer`
3,289, `cli_group` 11,830), replayed from 8 recorded router calls. They
are sent at once, as `coder::router::judge::ask` sends them, and the
time is wall time for the set (`fixtures/clef/tools`, see below).

| Build | 1k | 4k | 16k | Router set (24.7k tokens) |
| --- | --- | --- | --- | --- |
| M2 (per-warp delta scan) | 0.215 / 0.212 s | 0.683 / 0.673 s | 2.93 / 2.92 s | 4.58 / 4.60 s |
| **Staged delta scan (kept, default)** | **0.206 / 0.202 s** | **0.640 / 0.628 s** | **2.75 / 2.76 s** | **4.22 / 4.23 s** |
| Gate | ≤ 0.20 s | ≤ 0.65 s | ≤ 3.25 s | — |

- **The staged scan** (`delta_scan128_kernel`). A CTA holds 32 state rows
  of one value head. Eight lanes share a row, each holding 16 key
  entries, so a dot product takes a 3-level shuffle instead of 5. The
  token inputs come through shared memory in double-buffered tiles of 16
  tokens (`cp.async`), so the serial per-token chain no longer waits on
  global loads. The kernel alone runs 2.6–3.0× faster
  (`fixtures/clef/tools/delta_scan_bench.cu`: 0.35 vs 0.90 ms per layer
  at 1,082 tokens). Its output matches the per-warp scan to 1.5e-8, and
  it does the same per-token arithmetic, so it stays bitwise
  chunk-invariant. `PSIONIC_CLEF_SCAN=0` restores the old scan.
- **Parity of the kept build.** On the 40 e2e requests (104 questions):
  - against the M2 CUDA build: top answer 100 %, max |Δp| 0.015;
  - against the CPU lane: 99.0 %, the same near tie as before;
  - against the HF f32 reference: 95.2 %, unchanged.
- **Gates.** 4k and 16k are met. 1k is 0.202–0.206 s against 0.20 s,
  which is not met yet.
- **The fused dequantize + tensor-core GEMM** (`fused_linear_kernel`,
  `PSIONIC_CLEF_FUSED=1`, off by default). It reads Q8_0/Q4_K tiles
  straight into shared memory as f16 and uses `mma.sync`. Each output
  sums its k tiles in a fixed order, and f16 partials over 16–256 k are
  promoted to f32 (`PSIONIC_CLEF_SEGMENT`).
  - Against an f32-accumulate cuBLAS reference
    (`fixtures/clef/tools/fused_linear_bench.cu`), the error is
    4–8e-4 relative RMS. Dequantize + cuBLAS f16 accumulate has a max
    error of about 1.
  - The f32 variant matches cuBLAS f32 bitwise.
  - Every variant is bitwise chunk-invariant (64-row chunks against
    whole).
  - It is not faster. It runs 95–110 TF against 120–137 TF for
    dequantize + cuBLAS f16, so end to end it lost about 5 % at 4k and
    16k. It stays opt-in until it is tuned. It is the route to the
    chunk-invariance bound at f16 speed: it removes the 2e-2 logit
    movement that comes from cuBLAS's per-shape f16 accumulation.
- **Server.**
  - A decision log line on stderr gives sizes and times only: tokens,
    questions, time waited for the device, and time run.
    `PSIONIC_CLEF_LOG=0` turns it off.
  - An exact logit cache keeps 256 prompts, keyed on the prompt ids
    and spans. A repeat is bitwise identical, so a repeated prompt is
    answered without the device.

**Round 2b: the fused kernel for short chunks (kept, default).** The
dequantize pass writes a full f16 copy of the weights (about 14 GB of
traffic) whatever the chunk size, so short prompts are bound by weight
reads. The fused kernel reads the 5 GB of quantized weights once.
`PSIONIC_CLEF_FUSED` (default `1024`) sends chunks of up to 1,024 tokens
through it and longer ones through dequantize + cuBLAS. Same session,
chunk 2048, median of 9, with llama.cpp b11538 last:

| Build | 175 tokens | 1k | 4k | 16k | Router set |
| --- | --- | --- | --- | --- | --- |
| cuBLAS only (`PSIONIC_CLEF_FUSED=0`), two runs | 0.078 / 0.077 s | 0.199 / 0.200 s | 0.615 / 0.627 s | 2.78 / 2.73 s | 4.39 / 4.23 s |
| **Fused up to 1,024 tokens (default)**, two runs | **0.058 / 0.058 s** | **0.201 / 0.200 s** | **0.625 / 0.626 s** | **2.71 / 2.70 s** | **4.21 / 4.22 s** |
| Fused up to 2,048 tokens | 0.056 s | 0.207 s | 0.711 s | 3.03 s | 4.73 s |
| llama.cpp b11538 | 0.040 s | 0.181 s | 0.684 s | 3.417 s | — |

- **Parity of the default build** (40 e2e requests). The short records
  now take the fused kernel.
  - Against the CPU lane: top answer 100 %, max |Δp| 0.0036. The
    cuBLAS f16 path gave 99.0 % and 0.018.
  - Against the HF f32 reference: 94.2 %, the CPU lane's own figure.
- **M2 gates in this session.**
  - 4k is 0.63 s against 0.65 s, which is met and ahead of llama.cpp's
    0.68 s.
  - 16k is 2.70 s against 3.25 s, which is met: 1.27× llama.cpp.
  - 1k is 0.200 s against 0.20 s, met only at the line. llama.cpp is
    0.181 s.
- **What is left at 1k.** About 18 ms of fixed cost per request beyond
  llama.cpp: the head, the host embedding gather, and HTTP. Compare
  0.058 s with 0.040 s at 175 tokens.

**Chunk invariance (2026-10-10).** This is
`clef::tests::cuda_chunks_and_cpu_agree`: the max |Δlogit| of chunks
{2048, 512, 64} against the whole prompt, on records of 155 and 7,274
tokens.

| Mode | 155 tokens | 7,274 tokens |
| --- | --- | --- |
| f16, cuBLAS projections (the M2 default) | 2.4e-2 | 2.7e-2 |
| f16, fused up to 1,024 tokens (the default now) | 2.2e-3 | 2.2e-2 |
| f16, fused everywhere (`PSIONIC_CLEF_FUSED=1`) | 2.2e-3 | 3.7e-3 |
| f32 (`--decision-accumulate f32`) | 1.2e-3 | 1.7e-3 |

- The fused projections are bitwise chunk-invariant. They take the f16
  movement from 2e-2 to 2–4e-3.
- What is left comes from attention: its cuBLAS score and `P V` GEMMs
  differ by shape, and the probabilities are f16. The `W_mem` GEMM
  differs by shape too.
- A flash-attention kernel with key tiles fixed at absolute positions,
  plus a fixed-order `W_mem` product, would make the whole trunk bitwise
  invariant. That is the next step.
- The 1e-3 bound is not met as written in any mode yet. The test's 1e-3
  short-prompt bound for f32 fails at 1.1–1.2e-3 with every projection
  path, so the test stays red until attention is fixed-order.

**Per-file relevance latency (measured).** This is
`scripts/bench/clef-relevance-bench.py --mode seq --warmup 3`, run on
coderos against the deployed `pylon-clef` (`7af29040e6`, chunk 2048) on
localhost. 72 requests averaged 1,532 tokens:

- latency p50 0.254 s, p90 0.284 s, p99 0.309 s;
- prefill 6.06k tokens/s;
- 3.95 decisions/s, one at a time;
- F1 0.67 at 0.5, AUC 0.85.

The M2 estimate of about 0.28 s is replaced by this measurement.

### Round 3 (2026-10-10): fixed-order flash attention (kept, default)

`flash_attention256_kernel` replaces the cuBLAS score and `P V` GEMMs.

- **Layout.** A CTA owns 16 queries and one KV head. Each of its 4 warps
  takes one of the 4 query heads that share that KV head, and keeps its
  16 × 256 queries in registers.
- **Order.** Keys stream in tiles of 16 at absolute positions, through
  double-buffered shared memory shared by the 4 heads. Each row runs the
  online softmax over the tiles in order. A tile past a row's diagonal
  is fully masked and leaves the row's state bitwise unchanged.
- **Precision.** Scores and `P V` accumulate in f32, with f16
  probabilities.
- **Kernel alone** (`fixtures/clef/tools/flash_attention_bench.cu`): 75
  TF at 4k and 87 TF at 16k, against the 4080's f32-accumulate peak of
  about 97 TF. At 16k one layer takes 25 ms. Against an f64 reference
  the error is 2.5–4e-4. Chunks of 2048, 512 and 64 are bitwise
  identical to the whole prompt.
- **`W_mem`.** It now goes through a fixed-order f32 kernel instead of a
  shape-dependent cuBLAS GEMM.
- **Knob.** `PSIONIC_CLEF_FLASH=0` restores the cuBLAS attention.

Same session, chunk 2048, median of 9, two runs each:

| Build | 175 tokens | 1k | 4k | 16k | Router set |
| --- | --- | --- | --- | --- | --- |
| cuBLAS attention | 0.057 / 0.062 s | 0.200 / 0.205 s | 0.627 / 0.672 s | 2.72 / 2.73 s | 4.21 / 4.21 s |
| **Flash attention (default)** | **0.058 / 0.056 s** | **0.198 / 0.200 s** | **0.600 / 0.605 s** | **2.50 / 2.52 s** | **3.98 / 4.02 s** |
| Flash, fused projections everywhere | 0.057 s | 0.206 s | 0.686 s | 2.83 s | 4.50 s |

**Chunk invariance now** (`cuda_chunks_and_cpu_agree`, which passes):

| Mode | 155 tokens | 7,274 tokens |
| --- | --- | --- |
| f16, default (fused up to 1,024 tokens) | 0 (bitwise) | 1.6–2.1e-2 (the 2,048-token and whole chunks run cuBLAS) |
| f32 (`--decision-accumulate f32`) | 0 | 7.3e-4 (meets 1e-3) |
| f16 or f32 with fused projections everywhere (`PSIONIC_CLEF_FUSED=1`) | 0 | **0 (bitwise, every chunk size)** |

**Parity.** On the 40 e2e requests the CUDA lane agrees with the CPU lane
on every top answer: max |Δp| 0.0045, median 0.0002. Against f32 it is
94.2 %, the CPU lane's own figure.

The trunk can now be bitwise chunk-invariant, but the default isn't
yet: fused projections everywhere cost 14 % at 4k and miss the 4k gate.
Making the fused kernel as fast as cuBLAS on large chunks closes this.

### Round 4 (2026-10-10): host work off the critical path, and the M2 gate

- **Host work overlapped with the device.**
  - The next chunk's embedding rows and rotary table are built on a
    second thread while the device runs the current chunk.
  - The head's lexical vectors (option spans' output-embedding means)
    are built on a thread during the prefill.
  - Results are bitwise unchanged.
- **The head's memory attentions run on the device.** The per-head
  `W_k^T q` and `W_v z + b` products used to run on the host; now they
  bracket the device attention in one round trip
  (`head_side_kernel`, `head_context_kernel`).
  - The head drops from 10 to 5 ms at 1k, and from 30/22 to 15 ms on
    router requests.
  - Answers move by at most 1.2e-7 in p.
- **Small-M kernel, tried and dropped.** A separate kernel for the
  32-row `ssm_alpha`/`ssm_beta` projections was slower than the 128-row
  fused tile.
- **Deployed** as `pylon-clef` `a9ab2671be`.

**M2 gate session.** RTX 4080, the deployed build on a side port with
`pylon-clef` stopped, chunk 2048. Psionic and llama.cpp b11538 were
interleaved for three rounds, each the median of 9 with a fresh nonce:

| Prompt | Gate | Psionic, rounds 1 / 2 / 3 | llama.cpp b11538, rounds 1 / 2 / 3 |
| --- | --- | --- | --- |
| 1,082 tokens | ≤ 0.20 s | **0.193 / 0.194 / 0.194 s** | 0.182 / 0.182 / 0.184 s |
| 3,917 tokens | ≤ 0.65 s | **0.596 / 0.599 / 0.595 s** | 0.684 / 0.687 / 0.680 s |
| 15,511 tokens | ≤ 3.25 s | **2.48 / 2.49 / 2.47 s** | 3.47 / 3.45 / 3.46 s |

All three latency gates are met in every round. 4k runs 1.15× and 16k
1.40× llama.cpp; 1k stays 6 % behind it.

**Per-file relevance (measured, deployed build).**
`scripts/bench/clef-relevance-bench.py --mode seq` on localhost, 72
requests averaging 1,530 tokens:

- p50 0.250 s, p90 0.277 s, p99 0.299 s;
- 6.2k prefill tokens/s;
- 4.05 decisions/s.

**The chunk-invariance bound, restated.** The bound as written was a
1e-3 logit bound across chunk sizes in the default mode. It becomes:

1. **Strict mode** (`--decision-accumulate f32`) meets 1e-3: 7.3e-4
   over chunks {whole, 2048, 512, 64} on a 7,274-token record, and 0 on
   a 155-token one.
2. **Invariant mode** (`PSIONIC_CLEF_FUSED=1`, f16 or f32) is bitwise
   chunk-invariant: every chunk size gives identical logits. It costs
   14 % at 4k (0.686 s, over the 4k gate), so it is not the default.
3. **The default (speed) mode** is bitwise invariant for prompts of up
   to 1,024 tokens, where every chunk takes the fused projections.
   Longer prompts move up to 2.2e-2 in logit, with the same top answer
   for every chunk size.

That movement comes only from cuBLAS's shape-dependent f16-accumulation
order in chunks over 1,024 tokens. It is about 0.005 in p. That is a
twentieth of the Q4_K_M weight noise every runtime shares: max |Δp|
0.18 against the f32 model.

A fused GEMM as fast as cuBLAS would make the default mode bitwise too.
That is follow-up work, not an M2 blocker.

**M2 is done.**

**The router set on one 4080.** The set is 24.7k tokens per chat turn.
The three requests queue on the one device, so they run one after
another: 4.2 s with this build, and 5.0 s on the deployed chunk 1,024.

The under-1 s target cannot be met by prefill speed on this card. The
GEMMs alone at 24.7k tokens are about 340 TFLOP. That is 1.8 s at the
4080's peak f16-accumulate rate.

Prefix reuse does not help this shape. Clef's prompt is the system
line, then the state, then the schema, and the state is 30–50 tokens of
the turn's message. Everything after it is the 24k-token schema, and
causal attention makes the schema's rows depend on the state. No
reusable prefix is longer than about 60 tokens.

What would meet it is fewer tokens on the router side: the `route`
question's 22 options take 5.1k tokens and `cli_group`'s 55 options take
11.8k, about 230 tokens per option. Two ways to cut them:

- ask `cli_group` (and `answer`) only when the route needs them;
- shorten the option descriptions for the Clef door.

**Router quality** (web chat goldens, router mode, 120 cases, same hour):

| Judge | Pass | Right ignoring time | Route argmax right | Route top p (p10 / p50 / p90) | Judge time p50 / p90 |
| --- | --- | --- | --- | --- | --- |
| Hosted Jev | 84 | 94 | 109 | 0.51 / 0.96 / 1.00 | 1.05 / 6.0 s (12 timeouts) |
| Clef-Flash (pylon-clef, M2 build, chunk 1,024) | 0 | 40 | 102 | 0.18 / 0.30 / 0.50 | 6.6 / 7.7 s |

Clef picks the right route almost as often as Jev, at 102 of 120. Its
probabilities are about three times lower, though, so the Jev-tuned
thresholds turn most turns away from the prepared answers.

**A Clef calibration map** (`crates/coder/fixtures/chat-router/calibration-clef-flash-v1.json`).
The published router eval (`ROUTER_EVAL_PUBLISH=1`) ran against the
deployed Clef server: 678 labeled rows, the split router, 0 errors. It
fitted both maps on the calibration partition (415 rows), and the
`probability-v2` gate passed both on the held-out split (261 rows):

| Question | ECE raw → mapped | Brier | NLL |
| --- | --- | --- | --- |
| `route` | 0.379 → 0.030 | 0.330 → 0.182 | 0.875 → 0.545 |
| `answer` | 0.094 → 0.052 | 0.168 → 0.162 | 0.511 → 0.489 |

The worker and `chat-goldens router` now choose the map by the model that
answered (`router::calibration::response_from_clef`). A Pylon's answer
names `clef-flash@sha256:…`, and a Psionic server's answer carries its
`psionic` block. Jev's answers keep `calibration-v2`. The goldens' router
mode used to read raw probabilities; it now applies the maps as the
worker does, so its Jev numbers below match production.
`ROUTER_EVAL_READINGS` replays a run's readings file to refit without
asking the judge again.

| Judge (goldens router mode, 123 cases, maps applied) | Pass | Right ignoring time | Route right | Answer right | Critical-flow wrong | Route top p (p10 / p50 / p90) | Judge p50 / p90 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Hosted Jev (`calibration-v2`) | 106 | 108 | 120 | 108 | 3 | 0.52 / 0.95 / 1.00 | 0.89 / 1.13 s |
| Clef-Flash, raw | 0 | 45 | 111 | 46 | 32 | 0.18 / 0.30 / 0.49 | 6.0 / 6.5 s |
| Clef-Flash, Clef map | 1 | 55 | 110 | 56 | 22 | 0.41 / 0.82 / 0.98 | 5.8 / 6.1 s |

With its own map, Clef reads on Jev's scale. Its route confidence median
goes from 0.30 to 0.82, and the critical-flow errors fall from 32 to 22.
The `answer` question is still the gap: 56 right against Jev's 108. That
gap is accuracy, not calibration, since the mapped probabilities are
honest. Clef-Flash on one 4080 therefore meets neither half of the router
gate (under 1 s, and at least Jev's golden score). Clef 27B would not fit
in the 4080's 16 GB beside the trunk's caches. Jev stays the router's
first door, with the Pylon as fallback and shadow.

**Router tokens, stage A (#11247, 2026-10-11).** Three changes, measured
with the web chat goldens on Jev and the routes-v5 eval:

1. **Calibration per question.** The split router merged its requests and
   kept only the main response's model, so a side question a fallback door
   answered (Clef beside Jev, or the reverse) was read on the wrong map.
   `router::merge` now keeps each side question's serving identity
   (`served_by`), and `router::calibration::apply_served` maps `route` by
   the main request's model and `answer` by its own.
2. **Only the questions a rule reads.** `cli_group` (55 options, 11.8k
   tokens) is asked after the main request, and only when its `route`
   argmax is `cli`, or `wallet` on a terminal (rule 3a's wallet descent),
   within what is left of `first::LATE` (`router::judge::needed_after`).
   A turn does not ask what no rule reads on its surface
   (`router::judge::unread_on`): `fanout`, `read_only`, and `summarize`
   only shape a terminal's multi-run plan, and on the website `engine`,
   `capability`, and `tool` only shape offers and cards the website never
   shows. `answer` stays beside the main request: a rule reads it on most
   routes, so waiting for the route would cost most turns a round trip.
3. **Compact option texts: tried, not shipped.** Cutting the `route`
   rubrics to about 60 tokens an option, and then to about 100, cost Jev
   accuracy on the routes-v5 held-out and calibration rows (678, asked the
   same hour; 39–41 rows in each run were lost to a TypeSafe 402):

   | Wording | Route accuracy | Canned precision / coverage | Dispatch precision | Gym recall | Refusal recall | Route map |
   | --- | --- | --- | --- | --- | --- | --- |
   | Shipped (unchanged) | 91.5 % | 98.9 % / 66.9 % | 98.0 % | 88.9 % | 91.3 % | passes |
   | ~100 tokens an option | 89.7 % | 97.0 % / 60.8 % | 94.0 % | 66.7 % | 82.6 % | fails |
   | ~60 tokens an option | 77.9 % | 96.5 % / 62.2 % | 85.5 % | 55.6 % | 87.0 % | fails |

   The long `not_for` lists and examples carry boundaries Jev uses, so the
   wording stays; the token target moves to stage B (a route-specific
   answer shortlist and hierarchical route classification, keeping
   `none`, risk, and the no-match options).

| Turn | Main request | Beside it (`answer`) | Clef tokens |
| --- | --- | --- | --- |
| Before (every surface) | 37.3 KB, 9,535 | 12.7 KB, 3,289, and `cli_group` 11,830 | 24.7k |
| Website | 27.8 KB, ~7.1k (est.) | 12.9 KB, 3,722 (measured) | ~10.8k |
| Phone or desktop | 33.3 KB, ~8.5k (est.) | 12.7 KB, ~3.3k | ~11.8k |
| Terminal | 37.3 KB, 9,535 | 12.7 KB, 3,289 | 12.8k |

A `cli` turn adds the `cli_group` request after the main one. The
estimates use the split's measured 3.9 bytes per token; the Clef server
counted the website's `answer` request (3,722 tokens), and its main request
could not be counted then (the 4080 was full with a second Clef server
benchmarking, and the request ran out of memory).
`router::judge::tests::a_typical_turn_fits_the_router_budget` holds each
surface to its measured size.

Web chat goldens, router mode, Jev (`calibration-v2`), 123 cases:

| Run | Pass | Right (bar) | Critical wrong | Judge p50 / p90 |
| --- | --- | --- | --- | --- |
| `main` `4a29a77d7b`, run 1 | 102 | 85 % | 6 | 0.51 / 0.62 s |
| `main` `4a29a77d7b`, run 2 | 100 | 82 % | 7 | 0.50 / 0.59 s |
| Stage A, run 1 | 101 | 83 % | 6 | 0.47 / 0.77 s |
| Stage A, run 2 | 101 | 82 % | 7 | 0.40 / 0.48 s |

The same critical cases fail before and after (`github.connect_repo#2/#3`,
`coder.login#3`, `coder.sync#1`, `pricing.cost#1/#3`, and in one run of
each `privacy.training#3`): a route or answer probability under its bar, the same
on `main`. Jev holds. Clef on the 4080 was not re-measured here.

**Router tokens, stage B (#11247, 2026-10-11): staged routing, opt-in.**
`CODER_ROUTER_STAGED=on` asks a turn in two requests
(`router::judge::Split::staged`). The first holds every main question, with
a coarse `route_hint` in place of `route`: each route by its one-line
description plus its rubric's examples, about 110 tokens an option. Its
probabilities pick a shortlist (the likeliest routes until they and `none`
hold 0.99, three to six of them, `router::judge::shortlist`). The second
asks `route` with the shortlisted routes at full rubric and every other
route by its one-line description, and `answer` over the shortlisted
routes' bank entries. `cli_group` follows only when that `route` reads it.
It is off by default: on Jev it saves about 15 % of the tokens for a second
round trip.

What holds Jev's accuracy, measured on routes-v5 held_out + calibration
(678 rows, Jev at TypeSafe, flat and staged runs in the same hour):

| Shape | Route acc. | Canned prec. / cov. | Dispatch prec. | Gym recall | Refusal recall | Jev tokens a turn | Judge p50 / p95 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Flat split (default), run 1 | 90.1 % | 99.0 / 71.9 % | 99.0 % | 86.1 % | 91.3 % | 13,986 | 236 / 573 ms |
| Flat split (default), run 2 | 90.6 % | 99.0 / 71.2 % | 98.0 % | 86.1 % | 91.3 % | 14,010 | 257 / 603 ms |
| Hint by description; shortlist only | 85.8 % | 99.5 / 67.3 % | 95.0 % | 69.4 % | 91.3 % | 8,887 | 368 / 739 ms |
| Hint with examples; shortlist only | 90.3 % | 99.0 / 69.8 % | 97.9 % | 71.3 % | 91.3 % | 10,964 | 493 / 1,043 ms |
| **Hint with examples; other routes described (shipped, opt-in)** | **90.1 %** | **99.0 / 72.7 %** | **97.0 %** | **85.2 %** | **91.3 %** | **11,834** | **520 / 1,150 ms** |
| The same, hint by description | 86.9 % | 99.5 / 70.1 % | 94.2 % | 80.6 % | 91.3 % | 10,183 | 515 / 1,120 ms |
| The same, second request's rubrics without examples | 85.1 % | 99.0 / 68.0 % | 89.0 % | 61.1 % | 91.3 % | 11,466 | 447 / 1,007 ms |

- **The hint needs the examples.** Asked alone on the same rows, the
  labeled route was among the shortlist (three or more routes at 0.95 /
  0.99) on 94.1 / 95.1 % of rows with descriptions only, 97.2 % with the
  rubrics' `what`, 99.4 / 99.6 % with description and examples (top-1
  88.9 %), and 99.3 / 99.6 % with the full rubrics (top-1 90.3 %, at 2.2
  times the bytes). Description plus two or four examples was no better
  than none (94.7 %, 95.1 %).
- **A shortlist alone lowers Jev's probabilities.** Asking `route` over
  three or four routes kept its argmax (route accuracy held) but lowered a
  sure route's probability: on 155 rows of the routes that dropped, 0.880
  over all routes, 0.845 over the shortlist, 0.822 with the others as bare
  names, and 0.876 with the others described. Per route, `eval.run` fell
  from 0.94 to 0.81 and `wallet` from 0.91 to 0.80, under the Gym and
  canned floors; so Gym recall fell to 71 %. Keeping every route as an
  option, the others by description, restores the scale. Jev itself moves
  by 0.03–0.05 on an identical request, so single rows are noise.
- **Dispatch.** The shipped shape made 3 wrong dispatch offers in 101 (the
  flat runs 1 in 97 and 2 in 99): `meta/034`, which flat run 2 also made,
  `cli/004`, and `eval.author/025`.

Web chat goldens, router mode, Jev (`calibration-v2`), 123 cases, product
notes on:

| Shape | Pass | Right (bar 90 %) | Critical wrong | Slow | Judge p50 / p90 |
| --- | --- | --- | --- | --- | --- |
| Flat split | 106 | 88 % | 3 | 2 | 0.99 / 1.37 s |
| Staged | 101 | 87 % | 3 (the same) | 6 | 1.18 / 1.64 s |

The same critical cases fail in both (`github.connect_repo#1/#4`,
`coder.login#3`); the staged run is right on one case fewer and slow on
four more, from its second round trip.

**Clef on the CoderOS-4080** (the production `pylon-clef`, `a9ab2671be`,
asked on localhost while it was otherwise idle, a fresh nonce per turn,
the median of three; a "What models do you use?" turn, whose shortlist,
`meta`, `product.kb`, and `general`, carries the most bank entries):

| Turn | Flat: tokens (main + `answer`, at once) | Flat | Staged: tokens (first + second) | Staged |
| --- | --- | --- | --- | --- |
| Website | 6,958 + 3,745 = 10,703 | 1.63 s | 4,340 + 5,102 = 9,442 | 1.45 s |
| Phone | 8,463 + 3,709 = 12,172 | 1.86 s | 5,845 + 5,087 = 10,932 | 1.67 s |

Other shortlists, once each, website / phone: `smalltalk` 7,147 tokens,
1.16 / 1.32 s; `work.dispatch` 6,943, 1.08 / 1.30 s; Gym 6,118, 0.93 / 1.15
s; `wallet` 7,741, 1.17 / 1.39 s. A staged website turn on Clef sends 6.1k–
9.4k tokens and takes 0.9–1.5 s, where the flat split sends 10.7k and takes
1.6 s. The first request alone is 4,340 tokens (0.65 s) on the website, of
which the hint is about 2.6k, so a turn under 1 s needs a cheaper first
stage; the Clef map (`calibration-clef-flash-v1`) was fitted on the flat
shape and was not refitted. Not done here, and in #11247's follow-up: the
dispatch and Gym questions (`capability`, `engine`, `fanout`, `read_only`,
`summarize`, `tool`; 5.5 KB on the phone, 9 KB on a terminal) asked in the
second request only when such a tier may be served, a routes-v5 run and
map refit on Clef, and the sub-second first stage.

TypeSafe ran out of credits (402) during the last runs, and so did the
Vercel AI Gateway; Jev through OpenRouter (`typesafe/jev-1.13`) is not on
the same scale: the flat split there scored route 75.0 %, canned coverage
11.5 %, and Gym recall 0.9 % (18 of 678 rows failed), so it cannot stand in
for the bar.

**Router tokens, stage C (#11297, 2026-10-11).** Three changes, measured
on the CoderOS-4080's production `pylon-clef` (`a9ab2671be`, asked on
localhost at night, one request at a time) and, for #11298, on Jev
through OpenRouter. TypeSafe was out of credits (402), so no Jev run at
TypeSafe's door is in this section; the commands for those are in the
issue's closing comment and `NEEDS_OWNER.md`.

1. **The dispatch and Gym questions only when read.** A staged turn takes
   `capability`, `engine`, `fanout`, `read_only`, `summarize` and `tool`
   out of its first request (`router::judge::DEFERRED`). The second request
   asks the dispatch ones only when the shortlist holds `work.dispatch` or
   `capability.missing` or the first request's `lane` argmax is
   `computer` (every dispatch rule in `router::policy` needs one of these),
   and `tool` only when it holds a Gym route
   (`router::judge::deferred_needed`). The first request is now the same
   on every surface: 16.2 KB, 4,348 Clef tokens, 0.68 s (it was 5,845
   tokens on the phone and about 7,000 on a terminal).
2. **Clef's map for the staged shape**
   (`crates/coder/fixtures/chat-router/calibration-clef-flash-staged-v1.json`).
   The published eval (`ROUTER_EVAL_PUBLISH=1`, `CODER_ROUTER_STAGED=on`)
   ran against the Clef server: 678 rows, 0 errors. Fitted on the
   calibration partition (414 rows) and gated on the held-out split:
   `route` ECE 0.398 → 0.025, `answer` 0.121 → 0.062; `probability-v2`
   passed both. `Calibration::clef_from_env` serves it when turns are
   staged, and the flat map (`calibration-clef-flash-v1`) otherwise.
3. **Staged by default for a Clef-first judge.** `CODER_ROUTER_STAGED` is
   `on` or `off`; unset, turns are staged when `OPENAGENTS_DECISIONS=pylon`
   (the judge asks the Pylons, a Clef door, first) and flat otherwise
   (`router::judge::staged_by_default`). The Jev-first worker is
   unchanged.

Clef, routes-v5 held-out rows (261), each answer read on Clef's map for
its shape (`ROUTER_EVAL_MAPS=on`), the same night:

| Shape | Route acc. | Canned prec. / cov. | Dispatch prec. | Gym recall | Refusal recall | Clef tokens a turn (678 rows) | Turn p50 / p95 (eval) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Flat, `calibration-clef-flash-v1` | 69.1 % | 94.6 / 33.3 % | 93.8 % | 43.2 % | 40.0 % | 12,271 | 1.91 / 1.98 s |
| Staged with step 1, staged map | **77.2 %** | 87.8 / 36.9 % | **96.4 %** | **81.1 %** | **70.0 %** | **10,583** | **1.75 / 1.92 s** |

On all 678 rows (the calibration partition is the maps' own fit rows):
flat 73.3 % route, 91.6 / 42.8 % canned, 94.7 % dispatch, 58.3 % Gym
recall, 56.5 % refusal recall; staged 81.0 %, 89.4 / 47.5 %, 96.9 %,
86.1 %, 78.3 %. On Clef the staged turn is better on every count but
canned precision, and neither shape meets Jev's bar: canned precision
under 98 % is Clef's `answer` accuracy, which a map does not fix (#11195).

**Under 1 s is not reached by the request's shape.** Each request's time
on Clef, with a fresh nonce, median of three:

| Request | Clef tokens | Time |
| --- | --- | --- |
| Staged first request (any surface) | 4,348 | 0.68 s |
| Flat main request, website / phone / terminal | 6,964 / 8,473 / 9,545 | 1.11 / 1.34 / 1.50 s |
| Flat `answer` beside it (phone) | 3,718 | 0.59 s |
| Second request, three-route shortlists: Gym / `smalltalk` / dispatch / "What models do you use?" | 1,786 / 2,814 / 4,099 / 5,096 | 0.30 / 0.46 / 0.66 / 0.82 s |

- A staged turn with a three-route shortlist takes 1.0–1.5 s on Clef. But
  Clef's hint probabilities are flat, so its shortlist never reaches 0.99
  of the mass and always holds six routes: 10.6k tokens and 1.75 s a turn
  in the eval.
- The hint carries the shortlist on Clef as on Jev. Asked alone on the
  678 rows, the labeled route was among the top 3 / 4 / 6 hint routes on
  94.4 / 97.6 / 99.0 % of rows with descriptions and examples (9,965
  bytes, 0.45 s), 85.3 / 89.7 / 94.4 % with descriptions only (4,673
  bytes, 0.26 s), and 94.7 / 96.5 / 98.1 % with each rubric's `what`
  (8,998 bytes). So the examples stay.
- The first request is 0.68 s by itself, and the smallest useful second
  request (three Gym routes) is 0.30 s. A sub-second turn on this card
  would cost the shortlist's accuracy. The levers left are the engine and
  the card: the H100 runs the same prefill 2.6–3 times faster (below),
  which puts a 10.6k-token staged turn near 0.6 s.

**Jev through OpenRouter (#11298).** Measured again the same night
through the chat worker's legacy failover (`OPENAGENTS_DECISIONS=legacy`,
TypeSafe benched at 402, OpenRouter's `typesafe/jev-1.13`, which answers
as `typesafe/jev-1.13-20260917`), the flat split on routes-v5's 678 rows:
route 90.4 %, canned 98.5 / 73.4 %, dispatch 98.0 %, Gym recall 88.9 %,
refusal recall 91.3 %, p50 277 ms, 0 errors, under $0.41 of OpenRouter
credit. That is TypeSafe's scale: refitting the maps on these readings
gives raw ECE 0.044 (`route`) and 0.101 (`answer`), where TypeSafe's run
behind `calibration-v2` had 0.048 and 0.105. The #11247 reading (route
75.0 %, canned coverage 11.5 %, Gym recall 0.9 %, p50 2.6 s, 18 errors)
was taken while TypeSafe and the Vercel AI Gateway both answered 402 and
did not repeat; OpenRouter stays in the legacy order on `calibration-v2`.

**After M2:** make the fused GEMM as fast as cuBLAS on large chunks
(it reaches about 130 TF before dequantization against cuBLAS's 145–170),
then make it the default for bitwise chunk invariance at f16 speed.

## H100 (2026-10-10)

A one-off benchmark on a plain (not confidential) spot `a3-highgpu-1g` in
`us-east5-a`: one NVIDIA H100 80GB HBM3, driver 580.178.04, the Deep
Learning VM image `common-cu129-ubuntu-2404-nvidia-580`. Psionic was built
on the VM from `origin/main` (`3ed7bf2a63`) with nvcc 12.9 and
`PSI_CUDA_ARCH=90`. It ran as `psionic-openai-server --decision-device cuda
--decision-chunk 2048 --decision-max-tokens 65536`, with f16 accumulate, the
default. llama.cpp was the b11538 CUDA 12.8 release, the build used above, at
`-ngl 99 -np 1 -fa on`, with `-c/-b/-ub 17408` (and 32768 for the 32k
prompt). Nothing else ran on the GPU. The VM was deleted after the run.
Details are in `crates/psionic/fixtures/clef/reports/h100-2026-10-10.json`.

**Latency.** These are the same prompts as the 4080 gate session
(`len{1k,4k,16k,32k}.json`), measured with `bench_latency.py`: the median
of 9 runs, a fresh nonce each run. Psionic and llama.cpp were interleaved
for three rounds; the cells below are rounds 1 / 2 / 3.

| Prompt | Psionic, H100 | llama.cpp b11538, H100 | H100 speedup | Psionic, 4080 (gate session) | llama.cpp, 4080 |
| --- | --- | --- | --- | --- | --- |
| 1,082 tokens | **0.074 / 0.077 / 0.073 s** | 0.135 / 0.136 / 0.138 s | 1.8× | 0.194 s | 0.182 s |
| 3,917 tokens | **0.210 / 0.215 / 0.209 s** | 0.462 / 0.464 / 0.464 s | 2.2× | 0.596 s | 0.684 s |
| 15,511 tokens | **0.843 / 0.851 / 0.842 s** | 2.218 / 2.220 / 2.219 s | 2.6× | 2.48 s | 3.46 s |
| 30,659 tokens | **1.78 / 1.79 / 1.79 s** | 5.54 / 5.53 / 5.53 s | 3.1× | 6.5 s | refused (one physical batch) |
| Process VRAM (nvidia-smi) | 8.6 GB after 32k | 10.7 GB (17k batch), 15.5 GB (32k batch) | | 7.5 GB at 16k | 10.3 GB |

- Psionic's runs are tight: every one of the 108 runs is within 10 % of
  its median.
- llama.cpp is bimodal on this card. 1 to 3 of every 9 runs take
  3.6–4.9× its median: for example 0.64 s against 0.136 s at 1k, 9.4 s
  against 2.22 s at 16k, and 20 s against 5.5 s at 32k. The medians above
  are its fast mode.
- At 16k the H100 runs Psionic 2.9× faster than the 4080 does. That puts
  every M2 latency gate (0.20 / 0.65 / 3.25 s) about 3–4× clear.

**Per-file relevance.** `scripts/bench/clef-relevance-bench.py --mode seq
--warmup 3`, on the same 72-request dataset (about 1,534 tokens each), on
localhost:

| | Psionic, H100 | llama.cpp, H100 | Psionic, 4080 (deployed) |
| --- | --- | --- | --- |
| p50 / p90 / p99 | **0.109 / 0.117 / 0.130 s** | 0.186 / 0.206 / 1.07 s | 0.250 / 0.277 / 0.299 s |
| Decisions/s, one at a time | **9.33** | 4.40 | 4.05 |
| Prefill tok/s (median) | 14.5k | 8.5k | 6.2k |
| F1 at 0.5 / AUC | 0.67 / 0.84 | — | 0.67 / 0.85 |

**Concurrency.** The same per-file requests (144 of them, two rounds),
sent from N clients at once. Decisions per second, then p50 / p90
latency per decision:

| Clients | Psionic, H100 | llama.cpp `-np 1` | llama.cpp `-np 4` |
| --- | --- | --- | --- |
| 1 | **9.4/s**, 0.108 / 0.116 s | 4.1/s, 0.187 / 0.206 s | — |
| 4 | **9.5/s**, 0.42 / 0.46 s | 5.7/s, 0.71 / 0.79 s | 5.0/s, 0.35 / 0.40 s |
| 16 | **9.4/s**, 0.95 / 3.4 s (1,867 busy retries) | 5.7/s, 2.8 / 3.0 s | 5.7/s, 2.3 / 2.5 s |
| 32 | **8.9/s**, 1.0 / 8.8 s (5,817 busy retries) | 5.7/s, 5.6 / 6.0 s | 5.7/s, 5.0 / 5.5 s |

- Psionic runs one decision at a time, so its throughput stays flat at
  about 9.4/s however many clients send.
- It also refuses with `503 busy` once more than 8 decisions are waiting
  (`RequestLimits::max_queue`, fixed at 8 and not a flag). With 16 or 32
  clients and no retry, 135 of the 144 requests were refused.
- The 16- and 32-client rows above are from a client that retries a 503
  after 50 ms. Their p90 is the retry wait.
- Batching several prompts into one prefill (M4) is what would raise
  throughput beyond 9.4/s. A queue-depth flag is a one-line follow-up.
  Both landed in #11441 (`PSIONIC_CLEF_BATCH_REQUESTS`,
  `--decision-max-queue`); see Round 7 below for what they buy on this card.

**The router's three-request replay.** `router_set.py` replays 8
recorded router calls (main 9,535, `answer` 3,289 and `cli_group` 11,830
tokens), each sent as three requests at once. Every pass ran on a newly
started server, so the exact logit cache was empty. (A second pass on the
same server answers from the cache in 7 ms.)

| | H100 | 4080 |
| --- | --- | --- |
| Psionic Clef-Flash, three passes | **1.40 / 1.38 / 1.37 s** | 3.98–4.22 s |
| llama.cpp Clef-Flash (`-np 1`) | 3.18 s p50, 8.7 s p90 (its slow mode) | — |
| Psionic Clef 27B, two passes | 4.00 / 3.99 s | does not fit |

On Flash the H100 takes the set from about 4.2 s to 1.4 s. That is
still over the router's under-1 s target. The three requests queue on one
device, and together they hold 24.7k tokens.

**Parity** (the 40 e2e requests, 104 questions; `e2e.py compare`):

| H100 CUDA lane against | Top answer | max \|Δp\| | median \|Δp\| |
| --- | --- | --- | --- |
| Psionic CPU lane | **100 %** | 0.0022 | 0.0001 |
| the 4080 M2 CUDA run (`psionic-cuda-f16-q4_k_m.jsonl`) | 99.0 % (the known `password_3` near tie, 0.218 vs 0.220) | 0.017 | 0.0007 |
| HF f32 reference | 94.2 % (the CPU lane's own figure) | 0.177 | 0.013 |

**Clef 27B** (`ggml-org/Clef-GGUF` `Clef-Q4_K_M.gguf`, 19.2 GB).

- **It fits.** Psionic's CUDA lane loads it unchanged and holds 18.1 GB
  after load and 21.8 GB after a 32k prompt. llama.cpp holds 24.2 GB at
  a 17k batch and 30.2 GB at a 32k batch.
- **It agrees with llama.cpp.** Psionic and llama.cpp 27B give the same
  top answer on 92.6 % of 95 e2e questions, with median |Δp| 0.008 and
  max 0.16. llama.cpp refused 3 of the 40 requests with a 400.

Clef 27B latency, median of 9 with a fresh nonce:

| Prompt | Psionic 27B, H100 | llama.cpp 27B, H100 | Speedup |
| --- | --- | --- | --- |
| 1k | **0.212 s** | 0.396 s | 1.9× |
| 4k | **0.611 s** | 1.359 s | 2.2× |
| 16k | **2.61 s** | 6.39 s | 2.4× |
| 32k | **6.00 s** | 15.5 s | 2.6× |

**Router goldens with 27B.** `chat-goldens router` (123 cases) ran
against each server through `CODER_DECISION_PROFILE=direct_local`, in the
same hour, with and without the Clef calibration map.

- **Product notes were off.** No embeddings key was on the VM, so every
  knowledge turn reports tier `grounded`. That caps Pass and the
  served-answer checks for every judge, which is why Pass and "right
  ignoring time" here can't be compared with the 4080 table above.
- **Judge-only columns.** "Route right" is the route check.
  "Answer top right" counts goldens that name answers (112 cases) and
  checks whether the judge's top `answer` option is one of them.
- **Map.** The Clef map was fitted on Flash; no 27B map exists yet.

| Judge (H100) | Pass | Right ignoring time | Route right | Answer top right | Route top p (p10 / p50 / p90) | Judge p50 / p90 |
| --- | --- | --- | --- | --- | --- | --- |
| Clef-Flash, Clef map | 26 | 26 | 111 / 123 | 72 / 112 | 0.41 / 0.82 / 0.98 | 1.47 / 1.48 s |
| Clef-Flash, raw | 13 | 13 | 111 / 123 | 72 / 112 | 0.18 / 0.30 / 0.49 | 1.47 / 1.48 s |
| **Clef 27B, Clef-Flash map** | 0 (all over the 2 s judge budget) | **44** | **118 / 123** | 74 / 112 | 0.66 / 0.96 / 0.98 | 4.30 / 4.33 s |
| Clef 27B, raw | 0 | 15 | 118 / 123 | 74 / 112 | 0.29 / 0.53 / 0.74 | 4.32 / 4.33 s |
| Hosted Jev (4080 table above, notes on) | 106 | 108 | 120 / 123 | 108 (served) | 0.52 / 0.95 / 1.00 | 0.89 / 1.13 s |

**What 27B closes, and what it doesn't.**

- **Route: mostly closed.** 27B gets 118 of 123 routes right, against
  Flash's 111 and Jev's 120. Its route confidence is higher too: a median
  of 0.96 through the Flash map, against 0.82 for Flash.
- **The `answer` question: not closed.** 27B's top answer is right on 74
  of 112 goldens; Flash's is right on 72. That leaves most of the gap to
  Jev in place.
- **Too slow for the budget.** At 4.3 s per turn on an H100, 27B misses
  the router's 2 s judge budget on every case. Clef-Flash on the H100
  fits the budget at 1.47 s.
- **Where that leaves the router.** On an H100, Clef-Flash is a viable
  speed tier. Neither Clef model reaches Jev's `answer` accuracy, so Jev
  stays the first door. A 27B calibration map, and fewer `answer`
  options (see above), are the next steps on accuracy.

### Round 5 (2026-10-11): receipt timing and CUDA head batching

- **Receipt timing.** Every `/v1/systemone` answer now carries
  `psionic.timing`:
  - `encode_ms`, `queue_ms`, `prefill_ms`, `head_ms` and `total_ms`,
    measured on the server;
  - `cached`, which is true for an answer from the exact logit cache. That
    answer ran no prefill or head.

  With this, queue time and inference can be separated from transport.
- **CUDA head batching.** The CUDA trunk now implements the batched head
  calls:
  - independent products in one submission;
  - each feed-forward in one submission, with the GELU in double like the
    host;
  - each memory-attention block in one submission.

  The head at 1k takes 3.5 ms, against about 5 ms before.
- **Fewer waits and less memory.**
  - The layer stack and the final memory/span step share one submission
    and one wait per chunk.
  - The cuBLAS attention score workspace is allocated only when that path
    is in use: about 364 MiB less at 16k with flash attention.
- **Device timestamps.** `ClefEvent::timing` records device timestamps
  for profiles.
- **Parity on the 40 e2e requests.**
  - Against the CPU lane: top answer 100 %, max |Δp| 0.0045.
  - Against the previous CUDA build: 100 %, max |Δp| 0.0026, median 0.
- **Speed.** It matches the deployed build `a9ab2671be` in the same
  session, two interleaved rounds of median-of-7:
  - 1k: 0.193 s, against 0.192;
  - 4k: 0.589 s, against 0.590;
  - 16k: 2.45 s, against 2.46.

### Round 6 (2026-10-12): Hopper projection GEMM (#11443)

The trunk projections on an H100 now run through a Hopper kernel
(`psionic-backend-cuda/src/kernels/clef_hopper.cu`) instead of the 4080's
fused `mma.sync` kernel (chunks up to 1,024 tokens) and dequantize +
cuBLAS (longer chunks).

- **Kernel.** Warp-specialized and persistent: one producer warp issues TMA
  loads of the raw GGUF bytes (128 weight rows x 256 k: one Q4_K
  super-block or 8 Q8_0 blocks per row) and of the f16 token tile (NT x
  64 k, 128-byte swizzle) into mbarrier-guarded shared-memory rings; two
  consumer warpgroups dequantize their 64 rows each straight into the
  `wgmma` A-fragment registers and issue `m64nNTk16` `wgmma` with f32
  accumulation. The dequantization of one 64-k stage overlaps the tensor
  work of the previous one. The epilogue adds into the f32 residual (or
  overwrites).
- **Chunk invariance.** Each output sums its k stages in order inside one
  CTA (no split k, no atomics), so a token's result does not depend on the
  other tokens in its chunk. The token tile (64, 128, 192 or 256) is picked
  per launch from the chunk size and the weight's row count to waste the
  least of the last wave; every tile gives bitwise the same per-token
  result (checked on all 14 shapes at 1, 17, 100, 1,082, 2,048 and 3,917
  rows, plus 64-row chunks against the whole).
- **Precision.** Q8_0 dequantizes in f16x2 to exactly the dequant kernel's
  f16 weights, and the result is bitwise equal to cuBLAS with f32 compute on
  those weights. Q4_K dequantizes in f16x2 by default (f16 scale and
  minimum, so a weight can differ from the dequant kernel's by one f16
  rounding step): 3.6e-4 relative RMS against the f32 reference, under the
  fused kernel's 4–8e-4. `PSIONIC_CLEF_HOPPER=exact` keeps the f32 Q4_K
  arithmetic (error 0) at about 10 % less kernel speed.
- **Selection.** On at load when the device is compute capability 9.0 and
  the build carries the kernel (`build.rs` compiles it for `sm_90a` when the
  target architecture is `sm_90`; other targets link a stub). Ada keeps
  today's kernels. `PSIONIC_CLEF_HOPPER=0` turns it off. Weights it takes no
  longer get f16 scratch slots.

**Kernel alone** (`fixtures/clef/tools/hopper_gemm_bench.cu`, H100 SXM,
2,048-row chunk, % of the 989 TF dense f16 peak; baseline is today's long-
chunk path, dequantize + cuBLAS f16 + the f32 conversion):

| Shape (W rows x k) | Hopper | Dequant + cuBLAS f16 | cuBLAS on resident f16 weights | Fused `mma.sync` |
| --- | ---: | ---: | ---: | ---: |
| Flash ffn gate/up, Q4_K 12,288 x 4,096 | 0.302 ms, **69 %** | 0.403 ms | 0.275 ms | 0.959 ms |
| Flash ffn down, Q4_K 4,096 x 12,288 | 0.283 ms, **74 %** | 0.330 ms | 0.259 ms | 0.944 ms |
| Flash qkv / q, Q8_0 8,192 x 4,096 | 0.209 ms, 66 % | 0.289 ms | 0.195 ms | 0.813 ms |
| Flash z / out, Q8_0 4,096 x 4,096 | 0.106 ms, 65 % | 0.138 ms | 0.093 ms | 0.411 ms |
| 27B ffn gate/up, Q4_K 17,408 x 5,120 | 0.551 ms, **67 %** | 0.681 ms | 0.489 ms | 1.681 ms |
| 27B ffn down, Q4_K 5,120 x 17,408 | 0.569 ms, **65 %** | 0.625 ms | 0.507 ms | 1.661 ms |
| 27B qkv, Q8_0 10,240 x 5,120 | 0.318 ms, 68 % | 0.428 ms | 0.301 ms | 1.172 ms |
| 27B out, Q8_0 5,120 x 6,144 | 0.223 ms, 58 % | 0.262 ms | 0.186 ms | 0.772 ms |
| Per 2,048-token chunk, all projections: Flash / 27B | **43.1 / 156 ms** | 54.8 / 188 ms | 38.3 / 138 ms | — |

- The large FFN shapes clear the 60 % bar on both models. The kernel is
  about 12 % behind cuBLAS on f16 weights that would have to stay resident
  (2x the weight memory and not chunk-invariant), and 15–30 % ahead of
  today's path on every large shape.
- Short chunks: at 1–100 rows the kernel takes 25–105 µs per projection,
  faster than the fused kernel (85–240 µs) on every shape. Alpha/beta (32
  or 48 rows) take about 30 µs against cuBLAS's 15 µs; that is under 2 ms
  per chunk.
- What did not help: 2-CTA clusters multicasting the token tile (within
  2 %, so the L2 -> SM traffic is not the bound), and four A-fragment
  buffers (`ptxas` serializes the `wgmma`). What did: the f16x2
  dequantization (+11 % on Q4_K, +5–10 % on Q8_0 over f32 arithmetic), and
  a `fence.proxy.async` before releasing a raw stage (without it the TMA
  overwrite races the generic-proxy reads).

**End to end** (one spot H100 80GB, `us-central1-a`, driver 580, both
servers built on the same day from `661e02d3ed` with nvcc 12.9 and
`PSI_CUDA_ARCH=90`, the candidate with this change; `bench_latency.py`,
median of 9 with a fresh nonce, two interleaved rounds, chunk 2048, f16
mode; the 32k prompt is the 16k state twice, 30,660 tokens):

| | 1k | 4k | 16k | 32k |
| --- | ---: | ---: | ---: | ---: |
| Flash, today | 0.080 / 0.079 s | 0.220 / 0.220 s | 0.850 / 0.852 s | 1.79 / 1.79 s |
| **Flash, Hopper GEMM** | **0.068 / 0.067 s** | **0.197 / 0.198 s** | **0.782 / 0.783 s** | **1.66 / 1.67 s** |
| Flash speedup | 1.17x | 1.11x | 1.09x | 1.08x |
| Flash, `PSIONIC_CLEF_HOPPER=exact` | — | 0.216 s | 0.827 s | — |
| 27B, today | 0.207 / 0.203 s | 0.635 / 0.617 s | 2.67 / 2.59 s | 6.02 / 6.07 s |
| **27B, Hopper GEMM** | **0.180 / 0.196 s** | **0.602 / 0.622 s** | **2.54 / 2.50 s** | **5.84 / 5.85 s** |
| 27B speedup | 1.08x | 1.02x | 1.04x | 1.03x |

- **The 1.5x target at 4k and 16k is not met.** The projections are about
  half of a 16k prefill: with `PSIONIC_CLEF_SKIP=gemm,dequant` today's
  Flash build takes 0.42 s of its 0.85 s, and the Hopper build 0.41 s of
  0.78 s. So even free projections would give 2x at most, and 1.5x would
  need them at about 0.14 s (over 2,000 TF). The rest (Flash at 16k:
  attention about 0.12 s, the DeltaNet scan about 0.13 s, norms, conv and
  element-wise kernels the remainder) is #11444.
- 27B gains less than Flash: its dequantization already ran on a side
  stream under longer GEMMs, and its large shapes are where cuBLAS f16 was
  closest to this kernel.
- Process memory is about 0.2 GB lower (Flash 8.0 GB after 32k, against
  8.2 GB; 27B 21.5 GB against 21.8 GB): no f16 weight scratch.

**Parity** (`e2e.py compare`, the 40 e2e requests, 104 questions):

| Against | Flash, Hopper | Flash, today | 27B, Hopper | 27B, today |
| --- | --- | --- | --- | --- |
| Psionic CPU lane | **100 %, max \|Δp\| 0.0014** (exact Q4_K: 100 %, 0.0010) | 100 %, 0.0025 | — | — |
| today's CUDA build | 100 %, 0.0030 | — | 98.1 %, 0.018 (two near ties at 0.49/0.51) | — |
| today's build with `--decision-accumulate f32` | — | — | 99.0 %, 0.013 (one near tie, 0.492 vs 0.495) | 99.0 %, 0.011 |

The f32 accumulation brings the Flash CUDA lane closer to the CPU lane
than today's f16 default. There is no 27B CPU-lane fixture; against the
f32-accumulate reference the Hopper build agrees as closely as today's
default does.

**Chunk invariance** (`clef::tests::cuda_chunks_and_cpu_agree` on the
H100, which passes): with the Hopper GEMM the default f16 mode is now
bitwise chunk-invariant on Flash. Before, only `PSIONIC_CLEF_FUSED=1` was,
at a speed cost; the default moved 1.6–2.1e-2 in logit on long records.

| Mode | 155 tokens, chunks 2048 / 512 / 64 | 7,274 tokens, chunks 2048 / 512 / 64 | CUDA vs CPU lane, 155 tokens |
| --- | --- | --- | --- |
| f16 (default), Hopper | 0 / 0 / 0 (bitwise) | 0 / 0 / 0 (bitwise) | max \|Δlogit\| 2.2e-3, max \|Δp\| 2.5e-5 |
| f32, Hopper | 0 / 0 / 0 | 0 / 0 / 0 | — |
| f16 and f32, `PSIONIC_CLEF_HOPPER=exact` | 0 / 0 / 0 | 0 / 0 / 0 | max \|Δlogit\| 3.1e-4, max \|Δp\| 1.9e-5 |

**Default.** On: it is faster at every measured size on both models, it
passes parity, and it makes the default bitwise chunk-invariant. The Q4_K
dequantization defaults to f16x2; `PSIONIC_CLEF_HOPPER=exact` is the strict
mode (on 27B it matches the f32-accumulate reference to 100 % of top
answers, max |Δp| 0.0027, against 0.013 for the default).

**Next.** The projections are no longer the larger share: #11444
(attention, the DeltaNet scan, CUDA graphs) is where the 1.5x has to come
from. In the kernel, a ping-pong schedule that overlaps one warpgroup's
epilogue with the other's main loop, and a split-k for the 32/48-row
alpha/beta weights (fixed per weight, so still chunk-invariant), are the
remaining known gaps to cuBLAS. Cost of this round: about 1.7 h of one spot
H100 (about $11) plus a CPU build VM.

### Round 7 (2026-10-12): concurrent requests on CUDA (#11441)

**What runs.** `PSIONIC_CLEF_BATCH_REQUESTS=2..4` now enables the batch worker
on CUDA too, with the Metal contract: the same settings, admission, cancellation
between chunks, serial priority and receipts
(`psionic.batch.{chunks,max_requests,grouped_ffn}`, effective
`psionic.prefill_chunk`, per-request `psionic.timing`).

- Each request keeps its own conv/DeltaNet state, f16 K/V caches, flash
  attention, `W_mem` rows, span sums, chunk scratch and head.
- Per layer, the mixers run per request. Then the residual rows are packed
  into shared buffers, the three FFN projections run once over the cohort,
  and the rows are copied back.
- Each member's rows use the kernel its serial prefill would use for that
  chunk:
  - on sm_90, that is the Hopper GEMM for every member;
  - elsewhere, fused-kernel members run together, and dequantize + cuBLAS
    members run together as one GEMM per projection.

  The Hopper and fused kernels compute rows independently, so those rows are
  bitwise the same as serial.
- The request's thread decodes its embedding rows and rotary table while
  other requests run. Before this, the worker decoded them between steps,
  about 20 ms of idle GPU per 1.5k-token request.
- Memory: the session estimate covers backbone scratch, state, caches, memory
  rows, spans, evidence views and a head allowance. It is checked against the
  configured budget and against free device memory.

**Parity** (`fixtures/clef/tools/cuda_batch_check.py`, H100, synthetic
1.5–1.9k-token cases). Each mode compares 20–22 successful answers with the
same binary's serial answers. Every mode also checked shared FFN work,
recovery after a disconnect, and recovery after an overload burst (6 `busy`
with 2 slots, 4 with 4 slots, the rest answered).

With the Hopper GEMM (Round 6; rows are computed independently in a fixed k
order), every batched answer is bitwise identical to serial:

| Chunk | Model | 2 slots | 4 slots |
| --- | --- | --- | --- |
| 512 | Flash | identical (max \|Δp\| 0) | identical |
| 2048 (CUDA default) | Flash | identical | identical |
| 2048 (CUDA default) | 27B | identical | identical |

Before Round 6 was merged, the same check ran on the fused / dequantize +
cuBLAS kernels:

- the fused shapes (chunks 256 and 512) were identical;
- the cuBLAS cohort GEMM at chunk 2048 was within 8.8e-5, with every top
  answer the same.

One early build put 512-token members through one 2,048-row cuBLAS GEMM. It
was off by 0.0014. That is why, off sm_90, each member keeps its serial
kernel.

**Throughput** (`harness.py --target h100-clef-flash|h100-clef-27b`, cold,
45 s per point, one spot H100 in us-central1-a, one session). Every column
is the same binary, built from main with Round 6. "Serial" is batching off,
the code main already ran. The batched columns use the default CUDA cohort,
where each request keeps chunk 2048.

| Flash, decisions/s (p50 / p99) | serial | 2 slots | 4 slots | 4 slots, chunk 512 |
| --- | --- | --- | --- | --- |
| 1.5k × 1 q, 1 client | 9.29 (0.107 / 0.119 s) | 9.06 (0.110 / 0.121 s) | 9.08 (0.109 / 0.121 s) | 7.94 (0.126 s) |
| 1.5k × 1 q, 4 clients | 9.32 (0.43 / 0.45 s) | **11.20 (0.35 / 0.40 s)** | 10.44 (0.35 / 0.56 s) | 10.36 |
| 1.5k × 1 q, 8 clients | 9.32 (0.86 / 0.88 s) | **11.27 (0.70 / 0.76 s)** | 10.12 | 9.82 |
| 1.5k × 1 q, 16 clients | 9.31 | 11.02 | 10.06 | 10.04 |
| 4k × 13 q, 4 clients | 45.13 | **47.18** | 46.52 | 41.96 |

| 27B, decisions/s | serial | 2 slots | 4 slots |
| --- | --- | --- | --- |
| 1.5k × 1 q, 1 client | 3.57 (0.277 s p50) | 3.51 (0.281 s) | 3.50 (0.282 s) |
| 1.5k × 1 q, 4 clients | 3.58 | **3.71** | 3.61 |
| 1.5k × 1 q, 16 clients | 3.59 | 3.70 | 3.61 |
| 4k × 13 q, 4 clients | 15.77 | **15.96** | 15.96 |

Before the merge, the same comparison against the main bundle `42334c876b`
(chunk 2048 on dequantize + cuBLAS) showed:

| Before the merge | serial | 2 slots |
| --- | --- | --- |
| Flash, 4–8 clients | 8.9/s | 11.0/s |
| Flash, 13 questions | 42.1/s | 45.5/s |
| 27B | 3.44/s | 3.65/s |

Findings:

- **Gains.** Two slots are the best setting. Flash: 1.21× at 4–8 clients,
  1.05× at 13 questions. 27B: 1.04×.
- **One client stays within 5 %.** 0.107 s → 0.110 s p50 (+2.6 %).
- **Short chunks still lose.** Four slots at chunk 512 do no better than
  chunk 2048 and cost 18 % at one client. The CUDA default stays at chunk
  2048.
- **Below the issue's 1.5× target.** Batching does not make a row cheaper,
  and the GPU is mostly computing, not waiting:
  - Nsight Systems (Flash, 1.5k tokens, before Round 6): each decision runs
    about 80 ms of kernels, serial or batched. At 1.5–1.9k rows per request
    the GEMMs are already large, so packing four requests leaves the FFN time
    per row unchanged.
  - 27B keeps the GPU 85–100 % busy from one client up.
  - Most of the serial lane's idle time was host-side embedding decode,
    which the worker now overlaps. Even with no idle time left, Flash would
    top out near 1.4×.
- **What is left.** The per-decision kernel time: attention, the DeltaNet
  scan and launch overhead (#11444). Batching multiplies whatever that saves.
- **Cost.** Five spot runs, about 2 hours of H100 (about $12.5), and three
  11-minute Cloud Builds.

### Round 8 (2026-10-12): prefix reuse on CUDA, off by default (#11442)

**What runs.** `PSIONIC_CLEF_PREFIX_CACHE_BYTES` now works on CUDA too, with
the Metal contract (`clef/prefix.rs`, `qwen35/clef_cuda/prefix.rs`):

- A snapshot is one device buffer holding every layer's conv and DeltaNet
  state, the prefix rows of each attention layer's f16 K/V, and the prefix
  memory rows. It is keyed on every prefix token (system prompt + state) and
  the effective chunk size. A hit restores into the request's fresh buffers
  and prefills only the suffix. A miss splits its chunks at the prefix end and
  captures there.
- Several snapshots, least recently used out, within the byte budget (at most
  32 GiB and a quarter of the device; at most 64 entries). Unset or 0 is off,
  the default.
- The serial lane and the batch worker both restore and capture. A batched
  capture is pending until its request finishes and is released if the
  request is cancelled.
- `usage.cached_input_tokens` and `psionic.timing.restored_tokens` report the
  restored tokens; `psionic.prefix_cache` reports entries, bytes and budget.

**Status: not verified, so it stays off.** The parity and speed check
(`fixtures/clef/tools/cuda_prefix_check.py`) did not run on the H100: the
first attempt stopped on a bug in the script (fixed since, and dry-run
against a fake server). The acceptance numbers (zero logit difference, ≥ 5×
on a 4k state with a 1k schema) are still open.

**Two-stage lab** (H100, bundle `c562886094`, `eval_closed.py --k 24`, four
closed issues, tier 2 over 24 candidates then tier 3 over the top 8, the issue
as the whole state via `issue_files.shared_request`). The second pass did
restore its prefix (238–299 tokens), but the issue texts are short and the
candidate questions are 12–15k tokens, so reuse saves nothing measurable:

| Seconds per issue (run A / B) | reuse | cache off |
| --- | --- | --- |
| Flash | 0.694 / 0.696 | 0.726 / 0.729 |
| 27B | 2.083 / 2.121 | 2.023 / 2.021 |

The 27B two-stage pass reaches AUC 0.90–0.91 and recall@6 0.79–0.86, against
0.88 and 0.71 for single-pass batched tier 3 over the top 12 (the same
four issues on the H100 earlier the same day). Some answers
differ slightly between the two servers (27B up to 0.03, Flash up to 0.09 on
one file). The reuse server splits a cold request's chunks at the prefix,
and the cache-off server does not. Until the parity check shows otherwise,
treat that as chunk-split drift.

Raw rows: `gs://openagentsgemini-decision-throughput/clef-11442/r1/out/`.

## M3 status (2026-10-10, in progress)

The [October 10 Mac performance audit](../audits/2026-10-10-clef-mac-performance/README.md)
records the later paired comparison, numerical checks, partial configuration
sweep, and preparation incident. Those measurements include desktop GPU
contention and do not pass the idle-Mac gate. The tested runtime patch remains
an experiment; this audit changes no production runtime default.

The Metal lane runs the model; the dated rounds below record its measurements.
The current implementation adds opt-in kernels and M4 features with the
passing checks and remaining acceptance gaps recorded below. Issue:
[#11196](https://github.com/OpenAgentsInc/openagents/issues/11196).

- **Kernels:** `crates/psionic/crates/psionic-backend-metal/src/kernels/clef_prefill.metal`,
  with `clef_prefill.rs`.
- **Trunk:** `crates/psionic/crates/psionic-serve/src/qwen35/clef_metal.rs`
  (`ClefMetalTrunk`).
- **Selection:** `--decision-device metal`. On macOS, `auto` picks Metal.
  CUDA and Metal sit behind one `DeviceTrunk`.

### Design

- **Projections.** Every trunk weight is dequantized to f16 once at load
  and stays resident in unified memory (13.8 GB for Clef-Flash).
  - Each projection is one f16 × f16 → f32 GEMM on the Metal Performance
    Primitives `matmul2d` tensor op, which runs on the M5's neural
    accelerators.
  - Weights are read with no per-request dequantization.
  - Accumulation is f32.
- **Gated DeltaNet.** The sequential scan from M2. One threadgroup covers
  64 state rows, with token inputs staged through threadgroup memory.
- **Attention.** Scores and `P V` run on the same tensor op, one query head
  at a time, with a causal softmax between them. A key past a query's
  diagonal gets probability exactly 0.
- **Everything else** is ported from the CUDA kernels with the same
  arithmetic and fixed reduction orders. That covers the norms, conv,
  attention prep, gates, span sums, the fixed-order `W_mem`, and the head's
  device memory attention.

### Kernel checks on the M5 Max

`fixtures/clef/tools/metal_kernels_harness.swift` runs on synthetic data.
The Mac was shared with other agents' work (load average 8–17, including a
GPU visualizer), so the times are upper bounds. The tensor-op GEMM reached
55–64 TF when the GPU was free and 28–36 TF under that load.

| Kernel | Correctness | Chunk invariance | Time |
| --- | --- | --- | --- |
| Tensor-op GEMM, f16 → f32 | ≤ 3e-6 against f64 | bitwise (64-row chunks against whole) | 55–64 TF free; MPS on the same shapes 59–64 TF |
| Delta scan, 1,082 tokens | 2e-8 against a CPU reference | sequential, same arithmetic | 1.7 ms per layer under load, about 41 ms per 1k request |
| Attention on the tensor op | 1e-4 against f64 | bitwise (512-token chunks against whole) | 2.3 / 9.6 / 204 ms per layer at 1k / 4k / 16k, under load |

What was tried and dropped:

- **Hand-written flash attention.** I tried a SIMT version and one on
  `simdgroup_matrix` with keys at absolute positions. Both were correct and
  bitwise invariant, but they ran at only 1.4–2.5 TF.
- **Scan layouts.** The per-row simdgroup scan (llama.cpp's Metal layout)
  ran at 2.4–2.7 ms. Threadgroups of 128 to 1,024 threads ran at 1.7–2.8
  ms.

### Round 2 (2026-10-10): the model on Metal

The Clef-Flash Q4_K_M GGUF (revision `4a192915`, digest `fd3e9060…`) ran
on the M5 Max through `psionic-openai-server --decision-device metal`,
built from `50f2f1bcc0`.

- **Parity.** On the 40 e2e requests (104 questions):
  - against the CPU lane: top answer 100 %, max |Δp| 0.0008, median
    6e-5, tighter than CUDA's f16 path (max 0.0045);
  - against the HF f32 reference: 94.2 %, the CPU lane's own figure;
  - against the CUDA lane: 100 %, max |Δp| 0.0044.
- **Chunk invariance.** `cuda_chunks_and_cpu_agree` ran with
  `PSIONIC_CLEF_TEST_DEVICE=metal`, and every chunk size matched the whole
  prompt bitwise: max |Δlogit| **0** over {2048, 512, 64} on the 155- and
  7,274-token records, in both modes. Against the CPU lane the logits
  differ by at most 2.8e-4, inside the 1e-3 bound. The test passes.
  - So on Metal the M2 chunk bound holds as written, in the default mode.
  - That is because every projection is a fixed-order tensor-op GEMM, and
    attention reads keys at absolute positions with exact zeros past the
    diagonal.
- **Latency under load.** The Mac's load average was 65–155 here, with a
  GPU visualizer and browser GPU work running, so these are not gate
  numbers. Psionic and Ollama 0.40 `clef-flash` were interleaved in one
  session, median of 5, fresh nonce:

  | Prompt | Psionic Metal, rounds 1 / 2 | Ollama 0.40, rounds 1 / 2 |
  | --- | --- | --- |
  | 1,082 tokens | 1.15 / 1.18 s | 1.16 / 1.05 s |
  | 3,916 tokens | 3.75 / 3.60 s | 4.27 / 3.81 s |
  | 15,510 tokens | 15.8 / 15.8 s | 18.2 / 16.3 s |

  Under the same load Psionic is ahead at 4k and 16k and even at 1k. The
  official gate needs a quiet Mac.
- **Where 4k goes under that load.** `PSIONIC_CLEF_PROFILE=1` gives the
  device time, and `PSIONIC_CLEF_SKIP=gemm|delta|attention` takes each
  phase out:
  - device time: 4.4 s;
  - GEMMs: about 3.4 s;
  - delta scan: about 0.27 s;
  - attention: about 0.3 s;
  - the rest: about 0.4 s;
  - head: 0.27 s, of which 46 linears took 0.1 s.

  The GEMMs ran at about 16 TF in the model, against 50–60 TF for the
  same shapes in short standalone runs.
- **Sustained throughput; cause unresolved.**
  `fixtures/clef/tools/metal_kernels_harness.swift` and a
  48-GEMM command buffer ran at 45 TF for the first 200 ms, then 15–18 TF,
  with the same or distinct weights. These elapsed times do not distinguish
  GPU scheduling, frequency policy, heat, or memory behavior. Later short
  probes retain 50–60 TF across multiple rounds; other probes slow down
  within the GPU interval. No clock or scheduling trace establishes the
  cause. Shared-machine measurements do not establish the idle-machine
  gate. At 55–60 TF sustained, the projections
  alone would be about 0.26 s at 1k and 3.6 s at 16k.

### Round 3 (2026-10-10): the head in fewer round trips

Before this round, the head made 52 GPU round trips: 46 small products,
each with its own command buffer and wait, plus 6 memory attentions.
Under load that took 270 ms at 4k. The head now asks the device in
batches:

- independent products in one command buffer (`linear_batch`): the three
  option projections, the question and global projections, and each
  field layer's `k`, `v` and `q`;
- each feed-forward as one call: up, bias, GELU (an f32 `erf` within
  1.5e-8 of the f64 one), down and bias;
- each memory-attention block as one call: the query projection, the
  attention and the output projection.

The CPU and CUDA paths keep their per-op behaviour through the trait
defaults.

Results, under similar load (load average about 30–40):

- **Round trips:** 23 instead of 52.
- **Head time at 4k:** 58–68 ms, against 270 ms.
- **Parity:**
  - against the previous Metal build: max |Δp| 1.8e-7;
  - against the CPU lane: still 100 % top-answer agreement, max |Δp|
    0.0008.

**Round 4 (2026-10-10): the 1k fixed cost.**

- **Where 1k goes.** At 1k with the GEMMs skipped
  (`PSIONIC_CLEF_SKIP=gemm`), a request takes 88 ms:
  - device 61 ms, of which the delta scan is about 41 ms;
  - head 14 ms;
  - host work about 13 ms.

  With GEMMs at a quiet machine's rate (about 0.26 s), 1k lands at about
  0.35 s.
- **Tried: a chunked (WY) delta rule on Metal** (`clef_delta_chunk128`).
  - It was correct: 3e-8 against the CPU reference.
  - Its chunks of 16 sat at absolute positions, so it kept chunk
    invariance.
  - It was slower: 3.6–5.8 ms per layer, against 1.6–1.8 ms for the staged
    scan. Its 16 × 16 products and the triangular solve ran on scalar
    threads.
  - Dropped. A version on `simdgroup_matrix` is the follow-up.
- **Kept: the gate projection `z` beside the scan.**
  - The batch encoder is now concurrent, with a buffer-scope barrier
    before every dispatch except where an op is marked independent.
  - `z` runs while the scan, which uses few cores, holds the GPU.
  - Results are bitwise identical.
  - Timing under load is neutral to slightly positive: small requests
    0.10 → 0.093 s, 4k 3.0 → 2.95 s, 1k within noise.

**The gate run.** A watcher on the Mac (`gate2.sh` in the session
scratch) waits for a quiet machine: the 1-minute load average below 5 for
10 minutes, and the GPU visualizer closed. It aborts and reschedules if the
load goes above 8 mid-run. It then runs Psionic Metal and Ollama
0.40 `clef-flash` interleaved, three rounds of median-of-5, and records
the load for each run.

### Gate attempt (2026-10-10 19:12–19:23)

- **Trigger.** The watcher started the run when the 1- and 5-minute load
  averages were 8.5 and 9.9 and the visualizer was closed.
- **Load during the run.** It rose from 8.6 to 19 while the run went on.
- **Method.** Ollama 0.40 `clef-flash` and Psionic Metal (`edd58c7d0f86`,
  the batched head) were interleaved. Each cell is the median of 5 with a
  fresh nonce.

| Round (1-minute load at start) | Psionic 1k | Ollama 1k | Psionic 4k | Ollama 4k | Psionic 16k | Ollama 16k |
| --- | --- | --- | --- | --- | --- | --- |
| 1 (8.6 / 9.5) | 0.85 s | 0.51 s | **2.35 s** | 2.59 s | **9.83 s** | 11.17 s |
| 2 (10.3 / 14.6) | **0.37 s** | 0.98 s | **2.79 s** | 3.35 s | **12.50 s** | 13.91 s |
| 3 (19.0 / 16.4) | **0.95 s** | 0.99 s | **3.08 s** | 3.40 s | **13.03 s** | 14.01 s |
| Gate | ≤ 0.35 s | — | — | — | ≤ 7.5 s | — |

What the attempt shows:

- **Against Ollama.** Psionic is faster at 4k and at 16k in every round,
  by 9–17 %.
- **At 1k** the results swing with the load, from 0.37 to 0.95 s for
  Psionic and from 0.51 to 0.99 s for Ollama.
- **The absolute gates are not met** under this load: 1k is at best
  0.37 s against 0.35 s, and 16k is at best 9.8 s against 7.5 s.
- **Why the times are long.** As measured before, sustained GPU
  throughput on this shared machine falls to a third of its burst rate.
  The 16k times are what that predicts.
- **What a valid gate needs.** It has to run with the machine actually
  idle for the whole run (about 10 minutes), not just at its start. The
  watcher's trigger should hold for the run.

### Experimental fused TensorOps attention (2026-10-10)

`PSIONIC_CLEF_METAL_ATTENTION=fused-tensorops` selects a separate experimental
kernel at model load. The default, `staged`, keeps the existing score, softmax,
and value-product path. Unknown setting values fail loading. The fused source
is compiled only when selected, so it does not change default shader startup.

The fused dispatch streams 32-key tiles for 16 queries per threadgroup, using
TensorOps for QK and PV, stable online softmax, and f32 output accumulation.
It declares 19,456 bytes of threadgroup scratch and omits the full trunk score
and probability allocations: 192 MiB avoided for a 2,048-token chunk in a 16k
request. This is an allocation calculation, not a measured speed improvement.
Host checks cover grouped heads, absolute positions, tensor strides, integer
overflow, and all input/output buffer lengths.

Online softmax rounds unnormalized tile probabilities to f16; the staged path
rounds normalized probabilities. Their outputs need tolerance-based validation.
Offline Metal compilation and CPU shape checks do not establish GPU correctness,
chunk equivalence, answer accuracy, or latency. The hardware conformance tests
are explicitly ignored by default. The implementation round used no quiet lease
or GPU run. Subsequent synthetic GPU checks are recorded below; keep the path
experimental until a paired full-model comparison also passes.

Validation in this round: offline Metal 4 compilation passed, all seven host-plan
tests passed, and the Clef serving filter reported 20 passes. Five serving tests
returned early with model settings unset; the other 15 exercised CPU behavior.
All five ignored hardware tests compiled, including large score changes across
key tiles, but none ran. This provides no GPU parity or performance result.

### Evidence normalization shares the attention batch (2026-10-10)

The first use of an evidence layer now encodes `LN_m(M)` in its attention
consumer's command buffer. `attend_memory`, `attend_memory_projected`, and
`head_attend_block` each submit and wait once, including on a cold view, instead
of waiting separately for normalization. This applies to both trunk attention
settings and keeps the existing kernels, arguments, and dispatch barriers.

The request owns the normalized view only after the combined batch completes
successfully. An encoding or completion error leaves that cache
entry empty and its byte count unchanged. A warm view reuses the completed
buffer, and a new prefill starts a fresh cache. CPU tests cover these ownership
and cache transitions. Subsequent synthetic GPU checks are recorded below;
full-model output parity and latency remain unmeasured.

Validation: the release-profile Clef filter in `psionic-serve` reported 28 passes.
Eight new cache tests and 15 existing CPU checks exercised their assertions;
five model-dependent tests returned early with all `PSIONIC_CLEF_*` settings
cleared. The build used a build lease and two Cargo jobs. No quiet lease,
GPU run, or desktop-app or system-service control was used.

### Experimental projection tiles for the checkpoint's weights (2026-10-10)

`PSIONIC_CLEF_METAL_GEMM=weight-shapes` selects shape-specific f16 projection
kernels at model load. The default is `fixed-64`; invalid setting values fail
loading. This setting is independent of the attention setting. Its shader
library and four pipelines are created only when selected.

A header-only inventory of Clef-Flash Q4_K_M (revision `4a192915`, recorded
SHA-256 `fd3e90605e8103307dca37cb5a8cdb036267e2fe3cb2d908d80a8ceb9ec0638c`)
found six weight shapes across 248 trunk projections. Dimensions below are
`W[output, input]`; tiles are token rows by output columns.

| Weight shape | Projections per chunk | Operations | Experimental tile |
| --- | ---: | --- | --- |
| 12,288 × 4,096 | 64 | Feed-forward gate and up | 64 × 128 |
| 4,096 × 12,288 | 32 | Feed-forward down | 64 × 128 |
| 8,192 × 4,096 | 32 | Hybrid QKV and attention query/gate | 64 × 128 |
| 4,096 × 4,096 | 56 | Hybrid gate and mixer output | 64 × 128 |
| 1,024 × 4,096 | 16 | Attention key and value | 64 × 64 |
| 32 × 4,096 | 48 | DeltaNet alpha and beta | 64 × 32 |

Feed-forward weights account for 69.84% of theoretical trunk projection
arithmetic. The wider tile halves the number of threadgroups for the four
large shapes and lets each group reuse input rows across twice as many output
columns. The narrow tile removes the 32 unused logical columns in alpha/beta
tiles. Neither calculation establishes a speedup: edge handling may already
skip some unused work, and wider tiles can increase register pressure.

Both variants keep 64 token rows, four SIMD groups, dynamic full-width reduction,
f32 accumulation, and the existing dispatch barriers. Selection depends on the
exact weight dimensions, including the input width, rather than the chunk
length. The 1,024-column weights, unknown shapes, and f32 head products keep
their existing kernels. The planner checks signed TensorOps extents, buffer
sizes, and element offsets before binding buffers; device setup checks thread
width, threadgroup capacity, and static memory use.

Offline Metal 4 compilation passed. At implementation time, numerical agreement,
chunk equivalence, answer accuracy, and end-to-end speed were unmeasured.
The ignored hardware checks cover all six real weight shapes, residual
accumulation, nonaligned chunks, partial token tiles, and output guards against
the original kernel and sampled f64 products. Subsequent synthetic GPU checks
are recorded below; keep the mode experimental until paired full-model
comparisons also pass. The implementation round
used no quiet lease, GPU execution, or desktop-app or system-service control.

Validation: the backend Clef filter passed 13 CPU tests, including six new
projection-planner tests; seven hardware tests compiled but remained ignored.
The serving filter reported 28 passes: 23 CPU checks and five model-dependent
early returns with `PSIONIC_CLEF_*` settings cleared. Both checks used release
builds under build leases with two Cargo jobs. The new Rust modules pass pinned
formatting, and the patch passes whitespace checks.

### Synthetic GPU validation (2026-10-11)

For [PR #11271](https://github.com/OpenAgentsInc/openagents/pull/11271), all seven
ignored backend hardware tests passed on the M5 Max, one test at a time under
a GPU lease. The five attention checks cover dense and staged references,
partial tiles, absolute positions, grouped heads, causal masks, large score
changes, repeatability, nonaligned chunks, output guards, and buffer rejection.
The two projection checks cover all six checkpoint weight shapes, residual
accumulation, element offsets, tails, nonaligned chunks, baseline comparisons,
and sampled f64 products. Agreement means the bounds in the tests passed;
it does not establish full-model chunk equivalence or answer accuracy.

All three new serving GPU tests also passed under a GPU lease. They exercise
the evidence-attention consumers with synthetic 64-wide memory, 17 tokens,
three queries, two heads, and two evidence layers. Outputs meet the independent
f64 reference bound of `2e-5 + 2e-5 * abs(expected)` and match bit for bit when
normalization completes in a separate command buffer. Raw, cold, warm,
reset-request, and cache byte-count checks also pass.

Fresh release-profile CPU checks passed 13 backend tests and reported 28 serving
passes: 23 CPU checks and five model-dependent early returns with inherited
`PSIONIC_CLEF_*` settings cleared. The ten hardware tests remain ignored in
ordinary test runs and ran explicitly, one at a time, for this validation.
Builds used build leases and two Cargo jobs. New and modified test modules pass
pinned formatting, and the patch passes whitespace checks.

No model was loaded, no latency benchmark ran, and no quiet lease was requested.
Desktop apps and system services were not controlled. Projection test data stays
below 120 MB per case and evidence fixtures below 1 MiB; these bounds exclude
driver and shader allocations. Completed test commands drain their temporary
Metal objects before the next case. Experimental modes remain opt-in.

### Layer-major prefill order (2026-10-12)

`PSIONIC_CLEF_METAL_PREFILL_ORDER=auto|chunk-major|layer-major` picks how a
multi-chunk prefill walks the layer x chunk grid.

- **Layer-major** runs one layer across every chunk, with one command buffer
  per layer and the whole prompt's residual stream on the device. It encodes
  the same kernels on the same inputs in the same causal order, so logits are
  bitwise equal to chunk-major.
- **What it saves.** With `PSIONIC_CLEF_METAL_FFN_WEIGHTS=q4k-on-demand`, each
  Q4_K feed-forward weight is converted to f16 once per layer instead of once
  per layer per chunk.
- **When `auto` uses it.** Only with on-demand weights and more than one
  chunk. Observers, profiling, phase skips and a prefix-snapshot boundary
  keep the chunk-major walk.
- **Effect on the M3 gate.** None by default. Flash runs with resident f16
  weights, where `auto` stays chunk-major and nothing is converted per chunk.
  The change speeds up the bounded 27B on-demand configuration, not the Flash
  medians that the gate measures.
- **Measured (2026-10-12, shared machine).** With on-demand FFN weights,
  layer-major and chunk-major give identical answers and identical exported
  pooled rows at 4k and 16k. Over two opposite-order rounds of three samples,
  the paired chunk-major/layer-major time ratio is 0.959 at 4k (two chunks,
  so layer-major is slightly slower) and 1.088 at 16k (eight chunks, so
  layer-major is about 8 % faster). See the next section.

### Current main and a token-fast GEMM grid (2026-10-12)

The binary was built from main `0cf744f46b` plus the candidate below, which
is off unless selected, so the default path is main's. Runs used the same
settings as the uncached M4 study: resident f16, positional upload,
fused-tensorops attention, weight-shape GEMM, fused-field head, sequential
Delta, chunk 2048, prefix cache 0 and batch 1. The machine was shared
throughout: the one-minute load was 20–290, and another agent's headless
Blender render held the GPU at 54–89 % device utilization. These numbers
show relative effects only. They are not gate numbers. Evidence is in
`gs://openagents-bench-artifacts/clef/m3-m4-2026-10-11/2026-10-12/`, listed
in the bundle manifest.

**Flash on main, interleaved with the reset Ollama.** Two opposite-order
rounds, five measured unique states per length, with zero cached tokens on
every reply from both engines:

| | ~1k (1,075–1,082) | ~4k (3,913–3,920) | ~16k (15,507–15,513) |
| --- | --- | --- | --- |
| Native (main), median | 1.252 s | 3.452 s | 14.777 s |
| Native prefill phase, median | 1,141 ms | 3,361 ms | 14,608 ms |
| Reset Ollama 0.40, median | 0.744 s | 3.496 s | 14.269 s |
| Paired native/Ollama time | 1.571 | 0.986 | 1.028 |
| Target | ≤ 0.35 s | — | ≤ 7.5 s |

GPU contention hurts native more than Ollama. Earlier the same day, with
build load around 100–117 and no GPU contention recorded, the M4 study below
measured native Flash at 0.584 / 1.967 / 8.360 s against Ollama's 0.629 /
2.221 / 9.045 s. Native was faster at every length. Native streams f16
weights, four times Ollama's 4-bit bytes, so it is more exposed when another
GPU job takes bandwidth. That reading fits the data, but no counter was
captured to confirm it.

**Candidate: a token-fast GEMM grid, rejected.** It added
`PSIONIC_CLEF_METAL_GEMM=weight-major`, which uses the weight-shape tiles but
swaps the grid axes. All token tiles of one weight tile then launch back to
back, so the weight tile can be reused from cache. Each threadgroup still
runs the same MPP operation on the same slices, so the results are exact:

- **Synthetic GPU test:** bitwise equal to `weight-shapes` for every Flash
  and 27B trunk shape, at 1, 65 and 1,078 rows, in both overwrite and
  accumulate modes.
- **Full model:** answers on the 40 Flash e2e requests and the three long
  bodies are identical, and so are the exported pooled rows (`rows.f16`).
  27B is also identical on 37 e2e requests and the three long bodies. The
  other three e2e requests were refused by both modes alike, because the
  harness had re-serialized them.
- **Speed: slower.** Interleaved in the same run as the table above, the
  paired main/candidate time ratios are 0.818 / 0.829 / 0.770 at
  1k / 4k / 16k, so the candidate is 18–30 % slower.

Together with the four-token swizzle that was already rejected, this shows
that the default output-fast order is the better one. Launch order is not
what holds back sustained GEMM throughput. The patch and every run are kept
in the evidence (`weight-major-candidate/`). It is not on main.

### Next

- **Re-measure on an idle GPU.** The 1k gap to 0.35 s is mostly contention.
  Under build load alone it was 0.58 s, and under the GPU render it was
  1.25 s. Re-measure with no other GPU job, and record the GPU utilization
  next to the load.
- **Read fewer weight bytes.** Every exact reordering of the f16 GEMM has now
  failed. What is left is reading fewer weight bytes per FLOP: in-kernel Q4
  dequantization into the matmul tile, the way MLX's 4-bit kernels do it. It
  cannot be bitwise equal to the f16 path, so it needs the 1e-3 raw-logit
  bound and the 104-answer CPU agreement instead.
- **Overlap scans with GEMMs.** In layer-major order, the Delta scan of chunk
  c+1 depends only on chunk c's scan state. It could run beside chunk c's
  out-projection and FFN.

## M4 status (2026-10-11, validation in progress)

The current implementation adds Metal text batching, exact prefix reuse, image
inputs, 27B weight-format support, and CUDA memory checks for
[#11197](https://github.com/OpenAgentsInc/openagents/issues/11197). These features
have separate controls and acceptance checks. The uncached timing study
(2026-10-12, below) gives the current speed result: it misses the 1.5× gate.
No M3/M4 acceptance gate is complete.

The [validation bundle](../../crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/README.md)
(stored in `gs://openagents-bench-artifacts/clef/m3-m4-2026-10-11/`, with a
[per-file manifest](../../crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/MANIFEST.json)
in the repository) retains model identities, full reference answers and logits, test receipts,
failed attempts, and the real CUDA admission check. Its manifest records the
original and published hashes; local machine paths are replaced with labels.

### Runtime acceptance scope

The owner clarified on 2026-10-11 that M4 validates execution against the same
pinned Q4 model. BF16-versus-Q4 quality is separate artifact work in
[#11306](https://github.com/OpenAgentsInc/openagents/issues/11306), evaluated
with routes-v5 and the chat goldens. The 1.5× speed target remains unchanged.
The existing 104-question, sub-1e-6 result covers scoring-head equivalence;
the [full independent same-Q4 check](https://github.com/OpenAgentsInc/openagents/blob/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/same-q4-runtime-v1/README.md)
now covers all 40 requests and 104 questions. It fails the fixed top-answer
gate: both native head modes agree on 103 of 104 answers, although the maximum
probability difference `0.009059182` passes the separate `0.02` bound. One
yes/no answer crosses 50% (`text-029`, `日本1`). Exact encoder sources reproduce
all 20,894 reference token IDs. The independent reference uses the pinned
official Transformers operators, independently decoded Q4 weights, MPS f32
trunk execution, and a CPU f32 head; a CPU-versus-MPS pilot passes first.
Runtime correctness and the speed target remain open.

A [CPU head replay](https://github.com/OpenAgentsInc/openagents/blob/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/text029-head-isolation-v1/README.md)
reproduces the flipped answer by replacing only the false-option pooled
context with its native export, while holding the official head and full
reference memory fixed. Ordinary f16 rounding of reference pools does not
explain the difference. The remaining investigation is before head projection
and scoring; this diagnostic does not change the failed full-suite gate.

An [independent CPU run of the same case](https://github.com/OpenAgentsInc/openagents/blob/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/text029-cpu-reference-v1/README.md)
agrees with the accelerated reference on all three answers and still disagrees
with native execution on one. A
[64-layer comparison](https://github.com/OpenAgentsInc/openagents/blob/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/text029-layer-comparison-v1/README.md)
places the discrepancy in the backbone: the complete native observer memory
reproduces normal native scoring within `0.000002861` raw logits. Layer drift
still needs an operator-level explanation. A
[layer-35 replay](https://github.com/OpenAgentsInc/openagents/blob/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/text029-layer35-replay-v1/README.md)
shows that this layer mostly amplifies incoming differences. Modeling native
half-precision boundaries reduces the smaller local residual, but does not
identify an earlier defect or change the failed all-top gate. The
[layer-0 controls](https://github.com/OpenAgentsInc/openagents/blob/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/text029-layer0-replay-v1/README.md)
bring the independent result about 11.2 times closer to native by rounding both
linear weights and inputs through f16. A change to the Delta formulation alone
has little effect. This local result does not resolve the full-model gate. The revised
[QK128/key32 attention probe](https://github.com/OpenAgentsInc/openagents/blob/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/attention-qk128-key32-probe-v1/README.md)
preserves bitwise output agreement in synthetic checks and reduces median
kernel time by 8–18%. Its
[compiled integration](https://github.com/OpenAgentsInc/openagents/blob/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/attention-qk128-key32-integration-v1/README.md)
passes 25 CPU and four GPU tests, with exactly equal logits and pooled rows
for 110 Flash and 110 27B text answers, including long histories. All 14 image
answers and ten refusals pass. The
[fresh Flash latency comparison](https://github.com/OpenAgentsInc/openagents/blob/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/attention-qk128-key32-flash-latency-v1/README.md)
reports candidate medians of `0.511 / 2.374 / 10.326 s`, missing the targets
without a consistent gain over Stage 6. Native cache counts are zero; Ollama
reports 37–39 reused template tokens on 31 of 36 requests, so the all-engine
uncached gate fails. The shared harness now checks that top-level field. The
independent same-Q4 runtime discrepancy remains unresolved.

### Uncached native vs Ollama timing (2026-10-12)

This is the study from the uncached-comparison plan. It ran from 10:38 to
11:07 UTC on the shared Mac, with build load 98–123 and no task-owned
overlap. Evidence is in `gs://openagents-bench-artifacts/clef/m3-m4-2026-10-11/`
under `native-chunk2048-v1/`, `27b-ollama-reset-control-v1/` and
`uncached-timing-v1/`, listed in the bundle manifest.

**Prerequisites, all passed:**

- **27B Ollama reset control.** Five fresh stock processes and one
  persistent reset process made 13 scoring calls for 29 comparisons:
  - all top answers match, with a maximum probability difference of 0.0;
  - every reset reply reports `prompt_eval_cached_count = 0`, including the
    exact repeat;
  - the stock exact repeat reports 557 cached tokens, which shows that the
    field does detect reuse.
- **Native 27B at chunk 2048.** On the 1k, 4k and 16k bodies, compared with
  the same binary's saved chunk-512 proof: the maximum raw-logit difference
  is 0.0, and the peak is 48.64 GiB under the 52 GiB guard.
- **Timing wrapper.** It runs the pinned `mac_quiet_bench.py` (sha
  `7018be06…`) unedited. It rejects:
  - missing, boolean or positive cache telemetry;
  - token-count mismatches;
  - altered paired JSON;
  - partial coverage;
  - invalid samples.

  Its negative cases are recorded in `check-analyze-results.json`.

**Method:**

- **Rounds.** Two opposite-order rounds. Each round has one warmup and five
  measured unique states per length, giving 10 pairs per length.
- **Pairing.** The request bytes and nonces match within each pair.
- **Native settings.** Resident f16, positional upload, fused-tensorops
  attention, weight-shape GEMM, fused-field head, sequential Delta, chunk
  2048, prefix cache 0 and batch 1.
- **Native binary.** `0e7eb7cf…`, the positional-upload build from
  `46ed6cdcf3`. Its resident-f16 path is the same as main's: layer-major
  order applies only to on-demand weights, and `auto` keeps resident
  weights chunk-major.
- **Ollama.** The pinned reset build (`a315f991…`) serving the installed
  `clef-flash` and `clef` models, which are the 0.40 nvfp4 tensors.
- **Cache check.** Every native and Ollama reply reports zero cached
  tokens.

| Model | ~1k native / Ollama | ~4k native / Ollama | ~16k native / Ollama | Paired Ollama/native time |
| --- | --- | --- | --- | --- |
| Flash | 0.584 / 0.629 s | 1.967 / 2.221 s | 8.360 / 9.045 s | 1.097 / 1.129 / 1.085 |
| 27B | 1.990 / 1.977 s | 6.543 / 7.101 s | 28.302 / 28.610 s | **1.003 / 1.087 / 1.014** |

**Result: the M4 speed gate is not met.** Native 27B ties the uncached
Ollama `clef` at 1k and 16k and is 9 % faster at 4k. The gate is ≥ 1.5×.

- **Rounds agree.** Both rounds give the same picture. Individual 27B pair
  ratios range from 0.94 to 1.11.
- **Prefill dominates.** Native prefill is 1,891 / 6,458 / 28,182 ms of the
  27B medians.
- **Memory.** The native 27B peak was 52,382,098,616 bytes (48.8 GiB), and
  reset Ollama's was 29.9 GB.
- **The issue's reference numbers.** The issue lists Ollama 0.40 `clef` at
  3.9 / 14.3 / 60.3 s. In this session, the same model under the same
  uncached rules ran in 1.98 / 7.10 / 28.6 s. The gate is a paired,
  same-session comparison, so the older numbers do not count toward it.
- **What 1.5× would take.** Cutting about a third of the 27B prefill time.
  The M3 work on Flash has not found exact reorderings that do that. The
  token-fast grid (M3, above) is 18–30 % slower.

The independent same-Q4 runtime check also still stands at 103/104 top
answers (`text-029`, `日本1`).

### Metal controls and memory

These settings are read when the model loads. Invalid enabled settings stop
loading; batch limits are ignored while request batching is off.
The existing defaults remain staged attention and head execution, fixed-64 GEMM,
sequential DeltaNet, resident-f16 weights, no prefix snapshots, and no request
batching.

| Setting | Default | Effect and bound |
| --- | --- | --- |
| `PSIONIC_CLEF_METAL_ATTENTION` | `staged` | `fused-tensorops` selects the experimental attention kernel described above. |
| `PSIONIC_CLEF_METAL_GEMM` | `fixed-64` | `weight-shapes` selects the experimental projection tiles. |
| `PSIONIC_CLEF_METAL_DELTA` | `sequential` | `block-tensorops` selects the experimental 32-token block recurrence. Additional scratch is `ceil(chunk / 32) × value_heads × 37,120` bytes, at most 256 MiB per scratch allocation. |
| `PSIONIC_CLEF_METAL_HEAD` | `staged` | `fused-fields` runs the four field layers in one GPU submission. It supports at most 64 questions, adds 294,912 bytes of model vectors for the current Flash/27B heads, and adds up to 2,883,584 bytes of request scratch. This scratch is included in batch reservations. |
| `PSIONIC_CLEF_METAL_FFN_WEIGHTS` | `resident-f16` | `q4k-on-demand` keeps Q4_K feed-forward weights packed and converts one projection at a time into reusable f16 scratch. Other formats retain the resident path. The conversion slot is 96 MiB for Flash and 170 MiB for 27B, with a 256 MiB limit. |
| `PSIONIC_CLEF_METAL_WEIGHT_UPLOAD` | `mapped-copy` | `positional` reads packed matrices through the original open file into the existing upload buffer, avoiding bulk reads from mapped pages. It requires `resident-f16` and a memory-mapped source; conversion and inference arithmetic stay unchanged. The 27B text and image checks pass within the 52 GiB process cap at chunk size 512; this does not establish latency acceptance. |
| `PSIONIC_CLEF_PREFIX_CACHE_BYTES` | `0` | Enables one exact prefix snapshot when nonzero; maximum 536,870,912 bytes, including its token key. |
| `PSIONIC_CLEF_BATCH_REQUESTS` | `1` | `1` keeps serial execution; `2`–`4` enables the batch worker (Metal or CUDA) and bounds active requests. |
| `PSIONIC_CLEF_BATCH_TOKENS` | `2048` (CUDA: requests × `--decision-chunk`) | Total token rows in one cohort, at most 16,384 and at least the active-request limit. On CUDA the default keeps every request at the lane's prefill chunk; on the H100, shorter chunks cost more than the cohort saves (Round 7). |
| `PSIONIC_CLEF_BATCH_BYTES` | `4294967296` | Total request reservations, at most 32 GiB; includes request scratch, recurrent/KV state, memory rows, spans, and a conservative head allowance. |
| `PSIONIC_CLEF_BATCH_COHORT_BYTES` | `1073741824` (CUDA: `4294967296`) | Separate shared FFN scratch budget, at most 4 GiB. The model's actual scratch estimate must fit before the worker starts; on CUDA it must also fit in free device memory then. CUDA packs only the FFN rows: `hidden × 4 + ffn × 12` bytes per cohort token (1.34 GB for Flash and 1.88 GB for 27B at 8,192 rows). |

At the default 2,048 cohort tokens, the scratch estimate is 571,080,704 bytes for
Flash and 759,824,384 bytes for 27B. With both optional Q4 conversion and block
DeltaNet scratch, these become 747,765,760 and 1,052,114,944 bytes, respectively,
and still fit 1 GiB. These are allocation calculations, not process RSS or speed
measurements. Raising the cohort to 4,096 tokens requires a larger explicit
cohort budget. Request reservations, model weights, prefix snapshots, and host
allocations are separate from that cohort budget.

### Request batching and cancellation

The same worker (`clef/batch.rs`, `clef/worker.rs`) drives Metal and, since
#11441, CUDA (`qwen35/clef_cuda/batching.rs`). The worker groups independent
text requests between chunks. Each request owns
its recurrent state, attention caches, memory rows, spans, and scratch. At each
layer, mixer work stays separate; residual rows are packed into shared buffers
for the three FFN projections, then copied back to their owning requests. This
is shared matrix work across requests, not concatenation of prompts into one
attention context. Each head runs against its own completed request.

The effective per-request chunk is at most the configured cohort token count
divided by its request limit. New work joins between completed chunks, with a
bounded queue and byte reservation. A request exceeding the whole request-byte
budget receives `not_admitted` (413); a full queue receives `busy` (503).
On CUDA a request that does not fit in free device memory while nothing else
is active also receives `not_admitted` (413); with other requests active it
waits for their memory. `--decision-max-queue N` (default 8) is the number of
requests that may wait behind the running ones, serial or batched: a serial
lane admits `1 + N` decisions at once, a batched lane `requests + N`.
Profiling or skipped-kernel modes cannot be combined with batching.

Images and raw-row exports use the serial path. A waiting serial request stops
new batch admission until active requests drain, so a continuous text stream
cannot keep it waiting forever. Serial requests have priority; this is not a
global FIFO guarantee. The execution mutex excludes serial allocations while
the active batch owns its reservations.

Dropping an HTTP future marks its work cancelled. The worker checks cancellation
between completed GPU chunks, drops only that request's state, and releases its
reservation after the buffers are released. It does not interrupt an in-flight
Metal command or CUDA submission. Dropping the loaded model stops and joins its worker; the worker
shares weight storage and holds no reference cycle back to the model. Queue
admission happens before image decoding and tokenization.

Successful batched responses add `psionic.batch.{chunks,max_requests,grouped_ffn}`.
`psionic.prefill_chunk` reports the effective session chunk, including reductions
for a cohort budget or a short request.
`grouped_ffn` is true only when a chunk actually shared FFN work. Per-request
timings keep encoding, queueing, prefill, and head work separate. A request's
prefill time includes the shared cohort work it waited for; interleaved waits
are counted in queue time. Summing request prefill times is not a measure of
total GPU time.

### Prefix reuse and images

Prefix reuse matches every prefix token and the effective chunk size within one
loaded model. The snapshot contains conv/DeltaNet state, prefix K/V, and the
head's projected memory rows. A new request restores into fresh buffers and
computes its schema suffix. Cold and warm requests split at the same prefix
boundary. No question or option span may overlap that prefix.

One snapshot is allowed across the published cache and a pending batch capture.
Replacement frees the old entry first, and cancellation releases an unpublished
snapshot. Media, layer/row observers, profiling, and skipped kernels disable
prefix reuse. Raw pooled-head export can still check prefix reuse; it disables
the separate whole-answer logit cache. `usage.cached_input_tokens` reports
restored prefix tokens, while `psionic.timing.cached` reports a whole-answer hit.

Image requests require an explicit matching vision tower:
`--decision-vision-manifest PATH --decision-vision-blobs DIR`. The loader checks
the complete named tensor set, BF16 shapes, and output width: 4,096 for Flash or
5,120 for 27B. The 27B tower cannot stand in for Flash's tower. Vision projection
runs on the CPU; the text decoder can run on CPU or Metal. The CUDA decision
path refuses images. Image requests bypass both prefix reuse and whole-answer
caching, and video remains unsupported. The base64 decoder preserves the stored
pixel order, including when JPEG EXIF requests a rotation, matching the official
Torchvision byte-input path. The [preprocessing fixtures](../../crates/psionic/crates/psionic-serve/fixtures/clef-images/README.md)
check decoding, resize, patch order, media tokens, and multi-image positions. Accepted image inputs are bounded
base64 PNG, JPEG, or WebP, with at most four images and the existing 8 MiB body
limit; URLs are not fetched.

The staged public 27B artifacts are `ggml-org/Clef-GGUF` Q4_K_M at revision
`d2aa75cb425ae922b5c3c15ef3157d417b7f7641` and the official `Cloudflare/clef`
BF16 checkpoint at `0b331204bb13fbd2ca93a64df1956f5b55478ce5`. The GGUF is
19,232,219,200 bytes with SHA-256
`9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822`.
Use `--clef-head PATH` to select the official external head for a reference
comparison. Download verification establishes artifact identity, not answer
accuracy. Keep same-Q4 runtime comparisons separate from BF16 artifact
diagnostics and product answer-quality evaluation.

### CUDA and benchmark memory checks

CUDA loading now estimates resident weights, conversion slots, configured
token/chunk buffers, a head reserve, and a runtime margin before uploading
weights. It compares that estimate with current free and total device memory.
Larger head allocations are checked again when needed. These checks do not
reserve CUDA memory against another process. A memory refusal also stops `auto`
from silently loading the same model on the CPU. The real 27B startup check passed on the RTX 4080 for both explicit CUDA and
automatic device selection. Both reported `before_weight_upload`, with
20,927,797,248 bytes required, 5,549,588,480 bytes free, and 16,711,352,320
bytes of device memory. Both exited without starting a server or falling back
to the CPU; the aggregate GPU memory snapshot was unchanged afterward.

The shared Mac harness manages only process groups that it starts. It records
effective Clef settings and thread bounds. Every 0.5 seconds, it reads RSS and
macOS physical footprint for only the owned process group, accounting for the
larger value per process. It stops its group if a limit or configured memory
monitor fails. Each memory sampler has a five-second deadline; a timeout
records the failing command and rejects the run. Flash defaults
to `max_rss_gib: 36`. The bounded 27B image/text configuration uses Q4_K
feed-forward storage, a 52 GiB RSS limit, and `startup_timeout: 600`. Its
startup check requires 64 GiB available; execution requires a 12 GiB margin.
The native estimate includes the complete GGUF mapping, GPU weights, both
head copies, image projection, and request buffers. It does not cover arbitrary
large option counts or concurrent requests. The metric is
`vm_stat (free + inactive + speculative) × page_size`. This is sampled process
and host memory monitoring, not a hard allocation cap or a system reservation.
Native 27B and the official BF16 reference run sequentially.

### Validation commands and evidence status

Keep the existing per-worktree `CARGO_TARGET_DIR`. Build and CPU checks use a
build lease; explicitly selected hardware checks and model runs use GPU and
memory leases. Current Mac comparisons set `measurement_mode: "shared"`: they
record other machine activity and cannot establish an idle-machine target. They
do not stop desktop apps or request a quiet lease.

From the repository root, the following checks need no model weights:

```sh
openagents lease build --keep-target-dir -- cargo test --release \
  --manifest-path crates/psionic/Cargo.toml -p psionic-serve --lib clef
python3 crates/psionic/fixtures/clef/tools/mac_quiet_bench.py --self-test
python3 crates/psionic/fixtures/clef/tools/metal_batch_check.py --self-test
python3 crates/psionic/fixtures/clef/tools/cuda_batch_check.py --self-test
```

On a dedicated CUDA host (no leases), the CUDA batch check starts the binary
serially and with two and four slots:

```sh
python3 crates/psionic/fixtures/clef/tools/cuda_batch_check.py \
  --binary target/release/psionic-openai-server --model Clef-Flash-Q4_K_M.gguf \
  --model-id clef-flash --chunk 2048 --out /tmp/cuda-batch-check
```

Clear model-test environment variables before the CPU filter. Tests that print
that a GGUF or model-check setting is missing returned early; they are not model
validation. Compile ignored GPU tests under a build lease, then run the recorded
test executable under a GPU lease with `--ignored --test-threads=1`.

The three model harnesses accept the existing `mac_quiet_bench.py` JSON config
shape: absolute model/binary/request paths, an unused output directory, and
shared-machine mode. Set the following shell variables to their config files.
These commands reserve 36 GiB for Flash; use the reviewed 27B limits and memory
reservation for a 27B run.

```sh
openagents lease gpu -- openagents lease memory --amount 36 -- \
  python3 crates/psionic/fixtures/clef/tools/mac_quiet_bench.py "$CLEF_LATENCY_CONFIG"
openagents lease gpu -- openagents lease memory --amount 36 -- \
  python3 crates/psionic/fixtures/clef/tools/metal_prefix_check.py "$CLEF_PREFIX_CONFIG"
openagents lease gpu -- openagents lease memory --amount 36 -- \
  python3 crates/psionic/fixtures/clef/tools/metal_batch_check.py "$CLEF_BATCH_CONFIG"
```

Latency runs force `PSIONIC_CLEF_PREFIX_CACHE_BYTES=0` and reject nonzero
settings. They pair fresh request nonces and reject cached responses. The prefix
check instead compares exported raw logits with the cache disabled and enabled.
The batch check starts the same binary serially, with two request slots, and
with four slots; it forces zero prefix cache and `BATCH_TOKENS = chunk × slots`.
It requires actual grouped FFN evidence, exact answer identities, 100% top-answer
agreement, and maximum absolute probability difference at most `1e-3`, using
unrounded JSON values. It also checks disconnect recovery and bounded queue
refusals. Throughput comparisons from this check remain shared-machine results.

`guarded_reference.py`, `image_parity.py`, and `native_image_parity.py` prepare
the separate official BF16 and native image/text checks. The reference must
complete before the native comparison starts; do not overlap their model loads.

| Check on the current branch | Status |
| --- | --- |
| Backend synthetic Metal hardware checks, including attention, projection tiles, buffer lifetime/views/copies, packed formats including Q6_K, and block DeltaNet | Passed: 18 explicitly run Stage 5 checks, including additive-epsilon normalization and all eight observed 27B GEMM shapes. The direct-solve recurrence also matches the correlated-key oracle across unaligned chunks, with maximum difference below `3e-7`. These do not load Clef. |
| Harness self-tests: process cleanup, RSS/memory refusal, startup deadline, uncached settings, effective environment, and answer comparison | Passed; mock servers and synthetic child processes only. |
| Serving synthetic Metal checks: grouped FFN, reusable Q4 conversion, prefix state/memory restoration, and three evidence-normalization paths | Passed: all 6 explicitly run checks. |
| Integrated serving CPU and CLI tests | Passed on the final Stage 7 head-only source: 73 serving-filter tests and 24 backend-filter tests. Twenty backend and nine serving hardware tests remain explicitly ignored in these CPU invocations and have separate GPU evidence. The earlier nine CLI tests passed. Model-dependent early returns are not model validation. |
| Flash default Metal versus whole-prompt execution, 155 and 7,274 tokens, 3 chunk sizes and both accumulation modes | Passed: 12 comparisons have zero logit difference. CPU versus Metal has maximum logit difference `2.79e-4` and probability difference `1.35e-5`; repeated requests are bitwise stable. |
| Flash fused attention and weight-shape GEMM | Passed: the same 12 chunk comparisons have zero logit difference. CPU versus Metal has maximum logit difference `1.31e-3` and probability difference `5.55e-5`. |
| Flash compressed FFN versus resident f16 weights | Passed: 34 compared logits are identical. This is a storage/correctness result, not a speed claim. |
| Flash route versus the CPU goldens, 40 requests and 104 answers per variant | Passed again on Stage 5: default and fused-attention/weight-shape variants both match all 104 top answers. Maximum probability differences are `0.001047681` and `0.000989952`, respectively. These correctness runs overlap CPU diagnostics and are not latency measurements. |
| Block DeltaNet | **FAILED in the combined mode:** Stage 5 passes alone at maximum logit difference `8.871e-4`, but reaches `1.405e-3` with fused attention, weight-shape GEMM, and compressed FFN. It remains disabled by default. Both sides of the comparison use the same other optimizations. |
| Fused field head | Passed on Stage 6: Flash and 27B each preserve all 104 raw-logit and HTTP top answers against their staged heads, with maximum logit differences below `1e-6`. The source reduces head waits from 23 to 8. This is head-only parity, not independent full-runtime same-Q4 coverage. |
| Independent same-Q4 27B text runtime | **FAILED:** both Stage 6 head modes agree with the independent reference on 103/104 top answers. Maximum probability difference `0.009059182` passes the fixed `0.02` bound, but `text-029` / `日本1` changes its top answer. All 40 requests, 20,894 input IDs, spans, and options are covered. [Complete evidence](https://github.com/OpenAgentsInc/openagents/blob/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/same-q4-runtime-v1/README.md). |
| Shorter DeltaNet blocks | **Rejected:** the 16-token experiment reaches `1.084e-3` alone and `1.355e-3` combined, above the unchanged `1e-3` limit. Its patch and failures are retained; product source restores the preceding block32 experiment. |
| Official 27B BF16 image goldens and native comparison | Passed again on Stage 6 with fused fields: all six cases and 14 native top answers match the official reference and expected labels, with exact token counts. Maximum probability difference is `0.001779430`; all ten media refusal checks pass. |
| Real 27B CUDA and Auto startup refusal on the RTX 4080 | Passed: both refused before weight upload, without CPU fallback; GPU memory before/after was unchanged. This does not claim that CUDA context creation uses no memory. |
| Official 27B BF16 text goldens | Generated: all 40 cases and 104 answers, with exact reference token identity. An earlier capture failure is retained separately; the isolated retry and final resumed run passed. |
| 27B native BF16 text diagnostic (artifact issue #11306) | **FAILED on Stage 6:** both staged and fused field heads match 93 of 104 top answers, with maximum probability difference about `0.217407`. Stage 5 also fails: all 40 cases have matching token counts, but only 93 of 104 top answers match. Maximum probability difference is `0.217196109`; 64 questions exceed `0.02`. Against a separate BF16-backbone/CPU-f32-head reference, 94 of 104 match and maximum difference is `0.216096109`. Controlled original-weight and independently decoded GGUF executions attribute case 014's large miss to quantized backbone weights. Other mismatches are not individually attributed. |
| Fixed mixed-precision 27B candidate (artifact issue #11306) | **FAILED the original BF16 diagnostic:** applying the upstream fixed Q6 policy to 32 FFN-down matrices improves BF16 agreement to 97/104, but 60 questions exceed 0.02 and maximum probability difference remains 0.169436199. All token counts, 14 image answers, and ten refusal checks pass. The artifact fits the existing 52 GiB cap. Its recipe uses no acceptance errors, and it is not selected for production. [Recipe and full results](https://github.com/OpenAgentsInc/openagents/blob/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/fixed-down-q6-quality-v1/README.md). |
| DeltaNet prepared-diagonal probe | **Rejected on speed:** all 26 standalone correctness checks pass, but paired medians are about 1.37–1.41× sequential duration on Flash and 1.86–1.88× on 27B. No product change or full-model claim follows. [Evidence](https://github.com/OpenAgentsInc/openagents/blob/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/delta-diagonal-probe-v1/README.md). |
| Exact half lookup-table capability and actual-weight probe | Tiny checks pass. The actual Flash gate also passes all decoded weights and output checks, but LUT matmul takes 198.011 ms versus fixed64 MPP at 2.376 ms, so integration is rejected. Allocation snapshots cannot exclude transient private expansion. [Full probe](https://github.com/OpenAgentsInc/openagents/blob/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/lut-gate-probe-v1/README.md). |
| Fixed tile dispatch swizzle | Rejected: all 15 correctness checks and 240 timed output comparisons are bitwise identical, but full-FFN changes are small and inconsistent across 512/1,024/2,048 tokens. [All paired results](https://github.com/OpenAgentsInc/openagents/blob/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/gemm-swizzle-probe-v1/README.md). |
| Three-way Flash down K split | Rejected: 19 correctness checks and 240 timed output checks pass, but paired full-FFN candidate/baseline ratios are 1.223 / 1.001 / 1.075 at 512/1,024/2,048 tokens. Unchanged controls are noisy; no gain is established. [Evidence](https://github.com/OpenAgentsInc/openagents/blob/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/gemm-split3-probe-v1/README.md). |
| Independent SIMD projection tiles | Rejected: 23 numerical checks and 240 timed output comparisons pass, but paired full-FFN candidate/baseline ratios are 1.040 / 1.025 / 1.129 at 512/1,024/2,048 tokens. All three miss the fixed 10% improvement target. [Complete probe](https://github.com/OpenAgentsInc/openagents/blob/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/gemm-independent-simd-probe-v1/README.md). |
| Two-SIMD-group matrix probe | Unqualified: all 19 initial checks and 149 completed timing checks are bitwise equal, but CPU check overhead contributes to the 60-second deadline before the timing plan finishes. Completed 512-token full-FFN ratio is 1.068; no gain is established or integrated. [Partial evidence](https://github.com/OpenAgentsInc/openagents/blob/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/gemm-two-simd-probe-v1/README.md). |
| Attention with 128-key tiles | **Rejected after full-model validation:** the microprobe passes and shows 1.60–1.91× attention-kernel speedups, but the Flash corpus reaches raw-logit difference `0.004385114` against the unchanged `0.001` gate (35/104 questions exceed it). All top answers match. The patch is restored; no 27B or latency run follows. [Microprobe](https://github.com/OpenAgentsInc/openagents/blob/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/attention-key128-probe-v1/README.md) and [failed integration](https://github.com/OpenAgentsInc/openagents/blob/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/attention-key128-integration-v1/README.md). |
| Sustained M5 submission diagnostic | 512 unprofiled FFNs reproduce the command-duration rise. Four-deep queueing reduces command gaps to about 15 µs but sustained commands remain 26–27 ms. Submission gaps alone do not explain the slowdown; no thermal or clock cause is established. All output/guard checks pass. [Full experiment](https://github.com/OpenAgentsInc/openagents/blob/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11/unprofiled-sustained-v1/README.md). |
| Two-/four-request Flash HTTP batch acceptance | Passed: 15 successful requests per mode have identical full-precision probabilities to serial references. Actual grouped FFN work, disconnect recovery, and saturation recovery pass. Each overload burst returns three `busy` refusals. The run overlaps compilation; no throughput improvement is claimed. |
| Prefix raw-logit parity and reuse speed | Passed on Flash and 27B: all 21 question comparisons per model have zero logit difference; repeated-state medians improve by 6.83× and 9.42×, respectively. Changed schemas, state changes, eviction, and restored hits pass. Shared-machine runs overlap CPU compilation and export raw logits. |
| 27B batching | Passed on Stage 5: two and four slots each compare 15 successful responses with identical serial probabilities, actual grouped FFN work, disconnect recovery, and overload recovery. Each burst returns three `busy` refusals. This functional run overlaps CPU diagnostics and does not establish a throughput target. |
| Flash paired latency | Stage 6 runs two rounds in reversed order, with ten measured requests per size and variant. Staged-head medians are `0.749 / 2.555 / 11.450 s`; fused-field medians are `0.740 / 2.576 / 11.328 s`; Ollama 0.40.2 medians are `0.841 / 2.914 / 11.964 s`. Both native modes beat Ollama in each round but **fail M3’s absolute targets**. No consistent head speedup is established. Native caches are disabled. Retrospective inspection finds 37–39 cached prefix tokens in 32 of 36 Ollama responses; the original analyzer overlooked its top-level cache field. |
| Uncached Flash and 27B timing against the reset Ollama (2026-10-12) | **27B fails the 1.5× gate:** the paired Ollama/native time ratio is 1.003 / 1.087 / 1.014 at ~1k / 4k / 16k. Native medians are 1.990 / 6.543 / 28.302 s; Ollama's are 1.977 / 7.101 / 28.610 s. Flash is 1.097 / 1.129 / 1.085: native 0.584 / 1.967 / 8.360 s, Ollama 0.629 / 2.221 / 9.045 s. The reset control, the chunk-2048 check and the wrapper's negative cases all pass, and every reply reports zero cached tokens. Shared machine at load 98–123. |
| Token-fast GEMM grid (`weight-major`) | **Rejected on speed:** it is bitwise equal on every trunk shape, and full-model Flash and 27B answers and pooled rows are identical. Paired, it is 18–30 % slower than `weight-shapes`. Not on main. |
| 27B paired latency | Stage 6 resident-f16 medians are `2.490 / 9.193 / 43.437 s`, versus Ollama 0.40.2 at `3.112 / 10.520 / 43.243 s`: `1.250× / 1.144× / 0.996×`, all **below M4’s 1.5× target**. One fixed-order round has five measured requests per size. Retrospective inspection finds 37–39 cached prefix tokens in 14 of 18 Ollama responses. Both comparisons pair request bytes and token counts, run on the shared machine without task-owned competing work, and preserve load and memory receipts. They establish neither idle-machine nor fully uncached Ollama acceptance. |

Stage 5 corrects DeltaNet normalization in CPU, Metal prefill, and CUDA
prefill to `x / sqrt(sum(x*x) + 1e-6)`, matching the executed Transformers
reference. An independent f64 oracle passes on the RTX 4080 across 12 synthetic
cases and 367,488 comparisons. CPU and Metal oracles also pass, but the fresh
27B text comparison still fails.
The same build carries the serial chunk's final normalization and head-input
preparation in its existing command buffer, removing one wait, and extends
opt-in GEMM routing to the eight observed 27B weight shapes. A fresh chunk
sweep does not establish a material speed improvement: later samples slow down
across all three configurations. Every median and warmup remains in the report.

Head-only probes also reproduce PyTorch 2.9.1 MPS instability, including
nonfinite outputs with unchanged inputs and weights. The cause remains unknown.
An independent reference with the official BF16 backbone and CPU-f32 head
completes all 40 cases with strict immutable-input checks. It agrees with the
original goldens on 103 of 104 top answers, with maximum probability difference
`0.0043`; the large native gap remains. This is artifact-quality evidence for
#11306, separate from same-Q4 runtime acceptance. The evidence bundle retains both references,
all probe failures, resource bounds, and exact source and artifact hashes.

On failing cases 014 and 034, the Rust head and the official CPU-f32 head
agree within `9.54e-7` in logits on identical native inputs. Substituting
reference hidden rows restores every top answer; substituting reference lexical
rows does not. This locates the large gap in those backbone outputs. The native
Q4 decoder also matches 1,327,104 independently decoded sample values bitwise.
An independent CPU-f32 execution of the same GGUF then reproduces case 014's
native wrong answer within `0.000163851` in probability. Running the original
checkpoint weights through the same f32 operators restores the BF16 answer
within `0.008714512`. Replacing only the 192 FFN matrices also restores that
case within `0.0186441` of BF16. These controls attribute this case's large miss
to quantized backbone weights; they do not establish the cause of every mismatch
or validate a replacement quantization. The owner moved that artifact-quality
work to #11306; the M4 runtime speed gate remains open.

## File-relevance calibration (X1, 2026-10-10)

Roadmap X1 ([#11216](https://github.com/OpenAgentsInc/openagents/issues/11216))
calibrates Clef-Flash's file-relevance probabilities. Evidence class:
`measured`.

**Data and prompt.** The data is `file-relevance-v1`, the time-split
corpus from #11215 ([file-finding-bench.md](file-finding-bench.md#the-decision-corpus-built-from-this-bench-file-relevance-v1-11215)).
Each request covers one (issue, file) pair: the issue (cut to 2,500
characters), then `FILE: path` and the file's first 2,048 bytes. The
question is the noul "Is this file relevant to solving the issue?". The
measurements ran on coderos-4080 against `Clef-Flash-Q4_K_M.gguf` (head
`sha256:6e469970…b041`), CUDA lane, f16 accumulate, at about 0.18 s per
request (about 1k tokens).

**The map.**
- **Fit.** A Platt map `σ(a·logit(p) + b)` with a = 1.1191 and b = 2.1443,
  fitted on the 1,952 calibration items.
- **Rejected alternative.** Temperature alone (t = 3.07) cannot move the
  0.5 crossing, so it leaves F1 where it was.
- **Where it lives.** The map is
  [`crates/psionic/fixtures/clef/calibration/file-relevance-v1.json`](../../crates/psionic/fixtures/clef/calibration/file-relevance-v1.json)
  (`openagents.clef.calibration.v1`, version 1).
- **Serving it.** `psionic-openai-server --decision-calibration FILE` applies
  the map to the noul with the same instruction text.
  - A map fitted on another head is refused at load.
  - The answer carries the calibrated probability.
  - `psionic.raw` holds the raw one, and `psionic.calibration_digest` sits
    beside `head_digest`.
  - `/v1/models` lists the maps.
  - A CPU-lane server with the map answered three development items at
    0.9404, 0.7178 and 0.6952. Applying the map in Python to the CUDA raw
    probabilities gives 0.9417, 0.7177 and 0.6979. The difference is the
    CPU/CUDA backbone gap, not the map.

| Split | Items | | F1 @ 0.5 | Precision | Recall | Accuracy | ECE | Brier | Log loss | Confident errors | AUC |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| development | 2,520 | raw | 0.437 | 0.914 | 0.287 | 0.653 | 0.196 | 0.247 | 0.722 | 200 | 0.860 |
| development | 2,520 | **mapped** | **0.760** | 0.780 | 0.741 | 0.781 | **0.044** | 0.156 | 0.483 | 32 | 0.860 |
| locked (read once) | 343 | raw | 0.246 | 0.926 | 0.142 | 0.554 | 0.313 | 0.322 | 0.949 | 52 | 0.844 |
| locked (read once) | 343 | **mapped** | **0.695** | 0.833 | 0.597 | 0.732 | **0.042** | 0.173 | 0.518 | 1 | 0.844 |

**How the columns are scored.**
- ECE, Brier, log loss, accuracy and confident errors follow `gym::gate`'s
  Scores, read on the winning option: `true` when p ≥ 0.5.
- On locked, the F1 gain is +0.449 with a standard error of 0.046
  (bootstrap over the 24 issues).

**Gate result.**
- **probability-v2 on development.** Every criterion passes, scored in
  Python against the `gym::gate` criteria; no `gym` gate run was made.
  - Log loss 0.722 → 0.483.
  - Confident errors 200 → 32.
  - ECE 0.196 → 0.044, a 78% reduction against the 10% needed.
  - Brier 0.247 → 0.156.
  - Error rate 0.347 → 0.219.
- **Roadmap bar.** The locked F1 of 0.695 improves on the raw baseline,
  as #11216 asks, but stays under the roadmap's 0.75 bar for X1.

**What the target is.** The label says the fix changed the file: a commit
that fixed the issue changed it. That is not the same as "relevant", which
is what the prompt asks (audit LEARN-03). The map calibrates the answer to
that narrower target.

**The map holds only on the distribution it was fitted on (LEARN-06).**
The corpus is about 48% fix files by construction (fix files plus near
neighbours), and the map moves the 0.5 crossing to a raw p of about 0.14.
On the natural candidate distribution, the finder's top 100 for the 100
#11210 bench issues (9,948 rows, 6.0% fix files), the same map makes things
worse:

| Finder top 100, bench issues | F1 @ 0.5 | ECE of p(true) | Brier on p(true) |
|---|---:|---:|---:|
| raw | 0.288 | 0.114 | 0.076 |
| mapped | 0.208 | 0.415 | 0.258 |

That is ECE +0.301 ± 0.006 and F1 −0.079 ± 0.018, bootstrapped over
issues. The map file now records the target, the fitted distribution (the
corpus digest, the partition, a 0.48 positive rate and the candidate rule)
and this check, and it says it is valid only for candidate lists drawn the
same way. It is opt-in (`--decision-calibration`), and no door applies it.
A map for finder pools would have to be fitted on pool rows from the
calibration issues. That run was stopped to free the 4080 for production
decisions and is not done.

**Limits.**
- **Base rate.** See above: the map is bound to the corpus's base rate.
- **Ranking.** AUC does not change, because a monotone map cannot reorder
  candidates.

Reproduce:

```sh
python3 -I scripts/bench/file-relevance-clef.py run --corpus C --partitions calibration,development --base-url URL --out R
python3 -I scripts/bench/file-relevance-clef.py calibrate --corpus C --results R --out map.json
python3 -I scripts/bench/file-relevance-clef.py score --corpus C --results R_locked --map map.json --partitions locked --locked-read
```

## Sources read

- Cloudflare's reference implementation, `joint_schema_model.py`. The file is
  the same in `Cloudflare/clef-flash` and `Cloudflare/clef`. We also read
  `config.json`, `joint_head_config.json` and the `joint_head.safetensors`
  header for both models.
- The `ggml-org/Clef-Flash-GGUF` Q4_K_M header: metadata and tensor table.
- Ollama `v0.40.0` (`0d0720e`):
  - `decision/clef.go` and `decision/systemone.go` (encoder, answers, limits);
  - `server/routes.go` (`SystemOneHandler`);
  - `mlxrunner/model/clef/{model,head,encode,score}.go` (the MLX head);
  - `llama/clef/clef.{h,cpp}` and `llama/compat/002-clef.patch` (the llama.cpp path);
  - `create/clef.go` and `llama/clef/convert.py` (import and conversion).

  Our reference clone under `projects/repos/ollama` is `v0.33.2`, which has no
  Clef support, so we read `v0.40.0` from a scratch clone.
- Psionic:
  - `crates/psionic-serve/src/qwen35.rs`: the CPU, CUDA and Metal qwen35
    services;
  - `crates/psionic-serve/src/openai_http.rs`;
  - `crates/psionic-backend-{cuda,metal}/src/lib.rs`;
  - `crates/psionic-models/src/runtime_tokenizer.rs`;
  - `docs/INFERENCE_ENGINE.md` and `docs/qwen38/`.
- OpenAgents:
  - `crates/jev` (`config.rs`, `questions.rs`, `doors.rs`);
  - `crates/jev-hosted`;
  - `crates/coder` (`first.rs`, `coder-worker.rs`);
  - `crates/inference/src/upstream/psionic.rs`;
  - `crates/pylon` (`engine.rs`, `provider.rs`, `paid.rs`);
  - `crates/psionic`, the vendored serving copy.

## What Clef is

Clef is a decision model. It reads one prompt in a single prefill, generates no
tokens, and returns one logit per allowed option of every question.

| | Clef-Flash | Clef |
| --- | --- | --- |
| Backbone | Qwen3.5-9B (`qwen3_5`), post-trained | Qwen3.8-27B (`qwen3_5` config, `output_gate_type: swish`) |
| Layers | 32: 24 Gated DeltaNet + 8 gated full attention (every 4th) | 64, same 3:1 pattern |
| Hidden / FFN | 4096 / 12288 (SwiGLU) | 5120 / 17408 |
| Full attention | 16 Q heads, 4 KV heads, head_dim 256, partial rotary 0.25 (64 dims), interleaved MRoPE sections [11, 11, 10], θ = 1e7, sigmoid output gate, q/k RMSNorm | 24 Q heads, 4 KV heads, sigmoid output gate, matching the official reference implementation |
| Linear attention | 16 key heads, 32 value heads, dims 128, causal conv kernel 4 | 16 key heads, 48 value heads |
| Vocabulary | 248,320, untied `lm_head` | same |
| Joint head | 121.8 M params (243 MB bf16): width 1024, 2 routing layers, 4 field layers, 16 heads, FFN 4096 | width 1024, `hidden_size` 5120 |
| Trained prompt length | 16,384 tokens (reference `max_length`) | same |

The backbone is a stock Qwen3.5 text decoder that produces final hidden
states. It ends with the output RMSNorm, the same `last_hidden_state` that HF
returns, and **never runs the LM head as a matmul**. The `lm_head` matrix is
used only as an embedding table for option tokens.

### Prompt format (exact)

The reference `encode_record` builds the token sequence from separately
tokenized segments. Ollama's `encodeClef` keeps the same segment boundaries.
Each segment is encoded on its own with `add_special_tokens=False`, and the
chat markers inside the strings are parsed as special tokens. Tokenizing the
concatenated string instead gives different ids at segment joins.

```text
P  = "<|im_start|>system\nRead the complete state and schema. Decide every field jointly. Each answer must be exactly one of that field's allowed options.<|im_end|>\n<|im_start|>user\nSTATE:\n"
[images: "<|vision_start|><|image_pad|><|vision_end|>" * n  + "\n"   (processor-expanded, after P)]
S  = render(state)
     "\n\nSCHEMA FIELDS:\n"
per question k (request order), id, type:
     "\nFIELD {k}\nID: {id}\nTYPE: {type}\nINSTRUCTION: "
     render(instructions or id)                        <- question span
     "\nALLOWED OPTIONS:\n"
     per option j:  "OPTION {j}: "
                    render({"option_id": id, "description": d})   <- option span ("description" omitted when null)
                    "\n"
     "END FIELD\n"
X  = "\n<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\nJOINT SCHEMA DECISIONS:"
```

The pieces are:

- `render(v)` returns `v` unchanged when `v` is a string. Otherwise it returns
  Python `json.dumps(v, ensure_ascii=False, sort_keys=True, separators=(",", ":"))`.
  That keeps Python's int and float spelling (`1.0`, `1e-05`, big ints
  exact). Ollama reimplements it as `clefJSON`. Psionic needs serde_json with
  `arbitrary_precision` and a Python-repr float formatter.
- **Option order.**
  - noul: `true`, then `false`.
  - choice: criteria keys sorted by code point. The answer maps them back to
    request order.
  - score: `"0".."n-1"` in the given order.
- **Default noul descriptions:** "The proposition is true or the answer is
  yes." and "The proposition is false or the answer is no." These are
  overridden by `criteria.true` and `criteria.false`.
- **Truncation (reference only).** `max_length = 16384`. If
  `P + schema + X` is longer than that, the reference raises an error.
  Otherwise it cuts the state tokens from the end (`state_ids[:max - fixed]`)
  **silently**.

### Joint head math

Notation:

- `H ∈ R^{L×d}` are the final hidden states.
- `E` is `lm_head.weight` (`[V, d]`).
- `LN` is LayerNorm with affine weights and eps 1e-5.
- `GELU` is the exact erf form.
- `MHA(q, k, v)` is PyTorch `MultiheadAttention`: 16 heads, head dim 64,
  packed `in_proj` with biases, `out_proj` with bias, scale 1/8, **no mask**.
- `n(x) = x / max(‖x‖, ε)`.

```text
Ĥ   = LN_hidden(H)                                   # [L, d]
M   = Ĥ W_mem                                        # [L, 1024]   memory, all tokens (prefix, state, schema, suffix)
g   = Ĥ[L-1]                                         # global vector: last token of "JOINT SCHEMA DECISIONS:"
q_i = mean_{t ∈ Qspan_i} Ĥ_t                          # question vector, d-dim
c_ij = mean_{t ∈ Ospan_ij} Ĥ_t                        # option context, d-dim
ℓ_ij = mean_{t ∈ Ospan_ij} E[x_t]                     # option lexical, d-dim, rows of the untied lm_head

R   = concat_ij( W_oc c_ij + W_ol ℓ_ij + W_oq q_i )   # [N, 1024], N = Σ options, all questions in one sequence
2 × evidence routing:
    R = R + MHA(LN_q(R), LN_m(M), LN_m(M))            # options attend to every memory token; not to each other
    R = R + W2 GELU(W1 LN_f(R) + b1) + b2

b_i = W_q q_i
s_i = Σ_j softmax_j(⟨R_ij, b_i⟩ / √1024) R_ij
F_i = b_i + LN_sum(s_i) + W_g g + T[type_i]           # type: noul 0, choice 1, score 2
4 × field layer (nn.TransformerDecoderLayer, norm_first, GELU, no masks):
    F = F + MHA(LN1 F, LN1 F, LN1 F)                  # questions attend to each other ("decide jointly")
    F = F + MHA(LN2 F, M, M)                          # memory is NOT normalized here
    F = F + W2 GELU(W1 LN3 F + b1) + b2
F = LN_field(F)

o_ij   = LN_opt(R_ij)
prior  = exp(min(α, ln 100)) · ⟨n(ℓ_ij), n(q_i + g)⟩            # n with ε 1e-12
joint  = exp(min(β, ln 100)) · cos(F_i, o_ij)                   # ε 1e-8
       + w_out · GELU(W_r [F_i; o_ij; F_i⊙o_ij; |F_i − o_ij|] + b_r) + b_out
logit_ij = prior + σ(γ) · joint                                 # α, β, γ = prior_logit_scale, joint_logit_scale, residual_gate
p_i = softmax_j(logit_ij)
```

Answers, following the reference `systemone_answer`:

- **noul:** `noul = P(true)`.
- **choice:** `choice = argmax`, `probabilities` in request key order,
  `confidence = max p`.
- **score:** `score = Σ k·p_k`, `legend` = the criteria,
  `confidence = max p`.

The reference rounds to 4 decimals. Ollama computes `confidence` as
`1 − H(p)/ln n` instead, and llama.cpp uses a third formula. Psionic follows
the reference and documents it. Callers must read probabilities, not
`confidence`.

Consequences for serving:

1. **The head needs every hidden row**, not just the last one. Memory
   attention is unmasked over all L tokens.
2. **The backbone is causal.** Chunked prefill that carries KV, conv and delta
   state produces mathematically identical hidden rows. llama.cpp needs one
   physical batch of at least L tokens, and Ollama's patch sets
   `n_ubatch = n_ctx`, because its embedding mode returns per-token outputs
   only within one ubatch. That requirement is an artifact of the engine, not
   of the model.
3. **The head consumes `Ĥ` only through three quantities:**
   - `M = LN(H) W_mem` (1024-wide);
   - span means of `LN(H)` over question and option spans;
   - `g`.

   A chunked prefill can therefore stream them: project each chunk's memory
   rows, add each chunk's rows into the span sums, keep the last row, and drop
   the 4096-wide activations. Memory for the head drops to L × 1024 × 2 bytes
   (32 MB at 16k, 128 MB at 64k).
4. **`lm_head` is never multiplied.** Only the rows of option tokens are
   gathered (a few thousand at most), so `output.weight` (0.83 GB as Q6_K)
   and `token_embd` (0.57 GB as Q4_K) can stay host-resident and memory-mapped.
5. **Questions interact.** Adding or removing a question changes the other
   answers. Splitting a request is not answer-preserving, which matters for
   I3.
6. **The 26-option limit is Ollama's, not Clef's.** Ollama's generic
   decision compiler gives options letter codes `A`–`Z` for Tev1/Nimble, and
   the Clef path keeps the same `2–26` check (`clefField`, and again in
   `mlxrunner/model/clef/encode.go`). The reference has no cap, and the head
   scores spans, not letters.

### Artifact layouts to admit

The **`ggml-org/Clef-Flash-GGUF`** layout (llama.cpp b11538, ggml-org/llama.cpp#29831):

- `general.architecture = clef`.
- The qwen35 backbone keys are renamed `clef.*`. They include
  `clef.attention.recurrent_layers`, `clef.full_attention_interval = 4` and
  `clef.ssm.*`.
- The decision keys are `clef.decision.{type=clef, routing_block_count=2,
  block_count=4, head_count=16}` and
  `clef.attention.layer_norm_epsilon = 1e-5`.
- Head tensors:
  - `decision.{hidden,option,option_summary,field}_norm.{weight,bias}`;
  - `decision.proj_{memory,question,option_question,global,option_context,option_lexical}.weight`;
  - `decision.scorer[.bias]`, `decision.scorer_out[.bias]`;
  - `decision.scales[3]`;
  - `token_types.weight [1024, 3]`.
- Evidence layers are `dec.blk.0-1.{cross_attn_{q,k,v,o},cross_attn_norm,cross_attn_norm_kv,ffn_{up,down},ffn_norm}`.
- Field layers are `dec.blk.2-5`, which add `attn_{q,k,v,o}` and `attn_norm`.
- In the Q4_K_M file, the head matrices are Q8_0 and `scorer_out` is BF16.
  The backbone is Q4_K/Q6_K with Q8_0 attention and SSM projections, and
  `output.weight` is Q6_K. The vision tower ships separately as `mmproj-*`.

The **Ollama** layout (`llama/clef/convert.py`): the architecture stays
`qwen35` and gains `qwen35.decision.type = clef` and
`qwen35.decision.{hidden_size,width,routing_layers,layers,heads,feedforward}`.
The head tensors are `clef.<torch name>` in f32.

The **HF** layout: backbone safetensors, plus `joint_head.safetensors` (bf16,
torch names such as `evidence_layers.N.attention.in_proj_weight`,
`layers.N.multihead_attn.*` and `residual_scorer.{0,3}.*`) and
`joint_head_config.json`.

M1 admits the ggml-org GGUF as the primary artifact. It also admits the HF
head file beside a qwen35 GGUF, which gives an f32/bf16 head for parity runs.
Each of the three layouts maps onto one internal `ClefHeadWeights`.

## What Psionic has and what is missing

This table records the original pre-M1 implementation inventory. The dated
status sections above describe what has since been built and measured.

| Area | Today | Gap for Clef |
| --- | --- | --- |
| Qwen3.5 hybrid graph | `implemented_early`. CPU, CUDA and Metal `GgufQwen35TextGenerationService` in `psionic-serve/src/qwen35.rs`, serving `qwen3.5-0.8b-q8_0` on CUDA for Pylon. Qwen3.8 lanes are `implemented_early` on CPU and CUDA and `partial` on Metal. | Admit `general.architecture = clef` (the same graph with `clef.*` keys). Export per-token final hidden rows. Today only `final_hidden_and_logits_for_tokens` exists, a CPU diagnostic that returns only the last row. |
| Prompt ingestion | **Token-at-a-time on every backend.** Prompts run through `forward_token` (matvec per token, `gated_delta_step_f32`, `depthwise_causal_conv1d_step_*`). | This is the main gap. A decision is all prefill. At decode-like rates (order 10² tok/s for a 9B Q4 on the 4080; an estimate, not measured) a 16k request takes minutes. Psionic needs sequence prefill: quantized GEMM, sequence conv1d, the chunked gated delta rule and causal flash attention. |
| Quantized GEMM | CUDA: `quantized_matvec` and Q8_1 mmvq, plus cuBLASLt `matmul_f16_to_f32` over dense f16 mirrors. Metal: `quantized_matvec` and `quantized_matvec_batch`. | Add Q4_K, Q5_K, Q6_K and Q8_0 × activation GEMM (mmq or dequant-tile) on CUDA and Metal. |
| Full attention | Decode attention over an f16 KV cache with fused MRoPE. A bounded `attention_causal_sequence_{f32,bf16}` (training lane). | Add causal flash-attention prefill with GQA 16/4, head_dim 256, partial interleaved MRoPE and output gate, appending to the KV cache across chunks. |
| Gated DeltaNet | Single-step recurrent update (CUDA `gated_delta_step_f32`, CPU, Metal). | Add a chunked (WY/UT) gated delta rule kernel with chunk 64 that carries the state between chunks. Validate it against the step kernel. |
| Tokenizer | GGUF BPE with `qwen35` pretokenizer (NFC, per-digit) and special-token splitting (`encode_with_special_tokens`). | Encode segment by segment, and run a parity corpus against HF `tokenizer.json`. |
| Encoder / renderer | None. | Port `encode_record` and `render` (byte-exact, Python JSON rules). |
| Joint head | None. | Add the head on CPU (M1), then CUDA and Metal (M2, M3). Small matmuls, LayerNorm, 16-head attention against L memory rows, GELU, softmax. |
| Prefix reuse | `Qwen35SharedPrefixStore` (token prefix → backbone state). | Extend the cache entry with memory rows for the prefix tokens (M4). |
| Vision | qwen35 image and video projection on CUDA; qwen38 vision on CPU and CUDA. | Clef image placement after `P`. Metal media (M4). |
| HTTP | `psionic-openai-server`: `/v1/models`, `/v1/chat/completions`, `/v1/responses`, `/v1/embeddings`. | Add `/v1/systemone`, model capability `decision`, and receipts. |
| Quantization formats | GGML Q4_0, Q4_1, Q5_0, Q3_K, Q4_K, Q5_K, Q6_K, Q8_0, MXFP4, IQ3_S, IQ4_XS; BF16 and F16 dense. | Q8_0 and BF16 head tensors and Q6_K row gather from `output.weight` already have storage support. Ollama's Mac `mxfp8` and `nvfp4` are MLX formats, and we do not need them: we use the GGUF Q4_K_M, Q8_0 and BF16 builds. |

**Where the code lands.** `openagents/crates/psionic` is an import of
psionic `02e0bc85`, and its README says changes after the import happen
there. It is the copy `pylon-psionic` builds. Upstream `main` has not moved
since that commit, and the serving sources are identical.

Engine issues and this plan live in this repo, which is execution truth.
Engine PRs land in `openagents/crates/psionic`, each naming its psionic
issue, so Pylon ships them. This doc and `docs/INFERENCE_ENGINE.md` record
status here. If upstream development resumes, port the changes back as
whole crates.

## `/v1/systemone` route spec

Mounted on `psionic-openai-server` when a loaded model has the `decision`
capability. The CLI adds `--decision-max-tokens <n>` (default 16384) and
`--decision-chunk <n>` (default 2048).

**Request.** This is the TypeSafe/Jev System One body, which Ollama and the
Cloudflare reference also accept.

```json
{
  "model": "clef-flash",
  "state": "string | any JSON",
  "questions": {
    "<id>": {"type": "noul" | "choice" | "score",
             "instructions": "string | any JSON (optional; defaults to <id>)",
             "criteria": {"true": "...", "false": "..."} | {"<key>": "desc" | any | null} | ["level 0", "level 1"]}
  },
  "images": ["<base64 png/jpeg/webp>"],
  "truncation": "refuse" | "state_tail",
  "keep_alive": "ignored"
}
```

**Response.** The same shape as Jev and the reference. Answers come in
request order, and probabilities are not rounded.

```json
{
  "model": "clef-flash",
  "answers": {
    "urgent": {"type": "noul", "noul": 0.9731},
    "team":   {"type": "choice", "choice": "technical", "probabilities": {"billing": 0.02, "technical": 0.98}, "confidence": 0.98},
    "level":  {"type": "score", "score": 1.62, "legend": {"0": "...", "1": "...", "2": "..."}, "probabilities": {"0": 0.05, "1": 0.28, "2": 0.67}, "confidence": 0.67}
  },
  "usage": {"input_tokens": 3195, "output_tokens": 0, "cached_input_tokens": 0},
  "psionic": {"artifact_digest": "sha256:…", "head_digest": "sha256:…", "backend": "cuda",
              "execution_mode": "native", "prefill_chunk": 2048, "truncated_state_tokens": 0,
              "trained_length": 16384, "latency_ms": 612}
}
```

**Limits, compared with the other servers:**

| Limit | Ollama 0.40 | Reference / SGLang / MLX | Psionic |
| --- | --- | --- | --- |
| Options per choice or score | 2–26 (refused outside) | unbounded | 2–255 (Jev's client cap). The real bound is the token budget. Wide questions (52 and 55 options) are covered by fixtures before the limit is published. |
| Questions | 1–64 | unbounded | 1–64 by default (configurable) |
| Request size | 64 KiB of text and schema (a temporary memory guard, marked TODO in `routes.go`) | none | Admission is by **tokens**. The body is capped at 8 MiB including images. |
| Prompt tokens | the runner's context length; refused above it | 16,384; the state is truncated silently | Default budget 16,384 (the trained length). Above it, the default is `refuse`, with HTTP 413 and `{"error":{"code":"not_admitted"}}` so the Jev door fails over. `truncation: "state_tail"` reproduces the reference's truncation and reports `truncated_state_tokens`. An operator may raise `--decision-max-tokens` to 32k or 64k. Chunked prefill makes those lengths cheap, and the response then adds `out_of_training_distribution: true`. |
| Video | refused | supported | refused until M4+ |
| `GET /v1/models` | OpenAI list | — | OpenAI list, with `capabilities: ["decision"]` (Jev's `models().list()` expects TypeSafe's shape and is not used by the doors) |

Other error codes:

- `invalid_request` (400): malformed schema, unknown type, empty criteria,
  fewer than 2 options. These do not fail over, which is correct, because the
  question itself is wrong.
- `busy` / `overloaded` (503): the queue is full.
- `unavailable` (503): the model is not loaded.

## Correctness plan

Reference ladder, from strictest to loosest:

1. **Encoder exactness.**
   - Corpus: 200 or more synthetic public records, covering unicode, emoji,
     control characters, floats (`1.0`, `1e-05`, `1e16`), big ints, nested
     JSON states and instructions, 2–60 options, 1–13 questions, and empty or
     missing instructions.
   - Expected: token ids and spans from the reference `encode_record` with the
     HF `tokenizer.json`.
   - Tolerance: zero differences.
2. **Head exactness.**
   - Inputs: hidden states dumped from the reference backbone, fed to both the
     torch f32 head and the Psionic head.
   - Tolerance: max |Δlogit| ≤ 1e-3.
   - The reference runs its head in bf16. We keep f32 (Ollama's GGUF path
     does the same) and record the bf16-versus-f32 delta as a characterization
     row.
3. **Backbone parity on the same GGUF.**
   - Compared against llama.cpp b11538 run in embeddings mode with
     `-ub ≥ L`.
   - Tolerance: per-layer and final hidden cosine ≥ 0.999 and normalized RMSE
     ≤ 0.02, using the same harness as the retained qwen38 layer-zero
     comparator.
4. **End to end on the same GGUF.**
   - Compared against llama.cpp b11538 and Ollama 0.40 (`hf.co` import) on the
     fixture corpus (requests ≤ 16k tokens, ≤ 26 options for Ollama).
   - Tolerance: top answer 100%, max |Δp| ≤ 0.02, noul |Δ| ≤ 0.02.
5. **Quantized versus BF16 reference.**
   - Compared against Cloudflare `systemone` in BF16. It runs on CPU or MPS on
     the 128 GB Mac; we do not have an H200.
   - Characterization targets: top answer agreement ≥ 97%, median |Δp|
     ≤ 0.02, max |Δp| ≤ 0.10.
6. **Chunk equivalence.**
   - Chunk sizes {whole, 2048, 512, 64, 1} on every backend.
   - Tolerance: max |Δlogit| ≤ 1e-3 and identical argmax.
   - A repeated request on the same backend and chunk size is bitwise
     identical (deterministic reductions, no atomics in reductions).
7. **Product parity.**
   - Replay the 28 openagents product-note requests and compare with the
     recorded hosted Jev answers.
   - Agreement must be ≥ the Ollama baseline (94% for Clef-Flash, 97% for
     Clef 27B).
   - These request bodies come from the bench goldens. Only bodies cleared as
     public go into `fixtures/clef/`.

Fixtures go under `fixtures/clef/`:

- `encoder/`: records, ids and spans;
- `head/`: hidden-state slices plus logits, kept small by trimming L;
- `e2e/`: requests plus the probabilities from each comparator, with the
  comparator name, revision and artifact digest.

Receipts record the comparator revisions, and no private production data
enters them.

## Performance targets

Latency here means total request latency. For a decision model, time to
first token and total latency are the same thing, because no tokens are
generated. Benchmarks use a unique state nonce per request (no prefix cache),
5 measured runs after warmup with medians reported, and an idle device
(AGENTS.md GPU idle query; no competing Metal workload).

| Device / artifact | Comparator (measured 2026-10-09) | Gate | Stretch |
| --- | --- | --- | --- |
| RTX 4080, Clef-Flash Q4_K_M (M2) | llama.cpp b11538: 0.19 s at 1k, 0.65 s at 4k, 3.25 s at 16k (about 4.9–6.3k tok/s), about 10.2 GB VRAM at a 17k batch | ≤ 0.20 / 0.65 / 3.25 s; ≤ 7.5 GB Psionic-owned VRAM at 16k; 32k and 64k admitted | 1.2× faster at 4k and 16k |
| M5 Max, Clef-Flash Q4_K_M (M3) | MLX card, idle: 0.31 s at 1k, 7.0 s at 16k. Ollama 0.40 mxfp8 under heavy load: 1.25 / 4.6 / 19.2 s (about 850 tok/s flat) | idle ≤ 0.35 s at 1k and ≤ 7.5 s at 16k; beats Ollama in interleaved same-session runs | match the MLX card |
| M5 Max, Clef 27B Q4 (M4) | Ollama 0.40 nvfp4: 3.9 / 14.3 / 60.3 s | ≥ 1.5× faster than Ollama | — |
| Head cost | — | ≤ 5% of total latency at 4k | — |

The CUDA budget has room. At 16k, prefill is about 2 × 9 B × 16k ≈ 2.9e14
FLOPs, so llama.cpp's 3.25 s is already about 90 TFLOP/s effective on the
4080. Parity requires tensor-core quantized GEMM and a chunked delta rule, not
a cleverer algorithm.

The memory target follows from the layout:

| Item | Estimate |
| --- | --- |
| Device weights, without `output` and `token_embd` | about 5.1 GB |
| f16 KV cache: 8 layers × 2 × 4 × 256 × 2 B | 32 KB/token, 0.5 GB at 16k |
| Delta state | about 50 MB |
| Chunk-2048 scratch | under 1 GB |
| Memory rows | 32 MB |

## Integration into OpenAgents

**Status (2026-10-10, #11225).** Items 1 and 3 are built differently from
the plan below, free and through our API: `openagents pylon serve --decide`
answers NIP-DEC jobs with a local Clef server and advertises
`pylon/decision` on the `cj-decision` lane, and the gateway's
`POST /v1/systemone` (now public at `openagents.com/api/v1/systemone`)
sends every decision to such a pylon first, then Gemini on Vertex. Every
Jev caller resolves to that API by default (`jev_hosted::resolve`).
CoderOS-4080 is the first pylon (`coderos-4080-clef`, Clef-Flash Q4_K_M on
CUDA). See [the pylon guide](../compute/pylon.md#answer-decisions-clef).

1. **Jev door** ([#11191](https://github.com/OpenAgentsInc/openagents/issues/11191), after [#11189](https://github.com/OpenAgentsInc/openagents/issues/11189)).
   - `jev::Config::local("http://127.0.0.1:<port>", "clef-flash")` already
     talks to any loopback System One server, so `crates/jev` needs no
     protocol change.
   - The new door goes at the end of `jev_hosted::local_doors`, with
     `service.door = "local-clef"`.
   - Psionic's `not_admitted` refusals fail over, because `not_admitted` is in
     `DOOR_OWN_CODES`.
   - This fixes a gap in #11189's plan. `doors.rs` never fails over on a 400
     or 413, so a door that refuses with Ollama's 400 (more than 26 options)
     or 413 (more than 64 KiB) ends the chain instead of passing the request
     on.
2. **coderos-4080.** `pylon-psionic` loads Clef-Flash Q4_K_M next to
   `qwen3.5-0.8b-q8_0`. The budget: about 11.6 GB VRAM free beside the Pylon
   services, Clef ≤ 7.5 GB at 16k, and the current model 1.6 GB.
3. **Pylon sells decisions** ([#11192](https://github.com/OpenAgentsInc/openagents/issues/11192)).
   - A `<pubkey>:pylon/decision` capability with operation `decide`, priced
     per input token.
   - The beacon advertises the Psionic artifact digest and admission limits.
   - The buyer side is a carried door in `jev-hosted` through the gateway.
4. **Router** ([#11193](https://github.com/OpenAgentsInc/openagents/issues/11193)).
   - Native serving removes Ollama's limits but not the head's trained
     length, or Clef's reading of `none`.
   - The router's 24.5k-token decision needs a split, and gating questions
     must drop `none`, before any Clef door can take it inside the 6 s
     `first::LATE` budget.

## Milestones

**M1: correct lane** ([#11194](https://github.com/OpenAgentsInc/openagents/issues/11194)). This is the first milestone.

Scope:

- Admit the ggml-org Clef GGUF, plus an HF head beside a qwen35 GGUF.
- Port the byte-exact encoder.
- Run the f32 joint head on the CPU.
- Make the CPU qwen35 lane export streamed hidden rows. The existing
  token-at-a-time path is acceptable here.
- Mount `/v1/systemone` with the limits and refusals above and the `psionic`
  receipt.
- Build the fixture corpus and comparator harness, ladder steps 1, 2 and 4.

Done when:

- Zero encoder differences.
- Head |Δlogit| ≤ 1e-3.
- 100% top-answer agreement and |Δp| ≤ 0.02 against llama.cpp on the same
  GGUF, for records up to about 1k tokens. The CPU lane is slow by design.
- The `crates/jev` quickstart answers through `Config::local`.
- This document publishes the lane as `implemented_early` (CPU); see
  [M1 status](#m1-status-2026-10-09).

**M2: CUDA speed** ([#11195](https://github.com/OpenAgentsInc/openagents/issues/11195)). This milestone adds:

- chunked prefill (quantized GEMM, sequence conv1d, chunked gated delta,
  causal flash attention);
- streamed memory rows and span sums;
- the head on the device;
- host-resident `output` and `token_embd`.

It passes the chunk-equivalence gate and the RTX 4080 gate.

**M3: Metal speed** ([#11196](https://github.com/OpenAgentsInc/openagents/issues/11196)). It applies the same design on Metal, with device-resident chunk loops. It passes the M5 Max gate.

**M4: breadth** ([#11197](https://github.com/OpenAgentsInc/openagents/issues/11197)). This milestone adds:

- Clef 27B on the Qwen3.5 backbone, Metal first;
- image inputs;
- exact prefix reuse with cached memory rows;
- continuous batching of backbone chunks across requests.

The integration issues I1–I3 can start once M1 has landed. I1 goes live on
coderos after M2.

### Direct Q4_K projections (2026-10-12, #11196)

An opt-in GEMM reads Q4_K weight bytes directly: `clef_gemm_q4k` in
`kernels/clef_prefill.metal`, `ClefMetalBatch::gemm_q4k`, used for packed
feed-forward weights with `PSIONIC_CLEF_METAL_FFN_WEIGHTS=q4k-on-demand
PSIONIC_CLEF_METAL_Q4_GEMM=direct`. Each 128-token × 64-row tile decodes one
128-wide k slab of its weight rows into threadgroup f16, one Q4_K sub-block per
thread with its scale and min decoded once, and accumulates the tensor-op
product in a register cooperative tensor. No f16 copy of the weight exists.

- **Correctness.** Against decode-then-GEMM on synthetic shapes with partial
  tiles: within 1e-4 relative (`clef_q4k_direct_gemm_matches_decode_then_gemm`).
  On Clef-Flash (`cuda_chunks_and_cpu_agree` with `PSIONIC_CLEF_TEST_DEVICE=metal`):
  every chunk size equals the whole prompt bitwise, and against the CPU lane
  max |Δlogit| is 2.74e-4 and max |Δp| 1.1e-5, inside the 1e-3 bound.
- **Kernel speed** (`clef_q4k_direct_gemm_timing_against_f16`, Flash and 27B
  FFN shapes, shared machine). Sustained, 100 repeats: 0.92–0.97 of f16
  throughput at 4k tokens and 0.60–0.94 at 1k. The first version, decoding
  value by value in 64-wide steps, ran at 0.16–0.28. Decoding a whole
  sub-block per thread, 128-wide slabs, a register accumulator and 128-token
  tiles brought it near f16, but not past it.
- **Model speed** (Flash, `bench_latency.py`, 5 fresh-prefix runs, two
  interleaved rounds per mode, machine load 35–47): f16 resident 1.20 / 3.48 /
  14.9 s at ~1k / 4k / 16k, direct Q4_K 1.32 / 4.23 / 16.4 s. Direct is
  10–20 % slower on this machine. Its gain is memory: feed-forward weights
  stay 4-bit, about 3.5× smaller than resident f16, which matters for 27B
  (48 GiB resident).
- **Not the M3 lever on this hardware.** The f16 projections are not limited
  by weight bandwidth often enough for 4-bit reads to win, and the decode
  repeats per token tile. Remaining ideas: decode once per weight tile into a
  persistent f16 tile shared across token tiles (a split-k or weight-stationary
  grid), and overlapping decode with the tensor op across simdgroups.
