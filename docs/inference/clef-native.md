# Native Clef Decision Serving Plan

> Moved from the psionic repo on 2026-10-09: the standalone psionic repo is reference only. All Clef engine work and its issues live in this monorepo (`crates/psionic`, issues #11194–#11197).


Status: M1 `implemented_early` on the CPU (2026-10-09); M2 `implemented_early` on CUDA (2026-10-10), gate partly met; M3–M4 `planned`. See [M1 status](#m1-status-2026-10-09) and [M2 status](#m2-status-2026-10-10). File-relevance calibration map (X1): [below](#file-relevance-calibration-x1-2026-10-10).

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
(sha256 `fd3e9060…638c`). The fixtures and tools are in
`crates/psionic/fixtures/clef/`.

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

**Next for M2 speed:**

1. The head's 12 ms and the host embedding gather, for margin at 1k.
2. Make the fused GEMM as fast as cuBLAS on large chunks, then make it
   the default for bitwise chunk invariance at f16 speed.

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
| Full attention | 16 Q heads, 4 KV heads, head_dim 256, partial rotary 0.25 (64 dims), interleaved MRoPE sections [11, 11, 10], θ = 1e7, sigmoid output gate, q/k RMSNorm | 24 Q heads, 4 KV heads, swish output gate |
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

- Clef 27B on the Qwen3.8 lanes, Metal first;
- image inputs;
- exact prefix reuse with cached memory rows;
- continuous batching of backbone chunks across requests.

The integration issues I1–I3 can start once M1 has landed. I1 goes live on
coderos after M2.
