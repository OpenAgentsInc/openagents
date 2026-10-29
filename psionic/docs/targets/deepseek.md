# DeepSeek natively: what the pre-reset work teaches

Planning note, 2026-10-29, for Part IX of the
[book plan](../../book/PLAN.md) and chapter 13's qualification rules.
It condenses the pre-reset DeepSeek V4.1 Flash work, which ran from about
October 12 to 23, 2026, in the old Rust Psionic. Read the sources with
`git show 0b401561a0^:<path>`:

- `docs/psionic/deepseek-v41-native.md`: the architecture, phases and measurements.
- `docs/psionic/deepseek-v41-completion.md`: gates G1–G8 and their evidence.
- `docs/psionic/v41-single-box.md`: one GPU with an expert cache.
- `docs/psionic/deepseek-support-plan.md`: the model ladder and the DS0–DS6 queue.
- `docs/psionic/v41-metal-plan.md`: Apple GPU stages.
- `docs/psionic/deepinfer-lessons.md`: DeepSeek's released engine.
- The research notes in `docs/research/deepseek/`: DeepSeek's serving article, Project Maya, NVIDIA FLARE and the CSF checkpoint.
- The two audits, `docs/audits/2026-10-21-deepseek-six-hour-review.md` and `docs/audits/2026-10-22-deepseek-closed-issues.md`.

None of that code exists any more, and none of it is restored. The numbers
below are historical measurements of a Rust/CUDA implementation, not
Psionic claims.

## The model

`deepseek-ai/DeepSeek-V4.1-Flash-1017` at `1bae0e10…`: 510.3 GB in 48
shards. It has 40 decoder layers of one block type, plus 3 draft layers.
Width is 5,120 and the vocabulary is 129,280.

- **Attention.** There are 64 query heads and one shared 512-wide KV head
  (64 rotary dimensions), with a sink in the softmax. Layers 0–1 use only a
  128-token window. Layers 2–19 add a gated pooling compressor (ratio 2), and
  layers 20–39 an ungated one (ratio 1).
- **Cross-layer sharing.** KV is reused from layers 2, 8, 14 and 20, and the
  indexer from layers 2, 8, 14, 20, 24, 28, 32 and 36. Layer 20 chooses the
  top 2,048 blocks of 8 tokens, and layers 21–39 read them. Indexer top-k is
  512.
- **Hyper-connections.** Four residual streams of 5,120 values. Each
  sub-block has 24 coefficients, including a 4×4 mix normalized by 20
  Sinkhorn iterations.
- **Experts.** 384 routed experts (6 active) plus one shared expert, with
  square-root-softplus scores. A bias is used for selection only, and the
  route scale is 1.5. SwiGLU is clamped at 10.
- **Engram.** Hashed n-gram tables at layers 1 and 14, over a 99,092-id
  compressed vocabulary. Each table is about 101 GB. A decode token reads 24
  rows (6,336 bytes). The bucket multipliers come from NumPy's random number
  generator and must be reproduced exactly.
- **Weights.** Dense weights are FP8 E4M3 with UE8M0 scales per 32×32 block.
  Experts are FP4 E2M1 with a UE8M0 scale per 32 along K. This is not MXFP4:
  the nibble order differs and a 0xFF scale is NaN.
- **Memory.** Experts take 288.8 GB, Engram 202.8 GB and everything else
  18.7 GB. About 13 GB of weights are read per token.

The rest of the ladder comes from the support plan:

- R1-Distill-Qwen-1.5B: a dense Qwen2 model.
- V2-Lite: latent attention, 64 routed and 2 shared experts, 16B total and
  2.4B active.
- V3/R1: 671B.
- V3.2: adds sparse indexing.

## First rebuilt rung: R1 distill

[Chapter 56](../../book/ch056-execute-a-deepseek-r1-distill.md) now provides
`deepseek-r1-qwen2-cpu-wide-v1`: the original BF16
R1-Distill-Qwen-1.5B at `ad9f0ae0864d7fbcd1cd905e3c6c5b069cc8b562`,
with complete file authentication, all 28 Qwen2 layers, its own tokenizer,
a bounded text-history recipe and greedy generation. The tokenizer’s BOS
151646 takes precedence over the config’s conflicting 151643. Reasoning
and final intervals stay separate.

The retained six-token “Hi” prompt matches an owned independent full-prefix
CPU reference on every logit and layer residual, and its greedy greeting
finishes with EOS. This is one numerical reference request, not a quality
corpus, upstream BF16 runtime parity, serving, GPU throughput or V2/V4
support. The [verification record](../../../docs/verification/psionic-ch56-2026-10-29.md)
binds the actual executed builds and later original-shaped slices.
Routine checks use small owned models and recorded arrays; they do not
repeat full original-model CPU execution. [Chapter 57](../../book/ch057-deepseeks-front-door.md) adds a bounded V4.1-style
typed recipe and chunk-safe DSML protocol parser, with a separately named
R1 tokenizer adapter. Original V4.1 tokenizer/model qualification remains
separate; chapter 58 is the next execution rung.

## What the old work achieved

- **Oracle capture.** One run on eight rented B200s cost $36.85. It produced
  18,177 dumped tensors; a recapture produced 554,839 and showed same-mode
  repeatability was exact.
- **Loading.** All 96,085 tensors loaded and all 48 shard hashes matched;
  19 of 19 dequantization cases passed.
- **Kernels.** Per-kernel parity passed on several GPU generations, within
  stated summation bounds.
- **Metal.** A tail of layers on Metal matched CUDA bit for bit: 51 of 51
  greedy steps and 19 of 19 seeded steps.
- **Four stages on one machine.** About 30 tokens/s decode at 33.2 ms per
  token. Prefill ran about 120 tokens/s per serial stage. Peak memory was
  about 77 GB per GPU.
- **Across machines.** Four stages across machines ran at 29.7 tokens/s, of
  which links took 0.83–0.87 ms per token. Sending F32 residuals cost
  261 KB/token; BF16 would have halved that.
- **One GPU with an expert cache.** 24.3 tokens/s with a routing profile at
  an 89% hit rate. Layer-major prefill reached 541 tokens/s on a stage, 4.45
  times faster.
- **Packed KV.** FP8 window and FP4 compressed and index keys gave
  bit-identical logits.
- **Recovery.** Kill, freeze, fencing and machine-loss tests passed.

## What failed, and why it matters

- **Numerical parity never passed (gate G1).** The oracle's arithmetic
  depended on the GPUs it ran on and on its tensor-parallel layout. It
  rounded each rank's BF16 partial sums, its row reductions followed a layout
  that depends on batch shape, and its hardware exponential and FP8
  multiply-accumulate units rounded their own way. Emulating that got layer 0
  bit-exact. A one-ulp difference in the hyper-connection mix then flipped
  near-tie expert choices from layer 1 on, and the error grew with depth and
  context. The rules in chapter 13 follow from this: compare like modes only;
  compare distributions with frozen policies; measure on the device that ran;
  record near-tie regret for routers.
- **Policies were changed after the data was read.** One was frozen with
  defects. Its budgets, derived from short prompts, failed at long context.
  A router check measured an F64 stand-in instead of the GPU's values.
- **Evidence was weak in places.**
  - A command printed `passed: true` and exited 0 on a failed report.
  - Forged result lines passed a checker.
  - A missing profile defaulted to "qualified".
  - A rebuild ignored the architecture setting.
  - Launch never checked binary hashes, and one arithmetic flag was silently
    ignored.
  - A receipt overstated a prompt's token count.
- **Spend control failed.** A lost create reply left a billed machine
  untracked, a retry exceeded the spend cap, and billing counted queue time.
- **Resource limits were wrong.** Step buffers sized by context, not chunk,
  hit a codec limit. A parser re-decoded the whole answer every token, which
  was quadratic.
- **Shortcuts failed silently.** A lossy 2–3-bit profile produced unusable
  text. DeepSeek's own draft layers made single-request decode slower
  (1.17 versus 3.95 tokens/s).

## Efficiency techniques to rebuild in Coax

| Technique | Source | Chapter |
| --- | --- | --- |
| Expert cache with decayed-frequency heat, routing-profile seeding, host lane for misses, next-layer gate prefetch | Project Maya (MIT, ideas reimplemented); old single-box work | 64 |
| Layer-major chunked prefill, then grouped block-scaled products | Maya; old W17 work | 65 |
| FP8/FP4 packed KV where the reference already rounds | Maya; NVIDIA FLARE's compact records (528/288/68 bytes per entry) | 66 |
| One recorded device sequence per decode step; device-side routing ids and sampling; host one step ahead | Old CUDA-graph stages; DeepSeek's asynchronous scheduler (after SGLang and NanoFlow) | 67 |
| Engram served from mapped files or disk by row | FLARE's direct-I/O Engram; old host-mapped gather (3.2 µs) | 62 |
| Fixed then variable-length draft verification from a confidence head and a cost curve | DeepSeek's DSpark (+6% at its batch sizes) | 69 |
| Prefix cache of 128-token pages via content-hashed chains, storage independent of compute, per-traffic retention | DeepSeek 3FS cache (96% hit rate on 77K-token average inputs) | 119 |
| Prefill cost model, length buckets, pull scheduling; decode balancing on requests plus cache length | DeepSeek's load balancers | 120 |
| Small prefill groups, large decode groups, separate vision role; scale on unserved tokens and requests in flight | DeepSeek's serving layout (prefill:decode near 1:1) | 125 |
| Expert parallelism only inside a fast domain; data-parallel attention; redundant experts rebalanced from a load window | DeepSeek; busiest/mean 1.03–1.09 reported | 126 |
| Special tokens by boundary, chunk-safe streaming parser | DeepSeek's recipe | 57 |

DeepSeek reported 60K uncached prefill tokens/s and 14K decode tokens/s per
card, with users seeing 230 tokens/s. These show what batching at scale is
worth. They are not targets for a first single-request implementation.

## What we do not adopt

- **DeepSeek's code and libraries as dependencies.** That covers its engine
  (DeepInfer), kernels (FlashMLA, DeepGEMM, TileLang) and communication
  library (DeepEP). They are references only.
- **Bounded sliding-window replay as exact.** DeepSeek's own description
  makes it approximate.
- **The compressed-scale CSF checkpoint.** Its licence forbids reuploading.
- **DFlash2.** Its licence (CC BY-NC-ND) rules it out.
- **The redistribution licence of the V4.1 checkpoint itself.** This was
  left open at the reset. It is an owner question to settle before
  distributing weights or stage packages.
