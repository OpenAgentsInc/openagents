# Clef on Cloud TPU: survey of existing stacks, kernels, quantization and prices

2026-10-12. Research for [#11459](https://github.com/OpenAgentsInc/openagents/issues/11459)
and the plan in [clef-tpu.md](clef-tpu.md). Nothing here was run on a TPU;
the hands-on P0/P1 work is a separate track. Figures marked "measured" are
somebody else's published measurement; figures marked "estimate" are ours,
from chip specifications.

## The short answer

- **The hard part already exists, under Apache-2.0.** Google's vLLM TPU
  backend ([tpu-inference](https://github.com/vllm-project/tpu-inference))
  ships a fused, chunked Gated DeltaNet Pallas kernel (`gdn/v3`), and
  [SGL-JAX](https://github.com/sgl-project/sglang-jax) runs the Qwen3.5
  hybrid in native JAX with a copy of that kernel. SGL-JAX publishes a
  working recipe for **Qwen3.8-27B on a v6e-4**, which is Clef 27B's exact
  backbone.
- **Both Clef checkpoints load as stock models.** `Cloudflare/clef-flash`
  and `Cloudflare/clef` declare `Qwen3_5ForConditionalGeneration` /
  `qwen3_5`, the class both stacks already load. Only the joint head and the
  memory-row and span-sum plumbing are ours.
- **No stack serves a "prefill-only, read the hidden rows, run a custom
  head" workload out of the box.** Both are built around generating tokens.
  Pooling and embedding support exists in each, but it is untested for
  hybrid models.
- **The cheapest chip per token is v6e on spot.** It costs $0.584 an hour in
  the Americas multi-region and **$0.243 in us-east4**. Ironwood (TPU7x)
  costs more per Flash token than v6e. It is only worth it for 27B on one
  chip.
- **Recommendation:** write our own thin JAX worker as planned (path A), but
  build it from these parts rather than from scratch. Take the backbone code
  from SGL-JAX, use the tpu-inference `gdn/v3` kernel for the 24 DeltaNet
  layers and JAX's own flash attention for the 8 full-attention layers, and
  use MaxText's pure-JAX version as the reference. This should give a correct
  Flash in about **1–2 weeks** rather than 2–4. Estimated cost on spot v6e is
  **about $0.003–0.017 per million tokens**, against $0.11 on H100, $0.072 on
  A100 and Jev's $0.042.

## 1. TPU serving stacks and the Qwen3.5 / Qwen3-Next hybrid

| Stack | Version / activity (2026-10-12) | Runs the hybrid? | Our backbones | Prefill-only, hidden rows or a head | License |
| --- | --- | --- | --- | --- | --- |
| **[vLLM TPU / tpu-inference](https://github.com/vllm-project/tpu-inference)** | v0.31.0 (2026-10-08); releases every 1–2 weeks | **Yes.** Fused Conv1D + GDN Pallas kernel (`kernels/gdn/v3`, [PR #3016](https://github.com/vllm-project/tpu-inference/pull/3016), merged 2026-06-30). Qwen3-Next is served through the torchax (PyTorch-model) path. A JAX-native Qwen3-Next model is in review ([PR #3073](https://github.com/vllm-project/tpu-inference/pull/3073)). | The release support matrix lists **Qwen3.5-397B-A17B passing** unit, accuracy and benchmark tests on v7x. **Qwen3.5-9B is listed "untested"** on v6e and v7x. 27B is not listed. | Pooling exists (Qwen3-Embedding-8B passes), but "Step Pooling (Embedding)" and "hybrid kv cache" are untested in the feature matrix. Out-of-tree models are supported. There are open hybrid-cache bugs: [#3483](https://github.com/vllm-project/tpu-inference/issues/3483) (9.7 MB/token KV estimate) and [#3541](https://github.com/vllm-project/tpu-inference/issues/3541) (engine init fails on non-uniform hybrid groups). | Apache-2.0 |
| **[SGL-JAX](https://github.com/sgl-project/sglang-jax)** (SGLang's TPU engine) | Active (pushed 2026-10-12) | **Yes, natively in JAX.** `models/qwen3_5.py` with a `HybridLinearAttnBackend` / `GDNAttnBackend`. Its GDN kernel is a vendored snapshot of tpu-inference `gdn/v3` (`kernels/gdn/fused_chunk_parallel/PROVENANCE.md`), plus a pure-JAX chunked fallback. | **Qwen3.8-27B on v6e-4 (TP 4, BF16, text only): a published, validated recipe** ([cookbook](https://github.com/sgl-project/sglang-jax/blob/main/docs/cookbook/autoregressive/Qwen/Qwen3.8.md)). Its test suite covers Qwen3.5-27B, Qwen3.5-35B-A3B and Qwen3.8-27B. Flash's 9B is the same class but not named. | `--is-embedding` and `return_hidden_states` exist. Hybrid models need `--disable-radix-cache` or the unified radix tree. The recipe's benchmark mixes generation (512 in / 128 out: 1,567 input tok/s on 4 chips), so it is a working proof, not a prefill throughput figure. | Apache-2.0 |
| **[MaxText](https://github.com/AI-Hypercomputer/maxtext)** | Active; Qwen3-Next added 2026-03-05 ([supported models](https://maxtext.readthedocs.io/en/latest/reference/models/supported_models_and_architectures.html)) | **Yes, in pure JAX.** `jax_chunk_gated_delta_rule` and a naive version in `models/qwen3.py`. No Pallas GDN kernel. | **Ships `configs/models/qwen3.5-9b.yml`**, plus 35B-A3B, 397B and qwen3-next-80b, with HF checkpoint converters. | It is a training framework, and inference goes through a vLLM adapter. It is the best place to get a clean JAX **reference** of the backbone and to fine-tune Clef later. AQT is built in. | Apache-2.0 |
| [JetStream](https://github.com/AI-Hypercomputer/JetStream) | Last push 2026-01-05; Google's TPU serving docs now point to vLLM TPU | No hybrid support found | — | — | Apache-2.0 |
| [Optimum TPU](https://github.com/huggingface/optimum-tpu) | **Archived** (2026-01) | No | — | — | Apache-2.0 |
| Vertex AI Model Garden, vLLM TPU ([doc](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/open-models/vllm/use-vllm-tpu)) | Managed tpu-inference | The one-click list names Qwen3 32B/8B/4B. No Qwen3.5 or Qwen3-Next model card found. | — | A managed endpoint cannot run our head, so it is no use to us beyond the engine it wraps | — |

Google's own write-up on serving Qwen3.5-397B on Ironwood
([Google Developers Blog, 2026-07-14](https://developers.googleblog.com/systems-engineering-playbook-optimizing-qwen-35-397b-moe-on-ironwood-tpu7x/))
uses tpu-inference with the fused GDN kernel. In an 8k-in / 1k-out test at
concurrency 64 it reports **3,707 tokens/s/chip, 82 % of its own discounted
roofline** (measured, FP8, MoE, not our workload). That confirms that GDN
hybrid prefill on TPU can run near roofline with these kernels.

A [vLLM forum thread from August 2026](https://discuss.vllm.ai/t/support-for-qwen3-6-27b-35b-a3b-hybrid-gated-deltanet-models-on-tpu-v5e-8-with-vllm-and-sglang/2875)
says there is no TPU GDN kernel. That claim is out of date: it was answered
by a bot and predates the kernel work cited above.

## 2. Kernels for the gated delta rule and attention

| Kernel | What it does | Published speed (measured) | Fit for Clef |
| --- | --- | --- | --- |
| **tpu-inference `gdn/v3`** ([code](https://github.com/vllm-project/tpu-inference/tree/main/tpu_inference/kernels/gdn/v3)) | Fused causal Conv1D plus a **chunked WY-form** gated delta rule in Pallas. Ragged batches, one path for prefill and decode, configurable state dtype (f32 carry in VMEM). | From [PR #3605](https://github.com/vllm-project/tpu-inference/pull/3605) on **v6e-8**, with Qwen3.5-397B's GDN heads (16 key / 64 value, dim 128): **1×8,192-token prefill 0.66 ms at TP 4** (16 value heads per chip) and 0.44 ms at TP 8. Scaled to Flash's 32 value heads on one chip, this is about **1.3 ms per 8k tokens per layer**, or **~4 µs per token across all 24 GDN layers** (estimate). | Best kernel available. Coupled to vLLM's ragged metadata. SGL-JAX's `fused_chunk_parallel_adapter.py` shows how to drive it outside vLLM. |
| **[deltanet-tpu](https://pypi.org/project/deltanet-tpu/)** 0.2.1 (2026-10-12, [repo](https://github.com/maelie-labs/deltanet-tpu)) | Standalone Pallas chunked GDN, GDN-2 and KDA kernels, forward and backward, prefill and decode, segment resets. Brand new, with no users yet. | **v6e, 1 chip, 8,192 tokens, the exact Flash GDN shape (16 key, 32 value, dim 128): 11.7 ms, 700k tok/s per layer.** That is about 34 µs per token over 24 layers. On v5e: 516k tok/s per layer. | Easy to call, but about 9× slower than `gdn/v3` on our shape: Flash's per-value-head gates force q/k repetition. Useful as a second implementation for checking correctness. |
| MaxText `jax_chunk_gated_delta_rule` | Pure-JAX chunked GDN, no Pallas | Not published. deltanet-tpu's own pure-JAX fallback is about 0.55× its Pallas kernel. | The **reference** for our parity ladder |
| [flash-linear-attention](https://github.com/fla-org/flash-linear-attention) | The original chunked GDN algorithms, in Triton for GPUs, MIT | n/a (GPU) | Algorithm source for everything above. It does not run on TPU. |
| tpu-inference ragged paged attention v3 ([code](https://github.com/vllm-project/tpu-inference/tree/main/tpu_inference/kernels/ragged_paged_attention/v3)) | Paged causal attention for mixed prefill and decode. head_dim 256 is covered: it serves Qwen3.5-397B. | Used in the Ironwood numbers above | More than we need. With fixed-chunk prefill and no paging, JAX's built-in Pallas flash/splash attention (`jax.experimental.pallas.ops.tpu`) is enough. Attention is about 6 % of Flash's FLOPs at 16k context and under 1 % at 1.5k. |

What this means for speed: at 40 % of v6e's bf16 peak, Flash's matmuls take
about 46 µs per token (spec estimate). With `gdn/v3`, the DeltaNet core adds
about 4 µs, under 10 %, so **the scan is not the bottleneck** that
clef-tpu.md feared. With deltanet-tpu it adds about 34 µs, cutting
throughput by roughly 40 %.

## 3. Low-precision weights and activations

| Chip | Native low precision in the MXU (from tpu-inference's [hardware matrix](https://github.com/vllm-project/tpu-inference/blob/main/support_matrices/release/hardware_acceleration_and_dtype_support_matrix.csv)) | Peak vs bf16 | Supported checkpoint formats in tpu-inference |
| --- | --- | --- | --- |
| v5e | int8, int4 | int8 393 vs 197 TFLOPS (2×) | INT8 W8A8, INT4 W4A16 (AWQ). Status "untested" in the matrix. |
| v6e | int8, int4 | int8 1,836 vs 918 (2×) | Same |
| v7x (Ironwood) | **fp8 (e4m3, e5m2)**, no int8 listed | fp8 4,614 vs about 2,307 bf16 (2×) | FP8 W8A8/W8A16, MXFP4, NVFP4 W4A16 |

- **Tooling.** [Qwix](https://github.com/google/qwix) (Apache-2.0) is
  Google's current JAX quantization library and supersedes
  [AQT](https://github.com/google/aqt) for new work. It does post-training
  quantization of any Flax model with one call: native int4, int8 and fp8,
  per-channel or sub-channel, weight-only or dynamic-range (weights and
  activations). tpu-inference ships Qwix configs such as `int8_default.yaml`
  (W8A8 on every module), so the path is proven.
- **What speeds up prefill.** Prefill is compute-bound, so **weight-only
  int8 (W8A16) saves HBM but does not make prefill faster.** Only W8A8
  (int8 on v5e/v6e, fp8 on v7x) uses the 2× MXU rate. No published TPU
  measurement of W8A8 prefill speedup was found. We expect **about 1.4–1.7×**
  on the matmul part (an estimate: the 2× peak, less quantize overhead, with
  the GDN core, norms and head staying bf16).
- **int4** only saves memory on TPU; there is no int4 MXU rate to gain for
  prefill. It matters only to fit 27B on one v6e chip.
- **Accuracy.** W8A8 must pass the same ladder (top answer ≥ 97 %,
  median |Δp| ≤ 0.02) and gets its own calibration maps, as the owner
  accepted on 2026-10-12.

## 4. Prices, availability and what fits

Per chip-hour, from the Cloud Billing catalog (Compute Engine SKUs, read
2026-10-12 with the automation account):

| Chip | On-demand | Spot | 1-yr / 3-yr commitment | Zones in our project |
| --- | ---: | ---: | ---: | --- |
| v5e | $1.20 | **$0.359** Americas; $0.57 us-east5; $0.79 us-south1; $0.49 us-east4 | $0.84 / $0.54 | us-east5-a/b, us-central1-a, us-south1-a, us-west1-c, us-west4-a |
| **v6e** | $2.70 | **$0.584** Americas (us-central1, us-east1); **$0.243 us-east4**; $0.66 us-south1; $1.62 us-east5 | $1.89 / $1.22 | us-central1-a/b/c, us-east1-d, **us-east4-a**, us-east5-a/b, us-south1-a, us-west1-c |
| v5p | $4.20 | $0.93 Americas; $0.36 us-east4 | $2.94 / $1.89 | us-central1-a, us-east5-a/b, us-east1-d, us-south1-a |
| TPU7x (Ironwood) | $12.00 | $4.12 Americas; $4.80 us-east5 | $8.40 / $5.40; DWS 6.00 | Compute Engine machine types `tpu7x-standard-1t` and `-4t` in us-central1-b/c, us-east1-b, us-east5-a. Not offered as a TPU VM accelerator type in our zones. |

Quotas (Cloud Quotas API, read 2026-10-12):

- **v6e spot.** Compute Engine `PREEMPTIBLE-TPU-V6E` is 1,536 chips in
  us-east1 and us-south1 and has no override in us-east4 or us-east5. The TPU
  API v6e quotas read "no limit" (clef-tpu.md).
- **v5e spot.** `PREEMPTIBLE_TPU_LITE_PODSLICE_V5` is 1,536 in us-east5,
  us-east4 and us-south1.
- **TPU7x.** `PREEMPTIBLE-TPU7X` has no value set in any region, so assume
  zero and **file a quota request before planning Ironwood**. Quota steps:
  Console → IAM & Admin → Quotas → filter "TPU7X" → Edit → region. Grants
  usually take a few days.
- **Capacity, not quota, is the practical limit**, especially for spot
  (clef-tpu.md). Queued resources are the way to wait for capacity.

What fits (HBM per chip: v5e 16 GB, v6e 32 GB, v5p 95 GB, v7x 192 GB):

| Model | bf16 | int8 | Best shape |
| --- | --- | --- | --- |
| Clef-Flash (~18 GB bf16) | v6e-1 (14 GB left for batch state); v7x-1 | v5e-1 (9 GB in 16 GB, small batches); v6e-1 with big batches | **v6e-1** |
| Clef 27B (~54 GB bf16) | v6e-4 TP 4 (SGL-JAX's validated shape); v5p-4; **v7x-1** (one chip, no tensor parallelism) | v6e-1 (27 GB in 32 GB: tight, small batch, about 1 GB K/V at 16k); v7x-1 fp8 | **v6e-4 bf16** first, then v6e-1 int8 if the ladder passes |

## 5. Cost per million tokens (estimates)

Method: about 1.7e10 FLOP per Flash token and 5.4e10 per 27B token
(clef-tpu.md), at 40 % of peak, plus the DeltaNet core time from section 2.
These are full-busy chip figures; at 50 % utilization, double them.

| Setup | Flash tok/s per chip | Spot $/M | On-demand $/M | 3-yr commitment $/M |
| --- | ---: | ---: | ---: | ---: |
| v6e bf16 + `gdn/v3` | ~20,000 | **0.0081** (us-east4: **0.0034**) | 0.038 | 0.017 |
| v6e bf16 + deltanet-tpu | ~12,500 | 0.013 (us-east4: 0.0054) | 0.060 | 0.027 |
| v6e bf16, pessimistic (25 % MXU, slow scan) | ~9,300 | 0.017 | 0.081 | 0.036 |
| v6e W8A8 int8 + `gdn/v3` | ~28,000 | 0.0058 (us-east4: 0.0024) | 0.027 | 0.012 |
| v5e int8 + `gdn/v3` | ~8,300 | 0.012 | 0.040 | 0.018 |
| v7x bf16 | ~49,000 | 0.023 | 0.068 | 0.031 |
| v7x fp8 | ~75,000 | 0.015 | 0.044 | 0.020 |
| *Measured:* H100 spot, Psionic Q4 | 15,000–18,000 | 0.10–0.11 | 0.20 | — |
| *Measured:* A100 spot, Psionic Q4 | 8,500 | 0.072 | 0.120 | — |
| *Reference:* Jev's price | — | 0.042 | — | — |

Clef 27B: v6e-4 bf16 runs at about 5,500–6,800 tok/s per chip after tensor-parallel overhead, or **$0.024–0.030/M on spot**
($0.11–0.14 on-demand). v6e-1 int8 runs at about 10,000 tok/s, **$0.016/M on spot**.
v7x-1 bf16 runs at about 17,000 tok/s, $0.067/M on spot. All of these
are far under our H100 ($0.33/M) and Workers AI ($0.24/M).

The pattern: **spot v6e beats Jev's $0.042 by 2.5× (pessimistic) to 12× (us-east4 with
`gdn/v3`).** On-demand v6e reaches it only with a fast GDN kernel or int8.
Ironwood never beats v6e per Flash token.

## 6. Recommendation

**Fastest path to a correct Clef-Flash prefill: path A, built from existing
parts.** That means our own small JAX worker, not adopting a serving engine.

1. **Why not adopt vLLM TPU or SGL-JAX whole (path C)?** Both are generation
   engines. Their schedulers, paged KV and hybrid state pools, sampling and
   decode do not help a prefill-only decision. Their hybrid-cache paths are
   where the open bugs are (#3483, #3541, and radix-cache limits in SGL-JAX).
   Clef needs every token's `LN(H)·W_mem` and span sums, not one pooled
   vector. Pushing a 4,096-wide hidden row per token out through a
   pooling/hidden-states API would cost about 8 KB a token in transfer; doing
   the head inside the engine means forking it.
2. **What to borrow** (all Apache-2.0; keep the license headers and add a
   PROVENANCE note, as SGL-JAX does):
   - backbone definition and HF weight loading: SGL-JAX `models/qwen3_5.py`
     (it already loads `Qwen3_5ForConditionalGeneration` and skips the
     vision tower and MTP);
   - DeltaNet: the tpu-inference `gdn/v3` kernel, driven like SGL-JAX's
     `fused_chunk_parallel_adapter.py`, with **f32 recurrent state** (the
     kernel's state dtype is configurable; the Ironwood blog moved state to
     bf16 for speed, which we should not do before the ladder says so);
   - full attention: JAX's Pallas flash/splash attention with a fixed
     2,048-token chunk and causal masking;
   - reference: MaxText's pure-JAX `qwen3.5-9b` and HF torch f32 for the
     per-layer cosine rung; deltanet-tpu as a second, independent GDN kernel
     to compare against;
   - int8: Qwix PTQ (`int8_default`-style W8A8) in P3.
3. **A same-day smoke test for the P0/P1 owner** (not run here). Start
   SGL-JAX on one v6e-1 with `--model-path Cloudflare/clef-flash`
   (text-only, `--disable-radix-cache`) and `return_hidden_states`, then
   compare the final hidden rows on the 155-token parity record
   (`crates/psionic/fixtures/clef/`) against the HF reference. If they match,
   the backbone rung is known good before any of our code exists.
4. **Path D stays open.** rvLLM's TPU port (`projects/repos/rvllm/tpu`,
   `crates/rvllm-xla`) is a worked example of exporting StableHLO from JAX
   and running it from Rust over PJRT. Pallas kernels travel as custom calls,
   so the `gdn/v3` kernel would have to export cleanly. Check that before
   committing to D.

**Estimated timeline:** a correct Flash on v6e-1 in about 1–2 weeks
(versus 2–4 in clef-tpu.md), and P2 speed mostly from batching and compile
shapes rather than kernel writing.

**Risks:**

| Risk | Why | Mitigation |
| --- | --- | --- |
| Kernel coupling | `gdn/v3` expects vLLM-style ragged metadata and state slots | Copy SGL-JAX's adapter; pin a commit |
| Untested shapes | Neither stack names the 9B; the kernel is tuned on 397B heads (16/64), not Flash's 16/32 or 27B's 16/48 | P0 kernel microbenchmark at our shapes; deltanet-tpu as a cross-check |
| Numerics | bf16 state or activations move probabilities against calibrated thresholds | f32 state; bf16-vs-f32 ladder rung; separate calibration maps for TPU |
| Fast-moving upstream | tpu-inference releases weekly; SGL-JAX changes daily | Vendor the kernel at a pinned commit, with no live dependency |
| Spot capacity | us-east4 is cheapest but small; us-east5 spot v6e is $1.62 | Restarter across us-east4, us-central1, us-east1 and us-south1; queued resources |
| Ironwood quota | `PREEMPTIBLE-TPU7X` has no value set | File the request only if 27B on one chip is needed |
| No confidential computing | TPUs have no CC mode | Sealed decisions stay on H100 CC (as specified) |

## Sources

- tpu-inference: [repo](https://github.com/vllm-project/tpu-inference),
  [releases](https://github.com/vllm-project/tpu-inference/releases),
  [model support (README)](https://github.com/vllm-project/tpu-inference#readme),
  [`gdn/v3`](https://github.com/vllm-project/tpu-inference/tree/main/tpu_inference/kernels/gdn/v3),
  PRs [#3016](https://github.com/vllm-project/tpu-inference/pull/3016),
  [#3605](https://github.com/vllm-project/tpu-inference/pull/3605),
  [#3073](https://github.com/vllm-project/tpu-inference/pull/3073); issues
  [#3483](https://github.com/vllm-project/tpu-inference/issues/3483),
  [#3541](https://github.com/vllm-project/tpu-inference/issues/3541). Read at
  commit of 2026-10-12.
- SGL-JAX: [repo](https://github.com/sgl-project/sglang-jax),
  [Qwen3.8-27B recipe](https://github.com/sgl-project/sglang-jax/blob/main/docs/cookbook/autoregressive/Qwen/Qwen3.8.md).
- MaxText: [repo](https://github.com/AI-Hypercomputer/maxtext),
  [supported models](https://maxtext.readthedocs.io/en/latest/reference/models/supported_models_and_architectures.html).
- [deltanet-tpu on PyPI](https://pypi.org/project/deltanet-tpu/) and
  [repo](https://github.com/maelie-labs/deltanet-tpu).
- [Google Developers Blog: Qwen 3.5-397B on Ironwood](https://developers.googleblog.com/systems-engineering-playbook-optimizing-qwen-35-397b-moe-on-ironwood-tpu7x/).
- [vLLM forum: Qwen3.6 hybrid on TPU v5e-8](https://discuss.vllm.ai/t/support-for-qwen3-6-27b-35b-a3b-hybrid-gated-deltanet-models-on-tpu-v5e-8-with-vllm-and-sglang/2875).
- [Vertex AI: vLLM TPU in Model Garden](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/open-models/vllm/use-vllm-tpu).
- [Qwix](https://github.com/google/qwix), [AQT](https://github.com/google/aqt),
  [JetStream](https://github.com/AI-Hypercomputer/JetStream),
  [Optimum TPU](https://github.com/huggingface/optimum-tpu),
  [flash-linear-attention](https://github.com/fla-org/flash-linear-attention).
- [Cloud TPU pricing](https://cloud.google.com/tpu/pricing) and the Cloud
  Billing Catalog API (Compute Engine service `6F81-5844-456A`, TPU SKUs),
  read 2026-10-12.
- Checkpoint configs:
  [Cloudflare/clef-flash](https://huggingface.co/Cloudflare/clef-flash),
  [Cloudflare/clef](https://huggingface.co/Cloudflare/clef) (Apache-2.0).
- Local reference: `~/work/projects/repos/rvllm/tpu` (JAX → StableHLO →
  Rust/PJRT).
