# DeepSeek-V4-Flash just-in-time expert prefetching note

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-24

Status: source summary and infrastructure read. This is not yet an OpenAgents
benchmark, product claim, or verified serving recipe.

Sources:

- X thread: https://x.com/Ex0byt/status/2069542749460693363
- Operator-supplied thread excerpt in the 2026-06-24 Codex session.
- Operator-supplied screenshot `watttt.jpeg`, showing the first page of a
  June 2026 Tensorbend AI technical report titled "Just-in-Time Expert
  Prefetching for Frontier Mixture-of-Experts Models on Unified-Memory Edge
  Hardware".
- DeepSeek official model card:
  https://huggingface.co/deepseek-ai/DeepSeek-V4-Flash
- vLLM recipe:
  https://recipes.vllm.ai/deepseek-ai/DeepSeek-V4-Flash
- Technical report HTML:
  https://arxiv.org/html/2606.19348

## Bottom line

The thread is important because it describes a plausible third lane between
"load the whole model in GPU VRAM" and "give up on frontier MoE locally":
repack the model into a small always-resident skeleton plus expert weight
segments, keep a hot expert pool resident, and prefetch a probably sufficient
superset of experts just in time for decode. The reported result is
DeepSeek-V4-Flash at about 11 tok/s on a single DGX Spark-class unified-memory
box, using SGLang inference plus a custom fused MoE kernel.

For OpenAgents, this does not make DeepSeek-V4-Flash a turnkey single-H100
Google deployment. The local GGUF artifact is 86.72 GB decimal / 80.76 GiB,
while our live single-H100 GPT-OSS 120B host has 81,559 MiB total HBM and is
already using about 76 GB with `--max-model-len 32768`. A one-H100 all-GPU
DeepSeek-V4-Flash deployment has essentially no headroom for runtime, KV cache,
or fragmentation.

But the thread does change what is worth trying next:

1. a single or multi-G4 RTX PRO 6000 Blackwell experiment, because 96 GB VRAM
   plus large host RAM is close to the 5090 plus RAM path discussed in the
   thread;
2. a 2x/4x H100 or 4x A100 40GB vLLM/SGLang compatibility run, because enough
   total HBM removes the first memory barrier;
3. a Psionic/Hydralisk research spike on expert prefetch, because the proposed
   split between skeleton, experts, hot pool, and offload bridge maps cleanly
   to our own serving/runtime boundary.

## What the thread claims

The main post says the road to GLM-5.2 is moving through DeepSeek-V4-Flash
first. It claims a non-pruned full DeepSeek-V4-Flash run at about 11 tok/s on
one DGX Spark, using SGLang inference plus a custom mega-kernel.

In follow-up replies, the author describes the mechanism as:

- predictive MoE prefetching;
- a fused decode kernel;
- model weight repacking into "skeleton" weights versus expert weights;
- overlapping compute with expert prefetch to hide slack;
- a resident warm pool for the finite set of experts expected during prefill
  and decode;
- an offload bridge that streams FP4 experts from RAM on demand;
- sparse attention kernels for consumer Blackwell;
- FP4/FP8 GEMV inside a fused MoE megakernel, with FP4 dequantization done in
  registers.

The thread says UMA systems should benefit most because zero-copy unified memory
turns expert movement into a high-bandwidth memory hierarchy problem rather
than an explicit PCIe transfer problem. The author says the same approach
should still work on an RTX 5090 with 32 GB VRAM and 256 GB RAM, but with lower
throughput pressure because PCIe copies replace UMA access and make hot expert
caching in VRAM more important.

## What `watttt.jpeg` adds

The screenshot shows a technical report framing the method as a
"bounded-slack coverage framework" for MoE inference on unified-memory edge
hardware, with applications to GLM-5.2 and DeepSeek-V4-Flash on NVIDIA DGX
Spark (GB10).

The abstract's useful claims:

- Running DeepSeek-V4-Flash, described as 284B total / 13B active, on a single
  128 GB unified-memory device exceeds fast-memory capacity if treated as a
  naive resident model. The cold tail of experts spills to NVMe.
- Exact prediction of the next top-k routed experts is structurally
  unattainable because a layer's gate input depends on the previous layer's
  expert outputs.
- The proposed workaround is not exact prediction. It prefetches the smallest
  "probably sufficient superset" of experts.
- The report claims an exact-recovery condition governed by the top-k margin
  versus input perturbation, and a coverage result where fetching predicted
  top-(k + delta) covers true top-k when the slack delta equals the number of
  experts in a score band near the cutoff.
- It sizes the resident cache by REAP saliency and recasts miss handling as a
  distributionally robust guarantee with on-demand fallback for the tail.
- For DeepSeek-V4-Flash specifically, it claims manifold-constrained
  hyper-connections do not invalidate the prefetch model. They add a bounded
  channel-remix term, while early Hash-routed blocks are deterministic enough
  to sharpen the slack budget.

That is a useful theoretical framing: stop asking for perfect expert prediction
and instead measure the routing drift, fetch a bounded superset, and make the
miss path explicit.

## Precision and 87 GB caveat

The thread language says "none" when asked about quantization and calls the
87 GB artifact the full model. Treat that phrasing carefully.

DeepSeek's official card lists DeepSeek-V4-Flash as 284B total, 13B activated,
1M context, and "FP4 + FP8 Mixed", with MoE expert parameters in FP4 and most
other parameters in FP8. The vLLM recipe says the same. The local GGUF file in
Downloads is named with quantization markers and is 86.72 GB decimal /
80.76 GiB.

So "full model" is likely best read as "not pruned, not a small distillation,
and not reduced to a subset of experts." It should not be read as BF16/FP16
full precision. If we cite this externally, say "unpruned / full expert set
DeepSeek-V4-Flash" rather than "non-quantized" until we have the exact artifact
and packing format.

## Fit against current Google infra

Current proven OpenAgents Google lanes:

- `g2-standard-8` L4: live GPT-OSS 20B Hydralisk lane, good for small/medium
  serving and evals.
- `a3-highgpu-1g` H100 80GB Spot: live GPT-OSS 120B vLLM probe, using roughly
  76 GB HBM at 32K context.
- A100 40GB quota and A2 machine types: visible and useful for multi-GPU
  compatibility work if allocation succeeds.
- G4 RTX PRO 6000: previously allocatable, 96 GB VRAM, the closest Google lane
  to the RTX 5090 plus host-RAM offload path discussed in the thread.
- H200/B200: visible in catalog, but recent Flex-start/calendar checks showed
  real capacity is not available for immediate work.

DeepSeek-V4-Flash fit:

| Lane | Read |
| --- | --- |
| 1x L4 | No. Good for smaller models only. |
| 4x/8x L4 | Possible low-QPS llama.cpp/offload smoke only; not the first production lane. |
| 1x H100 80GB | No for all-GPU weights. The artifact is already too large for usable HBM headroom. |
| 2x/4x H100 | Plausible if vLLM/SGLang DeepSeek-V4 support works and allocation succeeds. |
| 4x A100 40GB | Plausible memory-wise, weaker for custom FP4/Blackwell paths. |
| 1x RTX PRO 6000 96GB | Best single-GPU/offload experiment if G4 allocation succeeds again. Enough raw VRAM for a tight low-context attempt, and better match for consumer-Blackwell kernel ideas. |
| 2x/4x RTX PRO 6000 | Stronger local/offload research lane; prior GLM SGLang failure means kernel support must be proven before spending serious time. |
| H200/B200 | Best clean production targets in theory, but not immediately available. |

## Why this matters for GLM-5.2

GLM-5.2 is larger than DeepSeek-V4-Flash, so the prefetch idea matters even
more. The thread's architecture gives us a concrete way to think about a GLM
lane:

1. separate skeleton from experts;
2. keep skeleton resident;
3. maintain a hot expert pool;
4. prefetch top-(k + delta), not just top-k;
5. make miss fallback explicit and measurable;
6. co-design the sparse attention backend and MoE decode kernel;
7. benchmark on accepted-outcome cost, not just tokens/sec.

This is also the first path that makes the "GLM/DeepSeek on edge-ish hardware"
story feel technically coherent instead of just a memory wish.

## Recommended next step

Create a Hydralisk/Psionic research issue for a DeepSeek-V4-Flash expert
prefetch spike:

- Target one G4 RTX PRO 6000 first if allocation succeeds, with a 32K context
  cap and explicit CPU/offload mode.
- Fall back to the live H100 only for metadata/runtime compatibility checks,
  not full all-GPU load.
- Measure whether vLLM `deepseek_v4` support can run the official checkpoint
  on multi-GPU H100/A100 without custom kernels.
- Separately prototype the Psionic representation: `skeleton`, `expert shard`,
  `hot expert pool`, `prefetch planner`, `miss fallback`, and `routing drift`
  telemetry.
- Do not claim "non-quantized" or "full precision" in public copy. Use
  "unpruned / full expert set" until the artifact format is pinned.

The most valuable output would be a small table:

| Hardware | Runtime | Precision/artifact | Context | tok/s | TTFT | Expert-cache hit rate | Miss fallback cost |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |

That table would tell us whether this is a real OpenAgents supply lane or just
a beautiful demo.
