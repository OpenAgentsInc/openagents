# Baseten GLM-5.2 production inference notes

Date: 2026-06-24

Source:
[Baseten, "How we built the world's fastest API for GLM-5.2"](https://www.baseten.co/blog/how-we-built-the-worlds-fastest-api-for-glm-52/#prefill-decode-disaggregation-with-nvidia-dynamo),
last updated 2026-06-23.

Copyright note: this is an OpenAgents engineering summary and implementation
translation, not a verbatim mirror of the article.

## Why this matters

The Baseten article is useful because it explains GLM-5.2 serving as an
end-to-end production system, not as a single-GPU benchmark. The reported
280-plus token-per-second result is tied to model-specific runtime support,
Blackwell-oriented quantization, KV-aware routing, prefill/decode
disaggregation, speculation, and production workload tuning.

For OpenAgents, that means "get GLM-5.2 running" should not start as a public
Khala model claim. The next honest step is a Hydralisk model-profile and
admission preflight that proves we can allocate the right hardware, pin the
right engine, load the model in the right precision, and emit public-safe
capability/receipt evidence.

## Article facts to preserve

Baseten identifies GLM-5.2 as a frontier open model with these serving-relevant
traits:

- 744B total parameters, mixture-of-experts architecture, and 40B active
  parameters.
- Up to a 1M-token context window.
- Thinking and non-thinking modes.
- Shared DSA weights, which required runtime support.
- Multi-Token Prediction heads for speculative decoding.
- MIT license.

The performance stack in the article has six load-bearing pieces:

1. Model-specific runtime support for GLM-5.2 shared DSA.
2. NVFP4 weights on NVIDIA Blackwell, quantized from the original FP8 weights
   with NVIDIA ModelOpt.
3. Agentic benchmark calibration for quantized quality, including BFCL-style
   function-calling checks.
4. KV-aware routing with NVIDIA Dynamo to increase prefix/KV reuse across
   replicas.
5. Prefill/decode disaggregation with NVIDIA Dynamo.
6. Multi-Token Prediction speculation with acceptance-rate tuning.

The prefill/decode section is especially relevant for Hydralisk. The article
separates inference into:

- prefill: compute-bound processing of input tokens and KV construction;
- decode: memory-bandwidth-bound generation of subsequent output tokens.

Baseten's production read is that separate prefill and decode engines can avoid
resource contention and let operators allocate different GPU/engine mixes to
each phase. Their Dynamo-based implementation includes:

- a queue for saturated prefill engines;
- conditional disaggregation thresholds based on post-prefix-cache input length
  and prefill queue pressure;
- NIXL-based KV transfer between prefill and decode engines;
- KV-layout transposition when prefill and decode engines use different tensor
  parallel configurations.

Baseten reports that disaggregation doubled tokens per second for observed
GLM-5.2 workload shapes. Treat that as a strong design signal, not a directly
portable performance claim: the article does not publish the exact GPU count,
node topology, traffic mix, or engine patch set.

## Translation to OpenAgents

The article reinforces the existing OpenAgents inference docs:

- [`2026-06-23-gpu-inventory-model-fit-and-glm-52.md`](./2026-06-23-gpu-inventory-model-fit-and-glm-52.md)
  says our visible Google GPU quota makes GLM-5.2 possible in principle, but
  not production-proven.
- [`2026-06-23-hydralisk-python-nvidia-inference-stack.md`](./2026-06-23-hydralisk-python-nvidia-inference-stack.md)
  already names SGLang as the preferred Hydralisk starting point for
  GLM/MoE/long-context work, with Dynamo as the measured promotion path.
- [`2026-06-23-khala-disaggregation-dynamo-study.md`](./2026-06-23-khala-disaggregation-dynamo-study.md)
  treats disaggregation as powerful but not an MVP dependency.
- [`2026-06-23-khala-quantization-eval-gate-book-p1-7.md`](./2026-06-23-khala-quantization-eval-gate-book-p1-7.md)
  says a faster quantized model must pass an executed eval gate before sharing a
  public model identity.
- [`2026-06-23-khala-speculation-telemetry-book-p1-8.md`](./2026-06-23-khala-speculation-telemetry-book-p1-8.md)
  says speculation needs acceptance metrics and latency/cost telemetry, not just
  a feature flag.

The next step should therefore be a Hydralisk-owned preflight, not a Worker
catalog change.

## Proposed next issue

Create a Hydralisk issue for:

**GLM-5.2 SGLang high-memory admission/profile preflight**

The preflight should produce a public-safe evidence packet with:

- a successful or explicitly blocked GCE allocation attempt for one exact
  high-memory GPU lane, in this preference order:
  - B200 or RTX PRO 6000 Blackwell;
  - H200;
  - H100;
- `nvidia-smi`, driver, CUDA runtime, NCCL, GPU count, GPU memory, and topology;
- pinned container or `uv` environment;
- model id and immutable model revision for GLM-5.2 FP8;
- pinned SGLang version and launch command;
- model-profile JSON covering context length, tensor/expert/pipeline
  parallelism, DSA support, parser settings, MTP/speculation settings,
  prefix-cache/HiCache settings, and Dynamo disabled/enabled state;
- Hydralisk capability JSON and receipt schema draft for GLM-5.2;
- a minimal smoke result:
  - either load-only plus explicit blocker refs,
  - or one tiny non-private prompt completion with usage and latency fields;
- an honest follow-up recommendation for whether to proceed to:
  - monolithic SGLang benchmark,
  - Dynamo KV-aware routing lab,
  - prefill/decode disaggregation lab,
  - or no-go until more quota/hardware is available.

## Acceptance boundary

Passing this issue does not mean OpenAgents can publicly claim self-hosted
GLM-5.2. It only means Hydralisk has a reproducible model profile and admission
record. Public Khala routing needs later gates:

- model load and generation receipts on repeatable hardware;
- quantization eval parity against an accepted baseline;
- TTFT, TTFAT, ITL, wall-clock, cache, and cost-per-accepted-outcome telemetry;
- long-context and agentic evals;
- capacity and fallback policy;
- product-layer decision to consume GLM-5.2 only behind `khala` /
  `openagents/khala`, not as a raw public model selector.

## Issue body seed

```markdown
## Goal

Create the first Hydralisk GLM-5.2 model profile and high-memory GPU admission
record. This is the next step toward running GLM-5.2 on our infrastructure, not
a public product claim.

## Context

Baseten's 2026-06-23 GLM-5.2 production writeup attributes their 280+ TPS API
to GLM-specific runtime work, Blackwell NVFP4 quantization, Dynamo KV-aware
routing, Dynamo prefill/decode disaggregation, and GLM-5.2 MTP speculation.
OpenAgents docs say our visible GCP quota makes GLM-5.2 possible in principle,
but unproven. Hydralisk is the Python/NVIDIA lane for this exact kind of
preflight.

OpenAgents note:
https://github.com/OpenAgentsInc/openagents/blob/main/docs/inference/2026-06-24-baseten-glm-52-production-inference-notes.md

## Tasks

- [ ] Attempt one exact high-memory GPU allocation in this order: B200 or RTX
      PRO 6000 Blackwell, H200, H100.
- [ ] Capture public-safe hardware evidence: driver, CUDA, NCCL, GPU memory,
      topology, zone, machine type, and quota/admission result.
- [ ] Pin a SGLang GLM-5.2 FP8 environment or container.
- [ ] Write `profiles/glm-5.2-fp8-sglang.json` with model revision, engine,
      context, parallelism, DSA/parser/MTP/cache settings, and Dynamo state.
- [ ] Add or extend a Hydralisk capability/receipt shape for GLM-5.2 profile
      evidence without raw prompts, secrets, hidden reasoning, or model weights.
- [ ] Run either a load-only smoke with blocker refs or a tiny public-safe
      completion smoke with usage and latency.
- [ ] Recommend the next lane: monolithic SGLang benchmark, Dynamo KV-aware
      routing, PD disaggregation, or no-go pending capacity.

## Done when

The repo has a committed profile/runbook plus a public-safe evidence packet
showing either a reproducible GLM-5.2 admission baseline or explicit blockers.
```
