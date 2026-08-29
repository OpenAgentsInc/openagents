# Qwen 3.8 inference pipeline

- Class: conceptual pipeline
- Status: describes the OpenAgents approach; load through fixture
  `gen.done` landed (#344–#347, #352–#356). Hybrid 64-layer decode is
  #357. Speed vs Ollama is [PARITY.md](./PARITY.md).
- Date: 2026-08-29
- Intent: [INTENT.md](./INTENT.md)
- Plan: [PLAN.md](./PLAN.md)
- Facts: Psionic `docs/qwen38/` in the sibling repository
- llama.cpp bytes: [LLAMA_CPP_INFERENCE_PIPELINE.md](./LLAMA_CPP_INFERENCE_PIPELINE.md)
- CLI statuses: [CLI.md](./CLI.md)

You already run Qwen 3.8 locally through Ollama (`qwen3.8:27b-mtp-q8_0`). This
document is the same path inside `openagents`: every step from a weight file
on disk to a generated token, including the tensor operations each step uses.

Where a step already exists in Psionic, the text says so. Where it does not,
or where OpenAgents has not wired it into the CLI, the text says that too.

## The loop in one page

Inference is repeated next-token prediction.

1. Turn conversation text into token IDs.
2. Look up each ID in the embedding table. You get one hidden vector per
   token.
3. Run those vectors through 64 decoder layers.
4. Normalize the last hidden vector and multiply by the language-model head.
   You get a *logit* for every vocabulary entry.
5. Sample or greedily pick one next token ID.
6. Append that ID and repeat from step 2 for a single new position until an
   end-of-sequence ID or a length limit.

Steps 2 through 4 use the weights. Everything else is control: files,
admission, backends, sampling, tools, and streaming.

```text
file on disk
    → admit (identity, digest, family, memory)
    → load tensors onto a backend
    → tokenize and apply the chat template
    → embed
    → prefill all prompt positions
    → for each new token: one decode step
    → sample
    → stream the piece of text to Coder
```

The terminal labels for that path (`Looking for GGUF`, `Reading metadata`,
`Inference complete`, and the rest) are [CLI.md](./CLI.md). You watch
them on `openagents inference run` while the slices land.

## Status labels

Implemented in Psionic
: Landed in `OpenAgentsInc/psionic` with retained evidence.

Partial in Psionic
: Code exists; the claim is incomplete.

Planned in Psionic
: Named in the Qwen 3.8 roadmap, not the first Coder claim.

OpenAgents not started
: This repository has no in-process path for that step yet. Coder
  `--local` still uses Ollama. Load through `map.done` is started.

## Model numbers

Keep three names distinct:

- Product identity: `Qwen/Qwen3.8-27B`
- Served identity: `qwen3.8-27b`
- Decoder architecture: `qwen3_5_text` (a converted GGUF often stores
  `qwen35`)

| Quantity | Value |
| --- | ---: |
| Hidden width | 5,120 |
| Decoder layers | 64 |
| Vocabulary size | 248,320 |
| FFN width | 17,408 |
| Native context | 262,144 tokens |
| Full-attention interval | every 4th layer |
| Query heads (full attention) | 24 |
| Key/value heads (full attention) | 4 |
| Full-attention head width | 256 |
| Linear-attention QK heads | 16 |
| Linear-attention value heads | 48 |
| Linear-attention head width | 128 |
| Convolution width | 4 |
| Official BF16 tensor bytes | 55,562,855,904 |
| License | Apache-2.0 |

The 64 layers repeat this pattern 16 times: three *Gated DeltaNet*
(linear-attention) layers, then one gated full-attention layer. Every layer
also has a *feed-forward network* (FFN).

**Status:** Facts implemented in Psionic R1. OpenAgents admits
architecture `qwen35` and has mapped the development Ollama
`qwen3.8:27b-mtp-q8_0` blob in-process (`Weights ready`, 27.1 GiB
mapped, RSS ~184 MiB). It has not bound a content-addressed store
copy or a generate loop.

## Primitive: tensor

A *tensor* is a named array of numbers with a *shape* (length of each axis)
and a *dtype* (how each number is stored).

- A vector of hidden width: shape `[5120]`, often F32 while computing.
- A batch of token hidden states: shape `[sequence_length, 5120]`.
- A weight matrix: shape `[5120, 17408]` for one FFN projection.

*Rank* is the number of axes. Rank-1 is a vector. Rank-2 is a matrix.

The network is a sequence of tensor operations. Qwen 3.8 uses the following
ones throughout the trunk.

### Matrix multiply

Most of the compute is:

```text
Y = X W
```

`X` is activations (from the previous layer or from embeddings). `W` is a
learned weight. If `X` is `[N, 5120]` and `W` is `[5120, 17408]`, then `Y` is
`[N, 17408]`. Prefill uses `N > 1`. Decode after the prompt uses `N = 1`.

If a file stores `W` transposed, account for that layout. The multiply is
the same operation.

### Residual add

```text
X_out = X_in + F(X_in)
```

`F` is attention or the FFN. The add keeps a path for the original signal.
Every Qwen 3.8 trunk layer uses two residuals: one after attention, one after
the FFN.

### Elementwise nonlinearities

RMSNorm
: Scale a vector by the root-mean-square of its elements, then multiply by a
  learned gain.

SiLU (Swish)
: `x * sigmoid(x)` in the FFN and in several gates.

Sigmoid
: Attention output gate, and some DeltaNet gates.

Softplus
: DeltaNet time-step gate.

Softmax
: Full attention over positions.

L2 or RMS on Q and K
: DeltaNet query and key normalization.

RMSNorm for a vector `x`:

```text
rms = sqrt(mean(x * x) + epsilon)
y   = (x / rms) * gain
```

`gain` is a learned per-channel tensor. `epsilon` is a small constant from the
config.

### Rotary position embedding (RoPE)

Full-attention queries and keys are rotated as a function of *position*, so
dot products depend on relative distance. Qwen 3.8 uses *partial* RoPE: only
a fraction of each head (`partial_rotary_factor = 0.25`) is rotated.

*MRoPE* (multi-axis RoPE) splits rotary dimensions into sections
`[11, 11, 10]` so image or video layouts can use more than one position axis.
Text-only generation still consumes that layout. Vision understanding is a
separate capability and is out of the first Coder claim.

**Status:** RoPE and MRoPE are part of the shared `qwen3_5_text` graph in
Psionic. YaRN to 1,000,000 tokens stays out until you admit it as its own
capability.

## Primitive: quantization

The official checkpoint is *BF16* (Brain floating point, 16 bits per
weight). 27 billion parameters at 2 bytes is about 52 GiB of weights before
overhead. That exceeds a 16 GiB GPU, and a full copy exceeds typical laptop
RAM.

*Quantization* stores weights in fewer bits. At multiply time the backend
*dequantizes* a block into F16 or F32, multiplies, and discards the wide
copy. Activations and recurrent state often stay F32 even when weights are
quantized.

GGML and GGUF type codes you will see:

F32, F16, BF16
: Uncompressed floats.

Q8_0
: 8-bit blocks. Ollama's `qwen3.8:27b-mtp-q8_0` uses this family.

Q3_K, Q4_K, Q5_K, Q6_K
: K-quant block formats.

IQ3_S, IQ4_XS
: Importance-matrix variants.

`UD-Q3_K_XL`
: Not a GGML type. An Unsloth recipe that mixes ordinary GGML types per
  tensor.

A GGUF is not a byte copy of safetensors. The converter renames tensors,
folds constants (for example `A_log` to `-exp(A_log)`), tiles value heads,
and writes GGUF metadata. Do not compare raw safetensors bytes to GGUF bytes.
Compare converted semantics and token-level or intermediate parity.

**Status:** Psionic R5 admits the mixed types in the selected Unsloth GGUF.
OpenAgents must pick one first artifact. The Ollama tag
`qwen3.8:27b-mtp-q8_0` and Unsloth `UD-Q3_K_XL` are different files. You
cannot treat them as the same model by name.

## Primitive: file formats

### Safetensors

Hugging Face ships `model-*.safetensors` shards plus an index JSON. Each
tensor has a name, dtype, shape, and byte range. Psionic uses this as
architecture authority: names, shapes, dtypes, shard map. Full-width BF16
generation from those shards is not the first laptop path.

### GGUF

[GGUF](https://github.com/ggml-org/ggml/blob/master/docs/gguf.md) is a single
file: header, key-value metadata, tensor table, then payloads. Metadata
carries architecture name, context length, tokenizer, RoPE fields, and
hybrid SSM fields. The runtime maps names onto the `qwen3_5_text` graph.

### Tokenizer files

`tokenizer.json` plus template (`chat_template.jinja` upstream). Matching
Qwen 3.6 vocabulary size does not make the tokenizer identical. Psionic
binds the Qwen 3.8 tokenizer digest.

**Status:** R1 through R5 in Psionic. After you run `openagents inference add`,
OpenAgents stores GGUF files under
`~/.openagents/inference/models/<digest>`. Weights never enter git.

## Stage: admit the artifact

Admission is a closed-world check that runs *before* generation.

1. Hash the file (SHA-256). That digest is the artifact ID.
2. Read GGUF metadata. Architecture must match the expected hybrid family.
3. Classify every tensor: required decoder, optional MTP, ignored vision,
   or unsupported.
4. Record quantization mix, context limit, tokenizer digest, template
   digest, V-head layout (`tiled` for the pinned converter), and MTP
   disposition.
5. Estimate bytes for weights, KV cache, DeltaNet state, and scratch at the
   requested context.
6. If required tensors are missing, layout provenance is unknown, or
   estimated memory exceeds the backend budget, refuse.

If a community GGUF is not on the allowlist, fail closed unless it passes the
same converter parity or tensor-level checks.

**Status:** Implemented in Psionic for the pinned Qwen 3.8 facts and the
qualified GGUF set. OpenAgents plans `openagents psionic inspect`,
`openagents psionic admit`, and `openagents inference add` as the product
wrappers.

## Stage: load onto a backend

A *backend* is the device that owns matrix multiplies and state.

| Backend | First Coder intent | Psionic status |
| --- | --- | --- |
| CPU | Correctness, and hosts without a GPU | Implemented (R6) |
| CUDA | Not the first macOS Coder ship | Implemented (R7) |
| Metal | First Apple Silicon product path | Partial (R10) |

Load sequence:

1. Check live free memory (RAM, VRAM, or unified memory).
2. Allocate weight buffers on the device.
3. Upload or memory-map tensors. Keep quantized weights compressed. Unpack a
   dense F16 mirror only if the plan names that conversion.
4. Allocate *hybrid state*:
   - KV cache only on full-attention layers; it grows with context.
   - Convolution state and DeltaNet state only on recurrent layers; size is
     almost independent of context.
5. Zero recurrent state.
6. Publish residency: what is on device, what is refused. Do not hide host
   fallback as GPU inference.

OpenAgents compiles this library into `openagents`. The process that runs
Coder is the process that holds the weights. `openagents inference serve` can
expose the same load over loopback HTTP. That is the same library, not a
second engine install.

**Status:** CPU and CUDA load implemented in Psionic. Metal incomplete
in the sibling graph. OpenAgents in-process mmap + Metal shared wrap
is landed (`map.done`, Coder `/load`). Context allocation is #352.

## Stage: tokenize and apply the chat template

A *token* is an integer in `0` through `248319`. The network does not take
characters as input.

### Byte-pair encoding (BPE)

The tokenizer splits text with a Qwen2 BPE model:

1. NFC-normalize Unicode (Psionic keeps official NFC IDs; some GGUF
   tokenizers skip NFC).
2. If the Qwen 3.5 or 3.8 pretokenizer requires it, split numbers by Unicode
   numeric class.
3. Merge byte-pair ranks until the sequence is vocabulary IDs.

Special IDs include `<|im_start|>` (248045), `<|im_end|>` (248046),
`<|endoftext|>` (248044), `<think>` / `</think>`, and tool-call markers.

### Chat template

The template turns messages (system, user, assistant, tool) into the string
the tokenizer sees. Qwen 3.8 defaults:

- thinking on
- `reasoning_effort` in `low`, `medium`, or `xhigh`; default `xhigh`
- `preserve_thinking` on
- tools as XML-like `<tool_call>`, `<function=`, `<parameter=`
- adjacent tool results grouped on the user side
- images and videos refused in system messages

**Status:** R2 implemented in Psionic (`qwen3.8.chat_template.v1`). Coder
today speaks Ollama's JSON tools, not this template. The in-process provider
must map Coder tool calls onto this contract or refuse.

## Stage: embed

The embedding table is a matrix `[248320, 5120]`. Token ID `t` selects row
`t`. After this step you have `H` with shape `[T, 5120]` for prompt length
`T`.

Embeddings and the LM head are *untied*: two different matrices.

**Status:** Implemented as part of native generation in Psionic. Quantized
embedding rows use native row lookup. The R7 CUDA path keeps `Q3_K`
embeddings compressed.

## Stage: prefill versus decode

*Prefill* runs the prompt in one pass, or in chunks. Every prompt position
produces keys, values, and DeltaNet updates. You need the last position's
hidden state to predict the first new token, but you must visit all
positions so later decode can attend and so recurrent state is current.

*Decode* appends one token (`N = 1`). Full attention reads the KV cache
instead of recomputing the prompt. DeltaNet reads the compact recurrent
state instead of the full history.

If a decode step cannot cancel, a 27B Q8 generation holds the Coder turn
until it finishes. Check cancellation between bounded submissions.
OpenAgents requires cancel on the in-process path.

**Status:** Prefill and decode implemented on CPU and CUDA in Psionic.
Streaming: re-check whether Qwen 3.8 publishes token-time events or wraps a
completed string. Metal and OpenAgents Coder cancel: not done.

## Stage: one decoder layer

For layer index `i` in `0` through `63`:

1. RMSNorm on the residual stream.
2. If `i % 4 != 3`, run Gated DeltaNet. Otherwise run gated full attention.
3. Add the attention output to the residual.
4. RMSNorm.
5. Gated SiLU FFN.
6. Add the FFN output to the residual.

After layer 63: final RMSNorm, then the LM head.

MTP (multi-token prediction) tensors exist on the checkpoint. Standard
generation skips that tail. Speculative MTP is a separate contract that
rolls back recurrent state. You do not need it for the first Coder loop.

**Status:** Shared hybrid graph implemented in Psionic. MTP skip implemented
for ordinary generate. CPU MTP speculative path exists as correctness-only
(R9A), with no speed claim.

## Primitive: Gated DeltaNet (linear attention)

Full softmax attention costs memory and compute that grow with sequence
length squared per layer, plus a KV cache that grows linearly with length.
*Linear attention* (DeltaNet) keeps a fixed-size state and updates it as
tokens arrive. Cost per new token does not grow with prompt length the way
softmax KV does.

The Qwen model card names this block *Gated DeltaNet*:

1. Project the hidden state to Q, K, V, and extra gates (Z, alpha, beta).
2. Run a causal depthwise *convolution* (width 4) over the sequence, with
   SiLU. Store a short conv state so decode can continue the window.
3. L2-normalize Q and K.
4. Map 16 key heads onto 48 value heads using the *tiled* GGUF layout.
5. Update a recurrent state with a decay gate:

   ```text
   gate = softplus(alpha + dt_bias) * A
   ```

   `A` is stored in converted GGUF form (negative exponential of `A_log`).
   `beta` is sigmoid-gated. The state is F32. llama.cpp stores it
   transposed.
6. Produce an attention-like output from Q and the state.
7. RMSNorm gated by `SiLU(Z)`.
8. Output projection back to hidden width 5,120.

If you lower context, recurrent bytes do not shrink. Admission must publish
recurrent bytes and KV bytes separately.

**Status:** CPU and CUDA ops implemented in Psionic with recurrent
intermediate parity against pinned llama.cpp. Metal DeltaNet is the R10 gap.

## Primitive: gated full attention

Every fourth layer uses *softmax attention* with *grouped-query attention*
(GQA): 24 query heads share 4 key/value heads.

1. Project Q and a query gate together; project K and V.
2. Per-head RMSNorm on Q and K.
3. Partial interleaved MRoPE on Q and K.
4. For each query head, scores = `Q K^T / sqrt(d)`, softmax over positions,
   then weighted sum of V. Prefill attends within the prompt (causal mask).
   Decode attends to cached K/V plus the new position.
5. Multiply by `sigmoid(query_gate)` (`attn_output_gate`, swish or sigmoid
   as configured).
6. Output projection to 5,120.

*KV cache:* store K and V per full-attention layer per position. There are
16 full-attention layers (64/4). Cache grows with tokens. At native context
262,144 that cost is large. The first CUDA gate in Psionic is 4,096 tokens,
not the native maximum.

F16 KV bytes for this shape:

```text
kv_bytes = 2 * 16 * 4 * 256 * n_ctx * 2
         = 64 KiB * n_ctx
n_ctx = 4096   → 256 MiB
n_ctx = 8192   → 512 MiB
n_ctx = 262144 → 16 GiB
```

(The earlier 128 KiB × `n_ctx` line double-counted F16 K+V.)

Gated DeltaNet recurrent + conv state does not shrink when `n_ctx`
drops. Publish `cache_kv_bytes` and `cache_gdn_bytes` separately.
OpenAgents #352 allocates these on the existing mmap; default runtime
`n_ctx` is 4096.

**Status:** Implemented on CPU and CUDA in Psionic. OpenAgents must pass
requested context into memory admission (the analogue of Ollama `num_ctx`).
Ollama today sends no `num_ctx`.

## Primitive: feed-forward network

Dense gated SiLU FFN (SwiGLU-style):

```text
gate = SiLU(X W_gate)      # width 17,408
up   = X W_up
hid  = gate * up           # elementwise
out  = hid W_down          # back to 5,120
```

Most parameters in each layer sit in these three matrices.

**Status:** Implemented in the shared graph.

## Stage: logits and sampling

After the final RMSNorm, the last position's vector `h` (`[5120]`) meets the
LM head `W_out` (`[5120, 248320]`):

```text
logits = h W_out
```

Each logit is an unnormalized score for one vocabulary ID.

Greedy
: Pick `argmax(logits)`. Use this for parity tests.

Sampling
: Upstream generation config uses temperature 1.0, `top_p` 0.95, `top_k`
  20. The non-thinking card defaults differ (temperature 0.7, `top_p` 0.8,
  presence penalty 1.5). Coder must send an explicit plan. A silent mismatch
  with Ollama's defaults is a product bug.

Stop
: IDs 248046 (`<|im_end|>`) and 248044 (`<|endoftext|>`).

**Status:** Greedy and bounded sampling implemented in Psionic generation.
Coder wiring not started.

## Stage: stream to Coder

After each decode step the runtime can:

1. Convert the new token ID to UTF-8, or hold an incomplete multibyte
   sequence.
2. Parse `<think>` … `</think>` as reasoning versus answer text.
3. Parse `<tool_call>` XML into structured name, arguments, and call IDs.
4. Emit usage (prompt tokens, completion tokens) at the end.

Coder's local loop today parses Ollama NDJSON (`thinking`, `content`,
`tool_calls`). The in-process provider should emit the same internal events
Coder already uses: reasoning delta, text delta, tool call, usage, finish.

Token-time streaming means the first event leaves the decode loop when the
first token exists. Completing the whole string and then sending one delta
is a different mode.

**Status:** R8 OpenAI server in Psionic is `implemented_early` (CPU and
CUDA): chat completions, Responses, tools, reasoning effort. Confirm
streaming shape before Coder depends on it. OpenAgents
`PsionicLocalProvider`: not started. Ollama path: shipped.

## What this pipeline excludes (first claim)

Vision encoder (27 layers)
: Inventoried. Generation refuses image and video input.

YaRN 1M context
: Separate admission.

Training and LoRA
: Out of the Coder slice.

Mesh and cluster
: Out of the Coder slice.

Silent Ollama fallback
: Forbidden.

Shipping weights in the CLI binary
: Forbidden.

## OpenAgents wrapping (planned)

After the library is in-tree:

1. Run `openagents inference add` to admit and copy a GGUF.
2. Run `openagents inference doctor` to report backend, digest, and memory.
3. Run `openagents coder --model psionic:<id>` to load in-process and run
   the loop in this document.
4. ATIF records digest, backend, CLI version, and locality `local`.

Keep `--model ollama:` until replacement gates in [PLAN.md](./PLAN.md) pass.

## Implementation map

| Pipeline stage | Psionic (sibling, 2026-08-17 and 2026-08-28) | OpenAgents CLI |
| --- | --- | --- |
| Artifact facts and identity | Implemented (R1) | Not started |
| Tokenizer and chat template | Implemented (R2) | Not started (Ollama template today) |
| Checkpoint and GGUF admission | Implemented (R3, R5) | Planned `psionic inspect` / `inference add` |
| BF16 bounded evidence | Implemented (R4); not generation | Not required for laptop GGUF path |
| CPU generate | Implemented (R6) | Planned in-process |
| CUDA generate | Implemented (R7), 4096-token gate | Later packet; not default macOS |
| OpenAI HTTP serve | Implemented early (R8) | Optional same-process `inference serve` |
| Release gate | Implemented (R9) | Consume as import evidence |
| Metal | Partial (R10) | Required for first Apple Silicon ship |
| Vision | Partial (R11) | Out of first Coder claim |
| In-binary Coder provider | Not applicable | Not started |

## Sources

- [Owner intent](./INTENT.md) and [initial plan](./PLAN.md) in this folder
- Psionic `docs/qwen38/MODEL_FACTS.md`, `IMPLEMENTATION_ROADMAP.md`,
  `PSIONIC_GAP_ANALYSIS.md`, `LLAMA_CPP_CODE_AUDIT.md`, and
  `FIRST_GGUF_TARGET.md`
- [Qwen 3.8-27B model card](https://huggingface.co/Qwen/Qwen3.8-27B)
- llama.cpp Qwen3.5 graph (pinned in the llama.cpp audit): trunk order,
  DeltaNet block, full-attention block, hybrid memory
