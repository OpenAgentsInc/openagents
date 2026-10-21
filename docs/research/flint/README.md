# Recursive Flint on OpenAgents Pylons

Status: research note, 2026-10-21. Nothing here is implemented. Sources were
read on the day Flint was released, so upstream runtime support is moving;
recheck the pull requests listed in [Runtime support](#runtime-support)
before you start the work.

Flint is Recursive SI's first model: an open-weights coding model for local
hardware. This note records what Recursive published about it, then works out
what it takes to serve Flint on OpenAgents Pylons and whether it fits Coder
and Microcoder.

Each section separates **published facts** (with a source) from **our
inference** (labeled as an estimate or an inference). Weights weren't
downloaded and none of Recursive's code was run. Only `config.json`,
`generation_config.json`, `tokenizer.json`, the safetensors index, the model
cards, and the SGLang kit's README were fetched and read as data.

## Contents

- [Verdict](#verdict)
- [Sources](#sources)
- [What Flint is](#what-flint-is)
- [Formats and file sizes](#formats-and-file-sizes)
- [License](#license)
- [Benchmarks and speed claims](#benchmarks-and-speed-claims)
- [Runtime support](#runtime-support)
- [(a) Can a Pylon serve Flint today?](#a-can-a-pylon-serve-flint-today)
- [(b) Native Psionic support](#b-native-psionic-support)
- [(c) Hardware fit](#c-hardware-fit)
- [(d) Fit with Coder and Microcoder](#d-fit-with-coder-and-microcoder)
- [(e) Steps to put Flint on Pylons](#e-steps-to-put-flint-on-pylons)
- [Local measurements on this Mac](#local-measurements-on-this-mac)
- [Open questions](#open-questions)

## Verdict

- **Not today.** No Pylon can serve Flint as it stands. The only llama.cpp
  support is an open, unmerged pull request, Psionic has no `flint` family,
  and Pylon's llama.cpp lane starts every model with a 4,096-token context
  and a 512-token default output cap, which a reasoning model that often
  thinks for thousands of tokens overruns.
- **Measured on the M5 Max:** with the pull-request build, llama.cpp decodes
  Flint at 83–95 tokens per second end to end (126–129 in `llama-bench`),
  and Recursive's `rec` at 109–140. See
  [Local measurements on this Mac](#local-measurements-on-this-mac).
- **Shortest path:** Pylon's managed llama.cpp lane with Recursive's Q4_1
  GGUF on the Mac M5 Max (128 GB). Build `llama-server` from the pinned
  pull-request commit, add a Flint text profile with a larger context, a
  larger output cap, and a longer declared deadline, then qualify it with
  `scripts/bench/pylon-models/run.py`. This is about a week of work, most
  of it in Pylon rather than in the model.
- **The RTX 4080 and L4 can run it only with part of the model in system
  memory.** Recursive's GGUFs are 22.9 GB and 26.0 GB, so a 16 GB or 24 GB
  card holds only part of the experts. Expect about 45–65 tokens per second
  on the 4080, which is in line with the Nemotron 3 Nano 30B-A3B results we
  already measured there.
- **Native Psionic support is a mid-size project:** about 4–7 engineer-weeks
  for CUDA and Metal. Most parts exist in other families (GQA, QK
  RMSNorm, attention sinks, sliding windows, sigmoid top-k MoE routing,
  expert dispatch). What's new is per-layer NoPE, the `flint` tokenizer and
  template, Q4_1 and NVFP4 GPU kernels, and gluing it together.
- **License is clear:** Apache 2.0 on every Flint repository. We may serve
  it commercially and give the weights to Pylon operators, keeping the
  license and attribution with the files.
- **Not the network's default coding model yet.** Recursive's benchmark
  numbers are self-reported and unverified, and only one of our four Pylons
  holds the model entirely in fast memory. Pylon's conversation lane also
  refuses tool calls, so Flint's agent behavior isn't reachable through it.
  It's a strong candidate for a qualified coding lane on machines with 48 GB
  or more, and for local use through Coder's existing OpenCode route.

## Sources

| Source | What it provided |
| --- | --- |
| [Announcement](https://recursive.com/updates/introducing-recursive-flint) | Speed claims, measurement footnotes, the Eureka Machine, the runtime list, and the 512k context statement. |
| [`RecursiveSI/flint` model card](https://huggingface.co/RecursiveSI/flint) | Architecture table, benchmarks, generation settings, and license. Revision `2a247be0c0aec895ccb0ac7ca92229ec6df37288`. |
| [`config.json`](https://huggingface.co/RecursiveSI/flint/blob/main/config.json), [`tokenizer.json`](https://huggingface.co/RecursiveSI/flint/blob/main/tokenizer.json), `model.safetensors.index.json` | Hyperparameters, tokenizer type, and tensor names. |
| [SGLang kit README](https://huggingface.co/RecursiveSI/flint/blob/main/sglang/README.md) | Serving requirements, KV cache size, chat format, tool-call format, and reasoning lengths. |
| [Hugging Face API](https://huggingface.co/api/models/RecursiveSI/flint) and [`?author=RecursiveSI`](https://huggingface.co/api/models?author=RecursiveSI) | File lists, sizes, LFS digests, and revisions of all four repositories. |
| [`RecursiveSI/flint-int4`](https://huggingface.co/RecursiveSI/flint-int4) | MLX INT4 variant. Revision `3b93f9053b530488405ae9fca51d4900d6ce21f0`. |
| [`RecursiveSI/flint-int4-gguf`](https://huggingface.co/RecursiveSI/flint-int4-gguf) | Q4_1 GGUF for Apple Silicon. Revision `9c47151036bc2c34331142dda04488bceab2bc9f`. |
| [`RecursiveSI/flint-nvfp4-gguf`](https://huggingface.co/RecursiveSI/flint-nvfp4-gguf) | NVFP4 GGUF for NVIDIA. Revision `6ec43e19b565fc8d560e2d04985d7724f20673bf`. |
| [`recursive-oss/rec`](https://github.com/recursive-oss/rec) | Recursive's local engine and its throughput chart against llama.cpp. |
| [llama.cpp #30928](https://github.com/ggml-org/llama.cpp/pull/30928), [#30929](https://github.com/ggml-org/llama.cpp/pull/30929), [#30861](https://github.com/ggml-org/llama.cpp/pull/30861) | Flint support (open), Jinja hexadecimal escapes (merged), and MoE down-projection scaling (merged). |
| [vLLM #62938](https://github.com/vllm-project/vllm/pull/62938), [SGLang #45624](https://github.com/sgl-project/sglang/pull/45624), [mlx-lm #1998](https://github.com/ml-explore/mlx-lm/pull/1998) | Upstream support pull requests, all open on 2026-10-21. |

## What Flint is

### Published facts

Flint is a decoder-only mixture-of-experts (MoE) Transformer that Recursive
says was trained from scratch, through pretraining, mid-training, supervised
fine-tuning, and reinforcement learning, by a small team of people and
agents in under four months. Recursive credits its automated research system,
the Eureka Machine, with work on data, optimizers, training recipes,
infrastructure, and inference kernels. Source: the model card and the
announcement.

| Property | Value | Source |
| --- | --- | --- |
| Total parameters | 39.8B (the safetensors header counts 39,785,287,456 BF16 values) | Model card, HF API |
| Active per token | About 2.5B including the output head; about 2.2B without it | Model card |
| Layers | 49: 2 dense, then 47 MoE | Model card, `mlp_only_layers` |
| Hidden size | 2,048; dense FFN 6,144; expert FFN 512; shared expert FFN 1,024 | `config.json` |
| Experts | 256 routed, 6 per token, sigmoid scores, top-k weights renormalized, no router bias; plus one always-active shared expert with no gate | `config.json`, tensor names |
| Attention | GQA: 32 query heads, 4 KV heads, head dimension 128; no bias | `config.json` |
| Attention layout | 14 full-attention layers (0, 1, 5, 9, ..., 45, 48) and 35 sliding-window layers with a 513-token window | `layer_types`, `sliding_window` |
| Positions | RoPE (theta 10,000, NeoX layout per the llama.cpp port) on sliding-window layers; no positional encoding (NoPE) on full-attention layers | `no_rope_layer_indices`, llama.cpp #30928 |
| Normalization | RMSNorm (epsilon 1e-6), pre-attention and pre-FFN; per-head RMSNorm on Q and K | `config.json`, tensor names |
| Attention sinks | One learned logit per head (`self_attn.softmax_offset`, `softmax_type: learnable`) | Tensor names, `config.json` |
| Context | 262,144 tokens in the configuration; the announcement says up to 512k and recommends staying within 256k | `config.json`, announcement |
| Vocabulary | 150,144 entries: a 150,000-token byte-level BPE plus 144 special tokens | `config.json`, `tokenizer.json` |
| Output head | Untied from the embedding; Recursive serves it with FP32 logits | `tie_word_embeddings`, SGLang kit |
| Knowledge cutoff | June 2026 | Model card |

**Tokenizer.** A byte-level BPE with a three-stage pre-tokenizer: split digit
runs of 1 to 3, split CJK runs, then a GPT-4-style word regex. llama.cpp's
port names it `flint` with `clean_spaces` off. Special tokens start at
150000: `<|begin_of_text|>`, `<|end_of_text|>`, `<|system|>`, `<|user|>`,
`<|assistant|>`, `<|observation|>`, `<think>`, `</think>`, `<tools>`,
`<tool_call>`, `<arg_key>`, `<arg_value>`, `<tool_response>`, and
`<|developer|>`, plus reserved tokens.

**Chat template.** Role markers are single tokens (`<|system|>`, `<|user|>`,
`<|assistant|>`, `<|developer|>`, `<|observation|>`); there's no BOS token for
chat and no default system prompt. The template removes NUL characters and
`<think>` tags from user, system, developer, and tool text, and escapes role
markers there. It uses a `\x00` string escape, which is why llama.cpp needed
#30929. Generation stops on token IDs 150001, 150003, 150005, and 150018.

**Reasoning.** The model decides on each turn whether to think in
`<think>...</think>`; no switch or budget controls it. On GPQA-Diamond,
Recursive measured a median of 4.3K reasoning tokens, p95 of 33K, and p99 of
46K. They recommend `max_tokens` of at least 32,768 for hard problems and
evaluated with up to 81,920. Recommended sampling: temperature 1.0, top-p
0.95, top-k 20; greedy decoding isn't validated. Clients must send
`reasoning_content` back on assistant turns. Source: the SGLang kit README.

**Tool calls.** Tools are listed as JSON inside `<tools>` in a system block.
A call is written as `<tool_call>NAME<arg_key>K</arg_key><arg_value>V</arg_value>...</tool_call>`,
and tool results return in an `<|observation|>` turn wrapped in
`<tool_response>`. The format has no escaping, so an argument can't contain
`</tool_call>`. Source: the chat template and the SGLang kit README.

### Our inference

- The parameter split checks out against the tensor shapes: routed experts
  are 37.85B of the 39.79B parameters; everything else (attention 0.93B,
  embedding and head 0.62B, shared experts 0.30B, dense layers 0.08B,
  routers 0.02B) is 1.94B. Per token the model reads about 0.89B of expert
  weights plus about 1.63B of other weights, which matches Recursive's 2.5B.
- The tool-call markup and the `<|observation|>` role resemble GLM-4.5's
  format, and the pre-tokenizer resembles GLM-4's. The architecture as a
  whole (sigmoid routing, shared expert, sinks, sliding windows with NoPE
  full layers) is a mix of ideas from DeepSeek-V3, Qwen-MoE, GPT-OSS, and
  recent hybrid-attention models, not a copy of one of them.
- KV cache per token is small. Recursive states 28 KiB per token on the 14
  full layers (about 7.5 GB for a full 262,144-token context in 16-bit), plus
  a fixed 513-token window on the other 35 layers, about 35 MB. A 32K context
  needs under 1 GB of KV cache.

## Formats and file sizes

| Repository | Format | Files | Size | Intended runtime (per its card) |
| --- | --- | --- | --- | --- |
| `RecursiveSI/flint` | BF16 safetensors | 15 shards | 79.6 GB | SGLang 0.5.16 kit on one B300, B200, or H200 (141 GB or more per replica) |
| `RecursiveSI/flint-int4` | MLX, group-64 affine INT4, INT8 embeddings | 5 shards | 22.6 GB | `rec` and MLX on Apple Silicon |
| `RecursiveSI/flint-int4-gguf` | GGUF Q4_1 (the MLX INT4 codes with FP16 scales and offsets; floating tensors in F32) | `flint-int4-q4_1.gguf` | 25,992,966,976 bytes, SHA-256 `43c07baccdba42eb7528a9630ee15699b1942c297291dc1b39a061f508ee1a4b` | llama.cpp with Metal, 48 GB unified memory or more |
| `RecursiveSI/flint-nvfp4-gguf` | GGUF NVFP4 (ggml type 40) | `flint-nvfp4.gguf` | 22,863,084,160 bytes, SHA-256 `97cc8009ff9b132d5cf3c488e6767962bb5a5dc158a90ed661bbbddf876b8bde` | llama.cpp with CUDA; tested on B300, RTX PRO 6000 Blackwell, RTX Spark, and DGX Spark; 48 GB or more recommended |

No FP8 checkpoint, no Q8_0 or K-quant GGUF, and no community quantizations
existed on 2026-10-21. Recursive says the INT4 and NVFP4 weights come from a
distillation stage with the BF16 model as teacher, and that the output head
is also 4-bit. Both GGUF repositories also ship ready-made `opencode.json`
and `pi-models.json` client configurations.

**Inference:** a K-quant made by a third party from the BF16 weights would
skip Recursive's quantization-aware distillation, so its quality is unknown
until measured. Prefer Recursive's own GGUFs.

## License

### Published facts

- All four repositories are tagged `apache-2.0` and include the same
  `LICENSE` file, which is the unmodified Apache License 2.0 text. Source:
  each repository's file list and card.
- The model card says hosted API terms and data practices are separate from
  the weight license. No acceptable-use policy or extra terms come with the
  weights.
- The SGLang kit has its own `NOTICE`: it adapts SGLang code (Apache 2.0)
  and patches FlashAttention-4 (BSD 3-Clause), and its installer downloads
  NVIDIA's CUTLASS DSL under NVIDIA's license. That applies only to the kit,
  which we don't plan to use.
- `rec` is Apache 2.0. llama.cpp is MIT.

### Our reading (not legal advice)

- Apache 2.0 allows commercial use, serving the model over a network, and
  redistributing the weights to Pylon operators, without royalties.
- When we redistribute the files, include a copy of the license and keep any
  attribution. If we make our own quantization, mark it as a modified work
  (Section 4(b)).
- The license grants no trademark rights (Section 6). Call the model "Flint
  by Recursive" to describe it, but don't use Recursive's logo or imply
  endorsement.
- Pylon operators download the files from Hugging Face by pinned revision
  and digest, as the catalog already does for every other model, so we
  needn't host copies ourselves.

## Benchmarks and speed claims

### Published facts

Recursive's chart compares Flint BF16 and NVFP4 with Qwen 3.5, Qwen 3.6,
and Laguna XS 2.1 (33–35B-total, 3B-active MoE) and with Qwen 3.8, Muse
Glimmer, and Gemma 4 (27–31B dense). Numbers marked with a dagger were
measured by Recursive; the others are as reported by each developer. Source:
[`flint_benchmarks_nvfp4.png`](https://huggingface.co/RecursiveSI/flint/blob/main/assets/flint_benchmarks_nvfp4.png).

| Benchmark | Flint BF16 | Flint NVFP4 | Best 3B-active MoE shown | Best 27–31B dense shown |
| --- | --- | --- | --- | --- |
| Terminal-Bench 2.1 | 54.9 | 53.7 | 44.9 (Qwen 3.6) | 73.0 (Qwen 3.8) |
| SWE-bench Pro v2 (all measured by Recursive) | 89.7 | 88.5 | 73.8 (Qwen 3.6) | 94.9 (Qwen 3.8) |
| SWE-bench Verified | 74.2 | 74.1 | 73.4 (Qwen 3.6) | 76.0 (Muse Glimmer) |
| LiveCodeBench v6 | 80.3 | 80.8 | 80.4 (Qwen 3.6) | 90.3 (Qwen 3.8) |
| IFBench | 89.2 | 88.1 | 70.2 (Qwen 3.5) | 79.5 (Qwen 3.8) |
| IFEval | 95.1 | 94.6 | 91.9 (Qwen 3.5) | 94.7 (Muse Glimmer) |

Method, per the model card and announcement: temperature 1.0, top-p 0.95,
top-k 20. Agentic scores are average pass@1 over at least five attempts with
agent timeouts of up to 5 hours. Terminal-Bench uses the Terminus 2 harness
with internet access, and Recursive counts solutions found through web
retrieval as failures. SWE-bench uses mini-swe-agent without internet.

Speed claims, per the announcement:

- More than 125 tokens per second on an M4 Pro MacBook Pro.
- More than 150 tokens per second on a 64 GB NVIDIA RTX Spark "at short
  contexts".
- Up to twice as fast as other models in its class, and more than five times
  faster than dense models with similar scores.
- The footnote defines speed as the average decode rate over a 128k-token
  session, with prefill excluded.
- Not stated: the quantization, the batch size, or the engine. The page
  credits "our optimized inference engine" and Eureka-generated kernels,
  including a Metal megakernel that fuses the whole decode step on M4 and
  M5 chips.
- `rec`'s README says it delivers 1.4 to 2.6 times llama.cpp's throughput on
  an M4 Pro (48 GB) with 4-bit weights, prompts of 1K to 252K tokens, 1,024
  output tokens, five runs per context, and prefill excluded.

### Our inference

- The speed numbers are almost certainly single-stream decode (batch size 1)
  with 4-bit weights on Recursive's own engine, `rec` on the Mac. They don't
  describe llama.cpp. From `rec`'s own ratio, llama.cpp on an M4 Pro would
  give roughly 50–90 tokens per second.
- Expect llama.cpp to start slower still: the pull request says its MoE
  activation scaling costs 10–30% until a follow-up lands.
- The SWE-bench Pro v2 number (89.7) is very high next to the other columns.
  Treat all of these as claims until our own harness reproduces some of
  them; the qualification step in [(e)](#e-steps-to-put-flint-on-pylons)
  does this.

## Runtime support

| Runtime | Status on 2026-10-21 | Source |
| --- | --- | --- |
| SGLang | Recursive's kit for SGLang 0.5.16 (a wheel, a FlashAttention-4 patch, Docker image), datacenter GPUs only. Upstream support is open in #45624; a cookbook page is merged (#45625). | Model card, SGLang PRs |
| llama.cpp | Open pull request #30928 from a Recursive engineer (head `1b44e30775dcb2f93952116b05f89e5bd797fb00`), mergeable. It depends on #30929 and #30861, which are merged. The GGUF cards require "a release with Flint support", which doesn't exist yet. | llama.cpp PRs |
| vLLM | Open pull request #62938: BF16 and the NVFP4 GGUF (weight-only W4A16), with Flint kernels for small-batch Blackwell decode. | vLLM PR |
| MLX | Recursive's INT4 export runs on `rec`. Upstream `mlx-lm` support is open in #1998. | `flint-int4` card, mlx-lm PR |
| `rec` | Recursive's engine for M4 Pro, M5 Pro, and M5 Max with 48 GB or more. Mixed Python, Go, C++, and TypeScript, installed with `curl | bash`. | `rec` README |
| Transformers | Not supported yet; only the tokenizer and template load. | SGLang kit README |
| Ollama, Unsloth, LM Studio | Named in the announcement; no artifacts or merged support found. **Inference:** they follow once llama.cpp merges. | Announcement |

## (a) Can a Pylon serve Flint today?

**No.** Each path is blocked on something specific.

### Through Pylon's managed llama.cpp lane

Pylon already supervises `llama-server` for families without a native
Psionic decoder (`--text-engine llama-cpp`, or `--managed-llama` for serial
recovery; see [Choose another text model](../../compute/pylon.md#choose-another-text-model)).
This is the closest path, and it needs:

1. **A Flint-capable `llama-server`.** No release has the `flint`
   architecture. We'd build from the #30928 branch at a pinned commit.
   `OPENAGENTS_LLAMA_CPP_BIN` already points Pylon at any binary.
2. **A longer context.** `psionic_command` in
   [`crates/pylon/src/online.rs`](../../../crates/pylon/src/online.rs)
   starts every llama.cpp lane with `--ctx-size 4096`, and
   `scripts/bench/pylon-models/run.py` does the same. That's too small for a
   reasoning model and for Pylon's own 96 KiB input bound. Flint's KV cache
   is small enough that 32K–64K costs under 2 GB.
3. **A larger output cap and deadline.** Pylon's default `--max-tokens` is
   512, and a job has 90 seconds. The engine client also times out after 120
   seconds (`Psionic::new` in
   [`crates/pylon/src/engine.rs`](../../../crates/pylon/src/engine.rs)).
   `finished_text` correctly refuses an answer cut off by `length` or one
   with reasoning but no final text, so many Flint jobs would fail rather
   than return wrong output. At about 100 tokens per second, 90 seconds
   allows about 9,000 tokens, which covers Recursive's median reasoning
   length but not its long tail. Declared deadlines
   ([#12401](../../compute/pylon.md#declared-deadlines-12401)) exist, but a
   complete Pylon's ceiling is fixed at 90 seconds.
4. **Sampling.** `Sampling` sends only temperature, top-p, and seed. Flint
   wants top-k 20 and min-p 0. Pass these as `llama-server` launch flags
   (`--top-k 20 --min-p 0`), or add `top_k` to `Sampling`. Pylon's
   benchmark runner uses greedy decoding when thinking is off, which Flint
   doesn't support; the catalog row must set `thinking_required`.
5. **Reasoning separation.** #30928 doesn't touch llama.cpp's chat parser.
   Whether `llama-server` moves `<think>` text into `reasoning_content`
   instead of leaving it in `content` needs one check. If it doesn't, Pylon
   would return reasoning as the answer. The lane's DeepSeek-only
   `--reasoning-format deepseek` switch shows where a per-profile setting
   goes.
6. **A model ID for NVFP4.** Pylon names a model by a digest of its tensors
   ([`crates/pylon/src/model_id.rs`](../../../crates/pylon/src/model_id.rs)).
   Its block table knows Q4_1 (type 3) but not NVFP4 (type 40, 64-value
   blocks of 36 bytes), so it can't compute an ID for the NVFP4 file.

With those changes, the Q4_1 GGUF on the Mac is a working configuration.
The Metal lane passes `--n-gpu-layers all` in serial mode.

### Through native Psionic

`psionic-openai-server` refuses the file: `GgufDecoderFamily` has no `flint`
member, and the tokenizer loader has no `flint` pre-tokenizer. Psionic's GPU
backends also have no Q4_1 or NVFP4 kernels. See (b).

### Through the SGLang kit

Not on our hardware: it needs a 141 GB GPU, runs only BF16, and installs
patched third-party kernels. It's also outside the rule that Pylon's serving
path is Psionic or a supervised llama.cpp.

## (b) Native Psionic support

Psionic serves `qwen2`, `qwen35`, `qwen38`, and `gemma4` on CUDA,
`qwen35`, `qwen38`, and `gemma4` on Metal, and GPT-OSS through its own
server ([Psionic README](../../psionic/README.md)). Several Flint pieces
already exist somewhere in that code, though not all on both backends.

| Piece | Flint needs | Psionic today | Work |
| --- | --- | --- | --- |
| Family and metadata | `flint` architecture: per-layer attention type, RoPE pattern, 513 window, leading dense count, expert and shared-expert counts, weight normalization | No family; the GGUF metadata keys come from #30928 and could change before it merges | New family in `psionic-models`; pin the converter's key names to a merged llama.cpp commit |
| Tokenizer | Byte-level BPE, `flint` pre-tokenizer (3 regex stages), 144 special tokens, extra end-of-generation tokens | `Gpt2Bpe` exists; pre-tokenizers are a closed list with a `Custom` fallback | Add the `flint` pre-tokenizer and fixtures against `tokenizer.json` |
| Chat template | Flint template, `\x00` escapes, role-marker escaping, reasoning split | Templates are pinned by hash and unknown ones fail closed | Add a pinned Flint template with rendered fixtures; split `<think>` into `reasoning_content` |
| Tool-call parser | `<tool_call>` with `<arg_key>` and `<arg_value>` | Not needed for Pylon's text lane, which refuses tool calls | Defer |
| Grouped-query attention | 32 Q heads, 4 KV heads, head dimension 128 | Exists | None |
| QK RMSNorm | Per-head norms on Q and K | Exists (`qwen35`, `gemma4`, V4.1 stage) | Reuse |
| Attention sinks | One learned logit per head | CUDA in the GPT-OSS path; not on Metal | Port to the shared decoder on CUDA; write the Metal kernel |
| Sliding window | 513 tokens on 35 layers, full on 14, irregular pattern | Exists in `gemma4` and GPT-OSS with their own patterns | Generalize to a per-layer list; keep a 513-entry ring buffer for those layers |
| RoPE and NoPE | NeoX RoPE (theta 10,000) on window layers; none on full layers | RoPE exists; there's no per-layer skip | Small: a per-layer flag |
| MoE routing | Sigmoid scores, top-6 of 256, renormalized, no bias | Sigmoid routing exists in the DeepSeek V4.1 CPU reference and CUDA mix; softmax top-k in GPT-OSS and `gemma4` | Reuse V4.1's routing in the decoder path; add Metal |
| Shared expert | Always on, FFN 1,024, no gate | Exists in V4.1 | Reuse |
| Expert dispatch | Grouped matrix-vector products by expert ID | Exists for GPT-OSS (MXFP4) and `gemma4` sparse on Metal and CUDA | Add the Flint quantization types |
| Quantization | Q4_1 (Mac GGUF), NVFP4 (CUDA GGUF), F32 embeddings | Q4_1 on CPU only; no NVFP4 | Q4_1 matvec kernels for Metal and CUDA; NVFP4 dequantization on CUDA, and Blackwell FP4 paths later |
| Activation scaling | llama.cpp #30861 scales expert down-projections to keep 8-bit activation sums within F16 | Psionic's CUDA `q8_1` activation path has the same risk | Check for overflow on golden activations; add per-expert scaling if needed |
| Output head | FP32 logits recommended | Exists | Confirm |
| Long context | 262K positions | Tested much shorter | Qualify at 32K first |

**Estimate:** 4–7 engineer-weeks for qualified CUDA and Metal text serving,
assuming the work follows the DeepSeek DS0 pattern (a CPU reference
first, then golden activations, then each GPU backend):

- CPU reference with tokenizer and template fixtures: 1–1.5 weeks.
- CUDA decoder with sinks, sliding windows, NoPE, sigmoid MoE, and Q4_1:
  1.5–2 weeks.
- Metal decoder, including new sink and Q4_1 expert kernels: 1.5–2 weeks.
- NVFP4 on CUDA and qualification through Pylon: 1–1.5 weeks.

Wait for #30928 to merge before you freeze the GGUF metadata names. Until
then, managed llama.cpp is the right route; Psionic doesn't block it.

## (c) Hardware fit

Memory figures come from the published file sizes and Recursive's KV-cache
figure. Speeds are **estimates**. The only measured anchor is our own
Nemotron 3 Nano 30B-A3B row: a 22.4 GB 3B-active MoE that decoded at 53–57
tokens per second through llama.cpp on the RTX 4080 with system-memory
offload ([results](../../../scripts/bench/pylon-models/nemotron-small-rtx4080-2026-10-20.json)).

| Format | Weights | Plus KV at 32K | Plus KV at 262K |
| --- | --- | --- | --- |
| BF16 | 79.6 GB | about 80.5 GB | about 87 GB |
| Q4_1 GGUF | 26.0 GB | about 27 GB | about 33.5 GB |
| NVFP4 GGUF | 22.9 GB | about 24 GB | about 30.5 GB |
| MLX INT4 | 22.6 GB | about 23.5 GB | about 30 GB |

Add 1–3 GB of compute buffers to each. Each routed-expert layer is about
0.5 GB in Q4_1.

| Machine | Fits? | Format and placement | Expected decode, one stream |
| --- | --- | --- | --- |
| Mac M5 Max, 128 GB (`christohersmbp2`) | Yes, with the full context | Q4_1 GGUF on Metal, all layers on the GPU | Measured: 83–95 tokens per second end to end with llama.cpp and 109–140 with `rec`; see [Local measurements](#local-measurements-on-this-mac). The estimate was 80–150. The M5 Max has about twice the M4 Pro's memory bandwidth, and `rec`'s chart puts llama.cpp on an M4 Pro at roughly 50–90. |
| Other Apple silicon, 48 GB or more | Yes | Q4_1 on Metal | Recursive's 48 GB minimum; M4 Pro estimate 50–90 tokens per second with llama.cpp |
| Apple silicon under 48 GB | No | — | — |
| RTX 4080 16 GB, 126 GB RAM (`coderos-4080-net`) | Only with offload | Q4_1 with about half the expert layers in system memory (`--n-cpu-moe`); NVFP4 brings no gain on Ada | Estimate 45–65 tokens per second, near the Nemotron 30B-A3B result. It can't share the GPU with Gemma and Clef without pausing them. |
| GCE L4 24 GB, 32 GB RAM (`oa-pylon-gpu-1`) | Barely | Q4_1 with about 6–8 expert layers in system memory, which uses most of the VM's 32 GB of RAM; or NVFP4 with almost no room for KV, and Ada has no FP4 tensor cores | Estimate 25–45 tokens per second. Not recommended without a larger VM. |
| RTX PRO 6000 Blackwell 96 GB (not in the fleet) | Yes, with headroom | NVFP4 with the full context on native FP4 hardware; BF16 also fits at short context | Estimate 150–250 tokens per second; Recursive tested this card |
| Sealed Clef on Intel TDX (`c3-standard-8`) | No | 8 vCPUs and 32 GB with no GPU; it serves Clef decisions. The H100 sealed lane in [private inference](../../security/private-inference.md#the-sealed-gpu-lane-h100-in-confidential-computing-mode) would hold the NVFP4 or BF16 file, but it's a separate design. | Not a target today |

## (d) Fit with Coder and Microcoder

### What exists

- Microcoder's providers are a closed set: `codex`, `claude`, `vertex`,
  `devin`, `opencode`, and `grok`
  ([`crates/microcoder-loop/src/capacity.rs`](../../../crates/microcoder-loop/src/capacity.rs)).
  None of them is a generic OpenAI-compatible endpoint or a Pylon.
- Coder's OpenCode route hands a whole turn to OpenCode, with the model
  named in OpenCode's `PROVIDER/MODEL` form (`CODER_OPENCODE_MODEL`; see
  [the delegate door](../../coder/runtime/delegate-door.md)). Recursive ships
  an `opencode.json` for Flint that points OpenCode at a local
  `llama-server` as `flint-local/flint`.
- Pylon's conversation lane carries text turns (system, user, and assistant)
  and refuses any answer that asks for a tool. It forwards no `tools`,
  `response_format`, or reasoning history.

### Our assessment

- **Local Flint for Coder works now** on a Mac with 48 GB or more. Run a
  Flint-capable `llama-server` on loopback, give OpenCode Recursive's
  configuration, and set `CODER_OPENCODE_MODEL=flint-local/flint`. This needs
  no OpenAgents code. Check that our OpenCode route lets OpenCode read a
  custom configuration file.
- **Pylon-served Flint for coding agents needs more than a model.** Flint is
  a tool-calling agent model, and its benchmark scores come from agent
  harnesses with tools and long sessions. Pylon's lane offers single text
  answers under 90 seconds. Microcoder's own loop asks for one structured
  `next_action` JSON reply per step, which a text lane could carry. That
  would need a Pylon-backed generator for Microcoder and a qualification of
  Flint's JSON output, because Pylon doesn't forward `response_format`.
- **Not the network's default coding model yet.** Reasons:
  - Recursive's scores are unverified.
  - Only the Mac holds the whole model in fast memory; the 4080 and L4 need
    offload.
  - llama.cpp support isn't merged, and native Psionic support doesn't
    exist.
  - Long reasoning conflicts with the current deadline and output bounds.

  If it qualifies, it's the best candidate we've seen for a "coding" text
  lane on machines with 48 GB or more. Its active size (2.5B) decodes like a
  small model, while its file fits a high-memory Mac or a Blackwell
  workstation card. Run it against `nemotron3-nano-30b`, which is already in
  the catalog at a similar footprint.

## (e) Steps to put Flint on Pylons

Each step is sized to be one issue. None are filed.

1. **Pin a Flint-capable llama.cpp.** Build `llama-server` from
   `alexey-rsi/llama.cpp` at `1b44e30775dcb2f93952116b05f89e5bd797fb00` (PR
   #30928) for Metal on the Mac and CUDA on CoderOS. Record the commit
   beside `prepare-llama-runtime.sh`. Replace it with the first llama.cpp
   release that includes Flint. Acceptance: `llama-server --version` reports
   the pinned commit, and a loopback chat answers on each machine.
2. **Add catalog rows.** In `scripts/bench/pylon-models/models.json`, add
   `flint-q4_1` (repo `RecursiveSI/flint-int4-gguf`, revision
   `9c47151036bc2c34331142dda04488bceab2bc9f`, file `flint-int4-q4_1.gguf`,
   25,992,966,976 bytes, SHA-256 `43c07bac...ee1a4b`, `backend: metal`) and
   `flint-nvfp4` (repo `RecursiveSI/flint-nvfp4-gguf`, revision
   `6ec43e19b565fc8d560e2d04985d7724f20673bf`, file `flint-nvfp4.gguf`,
   22,863,084,160 bytes, SHA-256 `97cc8009...8b8bde`, `backend: cuda`).
   Set `engines: ["llama-cpp"]`, `upstream_model: RecursiveSI/flint`,
   `upstream_revision: 2a247be0c0aec895ccb0ac7ca92229ec6df37288`,
   `parameters_billions: 39.8`, `active_parameters_billions: 2.5`,
   `thinking_required: true`, `reasoning_temperature: 1.0`,
   `reasoning_top_p: 0.95`, and `max_output_tokens: 16384`.
3. **Teach `model_id` NVFP4.** Add ggml type 40 with block `(64, 36)` to
   `block` in `crates/pylon/src/model_id.rs`, with a test. The beacon's
   `model` then reads `flint-q4_1@gguf-tensors:sha256:<hex>` or
   `flint-nvfp4@gguf-tensors:sha256:<hex>`. The two quantizations are
   different artifacts and must not share an identity.
4. **Make the llama.cpp lane configurable per profile.** Replace the fixed
   `--ctx-size 4096` with a profile value (Flint: 32,768 to start). Add
   `--top-k 20 --min-p 0 -fa on` for Flint, plus an expert-offload count
   (`--n-cpu-moe N`) for GPUs smaller than the file. Update `run.py` to
   match.
5. **Confirm reasoning separation.** Send one reasoning prompt through the
   pinned `llama-server` and check that `<think>` text arrives in
   `reasoning_content`, not `content`. If it doesn't, add a Flint reasoning
   format to the lane, as the DeepSeek profile does.
6. **Add a `flint` text profile.** Extend `serial::Profile` with Flint
   rows that verify the pinned file and select llama.cpp, one slot, the
   context from step 4, and an output cap of 8,192–16,384 tokens.
7. **Let one Pylon declare a longer deadline.** Allow a complete Pylon's
   beacon to declare a ceiling above 90 seconds for a reasoning profile, as
   a group head already can, and raise the engine client's 120-second HTTP
   timeout to follow the admitted deadline. Update NIP-PYLON's declared
   deadlines section.
8. **Qualify on the Mac.** Run
   `run.py --engine llama-cpp --models flint-q4_1 --thinking on` on the M5
   Max with its own Pylon home, as the
   [runbook](../../compute/pylon-network-runbook.md) describes, and retain
   `scripts/bench/pylon-models/flint-m5max-<date>.json`. Record decode
   tokens per second at 1K, 8K, and 32K prompts, reasoning lengths, exact
   checks, and jobs that ran out of time or tokens.
9. **Qualify on the RTX 4080.** The same run with Q4_1 and expert offload,
   pausing Clef as the Nemotron runs did, and compare it with
   `nemotron3-nano-30b`. Try the L4 only after this result, on a VM with at
   least 64 GB of RAM.
10. **Check coding quality ourselves.** Run a small fixed Terminal-Bench
    subset through the Gym with Flint behind OpenCode, and compare it with
    Recursive's 54.9. This decides whether Flint becomes a coding lane.
11. **Record license and attribution.** Add a short attribution line ("Flint
    by Recursive, Apache 2.0") where the catalog lists models, and keep
    upstream `LICENSE` files beside any copies we distribute. If we ever
    publish our own quantization, mark it as modified.
12. **Defer native Psionic.** Open a tracking issue for (b) after #30928
    merges and steps 1–9 show that Flint is worth a native lane.

## Local measurements on this Mac

Measured on 2026-10-21 on the owner's MacBook Pro, the machine that also runs
the "Mac M5 Max (network lane)" Pylon. That Pylon was left running and
untouched. This section replaces the Mac estimate in
[(c)](#c-hardware-fit) with measurements, and it answers the first open
question: llama.cpp at the pinned commit does return reasoning separately.

### Setup

| Item | Value |
| --- | --- |
| Machine | MacBook Pro `Mac17,6`, Apple M5 Max (18 CPU cores: 6 Super and 12 Performance), 128 GB unified memory, macOS 26.4 (25E246) |
| Background load | Other agents' Rust builds and Chrome ran throughout. Load average was 3 to 10, and the GPU showed 37% utilization from other processes while our servers sat idle. The Pylon's two Psionic servers were idle but held about 22 GB of memory. |
| Q4_1 GGUF | `RecursiveSI/flint-int4-gguf` at `9c47151036bc2c34331142dda04488bceab2bc9f`, `flint-int4-q4_1.gguf`, 25,992,966,976 bytes, SHA-256 `43c07baccdba42eb7528a9630ee15699b1942c297291dc1b39a061f508ee1a4b` (checked locally) |
| MLX INT4 | `RecursiveSI/flint-int4` at `3b93f9053b530488405ae9fca51d4900d6ce21f0`, fetched by `rec pull` (22.6 GB). `rec` verified the set; its digest is `fdc85248ad77a0fa9fbf0abda8b1525bea8ee74c11ada2ec95eb66113353d677`. |
| llama.cpp | `alexey-rsi/llama.cpp` at `1b44e30775dcb2f93952116b05f89e5bd797fb00` (PR #30928), plus a cherry-pick of `9febe63d5a379f8568bae7116a5b2ba844a4a4a6` (#30929, Jinja `\x` escapes), which the branch lacks. The local head is `5780a092`, which llama-bench reports as build 11798. |
| llama.cpp build | `cmake -B build -DGGML_METAL=ON -DGGML_ACCELERATE=OFF -DGGML_BLAS=OFF -DLLAMA_CURL=OFF -DLLAMA_BUILD_TESTS=OFF -DCMAKE_BUILD_TYPE=Release`. Accelerate is off because the macOS 26.5 SDK headers fail to compile `ggml-cpu.c` with it (`uuid_string_t` is undefined). Metal does the model's work, so this doesn't affect GPU results. |
| `rec` | v0.1.0. The release binary `rec-darwin-arm64` has SHA-256 `22f08e8505e5ace84475c23eefffd116b4b201ad7e796aaea76d223f0e3862de`, from source commit `e53663b262af4cb1ca498e596ca8b1a2d79e0800`. Engine: MLX on Metal, bundled runtime SHA-256 `09063110dfe90c36bc40a518a64c3e9110387d449adf000be85023cfc8d46de1`. |

Every third-party build, download, and benchmark script ran as the normal
user from a scratch folder, never as root. At the owner's request, `rec` was
then installed for normal use with its own installer (`install.sh`, read
first, pinned with `REC_VERSION=0.1.0`). The installer put `rec` in
`~/.local/bin` and its data in `~/.local/share/rec`. It also appended a PATH
line to `~/.zshrc`, although `~/.local/bin` was already on PATH.

### Method

- **Kernel throughput:** `llama-bench -ngl 99 -fa 1 -r 3`, with prompt
  processing at 512, 2,048, and 16,384 tokens and decode of 128 and 512
  tokens at context depths 0, 2,048, and 16,384. It runs one sequence and
  doesn't sample.
- **End to end:** the same OpenAI-compatible client against both servers.
  It streams one request at a time, with the chat template applied by the
  server and the card's sampling (temperature 1.0, top-p 0.95, top-k 20; for
  llama.cpp, min-p 0). Prompts were short (32 tokens), medium (1,641
  tokens), and long (14,950 tokens), built from llama.cpp's own C++ source.
  Each size had one warm-up and three measured runs with up to 512 output
  tokens, and each run's prompt starts with a unique tag so no prefix cache
  helps.
  - Time to first token is when the first reasoning or answer text arrives.
  - Decode rate is output tokens minus one, divided by the time after the
    first token.
  - Prompt rate is prompt tokens divided by time to first token.
- **llama-server:** `llama-server -m flint-int4-q4_1.gguf --alias flint -ngl all -c 32768 -np 1 -fa on -ctk f16 -ctv f16 --jinja --temp 1 --top-p 0.95 --top-k 20 --min-p 0 --host 127.0.0.1 --port 18431`.
- **rec:** `rec serve` on its default `127.0.0.1:11500` with the default
  262,144-token context.
- Peak memory comes from `/usr/bin/time -l` for llama.cpp and from
  `footprint` for `rec`'s engine process. Power wasn't measured, because
  `powermetrics` needs root.

### Results

llama-bench, Q4_1 on Metal (tokens per second, mean and standard deviation
of three runs):

| Test | First run (load 5–8) | Second run (load 8–10) |
| --- | --- | --- |
| Prompt, 512 tokens | 3,653 ± 7 | — |
| Prompt, 2,048 tokens | 3,497 ± 13 | — |
| Prompt, 16,384 tokens | 2,449 ± 180 | — |
| Decode 128 at depth 0 | 126.4 ± 0.6 | 113.9 ± 15.6 |
| Decode 512 at depth 0 | 128.8 ± 0.8 | — |
| Decode 128 at depth 2,048 | 113.6 ± 3.3 | 90.4 ± 4.1 |
| Decode 128 at depth 16,384 | 100.2 ± 1.8 | 80.1 ± 6.9 |

End to end, medians of three runs:

| Prompt | Engine | Time to first token | Prompt tokens/s | Decode tokens/s |
| --- | --- | --- | --- | --- |
| Short (32) | llama-server, Q4_1 | 0.10 s | 316 | 87 |
| Short (32) | `rec`, MLX INT4 | 0.35 s | 92 | 125 |
| Medium (1,641) | llama-server, Q4_1 | 0.99 s | 1,649 | 83 |
| Medium (1,641) | `rec`, MLX INT4 | 1.11 s | 1,481 | 140 |
| Long (14,950) | llama-server, Q4_1 | 10.39 s | 1,439 | 95 |
| Long (14,950) | `rec`, MLX INT4 | 8.82 s | 1,695 | 109 |

Individual end-to-end decode runs ranged from 78 to 96 tokens per second for
llama-server and from 106 to 152 for `rec`. A separate llama-server check
with 512 forced tokens gave 84 to 124 tokens per second with sampling and 76
to 88 with greedy decoding. That spread is too wide to blame on sampling
cost; it follows the machine's other load.

Peak memory:

- llama-server: 30.4 GB maximum resident set, which includes the 26 GB
  memory-mapped model, and a 4.9 GB peak footprint without the mapped file.
- llama-bench: 27.3 GB maximum resident set.
- `rec`: 24 GB physical footprint after the runs, 23 GB of it in GPU
  buffers, with a 28 GB peak.

Disk use: 26.0 GB for the GGUF and 22.6 GB for the MLX weights. Free space
stayed above 195 GiB throughout.

### Quality checks

Five tasks, one sample each, at the card's sampling with up to 16,384
output tokens. The first four are coding tasks whose code ran against fixed
tests in a separate `python3 -I` process. The fifth is a scheduling
question whose answer is 38.

| Task | llama-server, Q4_1 | `rec`, MLX INT4 |
| --- | --- | --- |
| Merge intervals | Pass, 454 tokens, 3.7 s | Pass, 224 tokens, 1.4 s |
| Fix a buggy binary search | Pass, 781 tokens, 6.6 s | Fail, 510 tokens, 3.7 s: fixed `lo` but left `hi = mid - 1` with an exclusive bound |
| Roman numerals, both directions (all 3,999 round trips) | Pass, 1,097 tokens, 9.7 s | Pass, 717 tokens, 6.0 s |
| O(1) LRU cache | Pass, 1,303 tokens, 12.1 s | Pass, 505 tokens, 3.8 s |
| Scheduling question | Pass (38), 1,175 tokens, 12.0 s | Pass (38), 848 tokens, 6.4 s |

Both servers separated reasoning cleanly. Every answer came back with
`finish_reason: stop`, reasoning in `reasoning_content`, and no `<think>`
tags in `content`. Answers were short and in the requested format: one code
block, or a two-sentence explanation ending with the `ANSWER:` line. With
one sample per task, the single `rec` failure says nothing about a
difference between the two quantizations.

### What the numbers show

Facts:

- On this M5 Max, `rec` decoded at 109–140 tokens per second (medians) and
  llama.cpp at 83–95 through its server. llama.cpp's sampling-free
  `llama-bench` reached 126–129 tokens per second at empty context in the
  quieter run.
- Under the same client, `rec` was 1.16 to 1.7 times as fast as
  llama-server at decode. At prompt processing it was slower for short
  prompts and about 18% faster at 15K tokens. llama.cpp's
  `llama-bench` prompt rates (2,400–3,650 tokens per second) are higher than
  either server's end-to-end rate.
- llama.cpp decode fell by about 20% from empty context to 16K tokens.

Inference and estimates:

- Recursive's claim is more than 125 tokens per second on an M4 Pro,
  averaged over a 128K-token session. This Mac reached that figure with
  `rec` at short and medium prompts but not at 16K, and it has roughly twice
  the M4 Pro's memory bandwidth (Apple's M4 Pro figure is 273 GB/s; we
  didn't measure this machine's). So this run doesn't confirm the M4 Pro
  claim. A test on an M4 Pro, or on a quiet machine across a 128K session,
  would.
- `rec`'s README claims 1.4–2.6 times llama.cpp; we saw 1.2–1.7 times at up
  to 16K tokens, which is consistent with the low end. The PR notes that
  llama.cpp's Flint path loses 10–30% to MoE activation scaling until a
  follow-up lands.
- Background load cost llama.cpp 10–20% between the two llama-bench runs, so
  quiet-machine numbers would be higher for both engines.
- For Pylon: the Mac serves Flint at about 85–125 tokens per second through
  llama.cpp, which fits the plan in [(e)](#e-steps-to-put-flint-on-pylons).
  At that rate, Pylon's 90-second deadline covers roughly 7,500–11,000
  tokens.

### What's left on this Mac

- `rec` 0.1.0 stays installed for the owner (`~/.local/bin/rec`, data and
  the 22.6 GB model in `~/.local/share/rec`). Its server was left running on
  `http://127.0.0.1:11500/v1`, an OpenAI-compatible endpoint whose key
  `rec auth token` prints. Stop it with `rec stop`; remove everything with
  `rec uninstall`.
- The Q4_1 GGUF (26.0 GB) and the llama.cpp build stay in the agent scratch
  folder for the Pylon steps in (e). llama-server was stopped.
- The `mlx-lm` PR branch (#1998) was installed in a scratch virtual
  environment but not measured, because `rec` already covers the MLX INT4
  weights.

## Open questions

- Does `llama-server` at the pinned commit parse Flint's tool calls? It does
  return `reasoning_content` correctly; see
  [Local measurements on this Mac](#local-measurements-on-this-mac).
- Does llama.cpp's NVFP4 path run correctly on Ada (the RTX 4080 and L4), or
  only on Blackwell? Recursive lists only Blackwell cards as tested.
- How much does llama.cpp's MoE activation scaling slow Flint on Metal,
  and when does the follow-up fix land?
- Will the GGUF metadata names in #30928 change before it merges? If they
  do, Recursive's published GGUFs may need a re-upload, which would change
  their digests.
- The announcement says 512k tokens, while the configuration and GGUFs say
  262,144. Which one is right?
