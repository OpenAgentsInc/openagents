# GPT-OSS 20B/120B GPU fit and serving path

Date: 2026-06-23

Scope: repeat the Google GPU inventory/model-fit analysis for OpenAI's
`gpt-oss-20b` and `gpt-oss-120b` model card, cross-read with the local Inference
Engineering notes and Baseten's GLM-5.2 production writeup. This is a planning
note, not a capacity reservation or a production promise.

Requested source note: `~/Downloads/2508.10925v1.pdf` was not present on this
Mac when checked. I used the official arXiv copy instead, downloaded to
`tmp/pdfs/2508.10925v1.pdf`, rendered the first page for a sanity check, and
extracted the PDF text locally before writing this note.

## Bottom line

We can run `gpt-oss-20b` on the Google infrastructure we have access to today.
The model card reports a 12.8 GiB checkpoint and says the model can run on
systems with as little as 16 GB of memory. Our live L4 lane has 24 GB VRAM, and
the project has 16 on-demand L4 GPUs plus 16 Spot L4 GPUs in `us-central1`.
That is enough for a same-day smoke deployment and a small self-hosted lane if
the runtime image, model download, and vLLM/SGLang serving path are pinned.

We can plausibly run `gpt-oss-120b` on this Google project, but not on the GPU
that is currently live. The model card reports a 60.8 GiB checkpoint and says
MXFP4 lets the model fit on a single 80 GB GPU. OpenAI's vLLM guide says the
120B model is best with at least 60 GB VRAM and can fit on a single H100 or a
multi-GPU setup. Our current live GPU list only shows L4 instances, so 120B
requires an owner-armed create test for a high-memory accelerator: H100 80GB,
H200 141GB, B200 180GB, or RTX PRO 6000 96GB.

The practical path is:

1. Put `gpt-oss-20b` behind Khala first on L4 as the low-cost self-hosted open
   reasoning lane.
2. Run a short high-memory allocation probe for `gpt-oss-120b`, preferably H200
   or B200 for headroom, H100 as the model-card minimum lane, and RTX PRO 6000
   G4 as the Blackwell-ish single-card probe if allocatable.
3. Use vLLM first, because OpenAI publishes a gpt-oss vLLM path and calls out
   MXFP4 support requirements. Treat SGLang/TensorRT-LLM as follow-on lanes
   after the vLLM path is measured.
4. Do not expose raw chain-of-thought. The harmony format includes analysis,
   commentary, and final channels; Khala should preserve the internal semantics
   needed for tool calls while filtering/summarizing user-visible reasoning.
5. Promote neither model to a production Khala alias until the benchmark
   harness has measured TTFT, inter-token latency, throughput, total wall time,
   cost per accepted outcome, cache behavior, GPU memory, and eval results on
   realistic traffic.

## Model card facts that matter for serving

| Fact | `gpt-oss-20b` | `gpt-oss-120b` | Serving implication |
| --- | ---: | ---: | --- |
| Total parameters | 20.91B | 116.83B | These are large open-weight models, but both are MoE, not dense at full size. |
| Active parameters per token | 3.61B | 5.13B | Per-token compute is closer than the total parameter names imply. |
| Layers | 24 | 36 | 120B has a deeper runtime path and larger KV/cache footprint. |
| Experts | 32 | 128 | Top-4 expert routing per token; runtime support for MoE matters. |
| Checkpoint size | 12.8 GiB | 60.8 GiB | 20B fits L4 memory; 120B needs high-memory single GPU or tensor parallelism. |
| Quantization | MXFP4 MoE weights | MXFP4 MoE weights | Engines must understand MXFP4; generic open-model loaders may not work. |
| Context length | 131,072 tokens | 131,072 tokens | Weight fit is not enough; KV cache can dominate long-context/batch planning. |
| Minimum fit claim | As little as 16 GB memory | Single 80 GB GPU | Good match for our L4 and H100/H200/B200/G4 lanes respectively. |
| API format | Harmony / Responses-style | Harmony / Responses-style | The adapter must render system/developer/user/tool messages correctly. |
| License | Apache 2.0 release | Apache 2.0 release | Good for self-hosted product experiments, with separate usage-policy/safety review. |

The model card also matters operationally because these are reasoning models.
Reasoning effort can be configured low/medium/high and higher effort increases
chain-of-thought length, latency, and cost. The eval section shows smooth
test-time scaling: high effort can materially improve benchmark outcomes, but it
also creates a long-output serving problem. For Khala, that means the router
should expose reasoning budgets and route cheap/fast traffic to 20B or low
effort, while reserving 120B/high effort for tasks that justify the latency and
GPU cost.

## Current Google infrastructure fit

Current `gcloud` context from the inventory refresh:

- project: `openagentsgemini`
- default region: `us-central1`
- default zone: `us-central1-a`

Current live GPU instances:

| Instance | Zone | Shape | GPU | Status | Scheduling |
| --- | --- | --- | --- | --- | --- |
| `gswarm508-clean2-20260325044551-contrib` | `us-central1-b` | `g2-standard-8` | 1 x L4 | `RUNNING` | standard |
| `gswarm508-clean2-20260325044551-coord` | `us-central1-a` | `g2-standard-8` | 1 x L4 | `TERMINATED` | standard |

Current `us-central1` quota signals relevant to gpt-oss:

| GPU family | Quota signal | Current usage | `gpt-oss-20b` | `gpt-oss-120b` |
| --- | ---: | ---: | --- | --- |
| L4 24GB | 16 on-demand, 16 Spot | 1 running | Yes: smoke, low-QPS, benchmark lane. | No: not enough VRAM for the 60.8 GiB checkpoint plus KV/runtime. |
| RTX PRO 6000 96GB | 16 Spot non-VWS, 16 on-demand VWS, 16 Spot VWS | 0 live | Yes, overpowered for 20B. | Plausible single-GPU lane if G4 allocation and MXFP4 engine support are proven. |
| A100 40GB | 16 on-demand, 64 Spot | 0 | Yes, good fallback. | Not as a single GPU; possible multi-GPU/tensor-parallel probe, but not first choice. |
| A100 80GB | 0/0 in older Compute quota | 0 | Yes, but unavailable by quota. | Would fit the model-card target, but current quota says do not plan on it. |
| H100 80GB / H100 Mega | 64 Spot/preemptible | 0 | Yes, overpowered. | Intended single-GPU minimum lane; watch KV headroom, batch size, and Spot reliability. |
| H200 141GB | 64 Spot/preemptible | 0 | Yes, overpowered. | Better production canary because memory headroom helps context and batch. |
| B200 180GB | 64 Spot/preemptible | 0 | Yes, overpowered. | Best high-headroom modern lane if capacity allocates. |
| GB200 192GB | Accelerator type visible | 0 | Yes, overpowered. | Visible accelerator type, but no explicit usable quota row was found. |

This means the Google path is not blocked by model size in the same way GLM-5.2
is. The blocker is proving actual allocation and standing up the serving stack.
For 20B, allocation is already live. For 120B, quota visibility and accelerator
visibility are encouraging, but the project needs a create/delete test that
actually lands a high-memory GPU.

## Inference-engineering read

The local Inference Engineering notes make three points that change how to read
the raw GPU inventory.

First, prefill and decode have different bottlenecks. Long coding and agent
prompts are prefill-heavy, so TTFT, prefix caching, chunked prefill, and
cache-aware routing matter. Decode is often memory-bandwidth-bound, so GPU
memory size alone is not the entire story.

Second, engine support is part of capacity. OpenAI's implementation notes call
out MXFP4 as a new-ish format that existing inference code may need to adapt
for gpt-oss. They publish basic PyTorch and optimized Triton references, and the
official run guide uses vLLM. That makes vLLM the first OpenAgents path because
it minimizes custom engine risk.

Third, quality and product fit decide whether a lane is real. The Khala
benchmark and quantization notes require realistic workload shapes, executed
acceptance evals, precision/backend disclosure, and cost per accepted outcome.
For gpt-oss this is especially important because both models are quantized
out-of-the-box with MXFP4 and because reasoning effort changes both quality and
cost.

Operationally, each gpt-oss lane should record:

- model id, exact revision, engine, engine version, CUDA/driver, GPU family, and
  quantization;
- prompt tokens, cached input tokens, completion tokens, internal reasoning
  token budget if measurable, TTFT, inter-token latency, total wall time, and
  queue delay;
- GPU memory high-water mark, utilization, batch size, active requests, and
  cold/warm start state;
- evaluation verdicts for code, tool use, long-context Q&A, and safety refusal
  behavior;
- whether the response used low/medium/high reasoning effort.

## Baseten GLM-5.2 lessons applied to gpt-oss

Baseten's GLM-5.2 article is useful here mostly by contrast. GLM-5.2 is a 744B
MoE with 40B active parameters and a 1M-token context window, and Baseten's
fastest path depended on model-specific runtime support, NVFP4 quantization,
KV-aware routing, prefill/decode disaggregation, MTP/speculation, batching, and
production workload tuning.

GPT-OSS 120B is much easier to make fit:

- the checkpoint is 60.8 GiB, not hundreds of GiB;
- the card explicitly targets single-80GB-GPU serving;
- the official OpenAI serving path already points at vLLM;
- both models are released already quantized in MXFP4.

But the Baseten lesson still stands: a model that fits in VRAM is not yet a good
API. For GPT-OSS, the analogs to Baseten's load-bearing work are:

- MXFP4-aware runtime and correctness checks;
- harmony-format request rendering and safe handling of analysis/commentary/final
  channels;
- prefix/cache-friendly routing for repeated agent and codebase contexts;
- measured reasoning-effort routing rather than one default for every request;
- continuous batching and queue policy before custom disaggregation;
- eval-gated promotion so a fast lane is judged by accepted outcomes, not only
  tokens per second.

Unlike GLM-5.2, GPT-OSS does not need multi-GPU frontier infrastructure for a
first honest deployment. It does need disciplined serving and evaluation work.

## What it would entail to run this today

### Phase 0: Capacity and runtime proof

For `gpt-oss-20b`:

1. Use the existing running L4 or create a clean `g2-standard-*` L4 VM in
   `us-central1-b`.
2. Install/pin NVIDIA driver, CUDA runtime, Docker/NVIDIA container runtime, and
   a vLLM version known to support gpt-oss MXFP4.
3. Pull `openai/gpt-oss-20b` and run the official vLLM server path.
4. Send one harmony-formatted low-effort request, one medium-effort request, one
   tool-call shaped request, and one 32k+ prompt smoke if memory allows.
5. Record first-token success, memory high-water mark, TTFT, ITL/TPS, and model
   revision in a public-safe evidence note.

For `gpt-oss-120b`:

1. Try one short-lived high-memory Spot create in `us-central1`, in this order:
   H200, B200, H100, then RTX PRO 6000 G4 if available. Use the exact machine
   family/accelerator pairing that GCP accepts.
2. If only A100 40GB is allocatable, use it as a multi-GPU compatibility probe,
   not the preferred product lane. The single-GPU model-card target is 80GB+.
3. Repeat the vLLM MXFP4 load and smoke. The success criterion is not
   `nvidia-smi`; it is first token plus telemetry.
4. If H100 succeeds but memory is tight under realistic context or concurrency,
   move the production canary to H200/B200/RTX PRO 6000 for headroom.

### Phase 1: Khala integration

1. Add model catalog entries:
   - `openagents/gpt-oss-20b`
   - `openagents/gpt-oss-120b`
2. Route them through a self-hosted provider adapter that exposes OpenAI-compatible
   chat/responses semantics while using harmony under the hood.
3. Preserve internal reasoning/tool-call semantics, but return only final
   user-visible content and safe reasoning summaries where product UI needs them.
4. Add receipt fields for model revision, engine, engine version, GPU,
   quantization (`mxfp4`), reasoning effort, and warm/cold state.
5. Add feature flags so the lane can be enabled for owner/internal traffic
   before public customers.

### Phase 2: Decision-grade benchmark

Use the existing Khala benchmark harness instead of hand-judging a demo.

Minimum matrix:

- model: 20B vs 120B;
- reasoning effort: low, medium, high;
- traffic: short chat, code artifact generation, verifier run, long-context
  codebase question, tool-call loop;
- context: 4k, 32k, 128k when feasible;
- concurrency: 1, small batch, saturation point;
- hardware: L4 for 20B; H100/H200/B200/G4 for 120B depending on what allocates.

Promotion criteria:

- correctness and tool-call behavior pass executed product evals;
- latency is acceptable for the target lane;
- 120B quality/cost beats 20B for the tasks it is supposed to own;
- high reasoning effort is budgeted and not accidentally the default for cheap
  traffic;
- no raw chain-of-thought is exposed;
- safety filters cover open-weight risks and instruction-hierarchy weakness.

### Phase 3: Production shape

For `gpt-oss-20b`, production can start as horizontal L4 replicas with continuous
batching, session/cache-aware routing, and a fallback to hosted/passthrough
providers when the queue is saturated.

For `gpt-oss-120b`, production should start as a small high-memory pool rather
than a large launch:

- one warm H200/B200/RTX PRO 6000 or H100 canary;
- autoscaling keyed to queued tokens, active sequences, GPU memory, and TTFT,
  not raw request count;
- route only expensive reasoning/code workloads to 120B;
- send cheap/latency-sensitive requests to 20B or provider passthrough;
- use on-demand/reserved capacity for public production if Spot interruption
  risk is unacceptable.

Prefill/decode disaggregation should not be on the first critical path. It is a
later optimization trigger if real traffic shows high-volume long-context
prefill pressure. That is exactly the line the Inference Engineering notes draw,
and Baseten's article supports it: disaggregation is valuable when the workload
shape justifies the operational complexity.

## Go/no-go

| Question | Answer |
| --- | --- |
| Can we run `gpt-oss-20b` today? | Yes. The live L4 lane should be enough for smoke and low-QPS service after vLLM/runtime setup. |
| Can we run `gpt-oss-120b` today on a live VM? | No. The only live GPU is L4, which is too small. |
| Can we run `gpt-oss-120b` today if GCP allocates our visible quota? | Probably yes. H100 80GB is the model-card target; H200/B200/RTX PRO 6000 give better headroom. |
| Is 120B production-ready for OpenAgents today? | Not yet. We still need allocation proof, runtime pinning, harmony adapter work, evals, safety policy, and telemetry. |
| Is this easier than self-hosted GLM-5.2? | Yes, by a lot. GPT-OSS 120B is a single-high-memory-GPU target; GLM-5.2 is a multi-GPU frontier-serving research lane for us. |

## Recommended immediate commands/work items

1. Cleanly allocate or reuse an L4 VM and run the official vLLM `gpt-oss-20b`
   smoke.
2. Owner-arm a one-hour high-memory create/delete probe: H200 first, B200 second,
   H100 third, RTX PRO 6000 G4 fourth.
3. Save a public-safe evidence note with hardware, driver, engine, revision,
   quantization, memory high-water mark, TTFT, ITL/TPS, and the exact request
   class.
4. Add a Khala model-router entry only after the adapter can render harmony and
   suppress raw CoT.
5. Run the benchmark harness over realistic OpenAgents traffic before calling
   either lane production.

## Sources read

Local:

- [`2026-06-23-gcloud-gpu-quota-inventory.md`](./2026-06-23-gcloud-gpu-quota-inventory.md)
- [`2026-06-23-gpu-inventory-model-fit-and-glm-52.md`](./2026-06-23-gpu-inventory-model-fit-and-glm-52.md)
- [`inference-engineering-book/book-reading-notes.md`](./inference-engineering-book/book-reading-notes.md)
- [`2026-06-23-khala-benchmark-harness-book-p1-5.md`](./2026-06-23-khala-benchmark-harness-book-p1-5.md)
- [`2026-06-23-khala-quantization-eval-gate-book-p1-7.md`](./2026-06-23-khala-quantization-eval-gate-book-p1-7.md)

External:

- [arXiv: gpt-oss-120b & gpt-oss-20b Model Card](https://arxiv.org/abs/2508.10925)
- [arXiv PDF: 2508.10925](https://arxiv.org/pdf/2508.10925)
- [OpenAI cookbook: How to run gpt-oss with vLLM](https://developers.openai.com/cookbook/articles/gpt-oss/run-vllm)
- [OpenAI cookbook: Verifying gpt-oss implementations](https://developers.openai.com/cookbook/articles/gpt-oss/verifying-implementations)
- [Baseten: How we built the world's fastest API for GLM-5.2](https://www.baseten.co/blog/how-we-built-the-worlds-fastest-api-for-glm-52/)
