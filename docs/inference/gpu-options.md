# Cloud GPU options for Clef decisions

2026-10-12. Whether another kind of Google Cloud compute we have quota for
serves Clef decisions more cheaply per token than a spot H100, or makes a
better always-on machine. Measured on an A100 40GB and an L4, priced from
Google's catalog the same day, with a paper estimate for TPU v5e.
Tracking: [#11446](https://github.com/OpenAgentsInc/openagents/issues/11446).
Break-even rule and market prices: [decision-profitability.md](decision-profitability.md).

## Answer

- **Always-on decisions server: an on-demand A100 40GB (`a2-highgpu-1g`).**
  - It costs $3.67/h, against $6.08/h for the spot H100 we run today.
    It is never preempted, and we have quota for 16 of them in each US
    region.
  - It serves about 55 % of the H100's tokens per second, so its cost per
    token ($0.120 per million at full use) is within 7 % of the spot
    H100's ($0.112).
  - Latency is about twice the H100's and close to the RTX 4080's: 0.22 s
    p50 at 1.5k tokens, 1.9 s at 16k (Flash); 0.62 s and 5.9 s (27B).
  - With a 1-year commitment it is $2.31/h ($0.076 per million). That is
    spot-A100 money without preemption, and cheaper per token than a
    spot H100.
- **Bulk work in the cloud: a spot A100**, at $0.072 per million Flash
  tokens. That is 36 % less than the spot H100, and 27B at Workers AI's
  $0.24 per million breaks even from about 88 % busy.
  - The spot L4 is cheaper per token ($0.062 per million) but has a
    quarter of the A100's speed and 3–4× its latency. Spot L4s also ran
    out in all three us-central1 zones we tried.
  - Use an L4 only for deferrable batch work where latency doesn't matter.
- **None of these GPUs reaches Jev's $0.042 per million for Flash at
  realistic use.** The only cloud row that touches it is an A100 on a
  3-year commitment at 100 % busy ($0.042).
- **Owned consumer GPUs (Pylons) are still 14–20× cheaper per token than
  any cloud row.**
- **TPU v5e looks cheapest on paper** (roughly $0.01–0.03 per million on
  spot), but it is unmeasured and needs a full JAX port. See the TPU
  section.
- **The H100 kernels (#11443) could change the ranking.** At about 60 % of
  the H100's f16 peak (about 33k tokens/s), a spot H100 costs about $0.05
  per million and beats the spot A100 again. Today's Psionic kernels get
  more of the A100's peak (about 48 %) than of the H100's (about a third).

## Comparison

**How to read it.** Tokens/s is input tokens processed per second, the unit
we are paid in.

- **Best 1.5k × 1q:** at one or four clients.
- **4k × 13q:** the batched bulk shape. All the $ columns use it, because
  it is closest to each machine's peak.
- **16k prefill:** `bench_latency.py` on the 15.5k-token prompt.
- **Decisions/s:** normalized to the H100 run's request sizes (1,481
  tokens for 1q, 4,956 for 13q), because this run's corpus build gave
  10–15 % longer prompts at the same labels (see Method).
- **$ / 1,000 decisions:** spot price, fully busy.
- **Revenue/h:** at 100 % busy.

| Machine | Model | Tokens/s, best 1.5k × 1q | Tokens/s, 4k × 13q | Tokens/s, 16k prefill | $ / 1M tokens, spot | on-demand | 1-yr commit | 3-yr commit | spot at 50 % busy | Decisions/s, 1q | Decisions/s, 13q | $ / 1,000 decisions, 1q (spot) | 13q (spot) | Revenue/h at $0.042/M | at $0.24/M | $/h spot / on-demand |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| H100 80GB (`a3-highgpu-1g`) | Flash | 14,627 | 15,036 | 18,400 | $0.112 | $0.203 | $0.141 | $0.089 | $0.225 | 9.9 | 39.4 | $0.171 | $0.0428 | $2.27 | | $6.08 / $10.98 |
| H100 80GB | 27B | 5,666 | 5,149 | 5,950 | $0.328 | $0.592 | $0.411 | $0.260 | $0.656 | 3.8 | 13.5 | $0.441 | $0.1250 | $0.78 | $4.45 | $6.08 / $10.98 |
| **A100 40GB (`a2-highgpu-1g`)** | Flash | 7,941 | 8,472 | 8,326 | **$0.072** | **$0.120** | $0.076 | $0.042 | $0.145 | 5.4 | 22.2 | $0.114 | $0.0275 | $1.28 | | $2.20 / $3.67 |
| **A100 40GB** | 27B | 2,792 | 2,896 | 2,626 | **$0.211** | $0.352 | $0.222 | $0.123 | $0.423 | 1.9 | 7.6 | $0.325 | $0.0806 | $0.44 | **$2.50** | $2.20 / $3.67 |
| L4 24GB (`g2-standard-8`) | Flash | 2,249 | 2,295 | 2,413 | $0.062 | $0.103 | $0.065 | $0.046 | $0.124 | 1.5 | 6.0 | $0.093 | $0.0236 | $0.35 | | $0.51 / $0.85 |
| L4 24GB | 27B (max 16,384 tokens) | 626 | 653 | 623 | $0.218 | $0.364 | $0.229 | $0.163 | $0.435 | 0.4 | 1.7 | $0.336 | $0.0829 | $0.10 | $0.56 | $0.51 / $0.85 |
| RTX 4080 (owned, CoderOS) | Flash | 6,237 | 6,127 | 6,254 | $0.005 | — | — | — | $0.011 | 4.2 | 16.1 | $0.008 | $0.0021 | $0.93 | | $0.12 (assumed) |

**Break-even** (the profitability rule, at 100 % busy):

| Machine and price | Earns at $0.042/M (Flash) | Cost per hour | Ratio |
| --- | ---: | ---: | ---: |
| H100, spot | $2.27/h | $6.08 | 0.37 |
| A100, spot | $1.28/h | $2.20 | 0.58 |
| A100, on-demand | $1.28/h | $3.67 | 0.35 |
| A100, 3-year commitment | $1.28/h | $1.29 | 0.99 |
| L4, spot | $0.35/h | $0.51 | 0.68 |
| 27B at $0.24/M: H100 spot | $4.45/h | $6.08 | 0.73 |
| 27B at $0.24/M: A100 spot | $2.50/h | $2.20 | **1.14** |
| 27B at $0.24/M: A100 1-year commitment | $2.50/h | $2.31 | **1.08** |

At 50 % busy, every ratio halves.

**Latency, one request at a time** (`bench_latency.py`, median of 9, fresh
nonce each run; Psionic serves one decision at a time, so this is also
the time per decision in the queue):

| Prompt | H100 Flash | A100 Flash | L4 Flash | 4080 Flash | H100 27B | A100 27B | L4 27B |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1,080 tokens | 0.074 s | 0.149 s | 0.491 s | 0.194 s | 0.212 s | 0.415 s | 1.718 s |
| 3,917 tokens | 0.210 s | 0.458 s | 1.566 s | 0.596 s | 0.611 s | 1.309 s | 5.817 s |
| 15,511 tokens | 0.843 s | 1.863 s | 6.426 s | 2.48 s | 2.607 s | 5.908 s | 24.90 s |

H100 and 4080 cells are from [clef-native.md](clef-native.md) ("H100"
and the M2 gate session).

**Harness points from this run** (75 s each; decisions/s as measured, at
this run's request sizes):

| Point | A100 Flash | A100 27B | L4 Flash | L4 27B |
| --- | --- | --- | --- | --- |
| c1, 1.5k × 1q (1.7k measured) | 4.63/s, p50 0.22 s | 1.64/s, p50 0.62 s | 1.33/s, p50 0.75 s | 0.37/s, p50 2.69 s |
| c4, 1.5k × 1q | 4.66/s, p50 0.86 s | 1.64/s, p50 2.44 s | 1.29/s, p50 3.09 s | — |
| 0.5k × 1q (0.7k) | 6.54/s | — | 2.96/s | — |
| 4k × 1q (4.2k) | 1.91/s | 0.68/s | 0.56/s | 0.16/s |
| 16k × 1q (15.8k) | 0.53/s, p50 7.6 s at c4 | 0.17/s | 0.14/s | 0.04/s (3 refused over 16,384 tokens) |
| 4k × 13q (5.5k) | 19.99/s | 6.84/s | 5.42/s | 1.54/s |
| router (8.7k) | 4.13/s | — | 1.14/s | 0.32/s |
| GPU memory, max | 7.3 GB of 40 | 20.5 GB of 40 | 7.1 GB of 23 | 20.3 GB of 23 |

Raw results: `gs://openagentsgemini-decision-throughput/clef-explore/runs/2026-10-12.jsonl`
and, with logs, `clef-explore/20261012-gpu/{a100,l4}/` and
`clef-explore/20261012-gpu-l4b/l4/`.

## Parity

Clef-Flash answers to all 40 of `crates/psionic/fixtures/clef/e2e/requests.jsonl`,
compared with the CPU-lane goldens (`psionic-cpu-q4_k_m.jsonl`) by
`e2e.py compare`:

| GPU | Questions | Same top answer | Max \|Δp\| | Median \|Δp\| | p99 \|Δp\| |
| --- | ---: | ---: | ---: | ---: | ---: |
| A100 (sm_80) | 104 | 100 % | 0.0049 | 0.00018 | 0.0021 |
| L4 (sm_89) | 104 | 100 % | 0.0049 | 0.00018 | 0.0019 |
| H100 (for reference, 2026-10-10) | | 100 % | 0.0022 | 0.00013 | |

27B parity was not rerun here. It is the same kernels and has been
checked on the H100.

## Availability and quota

Quota is from `gcloud compute regions describe`, 2026-10-12.

| | A100 40GB | L4 | H100 |
| --- | --- | --- | --- |
| On-demand quota | 16 GPUs in each of us-central1, us-east1, us-east4, us-east5, us-west1, us-west4, europe-west4, asia-southeast1 | 16 in each of the same | none (spot only) |
| Spot quota | 64 per region | 16 per region | as used today |
| Zones with the machine type | `a2-highgpu-1g`: us-central1-a/b/c/f, us-east1-b, us-west1-b, us-west3-b, us-west4-b, europe-west4-a/b, asia (none in us-east5, despite the quota) | `g2-standard-*`: us-central1-a/b/c, us-east1-b/c/d, us-east4-a/c, us-west1-a/b/c, us-west4-a/c, Europe, Asia (none in us-east5) | us-east5-a, us-central1-a, europe-west4-c |
| What we got today | Spot, us-central1-a, first try | Spot: no capacity in us-central1-a, -b, -c; got us-east1-b (twice) | (spot, as the docs record) |

- **On-demand A100 is the only data-center GPU here we can run without
  preemption without a quota request.**
- On-demand availability itself was not tested: we created no on-demand
  VM.

## Prices

All figures are Google's public list prices from the Cloud Billing
Catalog, read 2026-10-12. The SKUs' `effectiveTime` is
2026-10-12T07:00Z.

- Source: `https://cloudbilling.googleapis.com/v1/services/6F81-5844-456A/skus`
  (Compute Engine). The same numbers are published on
  [GPU pricing](https://cloud.google.com/compute/gpus-pricing),
  [VM instance pricing](https://cloud.google.com/compute/vm-instance-pricing)
  and [TPU pricing](https://cloud.google.com/tpu/pricing). Those pages
  render their tables with JavaScript, so we read the catalog they are
  generated from.
- How a machine is priced: each whole-machine price is GPU + vCPU × core
  price + GiB × RAM price, all from the same SKU family.
  - `a2-highgpu-1g`: 12 vCPU, 85 GB.
  - `g2-standard-8`: 8 vCPU, 32 GB.
  - `a3-highgpu-1g`: 26 vCPU, 234 GB.
- The commitment prices are resource-based CUDs (GPU, A2/G2/A3 core and
  RAM commitments).

| Machine, region | Spot | On-demand | 1-year commit | 3-year commit |
| --- | ---: | ---: | ---: | ---: |
| `a2-highgpu-1g`, us-central1 (A100 GPU $1.7603 / $2.9339 / $1.8484 / $1.0269) | $2.204 | $3.673 | $2.314 | $1.286 |
| `g2-standard-8`, us-central1 and us-east1 (L4 GPU $0.3352 / $0.5600 / $0.3528 / $0.2520) | $0.511 | $0.854 | $0.538 | $0.384 |
| `a3-highgpu-1g`, us-east5 (H100 GPU spot $5.4212) | $6.075 | — | — | — |
| `a3-highgpu-1g`, us-central1 (H100 GPU $5.8485 / $9.7966 / $6.7988 / $4.2999) | $6.555 | $10.98 | $7.62 | $4.82 |
| TPU v5e, per chip-hour, us-central1 | $0.359 | $1.20 | $0.84 | $0.54 |
| TPU v5e, per chip-hour, us-east5 | $0.570 | $1.20 | $0.84 | $0.54 |

**Notes on the prices.**

- Spot prices move. The docs' $6.40/h for the spot H100 is about 5 %
  above today's $6.08.
- us-east5 lists a spot L4 at $0.057/h, but offers no G2 machines, so we
  can't use it.

## TPU v5e (paper only, unmeasured)

We have quota for TPU v5e: 512 chips on demand and 1,536 preemptible in
us-central1, us-east4, us-east5, us-west4 and europe-west4. On paper it is
the cheapest per token. Nothing below was run.

**What the chip offers** (Google's v5e specification):

- 197 TFLOPS bf16 and 394 TOPS int8 per chip.
- 16 GB HBM2e at 819 GB/s.

**What one Clef token costs in compute.** Clef-Flash is a 9B dense
backbone: 32 layers, 24 Gated DeltaNet and 8 full attention
([clef-native.md](clef-native.md), "What Clef is"). Prefill is about 17
GFLOP per token, so the measured A100 rate of 8.4k tokens/s is about
48 % of the A100's 312 TFLOPS.

| Assumption | Tokens/s per chip | $ / 1M tokens, spot (us-central1, $0.36) | on-demand ($1.20) | 3-yr ($0.54) |
| --- | ---: | ---: | ---: | ---: |
| bf16 at 40 % of peak | ~4.6k | ~$0.022 | ~$0.072 | ~$0.033 |
| int8 weights and matmuls at 40 % of peak | ~9k | ~$0.011 | ~$0.037 | ~$0.017 |

On these assumptions, one v5e chip is about half an A100 at a sixth of
the spot price.

**What a port would take.** Psionic has no XLA or TPU backend; its Clef
lane is CUDA, Metal and CPU.

1. **Backbone and head in JAX.** Qwen3.5's gated full attention (partial
   rotary, MRoPE sections, sigmoid output gate) and the Clef joint head.
2. **Gated DeltaNet as a chunked scan.** Today's CUDA kernel is a
   sequential per-token scan. On TPU it should be the chunked WY/UT form:
   matmuls inside a chunk (good for the MXU) and `lax.scan` across
   chunks, probably as a Pallas kernel for speed.
3. **Weights.** GGUF Q4_K_M has no TPU kernels, so we would need bf16 (18
   GB: two chips, or `v5litepod-4`) or int8 weights through AQT. That
   changes numerics, so parity against the reference would have to be
   re-established.
4. **Static shapes.** XLA compiles once per shape, so prompts must be
   bucketed by length; padding wastes up to a bucket step.
5. **Serving.** A JAX server behind the same `/v1/systemone` contract and
   gateway door.

Rough effort: several engineer-weeks to parity and speed. Worth it only if
cloud volume, not Pylons, carries Flash at Jev's price. Measure one
`v5litepod-1` spot with a minimal JAX prefill before committing.

## Method

**Builds.** `psionic-openai-server` from `origin/main` `661e02d3ed`, built
by Cloud Build in NVIDIA's CUDA 13.0.2 devel image (the
`deploy/att/Dockerfile.gpu` base):

- `CUDAARCHS=80` for the A100 and `CUDAARCHS=89` for the L4;
  `cuobjdump` showed `sm_80` and `sm_89` cubins.
- Bundled with libcudart, libcublas and libcublasLt 13 like the H100
  bundle, at `gs://openagentsgemini-decision-throughput/bin/explore-661e02d3ed-sm{80,89}/`.
- The kernels need nothing newer than Ampere (`cp.async`, `mma.sync`
  m16n8k16).
- The one-off build recipe was not committed. It is the H100 bundle's
  recipe with the CUDA arch changed and the oa-att step dropped.

**VMs.** Spot `a2-highgpu-1g` (us-central1-a) and `g2-standard-8`
(us-east1-b):

- Image: Deep Learning VM `common-cu129-ubuntu-2404-nvidia-580`, driver
  580.178.04.
- Each VM ran from a startup script that pulled the bundle, harness and
  models from the bucket, measured, uploaded, and deleted the VM.
- Termination action DELETE, with a maximum run of 85 minutes.
- One model on the GPU at a time.

**Server and points.**

- Server: `psionic-openai-server --decision-device cuda --decision-chunk
  2048 --decision-max-tokens 65536`, as on the H100.
- On the L4, 27B ran with `--decision-max-tokens 16384`. At 65,536,
  Psionic's own memory estimate asks for 24.75 GB against 23.4 GB free
  (5.5 GB of it is per-request buffers), and the server refuses to load.
  At 16,384 it loads and peaks at 20.3 GB, but prompts over 16,384 tokens
  are refused (413).
- Harness: `scripts/bench/decision-throughput/harness.py --plan full
  --only …`, 75 s a point, with targets `a100-clef-flash`,
  `a100-clef-27b`, `l4-clef-flash` and `l4-clef-27b` in a copy of
  `targets.json`.
- Latency: `crates/psionic/fixtures/clef/tools/bench_latency.py` with
  `RUNS=9` on `docs/audits/2026-10-10-clef-mac-performance/requests/len{1k,4k,16k}.json`.
- Parity: `e2e.py run` and `e2e.py compare`.

**Request sizes.** The bucket corpus used here gave longer prompts than
the H100's 2026-10-11 run at the same labels:

| Label | This run | H100 run |
| --- | ---: | ---: |
| 1.5k | 1,705 | 1,481 |
| 4k × 13q | 5,510 | 4,956 |
| 16k | 15,800 | 8,400 |

So the tables compare tokens per second, and decisions per second are
normalized to the H100 run's sizes.

**Cost of this exploration:** about $5.

| Item | Cost |
| --- | ---: |
| Cloud Build (`E2_HIGHCPU_32`, 55 min) | ~$3.6 |
| A100 spot (25 min) | ~$0.9 |
| Two L4 spot runs (51 min) | ~$0.45 |
| Disks | ~$0.05 |

Every VM deleted itself. `gcloud compute instances list --filter
name~clef-explore` is empty.
