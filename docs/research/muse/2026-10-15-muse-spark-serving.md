# Serving Muse Spark Open on our Google Cloud

2026-10-15, release day. This note covers what Meta released, what it would take to
serve the model on project `openagentsgemini`, and what we should do first.
Meta's claims and our own checks are kept apart: anything we read from an API,
a file list, or `gcloud` is marked **checked**; anything else is Meta's or a
partner's statement. No weights were downloaded for this note.

## 1. What it is

Muse Spark Open is Meta Superintelligence Labs' open-weight release of its
Muse Spark line. It is a sparse mixture-of-experts (MoE) model with 816B total
parameters and 42B active per token, a 1M-token context, and text, image,
video, and audio input. Output is text only. Meta pitches it at coding,
agentic work, and long-horizon tasks.

| | Muse Spark Open |
| --- | --- |
| Released | 2026-10-15 ([blog](https://research.meta.ai/blog/muse-spark-open)) |
| Type | MoE transformer with a Perception Encoder ([model card](https://huggingface.co/meta-models/Muse-Spark-Open-816B-A42B)) |
| Parameters | 816B total, 42B active. **Checked:** the HF API reports 822.0B tensors in the BF16 repo (821.9B BF16 + 0.13B F32) |
| Language model | 62 layers, hidden 8192, 128 query heads and 16 KV heads (GQA), head dim 64, gated attention with QK-norm, 256 experts with top-8 routing; the experts hold about 97% of the parameters ([HF blog](https://huggingface.co/blog/meta-muse-spark-open)) |
| Attention | Hybrid: a repeating cycle of 3 sliding-window layers (2,048-token window) and 1 full-attention layer, and the last layer is always full. That gives 46 sliding-window and 16 full-attention layers ([vLLM blog](https://vllm.ai/blog/2026-10-15-muse-spark-open)) |
| Context | 1,048,576 tokens |
| Modalities in | Text, image, and video (1.8B Perception Encoder), plus audio (118M encoder, 24 kHz, one token per 80 ms). A video's audio track is not read, so send audio as a separate part |
| Modality out | Text |
| Tokenizer | 202,048-entry vocabulary. The HF blog says it is shared with Muse Glimmer, whose card lists 200,000 BPE tokens plus 2,048 specials. Chat template: Harmony-style channels (`<\|start\|>assistant to=…<\|message\|>`), with `reasoning_effort` set to `low`, `medium`, `high`, or `xhigh` (default `xhigh`) and a knowledge cutoff of 2026-01-04. **Checked** from the HF API's `config` field and chat template |
| Weights | BF16: **checked** at 1,644 GB in 65 safetensors shards. NVFP4: **checked** at 533 GB (`Muse-Spark-Open-816B-A42B-NVFP4`; experts in FP4 with FP8 scales, other weights in BF16) |
| Third-party quants | AMD Quark FP8 and MXFP4; Intel MXFP4 and INT4; GGUF from Q2 up (Unsloth, ggml-org); MLX 3-bit and 4-bit ([deploy guide](https://dev.meta.ai/resources/blog/build-with-muse-spark-open)) |
| License | Muse Model License, a custom license that is not OSI-approved (see section 8). Downloads are gated: **checked** `gated: manual` on both HF repos |
| Recommended sampling | temperature 1.0, top_p 1.0, top_k off; `high` or `xhigh` effort for coding and agentic work |

There is also a small sibling. **Muse Glimmer 30B** ([card](https://huggingface.co/meta-models/Muse-Glimmer-30B))
is a dense 29.8B model distilled from Muse Spark. It takes text and image
input, has a 131K context, and is licensed **Apache-2.0** with no gate
(**checked**). It is the only Muse model that fits small GPUs such as L4.

### Sources read

- Blog: [research.meta.ai/blog/muse-spark-open](https://research.meta.ai/blog/muse-spark-open)
- Model card and license: [HF BF16 repo](https://huggingface.co/meta-models/Muse-Spark-Open-816B-A42B), [HF NVFP4 repo](https://huggingface.co/meta-models/Muse-Spark-Open-816B-A42B-NVFP4), [LICENSE.md](https://huggingface.co/meta-models/Muse-Spark-Open-816B-A42B/blob/main/LICENSE.md), [collection](https://huggingface.co/collections/meta-models/muse-spark-open). File lists and sizes came from `https://huggingface.co/api/models/<repo>?blobs=true`. `config.json` itself is behind the gate and returned 401; the API's `config` field exposes the architecture, tokenizer config, and chat template
- Evaluation report: [Muse Spark Open Evaluation Methodology (PDF)](https://research.meta.ai/static/muse-spark-open-evaluation-methodology)
- Developer deployment guide: [build-with-muse-spark-open](https://dev.meta.ai/resources/blog/build-with-muse-spark-open)
- Engine posts: [vLLM](https://vllm.ai/blog/2026-10-15-muse-spark-open) ([recipe](https://recipes.vllm.ai/meta-models/Muse-Spark-Open-816B-A42B)), [SGLang/LMSYS](https://www.lmsys.org/blog/2026-10-15-meta-muse-spark-open) ([cookbook](https://docs.sglang.io/cookbook/autoregressive/Meta/MuseSparkOpen)), [Hugging Face](https://huggingface.co/blog/meta-muse-spark-open), [NVIDIA TensorRT-LLM guide](https://github.com/NVIDIA/TensorRT-LLM/blob/feat/musespark-open/docs/source/deployment-guide/deployment-guide-for-musespark-open-on-trtllm.md), [MLX-LM](https://github.com/ml-explore/mlx-lm)
- Code: Meta publishes no standalone repo for the model. The model code lives in the engines above, in Transformers (`MuseSparkForConditionalGeneration`), and under [github.com/meta-models](https://github.com/meta-models). There is no paper; the evaluation PDF is the only technical report, and the blog says a full safety report is "available soon"
- Prices: [OpenRouter models API](https://openrouter.ai/api/v1/models) and the [Cloud Billing Catalog API](https://cloud.google.com/billing/docs/reference/rest/v1/services.skus/list), both read 2026-10-15

### Benchmarks: claims vs evidence

Meta's model card compares Muse Spark Open only with **open-weight** models:
Inkling, Nemotron 3 Ultra, Beam, Mistral Large 4, Qwen3.8-2.4T-A95B, GLM 5.3,
and Kimi K3. **No source compares it with Gemini or any other closed model.**

| Benchmark (Meta's table) | Muse Spark Open | Best other | Source of the Muse number |
| --- | --- | --- | --- |
| AA Intelligence Index v4.3 | 39 | GLM 5.3: 45 | Artificial Analysis, 2026-10-07 (independent) |
| MCP-Atlas | **89.2** | Qwen3.8: 84.5 | Scale AI leaderboard (independent) |
| JobBench | **61.8** | GLM 5.3: 61.4 | Official leaderboard |
| Terminal-Bench 2.1 | 76.4 | Kimi K3: 85.0 | Artificial Analysis |
| DeepSWE 1.1 | 60 | Kimi K3: 68 | Artificial Analysis, with the Muse Code harness |
| GPQA Diamond | 89.0 | 93.5 | Artificial Analysis |
| HLE (with tools) | 61.0 | GLM 5.3: 62.5 | **Meta's own run**; the other models' numbers are self-reported |
| OSWorld-Verified | 80.4 | Kimi K3: 84.8 | **Meta's own run** |
| Multimodal (ZeroBench, SimpleVQA, IntPhys2, …) | leads 3 of 6 | Kimi K3 | **Meta's own runs**, graded by GPT-OSS-120B |
| StrongREJECT v2, lower is better | **0.5** | ≥62.9 | **Meta's own internal benchmark** |

What the evidence supports: on the independent rows, Muse Spark Open sits mid-pack
among large open models. It leads clearly on MCP tool use (MCP-Atlas) and on
refusal robustness, and it trails Kimi K3 and GLM 5.3 on coding and terminal
work. The "most capable US open-weight model" headline is a claim
about US models only. On Artificial Analysis the closed Muse Spark 1.3 and Gemini 4 Argon
score in the high 40s to low 50s on older index versions, and Gemini 3.8
Flash (high) scores 59 on v4.1.1. Those versions differ from v4.3, so we have **no like-for-like
comparison with Gemini Flash or Argon**. The experiment in section 7 measures one on our own fixtures.

## 2. Hardware sizing

### Weights

| Precision | Size | Basis |
| --- | --- | --- |
| BF16 | 1,644 GB | **Checked** shard sizes |
| FP8 | ~850 GB | Estimate: 822B × 1 byte, plus BF16 embeddings and norms |
| NVFP4 (Meta) | 533 GB | **Checked** shard sizes |
| GGUF ~2-bit | ~300 GB | Unsloth / HF blog |

NVFP4 is a Blackwell format. vLLM says its NVFP4 path needs Blackwell, but
Hugging Face's Inference Endpoints list NVFP4 on 8× H200 at 1M context.
Treat Hopper NVFP4 as unverified (it is probably a weight-only FP4 path). On Hopper
or TPU, plan for FP8 or BF16.

### KV cache

With BF16 KV, each layer stores 16 KV heads × 64 dims × 2 (K and V) × 2 bytes =
4 KiB per token.

- **Hybrid attention, as the vLLM blog describes it.** The 16 full layers cost 64 KiB per token.
  The 46 sliding-window layers cost a fixed 46 × 2,048 × 4 KiB = 386 MB per sequence.
- **All 62 layers full**, which matches the HF blog's "about 250 KB per token, 266 GB at 1M".
  This costs 248 KiB per token.

The vLLM statement that the cache at 1M is "just over ¼" of an all-full
cache matches the hybrid figure, so we size with that and keep the HF number as
the worst case.

| Context per sequence | Hybrid, BF16 KV | Hybrid, FP8 KV | All-full, BF16 (worst case) |
| --- | --- | --- | --- |
| 32K | 2.5 GB | 1.3 GB | 8.3 GB |
| 128K | 9.0 GB | 4.5 GB | 33 GB |
| 1M | 69 GB | 35 GB | 266 GB |

### What fits

Free memory is HBM minus weights, less about 10% for activations, CUDA graphs,
and the vision encoder.

| Shape (GCP name) | HBM | Fits | Room for KV | Sequences at 128K (hybrid BF16) |
| --- | --- | --- | --- | --- |
| 8× B200, `a4-highgpu-8g` | 1,440 GB | NVFP4, one node | ~760 GB | ~80 (or ~10 at 1M) |
| 8× RTX PRO 6000, `g4-standard-384` | 768 GB | NVFP4, one node, no NVLink | ~160 GB | ~17 (HF serves it at 131K here) |
| 8× H200, `a3-ultragpu-8g` | 1,128 GB | FP8 (NVFP4 unverified) | ~170 GB at FP8 | ~18 |
| 8× H100, `a3-highgpu-8g` / A3 Mega | 640 GB | Does not fit FP8. NVFP4 leaves ~40 GB | — | Needs 2 nodes (TP/EP over the network) |
| BF16 anywhere | — | 2× A4, or 4× 8 H200 (HF's Transformers recipe) | — | Multi-node; no reason to do it for serving |
| G2 / L4 (24 GB) | 192 GB on 8 | **No.** Even 2-bit (~300 GB) does not fit | — | Glimmer 30B only: BF16 (60 GB) on 4× L4, or FP8 on 2× L4 |
| TPU v6e (32 GB/chip) | — | FP8: v6e-32 is very tight (1 TB); v6e-64 (2 TB) is comfortable | — | NVFP4 does not apply on TPU |
| TPU v5p (95 GB/chip) | — | FP8 on v5p-32 (16 chips, 1.5 TB) | ~600 GB | — |
| TPU v5e (16 GB/chip) | — | FP8 needs about 64+ chips | — | Poor fit |
| TPU v7 Ironwood (192 GB/chip) | — | 4 to 8 chips; vLLM's only TPU numbers are on v7 | — | We hold no v7 |

The practical single-node choices are **one A4 (8× B200)** and **one G4
(8× RTX PRO 6000)**, both NVFP4. These are also the only two shapes Vertex
Model Garden offers (section 4).

### What we hold today (checked 2026-10-15, service account, read-only)

| Resource | State |
| --- | --- |
| TPU VMs | `p6-asia-northeast1-b` (v6e-1, on-demand, Clef lane), `cd-v6e1-ane1b-d` (v6e-1), `cd-v5e8-ue1c-s2` (v5e-8, spot, us-east1-c), `cd-v5p8-ue5a-s` (v5p-8 = 4 chips, spot, us-east5-a). None is large enough for this model |
| TPU queued resources | `cq-v6e8-uc1a-s` (v6e-8) and `cq-v6e4-uc1a-s` (v6e-4), both `WAITING_FOR_RESOURCES` in us-central1-a |
| GCE on-demand GPU family quota (H100, H100 Mega, H200, B200, RTX PRO 6000) | **0**, with no regional override. A100 16 (1 in use), L4 16, T4 8, V100 8 |
| Vertex AI serving quota, us-central1 | B200 **8**, RTX PRO 6000 **16**, H100 16, preemptible H100 56, L4 28, TPU v5e 32 |
| Vertex Model Garden | **Lists Muse Spark Open.** `gcloud ai model-garden models list --model-filter=muse` shows `meta/muse-spark-open@muse-spark-open-816b-a42b` and `@…-nvfp4` with CAN_DEPLOY=Yes and CAN_PREDICT=No, meaning self-deploy only and no serverless API. It also lists `meta/muse-glimmer@muse-glimmer-30b` |

The upshot is that **Vertex can deploy today on quota we already have**: one A4, or two
G4s. GCE and GKE cannot run either shape without a quota increase.

## 3. Prices used (list, us-central1/Americas, Cloud Billing Catalog, read 2026-10-15)

| SKU | $/h |
| --- | --- |
| GCE A4 B200, per GPU slice (VM included) | 16.11 on-demand, 4.9542 spot |
| GCE RTX 6000 96GB GPU (Iowa) | 1.0957; G4 core 0.0489/vCPU; G4 RAM 0.0059/GiB |
| GCE H200 141GB GPU | 9.3117; A3 Ultra core 0.0225; RAM 0.0013/GiB |
| Vertex prediction B200 + management fee, per GPU | 18.5265 + 2.4165 = 20.943 |
| Vertex prediction RTX 6000 + fee, per GPU | 1.26 + 0.1643 = 1.4243; G4 core 0.0562 + 0.0073; RAM 0.0068 + 0.0009/GiB |
| TPU per chip, on-demand / spot | v6e 2.70 / 0.5838; v5p 4.20 / 0.9261; v5e 1.20 / 0.3593 |
| Hosted Muse Spark Open, OpenRouter (Together, Fireworks, Baseten, Nebius) | $1.25 in / $4.25 out per 1M tokens |
| Gemini 3.8 Flash (our `vertex.rs` rate and OpenRouter) | $0.75 in / $3.75 out |
| Gemini 4 Argon (OpenRouter) | $2.00 in / $10.00 out |

Machine totals:

| Machine | Math | $/h |
| --- | --- | --- |
| GCE `a4-highgpu-8g` | 8 × 16.11 | **128.88** (spot 39.63) |
| Vertex `a4-highgpu-8g` | 8 × 20.943 | **167.54** (if Vertex bills vCPU and RAM separately for A4, add that) |
| GCE `g4-standard-384` | 8 × 1.0957 + 384 × 0.0489 + 1,440 GiB × 0.0059 | **36.04** |
| Vertex `g4-standard-384` | 8 × 1.4243 + 384 × 0.0635 + 1,440 × 0.0077 | **46.87** |
| GCE `a3-ultragpu-8g` | 8 × 9.3117 + 224 × 0.0225 + 2,952 × 0.0013 | **83.37** |
| TPU v6e-64 | 64 × 2.70 | 172.80 (spot 37.36) |
| TPU v5p-32 (16 chips) | 16 × 4.20 | 67.20 (spot 14.82) |

### Throughput assumptions

Meta and partners have published no B200 or RTX PRO 6000 numbers. The closest are SGLang's
8× **B300** runs with DFlash speculative decoding. The OpenHands replay reached
128 users × 61 tok/s ≈ **7,800 output tok/s per node**, and the AgentX
replay reached 48 × 73 ≈ **3,500 tok/s per node**. B200 is slower than B300 at FP4, so we
plan with **2,000 (low) and 5,000 (high) output tok/s per A4 node**. For G4
(GDDR7 memory, PCIe, no NVLink) we plan with **1,000 and 2,500**. All four figures are estimates, which the
experiment replaces with measurements.

Cost per 1M output tokens = $/h ÷ (tok/s × 3,600) × 10⁶. Prefill tokens are
much cheaper than this per token and are left out, so these numbers favour the hosted rows slightly.

| Lane | $/h | Low tok/s → $/M | High tok/s → $/M | Break-even vs hosted $4.25/M |
| --- | --- | --- | --- | --- |
| Vertex A4 | 167.54 | 2,000 → **23.27** | 5,000 → **9.31** | 10,950 tok/s sustained |
| GCE A4 | 128.88 | 2,000 → 17.90 | 5,000 → 7.16 | 8,420 tok/s |
| GCE A4 spot | 39.63 | 2,000 → 5.50 | 5,000 → 2.20 | 2,590 tok/s |
| Vertex G4 | 46.87 | 1,000 → **13.02** | 2,500 → **5.21** | 3,060 tok/s |
| GCE G4 | 36.04 | 1,000 → 10.01 | 2,500 → 4.00 | 2,360 tok/s |

For comparison, Gemini 3.8 Flash output costs $3.75/M and Argon $10/M. A
dedicated node is only cheaper than either if it stays busy around the clock at
high concurrency. At 50% utilization, double every self-hosted figure.

## 4. Option A: Vertex Model Garden (available now)

Steps:

1. The owner accepts the Muse license on the Model Garden card
   ([console](https://console.cloud.google.com/agent-platform/publishers/meta/model-garden/muse-spark-open)).
   Google may also want the HF gate form.
2. Deploy NVFP4. Model Garden's ready-made configs (**checked** with
   `gcloud ai model-garden models list-deployment-config`) are
   `a4-highgpu-8g` × 8 B200 and `g4-standard-384` × 8 RTX PRO 6000. Both use Google's
   SGLang image
   `us-docker.pkg.dev/vertex-ai/vertex-vision-model-garden-dockers/pytorch-sglang-serve:muse-spark-1-2-mg-gcsdl-20261015`.

   ```
   CLOUDSDK_CONFIG=…/gcloud-sa-config gcloud ai model-garden models deploy \
     --project openagentsgemini --region us-central1 \
     --model 'meta/muse-spark-open@muse-spark-open-816b-a42b-nvfp4' \
     --machine-type g4-standard-384 --accelerator-type NVIDIA_RTX_PRO_6000 \
     --accept-eula
   ```

   The BF16 entry also lists 8× B200, but 1,644 GB of BF16 weights do not fit in
   1,440 GB. That deployment presumably quantizes on load. Use the NVFP4 entry
   and ask Google if the BF16 entry matters.
3. The endpoint serves SGLang's OpenAI-compatible chat completions behind Vertex
   IAM. The gateway calls it with a service-account token (section 6).

- **Cost:** $46.87/h (G4) or $167.54/h (A4) per replica. This is billed to the
  prepaid GCP credit, which follows the Google-first policy.
- **Cold start (estimate):** about 20–40 minutes for a new endpoint: VM provisioning,
  a 533 GB copy from GCS, then SGLang warm-up and CUDA-graph capture. A new replica
  under autoscaling takes about the same time.
- **Autoscaling:** Vertex endpoint min/max replicas on GPU duty cycle. Scale-to-zero
  is not a safe default (it is in preview on Vertex at best), and a 30-minute cold start rules it out for
  interactive traffic, so plan on min = 1 while it is up. Quota caps us at 1 A4 or 2 G4 replicas.

## 5. Option B: vLLM or SGLang on GKE or GCE GPUs

Steps:

1. **Quota first.** GCE on-demand B200 and RTX PRO 6000 quota is 0. File a
   request for 8 (one node), or use spot A4 at $39.63/h with preemption.
   [`clef-decisions-h100.md`](../../inference/clef-decisions-h100.md)
   records how scarce spot capacity has been.
2. Stage the NVFP4 weights once in a regional GCS bucket. Load them with
   GCS FUSE or Anywhere Cache, or bake them onto a Hyperdisk ML volume that many readers share.
3. Run Meta's published command with `vllm/vllm-openai:muse-spark-open` (TP4
   with expert parallelism and DSpark speculative decoding, `--reasoning-parser muse --tool-call-parser
   muse`), or `sglang serve … --tp-size 4 --ep-size 4 --quantization
   modelopt_fp4 --speculative-algorithm DFLASH`. On 8 GPUs, use two TP4 replicas or
   TP8/EP8. The vLLM Muse support is
   [PR #61879](https://vllm.ai/blog/2026-10-15-muse-spark-open), still on its way to
   nightly, so pin the launch image tag.
4. On GKE, use an A4 or G4 node pool, a Deployment per replica, and the GKE
   Inference Gateway for KV-aware load balancing. On GCE, use one VM behind the
   internal IP pattern from [`gpu-chat.md`](../../inference/gpu-chat.md).

- **Cost:** $128.88/h (A4), $39.63/h (A4 spot), or $36.04/h (G4), about 23% under Vertex
  for the same hardware.
- **Cold start:** the same 20–40 minutes. Hyperdisk ML or a pre-pulled image cuts the
  load to a few minutes once the disk exists.
- **Autoscaling:** an HPA on queue depth or KV-cache use, from SGLang or vLLM
  metrics, plus the cluster autoscaler for nodes. It is limited by quota and capacity, not by
  configuration.

This option is the most control for the least money, but it costs a quota
request and the operating work of running an 800B model ourselves.

## 6. Option C: TPU (vLLM-TPU or JetStream)

Status today: **not open.**

- vLLM's TPU support runs on **TorchTPU**, and code access means contacting
  Inferact or Google.
- SGLang-TorchTPU recipes are "available soon" behind a sign-up form.
- The only published TPU numbers are on **v7 Ironwood**: over 650 tok/s on 4 chips and
  over 960 tok/s on 8 chips with DSpark, at unspecified concurrency.
- Neither JetStream/MaxText nor the open `tpu-inference` plugin mentions Muse.

If access opens, the steps would be:

1. Get TorchTPU or tpu-inference Muse support from Google.
2. Quantize to FP8 or INT8 ourselves, because NVFP4 is NVIDIA-only.
3. Run on v5p-32 (16 chips, $67.20/h, spot $14.82/h) or v6e-64 ($172.80/h, spot $37.36/h).
4. Shard experts across chips.

- **Capacity:** [`clef-tpu.md`](../../inference/clef-tpu.md) found no v6e
  capacity in the US on 2026-10-12, and our v6e-8 and v6e-4 queued
  requests are still waiting. We hold no v7 or large v5p slice.
- **Cost:** unknown until there are throughput numbers. v5p-32 spot is the
  cheapest hardware in this note per hour, which is why it is worth asking about.
- **Cold start and autoscaling:** queued resources take hours to days to arrive,
  and there is no autoscaling beyond adding slices. This lane is for batch or
  reserved capacity, not bursts.

The next Google meeting ([`clef-tpu-google-brief.md`](../../inference/clef-tpu-google-brief.md))
should ask for Muse TorchTPU access and v5p or v7 capacity.

## 7. Psionic and the gateway

### Psionic

Psionic is not the fast path. Today [`crates/psionic`](../../../crates/psionic)
serves GGUF decoders (Qwen 3.5/3.8, Gemma 4, gpt-oss) on CUDA, Metal, and CPU,
with MXFP4 for gpt-oss and a gemma4 26B-A4B sparse lane. The largest model it has
served is gpt-oss-120b. To run Muse Spark Open it would need:

- NVFP4 Blackwell kernels and BF16/NVFP4 safetensors loading
- 256-expert top-8 routing with expert parallelism
- multi-node tensor and expert parallelism over NCCL-class collectives
- hybrid sliding-window/global paged KV out to 1M
- the Perception Encoder and the audio encoder
- the Harmony template with tool and reasoning parsing
- streaming `/v1/responses` through the `psionic.rs` adapter, which forces `stream: false`
- a TPU backend for the TPU route (Clef's TPU work is a separate JAX path)

That is months of work against engines that support the model today.
Psionic can follow later, starting from the existing `sparse_expert_sharded`
lane.

### Gateway route

The router ([`crates/inference/src/router.rs`](../../../crates/inference/src/router.rs))
maps task classes to `ClassModel { model, upstream }` rows. A Muse route would
follow the `zai.rs` and `pro.rs` pattern:

1. Add an `upstream/muse.rs`, or a `gce`/`vertex` door from
   [`gpu-chat.md`](../../inference/gpu-chat.md) (`upstream/gpu.rs`). Configure it as a
   `ChatConfig` pointing at the Vertex endpoint's OpenAI-compatible chat URL, with a
   service-account bearer token, `privacy: private`, and one `ModelRow`
   `meta/muse-spark-open` that has a `Price` row priced from the measured
   $/M rather than the hosted list price.
2. Mount it in `upstreams()` in `crates/gateway/src/inference_routes.rs`.
3. Keep it **pinned only** (`route.only: ["muse"]`) until it passes the router gates. Only then
   add it as a candidate in the `code` or `reason` class through config `classes`. Do not
   put it in the default table: the endpoint is off most of the time, and a
   cold door should return `503 door_down` rather than stall a turn.
4. A hosted fallback row (`meta/muse-spark-open` in `openrouter.rs`
   `default_models()`) is possible, but OpenRouter is out of credit, so it would
   not answer today.

## 8. License and policy blockers

Read from [LICENSE.md](https://huggingface.co/meta-models/Muse-Spark-Open-816B-A42B/blob/main/LICENSE.md)
(Muse Model License, version "Muse Spark Open", 2026-10-15). The license looks
MIT-like but carries four conditions that matter to us:

1. **Attribution.** Every product that uses the model or a Derivative must
   "prominently display 'Built with Muse Spark Open'" in its UI, docs, website,
   or settings. This is easy, but it must ship with the route.
2. **Model-as-a-Service cap.** If we give third parties API access to models
   and **OpenAgents plus affiliates** grossed more than **$20M in any 12
   months**, we must notify Meta and sign a separate agreement *before*
   offering Muse in that service. The cap counts total revenue, not Muse revenue.
   Our gateway (`openagents/<class>`, the agent API, Pylon resale) is a
   Model-as-a-Service. The exemption for using the model "to power features" of our own product covers
   Coder and web chat, but not raw API access. Below $20M we are clear; the
   owner should record our status.
3. **Derivatives include distillation from outputs.** Any model "trained…
   distilled… using the Software **or any outputs**" is a Derivative and
   carries the attribution duty and the Usage Policy. Training Clef or any
   Psionic model on Muse outputs would bind that model to this license. We should
   keep Muse outputs out of training sets unless we accept that.
4. **Usage Policy (Exhibit A).** Clauses that touch our product:
   - The model is not for users under 18.
   - No "unauthorized or unlicensed practice" of law, finance, or medicine. This
     matters for any legal or financial Autopilot.
   - No malware creation. This matters for security-testing agents.
   - No "representing that the use of Muse Spark Open or outputs are human
     generated". This matters for any agent that posts as a person, such as social posts.
   - The deployer must disclose known dangers to end users.

Operational points:

- The download gate is manual and asks for a legal name, date of birth, and
  affiliation. That makes it an owner step.
- Meta asks deployers not to expose raw chain-of-thought and to add system-level
  guardrails.
- Meta claims no ownership of outputs, and the warranty disclaimer is standard.
- Unlike Llama licenses, there is no acceptable-use carve-out by country or
  by monthly active users. The $20M Model-as-a-Service cap is the commercial limit.

None of these blocks an internal experiment. Items 2 and 3 need an owner
decision before Muse becomes a routed product model.

## 9. Recommendation and experiment

**Recommendation:** do not stand up permanent capacity. Run one bounded
**Vertex Model Garden** trial on NVFP4. Start with G4 ($46.87/h, inside current
quota, billed to the Google credit), then optionally run A4 for an hour to see how
far throughput scales. Measure, tear down, and decide. TPU waits on Google access,
GCE/GKE waits on quota, and Psionic is a later project. Self-hosting only beats
Gemini 3.8 Flash on price at several thousand output tok/s around the clock. Unless
the quality numbers show something Flash or Argon cannot do (MCP-heavy tool use is
Muse's strongest independent result), the likely outcome is "use Gemini, keep Muse
as a pinned option".

**Experiment (≈ 6 h on G4 plus 1 h on A4 ≈ $450 of credit):**

| Step | What | Accept if |
| --- | --- | --- |
| E0 | The owner accepts the license and gate. Confirm Vertex quota (B200 8, RTX PRO 6000 16) | Endpoint deploy is allowed |
| E1 | Deploy NVFP4 on `g4-standard-384`; record time to ready | Ready in ≤ 45 min |
| E2 | Latency: 4K-token prompt, `reasoning_effort=low`, concurrency 1, 8, and 32 | TTFT p50 ≤ 2 s and p95 ≤ 5 s at concurrency 8; ≥ 40 tok/s per user at concurrency 8 |
| E3 | Throughput: ramp concurrency until p95 TTFT exceeds 10 s; record peak output tok/s | Measured $/M at peak ≤ $10 (Argon) on G4; ≤ $3.75 (Flash) earns a production look |
| E4 | Quality: `crates/gym/suites/chat-router-v5.json`, `crates/gym/questions/chat-router-route-v6.json`, and `crates/route-contract/fixtures/route-families-v1.json`, run against Muse, `google/gemini-3.8-flash`, and Gemini 4 Argon | Muse ≥ Flash within noise on the router gate (`crates/gym/gates/router-v1.json`) |
| E5 | Tool use: a fixed set of Coder tool-call turns through the `muse` parser | ≥ 98% well-formed tool calls; no raw reasoning leaks into the visible answer |
| E6 | Optional: the same E2 and E3 on `a4-highgpu-8g` for 1 h | Gives the B200 $/M for the table in section 3 |
| E7 | Undeploy, record actual spend, and update this note with measured numbers | Endpoint deleted; spend within 20% of the estimate |

If E3 and E4 both pass, the next step is the gateway door in section 7, pinned
only, and an owner decision on license items 2 and 3.
