# DeepSeek arc completion ledger

The active objective is to complete every DeepSeek chapter, including the
cross-stage chapter 141. A runnable component does not prove whole-model
execution, numerical qualification, device performance or recovery. Planned
rows stay planned until their full acceptance gates have evidence.

| Chapter | Required evidence | Current state |
| --- | --- | --- |
| 56 | Original R1 distill, original tokenizer/BOS, history stripping, separate reasoning/final, complete generation | Implemented CPU reference; [record](psionic-ch56-2026-10-29.md). Reassess original scope in the final audit. |
| 57 | Typed recipe boundaries, independently encoded text runs, linear chunk-safe UTF-8/delimiters, incremental tool events, missing-finish refusal | Implemented bounded protocol and R1 adapter; [record](psionic-ch57-2026-10-29.md). Original V4.1 tokenizer/full recipe remain unqualified. |
| 58 | Complete original V2-Lite, latent/expanded attention parity, 64 routed/2 shared experts, combine/router/cache continuation | Original shards verified; compressed/expanded MLA and original-parameter YaRN implemented with [component evidence](psionic-deepseek-v2-math-2026-10-29.md). Complete decoder, FFNs, admission and original execution still pending. |
| 59 | V3 selection-only bias, V4.1 sqrt-softplus/scale, deterministic ties, router margins in comparisons | Implemented CPU-wide routing and margin comparisons; [record](psionic-ch59-2026-10-29.md). |
| 60 | Window/compressed attention, both compressors, sink, source-layer sharing, candidate reuse, chunk/whole state parity | Planned. |
| 61 | Four residual streams, pre/post and 20-iteration Sinkhorn mixing, previous-mix collapse across boundaries | Planned. |
| 62 | Exact normalized/compressed n-gram history, reference prime layouts/multipliers, mapped/disk lookup without eager tables | Planned. |
| 63 | All FP4 E2M1/UE8M0 codes/scales, layout, NaN rejection, clamped SwiGLU and pre-down route weights | Planned. |
| 64 | Exact cache residency/heat/swap, host/mapped miss lane, between-step copies and next-layer prefetch | Planned. |
| 65 | Layer-major expert reuse, grouped block-scaled products, time-budget chunks and no silent disablement | Planned. |
| 66 | Exact FP8/FP4 cache grids, reference bit checks, separately bounded chunk step buffers | Planned. |
| 67 | Recorded device decode, device expert IDs/sampling, one-step-ahead host scheduling and lifetime checks | Planned. |
| 68 | Complete original V4.1 Flash on one machine, frozen reference qualification, TTFT/tokens/s/memory | Planned; current host has 16 GiB GPU memory and about 189 GiB free disk at initial inspection, less than the 510.3 GB original artifact set. |
| 69 | Original draft heads, fixed then confidence-driven variable verification, exact acceptance, measured committed tokens/time | Planned. |
| 141 | Full stage-boundary state in BF16, fencing/recovery, every split matches chapter 68 and measured per-hop latency | Planned. |

Chapter order may change to build prerequisites. The ledger does not replace
the [book plan](../../psionic/book/PLAN.md) or permit weaker completion gates.
