# GPU inventory model fit and GLM-5.2 feasibility

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-23

Scope: cross-reference the Google Cloud GPU quota inventory with the local
Inference Engineering book notes and Baseten's GLM-5.2 production writeup. This
is a planning note, not a capacity reservation or a product promise.

## Bottom line

We can run real model-serving work on the current Google Cloud project, but the
answer changes by model class.

The current `openagentsgemini` quota is good enough for small and medium
inference, owner-armed benchmark sweeps, quantization experiments, and the first
Pylon/Khala self-hosted lanes. It is not enough, by itself, to claim that
OpenAgents can run frontier-class GLM-5.2 production traffic at Baseten-style
performance.

GLM-5.2 is possible in principle on infrastructure we can see and request:
`us-central1` exposes B200, H200, H100, G4 RTX PRO 6000, L4, and A100 families,
and Cloud Quotas reports Spot quota for H100/H200/B200 plus RTX PRO 6000. But it
is not production-ready for us today. We have not proven allocation for the
multi-GPU H100/H200/B200/G4 shapes, we do not have a GLM-5.2 engine/eval lane
wired into Khala, and we do not yet run the KV-aware routing, quantization gate,
speculation telemetry, and prefill/decode disaggregation that Baseten says were
load-bearing for their fastest GLM-5.2 API.

The practical recommendation is:

1. Use a hosted GLM-5.2 API lane now for product experiments.
2. Use our L4/G4/A100/H100/H200/B200 access for measured self-hosted serving
   probes, starting below GLM-5.2 scale.
3. Treat self-hosted GLM-5.2 as a research/capacity lane until we prove exact
   multi-GPU allocation, engine support, quantized quality, and long-context
   routing with real Khala traffic.

## Inventory refresh

This note refreshes the GPU signal after the quota inventory document.

Current `gcloud` context:

- project: `openagentsgemini`
- default region: `us-central1`
- default zone: `us-central1-a`

Current live GPU instances from
`gcloud compute instances list --filter='guestAccelerators:*'`:

| Instance | Zone | Shape | GPU | Status | Scheduling |
| --- | --- | --- | --- | --- | --- |
| `gswarm508-clean2-20260325044551-contrib` | `us-central1-b` | `g2-standard-8` | 1 x L4 | `RUNNING` | standard |
| `gswarm508-clean2-20260325044551-coord` | `us-central1-a` | `g2-standard-8` | 1 x L4 | `TERMINATED` | standard |

The earlier G4 probe from
[`2026-06-23-gcloud-gpu-quota-inventory.md`](./2026-06-23-gcloud-gpu-quota-inventory.md)
no longer appears in the live instance list and no instance named
`oa-confidential-g4-probe-*` is currently visible. Treat that probe as evidence
that a Spot G4 RTX PRO 6000 shape was allocatable earlier on 2026-06-23, not as
current live capacity.

Current `us-central1` quota signals:

| GPU family | Current quota signal | Current usage | Planning read |
| --- | ---: | ---: | --- |
| L4 | 16 on-demand, 16 Spot | 1 on-demand L4 running | Best cheap always-on lane for small/medium models and batch work. |
| RTX PRO 6000 | 16 Spot non-VWS, 16 on-demand VWS, 16 Spot VWS | 0 current live | Strong single-node Blackwell lane if exact G4 shape can be allocated again. NVIDIA lists RTX PRO 6000 Blackwell Server Edition with 96 GB GPU memory. |
| A100 40GB | 16 on-demand, 64 Spot | 0 | Useful fallback for engines/models that do not need Hopper/Blackwell features. |
| A100 80GB | older Compute quota remains 0/0 | 0 | Do not plan on this lane without quota change or create proof. |
| H100 80GB / H100 Mega | 64 Spot/preemptible | 0 | Plausible large-model probe lane; requires create proof and topology detail. |
| H200 141GB | 64 Spot/preemptible | 0 | Better high-memory lane; visible only in `us-central1-b`; requires create proof. |
| B200 180GB | 64 Spot/preemptible | 0 | Most relevant self-hosted GLM-5.2 lane; visible in `us-central1-b`; requires create proof. |
| GB200 192GB | accelerator type visible in `us-central1-a/b` | 0 | Visible accelerator type, but no explicit usable quota row found in the refresh. |

Visible accelerator types in `us-central1` now include:

- `nvidia-l4` in `us-central1-a/b/c`
- `nvidia-rtx-pro-6000` in `us-central1-b/f`
- `nvidia-h100-80gb` and `nvidia-h100-mega-80gb` in `us-central1-a/b/c`
- `nvidia-h200-141gb` in `us-central1-b`
- `nvidia-b200` in `us-central1-b`
- `nvidia-gb200` in `us-central1-a/b`
- `nvidia-tesla-a100` in `us-central1-a/b/c/f`
- `nvidia-a100-80gb` in `us-central1-a/c`, but without useful quota

## What the inference engineering notes imply

The local notes in
[`inference-engineering-book/book-reading-notes.md`](./inference-engineering-book/book-reading-notes.md)
make the inventory more useful because they separate model fit from raw GPU
count:

- Model serving is a product-specific tradeoff. We should judge lanes by TTFT,
  inter-token latency, total wall time, throughput, cost, and accepted outcome,
  not a single tokens-per-second number.
- Long coding and agent workloads are often prefill-heavy. Prefix caching,
  chunked prefill, and cache-aware routing can matter more than single-request
  decode speed.
- Decode is memory-bandwidth-bound. Bigger memory and better bandwidth matter
  for long generations, not only tensor-core peak FLOPs.
- Hardware is not fungible. Memory size, memory bandwidth, interconnect,
  CPU-GPU transfer, cold-start loading, and engine support decide whether a
  model is practical.
- Proven engines should come first: vLLM, SGLang, and TensorRT-LLM before custom
  serving infrastructure, unless Psionic explicitly owns the research boundary.
- Quantization must pass executed Khala evals before it can share a public model
  identity. A faster lower-precision model is a product win only if
  cost-per-accepted-outcome improves.
- Prefill/decode disaggregation is powerful but should be gated by measured
  high-volume, long-context traffic. GLM-5.2 is the kind of model that can
  justify it, but that does not mean Khala MVP should depend on it.

That turns the quota inventory into this model-fit table:

| Model / workload class | Fit on our current access | Notes |
| --- | --- | --- |
| Embeddings, classifiers, rerankers, small utility models | Strong | L4 is enough for many always-on or batch lanes. Prefer async/batch economics where possible. |
| 7B-14B chat/code models | Strong | L4 works; G4/A100/H100 are faster or allow larger batch/context. Good Pylon whole-model starting point. |
| 30B-40B dense or active-MoE models | Good with right quantization/context | L4 can be tight; G4 RTX PRO 6000, A100, H100, H200, and B200 are better. Receipts must disclose precision and engine. |
| 70B-class models | Plausible, not automatic | Single RTX PRO 6000 96GB or H100/H200/B200 can be plausible with quantization and context limits. Long context and high throughput may require multi-GPU. |
| 100B-200B-class models | Research/probe lane | Multi-GPU or high-memory H200/B200/G4 shapes become important. Spot quota exists for modern cards, but allocation is not proven. |
| 400B+ MoE models | Capacity-gated | Active parameters may be modest, but resident expert weights, routing, KV cache, and throughput usually require multi-GPU topology and mature engine support. |
| 744B GLM-5.2 | Possible in principle, not proven for production | Requires multi-GPU high-memory hardware, model-specific engine support, quantization evals, KV-aware routing, and likely disaggregation for strong latency/throughput. |
| Image, ASR, TTS, video | Separate lane design | The book notes modalities have different batching and latency shapes; do not inherit chat metrics blindly. |

## Baseten GLM-5.2 read

Baseten's GLM-5.2 article is a production-inference case study, not just a
hardware announcement. It says their 280+ TPS result on NVIDIA Blackwell came
from a stack of optimizations:

- custom runtime support for GLM-5.2's shared DSA architecture;
- in-house NVFP4 quantization from FP8 weights, calibrated against agentic
  benchmarks;
- KV-aware routing with NVIDIA Dynamo;
- prefill/decode disaggregation with NVIDIA Dynamo, which Baseten says doubled
  TPS for observed workload shapes;
- GLM-5.2 MTP support for speculative decoding;
- production tuning for cache hit rate, prefill/decode ratios, parallelism, and
  batching.

The same article identifies GLM-5.2 as a 744B-parameter MoE model with 40B active
parameters and a 1M-token context window. The public Hugging Face model card also
emphasizes the 1M-token context, IndexShare sparse-attention improvement,
improved MTP layer, MIT license, and lists local deployment support for SGLang
and vLLM versions. That is encouraging, but it is not equivalent to Baseten's
Blackwell/NVFP4/Dynamo production stack.

For OpenAgents, the main inference-engineering read is:

- GLM-5.2's active parameter count makes per-token compute less terrifying than
  a dense 744B model, but the serving system still needs to hold or manage the
  full expert weight set, KV cache, routing state, and runtime overhead.
- Weight memory alone is already large. FP8 weight storage is roughly 744 GB
  before overhead. NVFP4 weight storage is roughly 372 GB before scale metadata,
  runtime memory, activations, KV cache, and fragmentation. A single L4, A100,
  H100, H200, B200, or RTX PRO 6000 cannot honestly be treated as enough.
- Multi-GPU is therefore table stakes. Four high-memory B200/H200/RTX PRO 6000
  GPUs may clear rough quantized weight storage, but production headroom,
  long-context KV, and throughput likely push the design toward larger
  multi-GPU nodes or multiple nodes. Baseten does not publish their exact GPU
  count in the article, so we should not invent one.
- Long context makes prefix caching and KV-aware routing first-class. Without
  cache-aware routing, repeated agent/codebase prompts keep paying prefill cost.
- Baseten's result is specifically Blackwell-optimized. Our B200 and RTX PRO
  6000 quota lanes are therefore more relevant than L4/A100 for reproducing the
  NVFP4 part of the stack.

## Can we run GLM-5.2?

### Hosted/API lane

Yes. The fastest honest path is to use a hosted GLM-5.2 API lane through Khala
while we collect real OpenAgents traffic shapes. Baseten's article says GLM-5.2
is available through its model API and as a dedicated deployment for high-volume
workloads. Fireworks/Together-style open-model provider lanes already fit the
inference gateway strategy in this folder. This gives us product signal without
pretending we have self-hosted frontier serving solved.

### Single-GPU self-host lane

No, not for full GLM-5.2 in a credible way.

The live L4 is useful for small/medium serving, but not full GLM-5.2. A single
RTX PRO 6000 96GB, H100 80GB, H200 141GB, or B200 180GB is still below the rough
weight memory required for GLM-5.2 even in 4-bit form once overhead and KV cache
are included.

Single-GPU work should target smaller models, distilled models, or aggressive
local smoke tests of engine compatibility. It should not be marketed internally
as "we can self-host GLM-5.2."

### Multi-GPU self-host lane

Possible in principle, unproven in this project.

The project can see and request modern GPU quota in `us-central1`, including
Spot H100/H200/B200 and RTX PRO 6000. That means the next honest self-hosted
step is an owner-approved create/delete or short benchmark on exact shapes:

- 8 x B200 if available, because Blackwell/NVFP4 is the most relevant lane to
  Baseten's result;
- 8 x H200 or H100 as a fallback for FP8/vLLM/SGLang compatibility;
- 4 x / 8 x RTX PRO 6000 G4 as a lower-cost Blackwell probe, while watching PCIe
  topology and P2P limits.

The probe should not stop at `nvidia-smi`. It needs to record driver/runtime,
engine, model load success, quantization, effective context, TTFT, inter-token
latency, total TPS, cache behavior, and a small executed Khala eval result.

### Production self-host lane

Not yet.

To call GLM-5.2 production-ready on our infrastructure, we would need:

1. Proven allocation for the exact multi-GPU H100/H200/B200/G4 machine family.
2. A pinned serving engine that loads GLM-5.2 and supports the chosen precision.
3. A quantization eval gate comparing original and reduced precision on executed
   Khala workloads.
4. Prefix-cache/session-affinity telemetry and cache-aware routing.
5. Speculation/MTP disclosure and acceptance telemetry.
6. Queue, cold-start, TTFT, ITL/TPS, total wall-clock, cost, and verification
   receipts.
7. A disaggregation/Dynamo design only after real traffic proves the trigger,
   or a separate explicit GLM-5.2 research deployment where disaggregation is the
   experiment.

## Recommended next probes

1. **Run hosted GLM-5.2 through Khala first.** Use it for code/agent evals,
   record prompt lengths, cacheable prefixes, output lengths, and accepted
   outcomes.
2. **Use L4 for cheap always-on serving and harness work.** It is live and
   on-demand, so it is the best low-friction place to exercise vLLM/SGLang and
   telemetry with smaller models.
3. **Re-create a G4 RTX PRO 6000 probe deliberately.** The earlier probe proved
   allocatability, but the current live list no longer includes it. Capture
   driver, VRAM, P2P topology for 1/2/4/8 GPU shapes, and a small model-serving
   benchmark.
4. **Owner-arm one high-end create test.** Try B200 in `us-central1-b` first,
   then H200/H100 fallback. Record whether quota translates into capacity.
5. **Test engine compatibility before scale.** Start with SGLang/vLLM model-card
   paths for GLM-5.2 FP8, then decide whether TensorRT-LLM or Dynamo work is
   justified.
6. **Do not skip the quantization gate.** Baseten's NVFP4 result depends on
   quality-preserving quantization. For Khala, a GLM-5.2 NVFP4 lane must disclose
   precision and pass executed product evals before sharing an unqualified model
   identity.
7. **Use disaggregation as a GLM-5.2-specific research trigger, not Khala MVP
   plumbing.** The local Dynamo study says to wait for measured high-volume
   long-context traffic. GLM-5.2 could become that trigger, but we should name it
   as a research lane until the measurements exist.

## Sources read

Local:

- [`2026-06-23-gcloud-gpu-quota-inventory.md`](./2026-06-23-gcloud-gpu-quota-inventory.md)
- [`inference-engineering-book/README.md`](./inference-engineering-book/README.md)
- [`inference-engineering-book/book-reading-notes.md`](./inference-engineering-book/book-reading-notes.md)
- [`inference-engineering-book/khala-investigation-notes.md`](./inference-engineering-book/khala-investigation-notes.md)
- [`2026-06-23-khala-benchmark-harness-book-p1-5.md`](./2026-06-23-khala-benchmark-harness-book-p1-5.md)
- [`2026-06-23-khala-quantization-eval-gate-book-p1-7.md`](./2026-06-23-khala-quantization-eval-gate-book-p1-7.md)
- [`2026-06-23-khala-speculation-telemetry-book-p1-8.md`](./2026-06-23-khala-speculation-telemetry-book-p1-8.md)
- [`2026-06-23-khala-disaggregation-dynamo-study.md`](./2026-06-23-khala-disaggregation-dynamo-study.md)

External:

- [Baseten: How we built the world's fastest API for GLM-5.2](https://www.baseten.co/blog/how-we-built-the-worlds-fastest-api-for-glm-52/)
- [Hugging Face: zai-org/GLM-5.2-FP8](https://huggingface.co/zai-org/GLM-5.2-FP8)
- [NVIDIA RTX PRO 6000 Blackwell Server Edition](https://www.nvidia.com/en-us/data-center/rtx-pro-6000-blackwell-server-edition/)
