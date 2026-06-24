# Hydralisk Python/NVIDIA inference stack

Date: 2026-06-23

Status: live day-zero serving lane. Hydralisk now has a standalone repo, a live
GCE L4/vLLM host for GPT-OSS 20B, public-safe capabilities/receipts, and
OpenAgents Worker routing/config hooks for `openai/gpt-oss-20b`. This is still
an internal dogfood lane for Khala traffic, not a broad product promise for
every GPT-OSS workload.

Scope: define **Hydralisk** as the Python/NVIDIA inference lane for OpenAgents:
the stack we use when the right move is to stay in conventional Python ML
practice, use NVIDIA's serving tools directly, and dogfood production inference
paths before or alongside the Rust-native Psionic stack.

## Bottom line

Hydralisk should be a sibling stack to Psionic, not a replacement for it. If it
becomes a standalone repository, treat it like a named lane in the same spirit
as Psionic: a focused stack with its own identity, contracts, and operational
truth, not a miscellaneous scripts folder.

Psionic remains the "build the substrate ourselves" Rust ML framework: tensors,
compiler, runtime, backends, serving, cluster execution, evidence, and receipts.
Hydralisk is the pragmatic Python lane: vLLM, SGLang, TensorRT-LLM, Triton,
NVIDIA Dynamo, CUDA containers, and model-specific serving recipes. It should
ship inference capability faster, produce hard benchmark/eval evidence, and
teach us which pieces are worth porting into Psionic later.

The first Hydralisk targets should be:

1. `gpt-oss-20b` on L4 with vLLM, as the cheap internal dogfood lane.
2. `gpt-oss-120b` on H100/H200/B200/G4-class high-memory GPUs with vLLM.
3. GLM-5.2 first as a hosted baseline and then as a high-memory SGLang/Dynamo
   research lane.

The first execution roadmap now lives in the standalone Hydralisk repo:
`/Users/christopherdavid/work/hydralisk/docs/gpt-oss-20b-khala-live-roadmap.md`.
It makes `gpt-oss-20b` on L4 the ASAP live-serving lane behind the OpenAgents
API for Khala.

## 2026-06-24 Live Update

Hydralisk issue #1 is complete. The live host is:

- project: `openagentsgemini`
- instance: `hydralisk-gptoss20b-l4-20260624000550`
- zone: `us-central1-a`
- shape: `g2-standard-8`, 1 x NVIDIA L4
- served model: `openai/gpt-oss-20b`
- engine: vLLM `0.23.0`
- proxy: Hydralisk bearer-protected FastAPI proxy
- public HTTPS origin: stored as the OpenAgents Worker `HYDRALISK_BASE_URL`
  secret, not committed here

The live serving path passed:

- host-local `scripts/smoke-gpt-oss-20b.sh`
- public HTTPS-origin `scripts/smoke-gpt-oss-20b.sh`

Public-safe evidence refs for Worker arming:

```text
HYDRALISK_GPT_OSS_20B_ENABLED=ready
HYDRALISK_BASE_URL=<worker secret>
HYDRALISK_BEARER_TOKEN=<worker secret>
HYDRALISK_GPT_OSS_20B_PREFLIGHT_REF=preflight.hydralisk.gpt_oss_20b.l4.20260624T002313Z
HYDRALISK_GPT_OSS_20B_RECEIPT_REF=receipt.hydralisk.gpt_oss_20b.l4.hydralisk-run-48adcddbb0ff47e097685da23f17cea6
```

The public-safe receipts observed on the host:

- `hydralisk-run-0e65f1ef6281413eab28f8aa71580c7b`: non-streaming,
  81 total tokens, 2578 ms wall time.
- `hydralisk-run-88ccd454ea4f4fc7baee9a72c4894527`: streaming,
  195 total tokens, 77 ms TTFT, 2050 ms wall time.

The current OpenAgents production path was deployed after the host smoke and
the 2026-06-24 model-id correction:

- Worker: `openagents-autopilot`
- OpenAgents commit: `009924bac5`
- Worker version: `b5c74b67-32a9-4865-a28e-83a878d0b81b`
- Smoke script: `apps/openagents.com/scripts/gpt-oss20b-production-smoke.mjs`
- Smoke model: `openai/gpt-oss-20b`
- Served model: `openai/gpt-oss-20b`
- Worker disclosure: `hydralisk-vllm`
- Supply-lane disclosure: `hydralisk`
- Completion response id: `chatcmpl_8434ec68f53249658d9f0d1f6bba1cba`
- Public receipt:
  `receipt.inference.charge.chatcmpl_8434ec68f53249658d9f0d1f6bba1cba`

The production smoke passed readiness, model-catalog, authenticated
non-streaming completion, streaming completion, usage/disclosure blocks, and the
infrastructure-leak guard. A live `/v1/models` check confirmed
`openai/gpt-oss-20b` is advertised and `openagents/khala-oss-20b` is not. The
public receipt endpoint dereferenced the receipt above. The earlier promotion
also separately confirmed that an unfunded agent token receives
`402 insufficient_credits` before provider dispatch, so the balance gate remains
active while the Hydralisk lane is armed.

Visible-content follow-up smokes:

- Non-streaming with `max_tokens: 128` returned `NONSTREAM READY`, stopped
  cleanly, and wrote
  `receipt.inference.charge.chatcmpl_550afe2c0e894dec8c3624b664331353`.
- Streaming with `max_tokens: 512` returned `STREAM READY`, stopped cleanly, and
  wrote `receipt.inference.charge.chatcmpl_dcd97345b3f14699b672544138597c3d`.

The higher streaming cap matters because GPT-OSS emits reasoning tokens before
visible content. Very small token caps can be correctly charged and receipted
while still stopping before assistant text appears.

Hydralisk repo follow-up `31e8b40` records the live promotion and the real
fresh-host fixes: current DLVM image family
`common-cu129-ubuntu-2204-nvidia-580`, `build-essential`/`ninja-build` for
FlashInfer JIT, a quoted GPU env value, and a systemd PATH that includes the
venv/CUDA toolchain.

## 2026-06-23 Implementation Update

The Hydralisk repository exists at `OpenAgentsInc/hydralisk`.

Repo-side Hydralisk work landed first:

- `0648235` added the L4/vLLM serving scaffold: FastAPI proxy, bearer auth,
  OpenAI-compatible `/v1/chat/completions` and `/v1/responses`, systemd units,
  Caddy example, and a GCE L4 runbook.
- `80a5455` added public-safe capabilities, run receipts, streaming usage
  capture, and the `scripts/smoke-gpt-oss-20b.sh` smoke path.

OpenAgents Worker work now defines the product-facing lane:

- adapter id: `hydralisk-vllm`
- supply lane: `hydralisk`
- public model id: `openai/gpt-oss-20b`
- upstream model: `openai/gpt-oss-20b`
- direct `gpt-oss-20b`: still Fireworks-first for day zero

The Worker arms Hydralisk only when all of these are present:

```text
HYDRALISK_GPT_OSS_20B_ENABLED=ready
HYDRALISK_BASE_URL=<worker secret>
HYDRALISK_BEARER_TOKEN=<worker secret>
HYDRALISK_GPT_OSS_20B_PREFLIGHT_REF=<public-safe preflight ref>
HYDRALISK_GPT_OSS_20B_RECEIPT_REF=<public-safe receipt ref>
```

When unarmed, `/v1/models` hides `openai/gpt-oss-20b` and
`/v1/chat/completions` returns `model_unavailable` for the requested model before
balance, premium, or provider dispatch. When armed, the route discloses
`openagents.worker: hydralisk-vllm` and the additive
`openagents.supply_lane: hydralisk`; the legacy `openagents.lane` remains the
model class (`open`) for backward compatibility.

2026-06-24 slug correction: the live GPT-OSS 20B lane is exposed as the raw
upstream model id, `openai/gpt-oss-20b`. `openagents/khala-*` ids are reserved
for models or coordinators that actually add Khala-specific behavior, such as
Blueprint-backed orchestration or identity semantics.

The previous blocker was live host promotion. It is now cleared by the
2026-06-24 L4 deployment above; the Psion L4 hosts were not reclaimed.

The important boundary is simple: Hydralisk can own Python serving mechanics and
model/runtime evidence, but OpenAgents/Khala keeps pricing, credits, payout,
referral, customer routing, and public product promises. Hydralisk receipts
should be consumed by the product layer, not become a second product authority.

## Why Hydralisk exists

The local Psionic docs draw a hard line: Psionic can learn from `llama.cpp`,
vLLM, SGLang, TensorRT-LLM, and MLX, but its shipped runtime should not quietly
become a required Python wrapper around somebody else's engine. That is the
right discipline for a Rust-native ML framework.

But it also creates a useful opening. We still need an honest place to do the
ordinary production thing:

- pull the official CUDA/PyTorch/vLLM/SGLang wheels or containers;
- run the vendor-supported engine for a model;
- use NVIDIA's current Blackwell/Hopper stack when that is where the performance
  work is happening;
- collect real workload telemetry before overbuilding our own scheduler; and
- expose OpenAI-compatible endpoints quickly enough to dogfood.

That place is Hydralisk.

Hydralisk lets us pursue two truths at once:

- Psionic is the long-term owned execution substrate.
- Hydralisk is the Python/NVIDIA lane for getting the modern model stack running
  today and learning from it under production-shaped load.

## Ownership boundary

| Layer                     | Psionic                                                            | Hydralisk                                            | OpenAgents / Khala                              |
| ------------------------- | ------------------------------------------------------------------ | ---------------------------------------------------- | ----------------------------------------------- |
| Runtime identity          | Rust-native ML framework                                           | Python/NVIDIA serving stack                          | Product gateway and business surface            |
| Default engines           | Psionic backends, owned Rust serving, reference-compatible kernels | vLLM, SGLang, TensorRT-LLM, Triton, NIM where useful | Provider adapters and model routing             |
| Scheduler and KV research | Owned implementation target                                        | Use vLLM/SGLang/Dynamo now, measure hard             | Decide when a lane is product-eligible          |
| Receipts                  | Psionic execution receipts and parity evidence                     | Hydralisk run receipts and engine evidence           | Accepted-outcome receipts, metering, settlement |
| Money authority           | None                                                               | None                                                 | Credits, pricing, payouts, referral             |
| Public promises           | Only after hardware validation rows                                | Only after benchmark/eval/capacity gates             | Product-facing claims and docs                  |
| Private data              | No raw secrets or prompts in receipts                              | Same                                                 | Same                                            |

Hydralisk should never blur into Psionic's identity. A Hydralisk win can become
a Psionic port candidate only after it produces enough evidence to define the
target behavior.

## Stack shape

Hydralisk should be boring in the best way: pinned containers, pinned Python
packages, clear model profiles, repeatable launch scripts, Prometheus metrics,
and receipts for every admitted run.

Recommended baseline:

- Python 3.12 with `uv` and checked-in locks.
- CUDA images from NVIDIA or the engine project, pinned by digest for production
  lanes.
- NVIDIA Container Toolkit on GCE/GKE nodes.
- `nvidia-smi`, driver, CUDA runtime, GPU topology, and NCCL checks before model
  admission.
- Hugging Face or model-provider weight fetch into explicit immutable model
  revisions.
- OpenAI-compatible HTTP API for clients, with additional Hydralisk capability
  and receipt endpoints.
- Prometheus metrics, request traces, and per-engine logs.
- GenAI-Perf or equivalent benchmark harness for common latency/throughput
  slices.

Engine selection:

| Engine                  | Hydralisk role                                                                                                                                             |
| ----------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| vLLM                    | Default first engine for gpt-oss and broad OpenAI-compatible serving. Use for the first L4 and high-memory GPU lanes.                                      |
| SGLang                  | Primary GLM/MoE/long-context/structured-generation lane. Use where model-specific parsers, MTP, HiCache, DSA, and agent/tool behavior matter.              |
| TensorRT-LLM            | Promotion path for stable, high-volume model/hardware pairs where engine build cost is justified.                                                          |
| Triton Inference Server | Packaging and deployment layer for TensorRT-LLM or other stable model services when it simplifies operations.                                              |
| NVIDIA NIM              | Use packaged NIMs when the target model exists and the packaging wins; do not wait on NIM for GLM if SGLang is the supported path.                         |
| NVIDIA Dynamo           | Routing, KV-aware scheduling, KV offload, NIXL transfer, and prefill/decode disaggregation around vLLM/SGLang/TensorRT-LLM when the workload justifies it. |

This stack is intentionally different from the Khala MVP note on Dynamo. Khala
should not depend on disaggregation before traffic proves the need. Hydralisk
can still maintain a Dynamo lab/canary lane so we know how to turn it on when
GLM-style workloads or volume make it load-bearing.

## API and receipt contract

Hydralisk should expose normal model-serving APIs:

- `GET /health`
- `GET /metrics`
- `GET /hydralisk/v1/capabilities`
- `GET /hydralisk/v1/receipts/{runRef}`
- `POST /v1/chat/completions`
- `POST /v1/responses`
- optionally `POST /v1/messages` when the selected engine supports an
  Anthropic-compatible surface

Every admitted run should produce a receipt. Public-safe receipts must not
contain raw prompts, private source, secrets, hidden chain-of-thought, or raw
tool payloads.

Minimum receipt fields:

```json
{
  "schema": "hydralisk.serve.run_receipt.v1",
  "runRef": "hydralisk-run-...",
  "requestRef": "khala-request-...",
  "model": "openai/gpt-oss-20b",
  "modelRevision": "hf-revision-or-digest",
  "servedModel": "openagents/hydralisk-gpt-oss-20b",
  "engine": "vllm",
  "engineVersion": "0.10.1+gptoss",
  "containerImage": "image@sha256:...",
  "cudaDriver": "...",
  "cudaRuntime": "...",
  "gpu": {
    "name": "NVIDIA L4",
    "count": 1,
    "memoryGbEach": 24,
    "topology": "single-node"
  },
  "parallelism": {
    "tensor": 1,
    "pipeline": 1,
    "data": 1,
    "expert": null
  },
  "quantization": {
    "weights": "MXFP4",
    "kvCache": "engine-default"
  },
  "context": {
    "requestedTokens": 0,
    "admittedMaxTokens": 0
  },
  "parsers": {
    "reasoning": "gpt-oss-harmony",
    "toolCalls": "engine-supported"
  },
  "cache": {
    "prefixCacheEnabled": true,
    "kvAwareRouting": false,
    "hitRate": null
  },
  "dynamo": {
    "enabled": false,
    "routerMode": null,
    "pdDisaggregation": false
  },
  "speculation": {
    "enabled": false,
    "method": null,
    "acceptedTokensMean": null
  },
  "usage": {
    "promptTokens": 0,
    "completionTokens": 0,
    "reasoningTokens": 0
  },
  "latency": {
    "ttftMs": 0,
    "itlMsP50": 0,
    "wallMs": 0
  },
  "quality": {
    "evalSuite": "khala-internal-smoke",
    "acceptedOutcome": true
  },
  "outputHash": "sha256:...",
  "sourceRefs": [],
  "blockerRefs": []
}
```

The exact JSON can evolve, but the principle should not: Hydralisk produces
enough evidence for Khala to meter, route, compare, and refuse honestly.

## Lessons from Baseten GLM-5.2

Baseten's GLM-5.2 writeup is useful because it is a system case study, not a
mere "we used bigger GPUs" story. Their reported 280+ TPS result came from a
stack of model-specific work:

- custom runtime support for GLM-5.2's shared DSA architecture;
- NVFP4 quantization on Blackwell, calibrated against agentic evals;
- high KV cache hit rates through NVIDIA Dynamo routing;
- prefill/decode disaggregation through NVIDIA Dynamo;
- GLM-5.2 MTP heads for speculative decoding; and
- workload-specific tuning for cache, prefill/decode ratio, parallelism, and
  batching.

Hydralisk should turn that into a default engineering checklist:

1. Every model needs a model profile, not just a name.
2. Every engine needs a pinned launch profile, not just "serve this model."
3. Every quantization mode needs an eval gate.
4. Long-context agent workloads need prefix/KV telemetry from day one.
5. Disaggregation is a measured promotion, not a default dependency.
6. Speculation is admitted only when the engine exposes acceptance metrics.
7. Tokens per second is not enough; measure TTFT, TTFAT, ITL, wall time,
   cache-hit rate, cost per accepted outcome, and failure class.

For GLM specifically, the public SGLang GLM-5.2 docs make SGLang the obvious
Hydralisk starting point: FP8 recommended deployment, DSA-specific attention
behavior, MTP speculative decoding, 1M context, reasoning/tool parsers, HiCache,
and explicit Hopper/Blackwell hardware guidance. That is much closer to a
Hydralisk profile than to a clean Psionic-native first implementation.

## Google GPU mapping

Use the quota inventory and model-fit docs as the current local truth. The
Hydralisk planning read is:

| Hardware lane          | Hydralisk use                                                                                 |
| ---------------------- | --------------------------------------------------------------------------------------------- |
| L4                     | GPT-OSS 20B smoke, small/medium model serving, cheap dogfood, eval harness.                   |
| A100 40GB              | Fallback for older CUDA paths, smaller quantized models, compatibility probes.                |
| RTX PRO 6000 Blackwell | Strong candidate for Blackwell/NVFP4-style experiments if allocation is proven again.         |
| H100 80GB              | GPT-OSS 120B single-GPU or multi-GPU path; GLM probe only with more GPUs and memory planning. |
| H200 141GB             | Better high-memory lane for GPT-OSS 120B and GLM FP8 SGLang probes.                           |
| B200 180GB             | Most relevant visible lane for Baseten-style Blackwell/FP4/GLM experiments.                   |
| GB200                  | Visible accelerator type, but not yet a quota-proven lane in our inventory.                   |

Spot quota is useful for experiments and benchmark sweeps. Public production
serving should either use non-preemptible capacity or have a typed fallback and
interruption policy that the product layer understands.

## GPT-OSS bootstrap path

The GPT-OSS lane is the best first Hydralisk dogfood because the model card work
and OpenAI's vLLM guide both point at a straightforward serving path.

### H1: GPT-OSS 20B on L4

Goal: create the first useful Hydralisk service on infrastructure we already
know how to touch.

Implementation sketch:

- GCE or GKE node with one L4.
- Pinned CUDA/vLLM image or `uv` environment.
- `vllm serve openai/gpt-oss-20b`.
- OpenAI-compatible `/v1/chat/completions` and `/v1/responses`.
- Harmony/reasoning/tool handling verified by executed requests.
- Hydralisk capability endpoint and receipt emission.
- Small benchmark matrix:
  - short chat;
  - tool call;
  - medium reasoning;
  - code edit;
  - repeated-prefix cache probe;
  - streaming and non-streaming.

Admission gate:

- no raw hidden reasoning in public receipts;
- no product claim beyond internal dogfood until evals pass;
- vLLM version, CUDA version, model revision, GPU shape, quantization, and
  latency metrics recorded.

Dogfood use:

- internal Khala route for cheap coding/question traffic;
- shadow Fireworks/OpenAI/hosted lane for accepted-outcome comparison;
- collect prefix-cache and latency telemetry.

### H2: GPT-OSS 120B on high-memory GPU

Goal: prove the larger model under the same Hydralisk contract.

Implementation sketch:

- H100/H200/B200/G4-class high-memory allocation.
- vLLM first, because OpenAI's guide explicitly supports both gpt-oss model
  sizes and says 120B is best with at least 60 GB VRAM.
- Same receipt, eval, and parser gates as 20B.
- Compare against hosted gpt-oss 120B lanes on:
  - quality;
  - TTFT and ITL;
  - wall time;
  - cost per accepted outcome;
  - failure/refusal rate;
  - tool-call correctness.

Promotion gate:

- stable allocation proof;
- repeatable benchmark output;
- clear cost basis;
- typed fallback route;
- no unresolved harmony or reasoning leakage issue.

## GLM-5.2 bootstrap path

GLM-5.2 should not be the first Hydralisk service. It should be the first
Hydralisk frontier-model campaign.

### G0: Hosted baseline

Use Baseten or another hosted GLM-5.2 provider through Khala first. This gives
us:

- real OpenAgents workload shapes;
- expected quality and latency;
- tool/reasoning behavior;
- prompt/cache structure;
- cost baseline;
- a comparison target for self-hosted Hydralisk.

### G1: Model profile

Create `hydralisk.models.glm52` before trying to claim service:

- model ids: `zai-org/GLM-5.2-FP8`, `zai-org/GLM-5.2`;
- architecture: MoE, DSA, MTP, 1M context;
- recommended first precision: FP8;
- engine: SGLang first;
- parsers: `glm45` reasoning, `glm47` tool-call parser where appropriate;
- cache: prefix/radix/HiCache telemetry;
- speculation: MTP/EAGLE profile and accepted-token metrics;
- context policy: default cap lower than 1M until memory and latency are proven;
- refusal policy for unsupported precision, unsupported GPU, and unbounded
  context.

### G2: Monolithic SGLang probe

Goal: load GLM-5.2 FP8 on high-memory hardware and serve a controlled local API.

Preferred hardware order:

1. H200 multi-GPU node.
2. B200 multi-GPU node.
3. RTX PRO 6000 multi-GPU node if allocation and software support are proven.
4. GB200/B300 only if quota/capacity appears later.

The first probe should be monolithic before Dynamo disaggregation. We need a
stable single service to establish model correctness, parser correctness,
memory headroom, and basic throughput before we split prefill/decode.

### G3: Dynamo KV-aware routing canary

Once monolithic SGLang works, add NVIDIA Dynamo for:

- KV-aware request routing;
- per-replica cache/load signals;
- metrics on prefix reuse;
- canary A/B against round-robin routing;
- route refusal when cache metadata is stale or workers are unhealthy.

This is the Baseten lesson that should land early for GLM-like agent traffic:
long system prompts, codebase context, and repeated conversation prefixes make
cache placement materially important.

### G4: Prefill/decode disaggregation lab

Only after G2/G3 produce evidence should Hydralisk test PD disaggregation.

Measure:

- prefill queue depth;
- decode queue depth;
- TTFT;
- TTFAT;
- ITL;
- GPU memory headroom;
- KV transfer latency;
- NIXL/Mooncake errors;
- accepted-outcome rate;
- total cost per accepted answer.

Promotion requires workload-specific improvement. Baseten saw a large gain for
their observed workload shapes; Hydralisk should prove or reject that for ours.

### G5: Product canary

The first product canary should be bounded:

- owner/internal traffic only;
- a fixed spend ceiling;
- hosted GLM fallback;
- no public claim of Baseten-equivalent speed;
- public-safe receipts;
- direct comparison to hosted GLM and GPT-OSS 120B for accepted outcome.

## Dogfood loop

Hydralisk should start useful, not theoretical.

Recommended loop:

1. Build `gpt-oss-20b` L4 as the first service.
2. Wire it as an internal Khala provider adapter behind a feature flag.
3. Shadow real internal requests against hosted lanes.
4. Emit Hydralisk receipts and Khala accepted-outcome summaries.
5. Use those workloads to choose the next benchmark cases.
6. Promote `gpt-oss-120b` only after high-memory allocation proof.
7. Use hosted GLM-5.2 for frontier comparison while building the SGLang profile.
8. Run GLM self-hosted as a shadow lane before exposing it as a customer route.

Every Hydralisk campaign should leave a Psionic handoff packet:

- exact model and revision;
- engine launch profile;
- GPU topology and driver/runtime versions;
- quantization profile;
- parser and template behavior;
- eval results;
- workload distribution;
- cache and prefill/decode telemetry;
- failure modes;
- target Psionic port candidates.

That handoff is how Hydralisk feeds the "build it ourselves" path without
blocking today on owning every layer.

## Repository shape

If Hydralisk becomes its own GitHub repository, start with a narrow shape:

```text
hydralisk/
  pyproject.toml
  uv.lock
  README.md
  hydralisk/
    serve/
      app.py
      openai_compat.py
      health.py
    engines/
      base.py
      vllm_engine.py
      sglang_engine.py
      trtllm_engine.py
      nim_engine.py
    models/
      gpt_oss.py
      glm52.py
    dynamo/
      router.py
      profiles.py
    receipts/
      schema.py
      writer.py
    bench/
      runner.py
      workloads.py
      compare.py
    evals/
      khala_smoke.py
      tool_calling.py
      reasoning_visibility.py
  deploy/
    gce/
    gke/
    containers/
  docs/
    gpt-oss-20b-l4.md
    gpt-oss-120b-high-memory.md
    glm-52-sglang.md
```

The repository should optimize for repeatability first. A thin, explicit Python
wrapper around proven engines is better than a broad framework that hides how
the model was actually served.

## Milestones

| Milestone | Result                                                                          |
| --------- | ------------------------------------------------------------------------------- |
| M0        | Hydralisk spec, repo decision, receipt schema draft, local benchmark fixtures.  |
| M1        | GPT-OSS 20B on L4 with vLLM, internal-only endpoint, receipts, and smoke evals. |
| M2        | Khala provider adapter for Hydralisk behind a feature flag.                     |
| M3        | GPT-OSS 120B high-memory vLLM proof, cost and quality comparison.               |
| M4        | GLM-5.2 hosted baseline with workload capture and eval profile.                 |
| M5        | GLM-5.2 FP8 SGLang monolithic self-host probe on high-memory GPUs.              |
| M6        | Dynamo KV-aware routing canary for GLM traffic.                                 |
| M7        | GLM prefill/decode disaggregation lab and A/B benchmark.                        |
| M8        | Bounded product canary or explicit refusal if cost/capacity/quality fail.       |

## Refusal conditions

Hydralisk should fail closed when:

- the model profile is missing;
- the engine/image/package set is unpinned;
- GPU shape, driver, or CUDA runtime are unknown;
- the model revision is mutable or unrecorded;
- quantization has no eval gate;
- the parser/template can leak hidden reasoning;
- the receipt would include raw private prompts, source, secrets, or tool
  payloads;
- the route is spot-only without fallback for product traffic;
- GLM is requested without enough high-memory GPUs and a supported SGLang
  profile;
- Dynamo routing/disaggregation is enabled without metrics proving health; or
- OpenAgents/Khala has no pricing, metering, or spend ceiling for the lane.

The refusal is a feature. It keeps Hydralisk useful as an engineering lane
instead of turning it into a new place for vague GPU optimism.

## Sources and local anchors

- [`2026-06-23-gcloud-gpu-quota-inventory.md`](./2026-06-23-gcloud-gpu-quota-inventory.md)
- [`2026-06-23-gpu-inventory-model-fit-and-glm-52.md`](./2026-06-23-gpu-inventory-model-fit-and-glm-52.md)
- [`2026-06-23-gpt-oss-20b-120b-gpu-fit-and-serving-path.md`](./2026-06-23-gpt-oss-20b-120b-gpu-fit-and-serving-path.md)
- [`2026-06-23-khala-disaggregation-dynamo-study.md`](./2026-06-23-khala-disaggregation-dynamo-study.md)
- [`inference-engineering-book/book-reading-notes.md`](./inference-engineering-book/book-reading-notes.md)
- [`inference-engineering-book/khala-investigation-notes.md`](./inference-engineering-book/khala-investigation-notes.md)
- Psionic `README.md`, `docs/INFERENCE_ENGINE.md`,
  `docs/INFERENCE_MESH_OWNERSHIP.md`,
  `docs/LLAMA_VLLM_SGLANG_INFERENCE_SPEC.md`,
  `docs/GPT_OSS_LOCAL_SERVING.md`, and
  `docs/MESH_LANE_SERVICE_MODE.md`
- [Baseten GLM-5.2 production writeup](https://www.baseten.co/blog/how-we-built-the-worlds-fastest-api-for-glm-52/)
- [OpenAI gpt-oss with vLLM guide](https://developers.openai.com/cookbook/articles/gpt-oss/run-vllm)
- [NVIDIA Dynamo introduction](https://docs.nvidia.com/dynamo/getting-started/introduction)
- [NVIDIA Dynamo KV-aware routing guide](https://docs.nvidia.com/dynamo/user-guides/kv-cache-aware-routing)
- [NVIDIA Dynamo vLLM backend guide](https://docs.nvidia.com/dynamo/backends/v-llm)
- [SGLang GLM-5.2 cookbook](https://lmsysorg.mintlify.app/cookbook/autoregressive/GLM/GLM-5.2)
- [SGLang PD disaggregation docs](https://docs.sglang.io/docs/advanced_features/pd_disaggregation)
- [vLLM disaggregated prefill docs](https://docs.vllm.ai/en/latest/features/disagg_prefill/)
