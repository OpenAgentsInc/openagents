# NVIDIA Flare: notes for Psionic and Pylons

Sources, read on 2026-10-21:

- [`NVIDIA/flare-local-inference`](https://github.com/NVIDIA/flare-local-inference)
  at commit `2f08b29cefd6836d627ee0a76cb53b1fd4ac78a7`. The repository was
  created on 2026-10-20 and has no tagged releases or changelog yet.
- The FlashInfer fork it pins,
  [`local-inference-lab/flashinfer`](https://github.com/local-inference-lab/flashinfer)
  at commit `03de0b663abe8f5dc69d4e10118587ab459b13eb`, which holds the
  b12x kernels.

We read the code as data and didn't build or run it. Paths below are
relative to the Flare repository unless they start with `flashinfer/`.
Facts come from the files cited. Paragraphs marked **Inference** are our
reading of what those facts mean for us.

**Verdict:** Flare is the closest match to our V4.1 work we have seen. It
serves the same checkpoint revision on the same GPU, so it's valuable as a
design source and a second SM120 implementation to compare against. It
isn't a good Pylon backend for V4.1, and it doesn't change the decision to
keep Psionic as the only engine in the V4.1 serving path. Four of its
designs are worth reimplementing in Psionic; they're listed in
[Designs worth reimplementing](#designs-worth-reimplementing).

## What Flare is

Flare is a local inference server for NVIDIA SM120 and SM121 GPUs, aimed at
long conversations, coding agents, and repeated tool use on workstations
and DGX Spark (`README.md:7-10`). It has two halves:

- **Ignition** is Python. It loads a checkpoint with PyTorch and
  Transformers, autotunes FlashInfer b12x kernels, compiles each forward
  with `torch.compile`, captures CUDA graphs, and exports a *runtime
  package* (`AGENTS.md:3-8`, `AGENTS.md:202-206`).
- **Helios** is a Rust library, and `flare` is the Rust CLI that links it.
  Helios owns sockets, tokenization, chat templates, scheduling, cache
  operations, graph replay, and streaming. Serving never calls Python
  (`AGENTS.md:207-215`, `docs/core-design.md:381-403`).

The design rules are strict: no eager fallback, no compilation or graph
capture while serving, no GPU allocation in steady state, and requests
beyond the prepared capacities fail admission (`AGENTS.md:211-213`). Its
own design note argues against a full rewrite in Rust, because that would
cut it off from the Python model and kernel ecosystem
(`docs/core-design.md:21-39`).

### Hardware

- The README and design docs name SM120 and SM121
  (`docs/core-design.md:3-6`). The preparation code also accepts SM103:
  `SUPPORTED_CAPABILITIES = ((10, 3), (12, 0), (12, 1))`
  (`ignition/planning.py:31-32`), and some components have SM103-specific
  paths (`ignition/components/*_sm103.py`).
- DGX Spark's GB10 is SM121 and needs its own package; an SM120 package
  fails the architecture check (`docs/model-preparation.md:602-604`).
- Published packages come in `gb10` and `gb202` hardware variants, and
  RTX PRO 6000 Blackwell selects `gb202` (`docs/cli-design.md:529-534`).
- Serving builds for Linux and for Windows 11 x64 and ARM64; the Windows
  runtime needs only the NVIDIA driver (`docs/windows.md:3-14`). There is
  no CPU, Apple, or non-NVIDIA path, and adding one is out of scope
  (`AGENTS.md:35-38`).

**Inference:** consumer RTX 50-series cards built on GB202 are SM120 too,
so they meet the architecture check. Flare's docs only name the RTX PRO
6000 and DGX Spark, so treat consumer cards as untested.

### Models

Recipes live in `ignition/recipes/`: DeepSeek V4.1 Flash, DeepSeek V4
Flash, GLM 5.3 Flash, Kimi K3, MiMo V2.6 Pro, Nemotron 3.5 Bolt, Qwen3.8
Flash, Qwen3.8 27B, and Step-5 Preview.

The DeepSeek V4.1 recipe pins exactly the checkpoint we serve:
`deepseek-ai/DeepSeek-V4.1-Flash-1017` at revision
`1bae0e1088993b9b82de08f59de5be5a34e42170`
(`ignition/recipes/deepseek41/recipe.py:43-47`). Its configuration check
requires block-32 FP8 weights with UE8M0 scales and FP4 experts, the same
formats as our kernels (`ignition/recipes/deepseek41/config.py:76-79`).
Specifically:

- **Tensor parallelism of four only.** The recipe declares rank costs for
  four ranks and no other count (`recipe.py:52-53`). Device weights are
  306.6 GB, about 76.6 GB per rank (`recipe.py:45`), so it needs four
  96 GB cards in one host, or four DGX Sparks joined by RoCE
  (`docs/model-preparation.md:263-281`).
- **Features:** text, reasoning, DSML tool calls, and images when the
  checkpoint has its vision encoder; DSpark speculative decoding with five
  draft tokens by default; and up to 64 concurrent requests
  (`docs/model-preparation.md:226-243`, `recipe.py:48-49`).
- **Experts:** MXFP4 experts run through b12x's W4A8 path, which quantizes
  activations to MXFP8 (`ignition/components/weights.py:803-805`). An
  optional flag compresses the expert scales losslessly to CSF while
  loading the original checkpoint (`docs/model-preparation.md:235-239`).
- **Engram:** the 188.8 GiB of tables are split by hash head across the
  four ranks. They're read from disk on demand through direct I/O, or held
  in RAM. With disk storage, the scale planes can stay resident at about
  1.4 GiB per rank (`docs/model-preparation.md:244-260`).
- **Performance:** the repository publishes no V4.1 throughput figure. It
  has a decode benchmark script (`scripts/benchmark_ds41_decode.py`) but no
  recorded results.

## Architecture

### Memory is planned before serving

Each recipe declares stored bytes, device bytes, draft bytes, and per-rank
costs for a state row, a cache page, and graph storage (`recipe.py:43-53`).
Preparation sizes scratch, live state, and the cache from the selected
concurrency and context, then captures every graph variant. Unsupported
capacities fail instead of shrinking silently
(`docs/core-design.md:424-428`). At load, Helios reserves memory for
resident Engram scales and any row cache before it sizes KV storage
(`docs/model-preparation.md:258-260`).

V4.1 cache pages cover 128 tokens. A per-request continuation row holds
everything needed to resume: sliding windows, pending compression state,
partial pages, DSpark windows, the last hidden state, and the three-token
Engram history (`ignition/recipes/deepseek41/pool.py:28-47`). The record
sizes are 528 bytes for a sliding-window entry, 288 bytes for a compressed
latent entry, and 68 bytes for an index entry (`pool.py:10-13`).

### FP8 KV cache

Every recipe defaults to FP8 E4M3 KV storage. DeepSeek recipes keep their
packed latent formats, including scale metadata and BF16 rotary fields, and
the writers and readers use matching scales
(`docs/model-preparation.md:41-48`).

### Long chats and prefix reuse

Helios keeps immutable prefix checkpoints on the GPU and can retain them in
host memory or on disk. Each checkpoint holds complete model state, not
only KV pages. Host offload is off by default until its latency thresholds
pass (`docs/prefix-cache-offload.md:3-17`).

### API surface

Helios serves OpenAI Chat Completions, Completions, and Responses, plus the
Anthropic Messages API and `GET /v1/models` (`docs/api.md:10-20`). Tools are
client-executed JSON functions with the full set of `tool_choice` modes,
parallel-call limits, and Lark or regex grammars for custom tools
(`docs/api.md:281-338`). Logprobs cover the full raw distribution, with up
to 20 alternatives per position for generated tokens and, when exported
with `--prompt-logprobs`, for prompt tokens (`docs/api.md:56-152`). The repository
also ships client configurations for Codex and Claude Code
(`docs/clients/`).

### Batching and speculative decoding

The scheduler picks among captured graphs for batch sizes up to the
prepared concurrency, interleaving prefill chunks with decode
(`docs/core-design.md:405-422`, `docs/scheduling-runtime-design.md`).
Speculation is a state transaction: proposals, target verification, and
the accepted-state commit use one shared contract, and DSpark verification
widths adapt to the draft's confidence and measured cost
(`docs/mtp-design.md`, `docs/model-preparation.md:240-242`,
`docs/model-preparation.md:296-297`).

### Multi-GPU and multi-node

All parallelism is tensor parallelism. One process serves every local
rank, one thread per rank (`docs/model-preparation.md:593-596`). Across
hosts, each DGX Spark runs one rank, and collectives use RDMA over RoCEv2
on ConnectX-7 ports. `flare orbit` and `flare agent` place and start the
ranks (`docs/multi-host.md:3-5`, `docs/multi-host.md:20-28`). There is no
pipeline parallelism, and a package fixes its rank count and architecture.

### Kernels

Flare has no kernels of its own beyond small Triton pieces in recipes. Its
GPU work comes from b12x, an experimental package in the FlashInfer fork
(`flashinfer/experimental/b12x/`, about 640 files), plus FlashInfer
sampling. Flare treats this coupling as a design rule and forbids
alternative backends (`AGENTS.md:195-200`). b12x kernels are written in
CuTe DSL, CUTLASS's Python kernel language. For example, the small-row
block-FP8 GEMV imports `cutlass.cute` and uses an MXFP8 `m16n8k32`
tensor-core MMA
(`flashinfer/experimental/b12x/gemm/blockscaled/_block_fp8_gemv.py:1-40`).
They aren't closed binaries. The repository and the fork are both Apache
2.0 (`LICENSE`, `NOTICE`; the fork's `LICENSE` and `NOTICE`). The DeepSeek
recipe's encoder assets carry DeepSeek's MIT license
(`ignition/recipes/deepseek41/assets/LICENSE`).

### Packages are signed, and their kernels are encrypted

Runtime packages are signed with Ed25519. Compiled kernels (cubins) and
graph records are encrypted with AES-256-GCM under a key that the user's
acceptance string derives. Flare's own docs say this authenticates the
publisher but doesn't hide the code from someone who runs it
(`docs/runtime-package-design.md:413-417`,
`docs/runtime-package-design.md:506-535`). Each load that isn't
`--offline` fetches `publisher.json` from the package's source repository
on Hugging Face (`docs/cli-design.md:714-725`), and first use needs an
interactive acceptance (`docs/signing.md:8-11`).

## Assessment for OpenAgents

### (a) As a Pylon backend

Pylon can already front any OpenAI-compatible server with
`openagents pylon serve --engine URL`, and Flare's `/v1/chat/completions`
fits that interface. In practice, it doesn't fit our V4.1 serving:

- **The V4.1 shape doesn't match.** Flare runs V4.1 only as four ranks of
  tensor parallelism inside one host or over RoCE. Our groups are pipeline
  stages on separate machines joined by ordinary TCP (`crates/stage-link`).
  Flare can't use a stage Pylon on its own, and it gives a contributor with
  one RTX card no way to join a V4.1 group.
- **Contributor hardware is mostly out of scope.** Flare has nothing for
  Macs, Linux boxes without NVIDIA cards, or SM89 cards like our RTX 4080.
- **Packaging is heavy.** Each package is built per GPU family and rank
  count by a Python, PyTorch, and FlashInfer toolchain, or downloaded as a
  signed package with encrypted kernels. Loading needs interactive
  acceptance and contacts Hugging Face unless `--offline` is set. Neither
  fits an unattended contributor Pylon well.
- **Policy.** The V4.1 plan keeps Psionic as the only engine in the serving
  path ([Psionic README](../../psionic/README.md#framework-comparisons)).
  Flare is Rust plus Python preparation; an optional external engine, like
  the llama.cpp profile, would need an owner decision.

**Inference:** the one plausible fit is an opt-in engine profile for a
contributor who owns an SM120 card and wants to serve a small model Flare
already prepares. Examples are Nemotron 3.5 Bolt, about 25 GB of device
weights (`ignition/recipes/nemotron_h/recipe.py:43-45`), or Qwen3.8 27B
NVFP4 on one GPU. That's a catalog question, not V4.1 work. We don't
recommend doing it now.

### (b) Designs worth reimplementing

The license allows reuse with attribution, but the kernels are CuTe DSL
Python, and our product code is Rust. Carry the designs over by
reimplementing them in Psionic's CUDA, and credit Flare and FlashInfer b12x
in the commit message.

1. **Engram tables on disk, not in pinned RAM.** Our stages for layers 1
   and 14 pin 101.38 GB of host memory per table. Flare reads table rows
   on demand through bounded direct-I/O staging and keeps only the scale
   planes resident (about 1.4 GiB per rank at TP4). **Inference:** this is
   the highest-value item for contributor machines, where host RAM is the
   constraint. Measure the decode-time cost of a row-cache miss before
   adopting it: today a lookup takes 6.2 µs from pinned memory (W3).
2. **Compact KV records.** Flare stores 288 bytes per compressed latent
   entry and 68 per index entry. Our W3 measurement stores 1,024 bytes of
   KV and 256 of index keys per row. **Inference:** if our W5 cache
   quantizers already round to the reference's grids, storing the packed
   bytes doesn't change our arithmetic and cuts per-position KV memory
   about 3.6 times. That helps long contexts and complete prefix
   snapshots. Confirm what the 288 bytes hold
   (`flashinfer/experimental/b12x/attention/_shared/mla/io.py`) and check
   it against the oracle before changing the format.
3. **Tensor-core paths for block-scaled weights.** Flare expands each
   32×32 UE8M0 block scale losslessly into the per-row, per-32 MXFP8 layout,
   so FP8 projections run on SM120's block-scaled MMA
   (`ignition/components/block_fp8_weights.py:32-43`). b12x's small-row
   GEMV splits K across eight warps and reduces in a fixed order, so it's
   deterministic. Our matrix-vector kernels reach about 60% of the card's
   bandwidth (1.1 TB/s FP8, 1.0 TB/s for six FP4 experts, W8).
   **Inference:** the MMA layout matters more once we batch, because
   single-row decode is limited by bandwidth. Don't copy b12x's W4A8
   activation quantization without a G1 check: it changes the arithmetic
   relative to the oracle.
4. **Planned memory and complete continuation state.** Declaring per-rank
   costs, reserving resident tables before sizing KV, and holding all
   resume state in one row match our `MeasuredMemory` placement and the
   complete prefix snapshots the
   [Inference Engineering reading](../../psionic/inference-engineering-lessons.md)
   asks for. Use Flare's continuation layout (`pool.py:28-47`) as a
   checklist of what a V4.1 snapshot must include.

We already capture each decode step as one CUDA graph (W7), so Flare's
graph capture confirms that approach rather than adding to it. DSpark's
adaptive verification is a later item, as the
[DeepInfer notes](2026-10-21-deepinfer-engine-notes.md) already say.

### (c) As a comparison for G1 v2

Flare is a second SM120 implementation of the same checkpoint revision,
and its four-GPU layout matches one `g4-standard-192` (four RTX PRO 6000s).
Its `/v1/completions` endpoint takes raw token IDs and returns prompt
logprobs with ranks, so it can score the oracle's exact prompt IDs by
teacher forcing (`docs/api.md:113-152`).

It can't be the acceptance reference:

- Its arithmetic differs on purpose: W4A8 experts with MXFP8 activations,
  FP8 KV, and tensor-parallel reductions. The eight-B200 oracle stays the
  reference.
- Logprobs stop at 20 alternatives, and G1 v2 needs full-vocabulary
  distribution error.

**Inference:** it's still a useful diagnostic. Where Psionic and Flare both
diverge from the oracle at the same position, that points to B200 arithmetic
or a genuine near-tie. Where only Psionic diverges, the bug is probably
ours. It's also the obvious throughput comparison for a single-host
four-GPU group, since Flare publishes no V4.1 numbers of its own. A run
needs owner approval, a cloud budget, Python preparation on the VM, and a
test account, because it would run third-party code on our machines.

### (d) Effect on our plans

- **No change to the V4.1 engine decision.** Keep Psionic as the only V4.1
  serving engine and the eight-B200 oracle as the G1 reference.
- **Consider tensor parallelism inside one machine.** **Inference:** in the
  `one-quad` placement, all four stages run on one four-GPU machine, so
  each token passes through the four GPUs in turn. NVIDIA chose tensor
  parallelism for the same card and model, which splits each token's work
  across all four. That could raise per-user speed above our 29.7 tokens/s
  without changing cross-machine groups. It's worth a sizing note before
  any code.
- **Add the backlog items in (b).** Disk-backed Engram tables first, then
  compact KV records, then tensor-core block-scaled kernels when batching
  starts.
- **Cross-machine pipeline groups set us apart.** Flare needs RDMA or one
  host. Pylon groups over ordinary networks, on hardware that contributors
  already own, are a different product from Flare's.
- **Watch the fork.** `local-inference-lab/flashinfer` has an active V4.1
  branch, `astra/ds41-optimizations-review-20261021`. The same group
  published the CSF checkpoint we decided not to adopt
  ([CSF notes](2026-10-21-lossless-csf-checkpoint.md)). Flare compresses
  scales from the original checkpoint at load time instead, so that
  decision still holds.
