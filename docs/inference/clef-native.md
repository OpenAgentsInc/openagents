# Native Clef Decision Serving Plan

> Moved from the psionic repo on 2026-10-09: the standalone psionic repo is reference only. All Clef engine work and its issues live in this monorepo (`crates/psionic`, issues #11194–#11197).


Status: `planned` (2026-10-09). Nothing in this document is implemented yet.

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
- `INFERENCE_ENGINE.md` publishes the lane as `implemented_early` (CPU).

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
