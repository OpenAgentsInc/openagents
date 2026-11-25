# Speculative encoding: many-to-many cache translation

Read-only technical study, 2026-11-25, of Ramp Labs' *Speculative Encoding:
Many-to-Many Cache Translation* (Ben Geist and Thomas Villeneuve), its reference
implementation, and its released checkpoint. The sibling checkout at
`~/work/speculative-encoding` was inspected read-only; no weights were
downloaded and no training, serving, or GPU benchmark scripts were executed.

## Source pins

| Source | Revision / identifier | License / notes |
| --- | --- | --- |
| Research post | [Ramp Labs on X (`2125625946229064118`)](https://x.com/RampLabs/status/2125625946229064118) | Ben Geist, Thomas Villeneuve (2026-11-25); builds on [Latent Briefing](https://labs.ramp.com/research/latent-briefing-kv-cache/) and PorTAL. |
| Reference repository | [`b10-tv/speculative-encoding`](https://github.com/b10-tv/speculative-encoding) at `151cdca8e3fa7470df8e64157e2f55a08c7e2991` | Apache-2.0 (`LICENSE` in root); inspected locally at `~/work/speculative-encoding`. |
| Released checkpoint | [`RampPublic/speculative-encoding`](https://huggingface.co/RampPublic/speculative-encoding) at `eec4d43036a15035817418be2e4591b8e42a06cf` | 9.79 GB FP32 (`translator/` + `compressor-128x/`); no standalone `LICENSE` file in the Hub tree (verify terms before redistributing weights). |

## Problem and core idea

In routers and multi-agent swarms, heterogeneous models increasingly hand
context to one another as text. The receiving model must prefill that text from
scratch—paying quadratic attention cost in document length—to rebuild its KV
cache, duplicating work the sender already performed. Earlier cache-to-cache
translators (such as SwaT) eliminate text re-prefill but train one translator
per `(source, target)` pair, requiring $n(n-1)$ separate networks for $n$
models.

**Speculative encoding** replaces pairwise translators with a single shared
encoder-decoder network that maps a frozen source model's per-layer hidden
states into a frozen target model's per-layer hidden states, after which the
target computes its own KV cache (and recurrent state, for hybrid models) through
its own frozen weights.

Four design ideas converge here:

1. **Hidden states instead of raw KV caches** (inspired by YOCO and
   DeepSeek-V4.1-Flash). Every layer of every decoder has one hidden vector per
   token, whereas KV layouts vary wildly across head counts, head dimensions,
   GQA ratios, sliding windows, KV-sharing layers, and linear attention.
   Forcing predicted hidden states through the target's frozen norms, $W^K$,
   $W^V$, and RoPE constrains the resulting cache to the target's native
   subspace.
2. **Shared core with thin per-model alignment** (from PorTAL). Shared
   transformer + MoE blocks carry the cross-family mapping; only per-model
   input/output projections and model-layer bias vectors scale linearly with
   $n$.
3. **Target token embeddings as decoder initialization** (from SwaT). Because
   model families use different tokenizers, the decoder starts from the target's
   own token embeddings of the document and cross-attends to source memory
   aligned by character spans.
4. **Two-stage cache-fitting then KL distillation** (from STILL). Head-RMS-scaled
   KV MSE initializes the cache; next-token KL divergence against the target's
   native rollout aligns downstream behavior.

## Architecture and code anatomy

Inspecting `speculative_encoding/translator/model.py`, `compressor.py`, `hf.py`,
`prompts.py`, and `encode.py` reveals the concrete tensor shapes and runtime
contracts behind the paper's equations.

### Shared translator (`UnifiedTranslator`)

Let $\mathcal{M}$ be the set of frozen models; model $m$ has $L_m$ layers and
width $D_m$. The paper's 2.35B-parameter architecture uses internal width
$d = 768$, $8$ attention heads (head dim $96$), FFN width $3072$, $8$ routed
experts with `top_k = 2` plus $1$ always-on dense MLP, local attention window
$w = 8$ tokens, $N_{\text{enc}} = 2$ encoder blocks, and $N_{\text{dec}} = 8$
decoder blocks.

- **Source input & encoder (`UnifiedEncoder`).** Captures $L_{\text{src}} + 1$
  hidden-state streams per token: the raw embedding output (stream $0$) followed
  by each decoder layer's output (before final norm). Each stream $\ell$ is
  RMS-normalized, scaled by a learned per-stream gain vector `gains[src][l]`,
  and projected by `maps[src][l]` ($D_{\text{src}} \to d$) in a single batched
  matmul (`torch.bmm`). Two shared `UnifiedBlock` layers then run over all
  source streams of the last $w = 8$ tokens, producing the source memory
  $\mu \in \mathbb{R}^{T_{\text{src}} \times (L_{\text{src}}+1) \times d}$.
- **Model-layer conditioning.** Every model has learned source and target state
  tables (`encoder.states[src]` of shape $[L_{\text{src}}+1, d]$ and
  `heads[tgt].state` of shape $[L', d]$). Inside each block, linear maps
  (`self_state_map`, `cross_state_map`, `memory_state_map`, `ffn_state_map`)
  project those vectors into additive shifts at every self-attention,
  cross-attention, and FFN step.
- **STILL-style local-window attention (`SelfAttention`, `CrossAttention`).**
  Queries and keys are L2-normalized per head (`F.normalize(..., dim=-1)`),
  rotated with interleaved RoPE ($\theta = 10{,}000$), and scaled by a learned
  per-head sharpness $\exp(\text{logit\_scale})$ (initialized to
  $\ln(d_{\text{head}})$), so SDPA runs at `scale = 1.0`. Attention spans all
  depth streams within a causal window of $w = 8$ tokens, making translation
  cost $O(T)$ in document length rather than $O(T^2)$.
- **Routed MoE FFN (`RoutedFeedForward`).** Every block pairs one always-on
  GELU MLP with top-2 of 8 full-width GELU experts. The router scores both
  normalized activation content and the unit-RMS model-layer shift:
  $\text{logits} = W_r c + W_s\,\text{rms}(s)$. Expert selection adds a
  non-learned `selection_bias` updated by exponential moving average load
  tracking (DeepSeek-V3 auxiliary-loss-free balancing; `mm_router_bias_decay =
  0.9`, `mm_router_bias_rate = 0.01`), while gate weights are renormalized from
  the unbiased softmax probabilities. All residual output projections
  (`self_attn.out`, `cross_attn.out`, `ffn.down`, `ffn.expert_down`,
  `memory_state_map`) are zero-initialized so blocks start as the identity.
- **Cross-tokenizer alignment (`prompts.py`).** Source and target keep their own
  tokenizers and chat templates (documents are rendered into the system role by
  default via sentinel splitting). Character offset mappings produce:
  - `causal_limits[t]`: number of source tokens ending at or before target
    token $t$ ends; target token $t$ attends only to the last $w$ source tokens
    strictly below `limits[t]`.
  - `fractional_positions[s]`: each source token's character midpoint expressed
    in target-token coordinates so source memory and target queries share the
    target's RoPE axis.
- **Decoder & depth-stream readout (`UnifiedDecoder`, `UnifiedHead`).**
  Each target document token's embedding is RMS-normalized and projected
  (`embed_proj`: $D_{\text{tgt}} \to d$), then broadcast across $L' \le
  L_{\text{tgt}}$ decoder streams (`L' = L_tgt`, `8`, or `4`). Eight shared
  decoder blocks apply local self-attention, local cross-attention into $\mu$,
  and routed MoE. When $L' < L_{\text{tgt}}$, a learned per-target matrix
  `depth_readout` ($\lambda^{\text{tgt}} \in \mathbb{R}^{L_{\text{tgt}} \times
  L'}$, initialized by linear depth interpolation `_interp`) mixes streams back
  to all $L_{\text{tgt}}$ target layers, followed by per-layer RMSNorm and
  batched output projections $P^{\text{out}}_{\text{tgt},\ell}$ ($d \to
  D_{\text{tgt}}$, no bias in the released checkpoint).
- **Reusable source memory (`EncodedMemory`, `encode.py`).** Because $\mu$
  depends only on the source model and the document, it can be computed once
  and saved as a single safetensors file (`speculative-encoding-memory/1`)
  carrying `memory`, document text, source token IDs/spans, chat role, and a
  16-hex-char SHA-256 `translator_fingerprint` of the source's `states` and
  `gains` in BF16. At width $768$ instead of $3840\text{–}5120$, $\mu$ is
  $5\text{–}7\times$ smaller than raw source hidden states and lets any target
  decode without loading the source LLM (`load_source=False`).
- **Exact halo chunking (`encode.py`).** Because every translator attention
  layer has a strict local window $w = 8$, translating long documents in serial
  chunks (`chunk = 32768`, or `16384` for Gemma on 80 GB GPUs) prepends exact
  halos (`_TRANSLATE_HALO = 64 >= 8 * 7` target tokens, `_SOURCE_HALO = 32 >=
  8 + 2 * 7` source tokens), drops the halo outputs, and reproduces whole-document
  translation bit-for-bit.

### Target frozen cache write and heterogeneous layers (`hf.py`, `encode.py`)

Given predicted target layer inputs $\hat{X}^{\text{tgt}} \in
\mathbb{R}^{T_{\text{tgt}} \times L_{\text{tgt}} \times D_{\text{tgt}}}$,
`ModelInterface.write_prefix` runs the target's own Hugging Face decoder over
the real document token IDs (preserving token-dependent side inputs such as
Gemma 4's per-layer embeddings) with forward pre-hooks replacing each layer's
input, MLP/MoE blocks monkey-patched to no-ops, and attention replaced by a
cache-only stub (`_CACHE_ONLY`) that writes post-RoPE keys/values and returns
zeros.

For heterogeneous targets, `HybridWriter` and `prefix_from_cache` preserve all
non-KV state in a typed prefix dictionary:

- **Full-attention layers:** post-RoPE `[2, P, kv_heads, head_dim]` K/V for the
  entire prefix $P = \text{header} + \text{doc}$.
- **Sliding-window layers (Gemma 3, Gemma 4):** only the last `window` entries
  plus `kv_first = P - n`.
- **KV-shared / no-`v_proj` layers (Gemma 4):** layers with
  `is_kv_shared_layer = True` allocate no cache entry; layers with
  `v_proj is None` set $V = K$.
- **Linear-attention / Gated DeltaNet layers (Qwen 3.5):** exact causal short
  convolution buffer `conv` (`[conv_dim, kernel]`) and recurrent state `ssm`
  (`[v_heads, k_dim, v_dim]`, FP32).

Every loaded model runs `ModelInterface.self_check()` on a 6-token probe to
verify relative cache, logit, continuation, and spliced-forward errors against
native execution (`1e-4` in FP32; `2e-2` cache / `5e-2` logit in BF16, raised to
`1e-1` for Gemma 4 12B). Models whose Hugging Face cache implementation cannot
continue over multiple tokens (notably Granite 4.0 `-h` Mamba hybrids) are
explicitly flagged (`continues_cache = False`) and excluded from the catalog.

### Query-conditioned compressor (`SlotCompressor`, `encode.py`)

For long documents where a question needs only a few passages, a frozen
translator can be followed by a query-conditioned compressor that reduces
full-attention cache length by ratio $r$ ($16\times$, $64\times$, or
$128\times$) in two steps:

1. **Selection (SnapKV / KVzip probe).** `encode.score()` runs the target model
   on the question tokens (`after_ids`) against the full translated cache and
   records, for each full-attention layer $\ell$ and KV group $g$, the maximum
   attention probability from any question query token and query head to each
   document key. `selection_scores()` takes the max over KV heads per layer,
   normalizes each layer's scores to sum to $1$ over the document, averages
   across layers, and keeps the sorted indices of the top $B = \lceil T / r
   \rceil$ positions.
2. **Synthesis (`SlotCompressor`).** Two blocks copied from the translator's
   last two decoder blocks—with `self_attn.out`, `cross_attn.out`, `ffn.down`,
   `ffn.expert_down`, and `memory_state_map` zero-initialized so training starts
   identically at the SnapKV selection—take as input the $B$ kept decoder states
   (plus learned `slot` bias) concatenated with the target's standalone
   question layer inputs (projected through the target's *encoder-side* `gains`
   and `maps`, mixed to $L'$ streams via per-target `question_mix` or
   `pool_question_rows`, plus learned `question` bias). Note an important
   implementation detail not explicit in the blog summary: while self-attention
   is global over all $(B + |Q|) \cdot L'$ rows, cross-attention over the full
   $T$-token translated document is **same-stream** (each stream reads all $T$
   rows of its own stream, chunked `MEMORY_STREAM_CHUNK = 8` streams at a time
   so 128k contexts fit in VRAM). Only the $B$ updated slot states exit through
   the target head into the target's frozen K/V writer at consecutive positions
   `header .. header + B - 1`.

For hybrid targets (`hybrid_cartridge`), only full-attention layers are
compressed: sliding-window layers keep the header plus the document's real last
$\min(\text{window}, B)$ tokens positioned directly before the question, and
Gated DeltaNet layers keep the full document's `conv` and `ssm` states
unchanged.

## Training, results, and released artifacts

### Training recipe

All 42 directed pairs across 7 frozen models are trained jointly in two stages:

- **Stage 1 (cache fitting):** 17,000 steps, lr $3\times 10^{-4}$ (300 warmup),
  weight decay $10^{-3}$, documents up to 8,192 tokens. Minimizes
  head-RMS-normalized MSE on every kept K and V tensor so layers, heads, keys,
  and values carry equal weight. Sliding-window layers score only in-window
  tokens; linear-attention layers match per-token layer inputs plus final
  convolution and recurrent states.
- **Stage 2 (KL distillation):** 8,000 steps, constant lr $10^{-5}$ (50 warmup).
  Teacher-forces question-answer pairs (`still-MCQ`, Qasper, HotpotQA, MuSiQue,
  ARC-Challenge, HellaSwag, WinoGrande, MMLU auxiliary train, GSM8K) and
  continuations (PG-19, WikiText), minimizing next-token KL divergence between
  the target conditioned on its native cache vs. the translated cache.
- **Gradient balancing:** Each target's gradient norm on shared parameters is
  rescaled to the mean norm across targets (GradNorm-style) before averaging,
  then shared and per-target gradients are clipped to norm $1.0$.
- **Compressor training:** Trained separately per translator with base models
  and translator frozen using the same KL distillation objective and gradient
  rescaling (`compressor-128x`: 2,400 steps, lr $10^{-4}$, ratio $128\times$).

### Reported benchmark results

These are Ramp Labs' published measurements (averaged over 3 seeds and all 42
directions; latency measured on one H200 with vLLM, source memory precomputed):

| Benchmark slice | Metric | Reported result |
| --- | --- | --- |
| Documents $\le 8{,}192$ tokens (12 tasks, 2,395 questions/direction, full depth) | Mean retention vs. target native cache | **96.9%** (worst of 42 directions: **93.1%**) |
| Same $\le 8\text{k}$ suite | Mean retention vs. weaker of `(source, target)` | **100.3%** |
| Same $\le 8\text{k}$ suite, generalization split | Unseen tasks vs. tasks in Stage 2 data | **97.4%** unseen vs. **96.5%** seen |
| Long documents (20 docs $\times$ 8 MCQs: code, finance, Gutenberg, legal) — 32k tokens | TTFT speedup / retention | **4$\times$** faster than prefill at **>90%** retention (92% full depth) |
| Long documents — 128k tokens (4–16$\times$ training length) | TTFT speedup / retention by decoder depth $L'$ | Full depth: **2.5$\times$** (83% ret.); $L'=8$: **6.1$\times$**; $L'=4$: **7.5$\times$** ($L'=8,4$ within 1–2 pts of full depth) |
| Query-conditioned compressor (16$\times$, 64$\times$, 128$\times$) | Retention vs. full translated cache | Within **2 pts** at 32k & 64k (even at 128$\times$); **beats full cache at 128k** by up to **+4.3 pts** at 64$\times$ full-depth (**+4.0 to +5.1 pts** for $L'=8,4$) |

### Released Hugging Face checkpoint (`RampPublic/speculative-encoding`)

Inspecting `translator/recipe.json`, `translator/meta.json`, and
`compressor-128x/config.json` at `eec4d43036a15035817418be2e4591b8e42a06cf`
shows that the public release is the **$L' = 8$ fast decoder**
(`PAPER7_D8_KLONLY_SEED1`, stage 2 step 8,000, `output_bias = false`), plus the
$128\times$ compressor (step 2,400):

| Artifact path | Size (FP32) | Contents |
| --- | --- | --- |
| `translator/encoder.pt` | 4,090.56 MB | Shared 2 encoder blocks + all 7 sources' `gains`, `maps`, `states` |
| `translator/decoder.pt` | 1,585.99 MB | Shared 8 decoder blocks (~396.5M params) |
| `translator/adapter-{llama31-8b,qwen35-9b}.pt` | 415.40 MB each | Target heads (`embed_proj`, `depth_readout`, per-layer `P^out`) |
| `translator/adapter-ministral3-8b.pt` | 440.57 MB | Target head |
| `translator/adapter-{gemma3-12b,gemma4-12b}.pt` | 578.25 MB each | Target heads |
| `translator/adapter-{ministral3-14b,qwen3-14b}.pt` | 645.07 MB each | Target heads |
| `compressor-128x/compressor-step-002400.pt` | 396.52 MB | 2-block `SlotCompressor` (~99.1M params, `ratio = 128.0`) |

All 7 trained endpoints together total 9.79 GB in FP32 (~2.35B translator
parameters + ~99M compressor parameters). Running `cli split --dtype bf16`
splits the bundle into `shared.pt` + per-model `source-<m>.pt` /
`target-<m>.pt` so serving one `(source, target)` pair loads ~1.5 GB in BF16.

The repository's `models.py` catalog pins 18 Hugging Face checkpoints across 8
families (`qwen3` 4B/8B/14B, `qwen3.5` 4B/9B, `gemma3` 4B/12B, `llama31-8b`,
`gemma4` 12B/E2B/E4B, `phi4-14b`, `ministral3` 8B/14B, `granite33` 2B/8B,
`granite42` 3B/8B), though only the 7 models above have weights in the public
checkpoint.

## Fast kernels, serving connectors, and prefix algebra

The reference repository contains substantial serving engineering beyond the
blog post:

- **Six-level CUDA fast-inference executor (`fast_inference.py`, `kernels.py`).**
  CPU and Apple MPS run level 0 (eager reference, no BF16 autocast). CUDA
  defaults to level 6:
  - **L1:** packed varlen CUTLASS `aten::_efficient_attention_forward` aligned
    to a 64-key block grid (bitwise identical to eager).
  - **L2:** device-side counted Triton `top2_experts` (histogram $\to$ prefix
    sum $\to$ scatter $\to$ grouped GEMM $\to$ 65,536-entry exact BF16 GELU
    lookup table $\to$ eager-order BF16 combine), removing the CPU-GPU host
    sync of `bincount` / `tolist()`.
  - **L3–L4:** `torch.compile` over `UnifiedHead`, whole `UnifiedBlock`, and
    `UnifiedEncoder` input RMSNorm with attention and experts wrapped as opaque
    custom ops.
  - **L5:** `aten::_flash_attention_forward` varlen without masks (or opt-in
    Triton `tiled_window_attention` sharing key-block loads across `BLOCK_M`
    consecutive query rows; noted as off for Ministral 3 due to a 128k kernel
    fault).
  - **L6:** 1D column-fastest L2-swizzled expert grids, unmasked weight loads
    for divisible shapes, and separate up/down tile sizes (`TUNED_UP =
    (128, 128, 64, 4, 3)`, `TUNED_DOWN = (128, 256, 32, 8, 5)`; bitwise
    identical to L5).
- **vLLM KV injection & cross-model prototype (`vllm_connector.py`,
  `vllm_engine.py`, `transformers_compat.py`).**
  `TranslatedKVConnector` and `HybridKVConnector` (`SupportsHMA`) implement
  vLLM v1's `KVConnectorBase_V1` to scatter translated KV tensors into paged
  blocks (and write Qwen 3.5 `conv`/`ssm` state blocks directly). A prototype
  `CrossModelKVConnector` + `FirstHitMultiConnector` (CPU-tested only, not yet
  run on GPU) maintains an in-process `CrossModelRegistry` keyed by
  model-independent NFC-normalized SHA-256 `document_key` behind LMCache or
  vLLM CPU offload, decoding from `EncodedMemory` on a background thread when a
  new target hits a document read by another source. Operational constraints
  documented in the source:
  - Requires `VLLM_ENABLE_V1_MULTIPROCESSING=0` so workers share in-process
    `STORE` (for `TP > 1`, workers load tensor slices from `PMT_VLLM_EXPORT_DIR`,
    full-attention only).
  - Disables prefix caching for gated-delta targets (`Qwen 3.5`) and caps
    concurrent long requests to avoid vLLM preemption dropping injection.
  - Requires an import-hook patch (`transformers_compat.py`) to bridge vLLM
    0.24 against `transformers 5.23` (Gemma 4 heterogeneous head dims and
    Ministral 3 Pixtral symbols), plus `gdn_prefill_backend="triton"` for
    Qwen 3.5 to avoid a 7.5-hour Hopper FlashInfer JIT hang.
- **Prefix algebra (`prefix_ops.py`).** Implements post-RoPE key rotation
  (`rotate_keys`, supporting partial rotary such as Qwen 3.5 and refusing
  length-dependent `dynamic`/`longrope` RoPE), prefix `concatenate`, `extend`,
  and `truncate`. Importantly, the code is explicit about exactness boundaries:
  - Concatenating separately encoded documents is exact against standalone
    offset writes, *not* a native cross-document read (later documents never
    attended to earlier ones).
  - Both `concatenate` and `truncate` raise `NotImplementedError` on any
    prefix containing Gated DeltaNet `conv`/`ssm` states (recurrent transitions
    cannot be concatenated without initial-state replay or rewound).
- **Token-level handoff (`handoff.py`).** `capture_rows()` + `encode_ids()`
  plumb a model's own generated tokens (e.g., an `A -> B -> C` relay) into
  another model's cache without text re-prefill. The module carries an explicit
  warning: the translator was trained only on fresh document-prefill rows, so
  translation quality on post-cache or self-generated continuation rows is
  *unmeasured*.

## How this maps to Psionic

Speculative encoding intersects several active Psionic subsystems and exposes
concrete architectural questions for Coax-native serving and multi-agent
workflows.

| Speculative encoding mechanism | Psionic counterpart | Architectural fit / gap |
| --- | --- | --- |
| `HybridWriter` / `hybrid_cartridge` (full KV + sliding tail + Gated DeltaNet `conv`/`ssm`) | [Ch. 11–12](../book/ch011-a-hybrid-sequence-model.md), [Ch. 176](../book/ch176-paged-state-and-a-tiered-prefix-cache.md), [Ch. 182](../book/ch182-disaggregate-prefill-and-decode.md), [Inference Engineering](serving/inference-engineering.md) | Direct validation of Psionic's rule that *"state is more than KV"*. Notably, `qwen35-9b` is the exact base architecture of Psionic's [Elephant](targets/model-targets.md) (`elephant-1`), `ministral3` matches Part VIII ([Ch. 81](../book/ch081-execute-ministral-4.md)), and `granite42-8b` matches [TwIL-8B](targets/twil-8b.md). |
| `EncodedMemory` (`speculative-encoding-memory/1`, $5\text{–}7\times$ smaller than hidden states, SHA-256 fingerprinted) | [Ch. 176](../book/ch176-paged-state-and-a-tiered-prefix-cache.md) tiered prefix cache, [Ch. 182](../book/ch182-disaggregate-prefill-and-decode.md) disaggregation, [Mesh](serving/mesh.md) | Fits as a **separate approximate cache profile** keyed by NFC content digest + model/translator fingerprint. Under Ch. 176's rule, translated or compressed caches must carry a distinct profile identity and never enter an exact native prefix-cache namespace. |
| Query-conditioned `SlotCompressor` (64$\times$–128$\times$ full-attention compression) | [Harness / Super-agent](harness.md) Router $\leftrightarrow$ Workers, [Ch. 225](../book/ch225-learn-which-model-to-call.md) model routing, Pylon bandwidth budgets | Enables a cheap reader or Router to ingest a 64k–128k codebase/document once and send either compact `EncodedMemory` or a query-conditioned cartridge across nodes/models rather than re-prefilling raw text on every specialist worker. |
| Device-counted Triton MoE (zero host sync, exact BF16 GELU LUT), tiled window attention, auxiliary-loss-free `selection_bias` | [Ch. 41](../book/ch041-attention-in-tiles.md), [Ch. 43](../book/ch043-fuse-and-replay.md), [Ch. 91](../book/ch091-routers-that-balance-themselves.md), [Ch. 99](../book/ch099-a-decode-step-with-no-host-waits.md) | Same zero-host-sync kernel discipline Psionic uses on CUDA/Metal. Every translator op (RMSNorm, RoPE, tiled local-window attention, top-2 GELU MoE, batched projections) is expressible in Coax compute kernels without cuBLAS or Triton. |
| Post-RoPE key rotation & prefix algebra (`prefix_ops.py`) | [Ch. 36](../book/ch036-many-questions-at-once.md), [Ch. 42](../book/ch042-deltanet-in-chunks.md), [Ch. 176](../book/ch176-paged-state-and-a-tiered-prefix-cache.md) | Confirms that KV-only concatenation/truncation breaks on Gated DeltaNet unless chunk-boundary recurrent snapshots (Ch. 42) are retained. |
| `ModelInterface.self_check()` & honest caveats on relays | [Ch. 17](../book/ch017-qualify-honestly.md), [Ch. 195](serving/mesh.md) | Aligns with Psionic's qualification discipline: translated caches are lossy profiles (measured by task retention / KL) and cannot satisfy Ch. 195's bit-exact re-execution receipt checks. |

### Open technical questions for Psionic

1. **Looped transformers ([Inkling 2 Mini](targets/inkling.md), Part XXVII).**
   Speculative encoding assumes one hidden state per layer ($L_m$ layers). A
   looped MoE model such as Inkling 2 Mini executes its middle block 3 times
   per token with distinct `(layer, loop_pass)` KV caches and native
   NVFP4/MXFP8 weights. Adapting Inkling as a source or target requires
   treating each `(layer, loop_pass)` as a distinct stream (or learning a
   pass-indexed projection) and training its thin alignment layers against a
   frozen core—an untested regime.
2. **Long-context retention decay.** Without compression, full-depth retention
   drops from 96.9% ($\le 8\text{k}$) to 92% (32k) and 83% (128k) because
   training stopped at 8,192 tokens. Coding agents routinely operate at
   32k–128k tokens; either longer-context Stage 2 training or mandatory
   query-conditioned compression (which lifts 128k retention by $+4\text{–}5$
   points) would be needed before production use.
3. **Multi-turn agent relays.** Coding and Super-agent workflows rarely hand
   off a single static system document; they hand off multi-turn tool traces,
   reasoning blocks, and partial edits. Because `handoff.py` explicitly leaves
   continuation-row translation unmeasured, Psionic would need an empirical
   qualification suite over real multi-turn agent logs before routing mid-task
   between models via translated caches.
4. **Memory footprint on local Apple Silicon / 16 GB CUDA.** A single BF16
   split pair (~1.5 GB for the translator + ~0.2 GB for a compressor) is small
   enough to sit beside a quantized local model on a 32–128 GB Mac, but
   capturing all `L + 1` source hidden states at 128k tokens requires tens of
   gigabytes of host RAM (e.g., 47 GB for Gemma 3 at 128k in `models.py`).
   Streaming source layers directly into the encoder chunk-by-chunk (layer-major
   or streaming `HostRows` into `EncodedMemory`) would be required on bounded
   hosts.

## What Psionic does not adopt

- **PyTorch, Triton, Hugging Face, or vLLM as serving dependencies.** Psionic
  keeps arithmetic, scheduling, paging, and cache identity in Coax. Private
  `aten::_efficient_attention_forward` ABIs and `transformers_compat` import
  hooks are upstream Python integration workarounds, not Psionic runtime
  contracts.
- **Mixing translated caches into exact prefix identities.** A translated or
  compressed cache is an explicitly named approximate profile (with its own
  retention/KL quality card), never interchangeable with an exact native prefix
  page in Ch. 176 or a bit-exact peer replay in Ch. 195.
- **Upstream H200 speedup numbers as Psionic claims.** The 4$\times$ (32k) and
  6.1$\times$–7.5$\times$ (128k) TTFT speedups exclude source prefill and were
  measured on H200 with vLLM. Any Coax implementation must measure end-to-end
  TTFT (including source encoding and transfer when applicable) and task
  retention on Psionic's own targets (Metal and CUDA) under Ch. 17 and Ch. 172
  before enabling speculative encoding by default.
