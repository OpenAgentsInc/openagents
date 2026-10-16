# Clef on Cloud TPU: spec and phased plan

2026-10-12. What it takes to serve our Clef decision models
(Clef-Flash 9B, Clef 27B) on Google Cloud TPUs, what it would cost, and the
plan with gates. Throughput figures in the planning sections are estimates
from published chip specifications unless they say measured; the status
section below is measured.
Tracking: [#11446](https://github.com/OpenAgentsInc/openagents/issues/11446).
Context: [decision-profitability.md](decision-profitability.md),
[gpu-options.md](gpu-options.md), [clef-native.md](clef-native.md).
Survey of existing TPU stacks, kernels, quantization and prices:
[clef-tpu-survey.md](clef-tpu-survey.md).

## Status after the speed work (2026-10-13, #11502)

[#11502](https://github.com/OpenAgentsInc/openagents/issues/11502). The live
lane on `p6-asia-northeast1-b` (`X-Decision-Order: tpu`) runs the version
described here. BF16 is still the served precision.

**Speed** (v6e-1, one request at a time, fresh prompts, request tokens/s
including the head; `bench/p2.py`, median of 5):

| Prompt | P2 (#11459) | **BF16 now (live)** | int8 (not served) |
| ---: | ---: | ---: | ---: |
| 1,100 | — | **22,257** | 24,342 |
| 1,536 | 19,789 | **26,736** | 28,741 |
| 4,096 | 20,517 | **27,857** | 34,367 |
| 16,384 | 20,016 | **26,824** | 33,462 |

**Latency through the door on the live machine**
(`fixtures/clef/tools/bench_latency.py`: fresh nonce, median of 9, the
`docs/audits/2026-10-10-clef-mac-performance/requests/len*.json` prompts
used for the H100 rows in [clef-native.md](clef-native.md)):

| Prompt | TPU before | **TPU now** | H100 Psionic Q4 (production, Round 12) | A100 Psionic Q4 (production, Round 12) |
| ---: | ---: | ---: | ---: | ---: |
| 1,082 tokens | 0.12 s (0.10–0.60) | **0.052 s** | 0.055 s | 0.131 s |
| 3,918 tokens | 0.24 s (0.19–0.63) | **0.152 s** | 0.161 s | 0.419 s |
| 15,510 tokens | 1.26 s (0.62–1.29) | **0.597 s** | 0.843 s (10-10 run) | — |

The e2e set through the door: 34 ms median, 64 ms p90 (was 46 / 84).

**Cost per million prompt tokens**, fully busy, at 4k-token prompts:

| Lane | tokens/s | Spot $/M | On-demand $/M |
| --- | ---: | ---: | ---: |
| v6e-1, BF16 now | 27,857 | **0.0058** ($0.584/h, US) | 0.027 ($2.70/h, US); 0.032 ($3.24/h, Tokyo, what P1 pays) |
| v6e-1, int8 | 34,367 | 0.0047 | 0.022; 0.026 |
| v6e-1, P2 (#11459) | 20,517 | 0.0079 | 0.037; 0.044 |
| H100, Psionic Q4 | 15,000–18,000 | 0.10–0.11 | 0.20 |
| A100, Psionic Q4 | 8,500 | 0.072 | 0.120 |

Jev sells at $0.042/M. At 50 % utilization, double every figure.

**Accuracy** (the 40 e2e requests, 104 questions, against
`reference-hf-f32.jsonl`):

| Lane | Top answer | median \|Δp\| | max \|Δp\| |
| --- | ---: | ---: | ---: |
| TPU BF16 now, through the live door | **104/104** | 0.0004 | 0.016 |
| TPU int8 (SmoothQuant + GPTQ, all seven projection families) | 102/104 (98.1 %) | 0.0042 | 0.071 |
| Psionic Q4_K_M on CUDA (production GPU lanes) | 98/104 (94.2 %) | 0.0129 | 0.177 |

What changed, in order of effect:

1. **A fused Pallas delta rule** (`delta.fused`, default `--scan fused`).
   The XLA delta rule was ~34 % of BF16 prefill: 33 ms of a 191 ms 4k
   request went to the 16-wide block solve, whose 16x16 matmuls had 16 in
   the lane dimension and ran at 0.6 TFLOPS. The kernel does one grid step
   per 64-token sub-chunk with all 32 heads batched. It covers the l2
   norms, the cumulative decay, `(I+L)^-1` by forward substitution on the
   VPU in 16-row blocks, the UT transform, intra-chunk attention and the
   state update. The state stays f32 in VMEM, and every matmul is at
   HIGHEST. It reads q/k/v straight from the conv output and writes `o` as
   `[T, Hv dv]` (the head split happens in VMEM by `einshape`), so no
   transposes surround it.
   - Against the token-step f32 reference it is within 1e-6 relative on
     random and near-parallel keys; the XLA path is 1.4e-5 / 4e-5.
   - Time: 0.163 ms per 512-token layer-chunk, against 0.362 for the XLA
     path.
   - This is why our worker trailed SGL-JAX's bf16 (~26k at 4k × 8): SGL-JAX
     runs a fused GDN kernel. We are now ahead of it with one request at a
     time.
   - On a v6e, one small matmul in a Pallas kernel is ~1 µs of latency
     whatever its size or precision, and 32 independent ones cost little
     more. So the kernel batches heads rather than lowering precision. A
     whole-matrix Newton-Schulz inverse, a per-head grid and software
     pipelining across sub-chunks were all slower.
2. **Prefix snapshots at chunk boundaries.**
   - Every chunk call streams all 18 GB of weights (~11 ms on a v6e). A
     prefix miss used to run the prefix and the rest as separate segments,
     an extra call plus dozens of eager copies. It now runs in its normal
     chunks and snapshots the state at the last chunk boundary at or below
     the door's prefix. A miss costs the same as a run without a prefix
     (1,080 tokens: 71 → 49 ms).
   - A hit is bitwise identical to a fresh run (max |Δlogit| 0, against
     0.001–0.003 before), at 3.1–8.7× the speed.
   - The old prefix path also compiled new programs for every distinct
     prefix length, so a fresh ticket paid up to ~0.5 s. That is the bimodal
     "before" latency above. Shapes now depend only on chunk sizes and cache
     buckets.
3. **Chunk plans and dispatch.**
   - A prompt's last chunk takes the smallest of 128/256/512/768 that holds
     the tail.
   - State init, snapshot and restore are one jitted call each (they were
     ~6 ms and ~5 ms of eager ops per request).
4. **Head buckets.** 4 (questions, options) shapes × 4 token buckets from
   2,048 (16 programs, was 48), so every request hits a warmed shape. A cold
   worker warms in ~6 min (was 19). A warm restart, with the persistent
   compile cache, is up in 45–70 s (was under 4 min). The cache can be
   built on any v6e with the same jax/libtpu (0.11.2 / 0.0.48) and copied
   (`gs://openagentsgemini-decision-throughput/tpu/11502/jaxcache-v6e-jax0.11.2.tar`).

**int8 (P3), not served.** Gate: ≥ 97 % top answers and ≥ 1.6× BF16.

- **Accuracy: met.**
  - Per-channel int8 weights with per-token dynamic int8 activations
    (int8 × int8 on the MXU). Head, norms and the GDN gates (`in_ba`) stay
    in bf16/f32.
  - SmoothQuant scales come from the activation maxima of the 160 encoder
    fixture records that the e2e gate does not use, with a per-projection α
    picked from 0.4–0.9 by output error (`bench/calibrate.py --search 40`).
  - Weights are then rounded by GPTQ on those activations
    (`bench/gptq.py`), which cuts the weight-induced output error 5–25×
    against round-to-nearest.
  - 102/104 (98.1 %), median |Δp| 0.0042. Both misses are near ties
    (0.216/0.234 and 0.495/0.505).
  - Earlier steps, for the record:

    | int8 recipe | Top answer | median \|Δp\| |
    | --- | ---: | ---: |
    | naive | 84.6 % | 0.021 |
    | SmoothQuant α 0.5 | 97.1 % | 0.0088 |
    | SmoothQuant, α searched | 92.3 % | 0.0054 |
    | block-Hadamard rotation (4096) | 96.2 % | 0.0037 |

    Raw top-answer agreement swings with the set's ~12 near ties; the
    median is the steadier signal.
  - Where the error comes from, with rotation: int8 weights alone give a
    median of 0.0029, int8 activations alone 0.0017.
  - The artifact (6.5 GB) and calibration maps are in
    `gs://openagentsgemini-decision-throughput/tpu/11502/`. Load with
    `--precision int8` and `CLEF_TPU_INT8_ARTIFACT=<dir>`.
- **Speed: met only against P2.**
  - Against today's BF16: 1.09× / 1.07× / 1.23× / 1.25×.
  - Against the P2 BF16 baseline: 1.45× at 1.5k, 1.68× at 4k, 1.67× at 16k.
  - The fused delta kernel (25 % of int8 time) and the per-token
    quantization passes (10 %) do not shrink with int8.
  - Hadamard rotation is more accurate, but online it costs more than int8
    saves (23.6k at 4k).

  The served lane therefore stays BF16. It is faster than P2 by 1.35× and
  at 104/104.

**Chunk invariance (gate 5): not met.**
- BF16 logits across prefill chunks 2048 / 512 / 64 still differ by up to
  0.028 (two near-tie flips). `bench/invariance.py` follows the
  per-projection inputs layer by layer.
- The first differences are 1-ulp f32 changes in XLA's fused matmul
  epilogues and in attention at different chunk heights; bf16 rounding
  then amplifies them. Plain matmuls are row-invariant.
  - Materializing every matmul input and output (`CLEF_TPU_MM_BARRIER=2`)
    makes layers 0–2 bitwise invariant.
  - The first attention layer still differs, even with a fixed 64-query
    block (`CLEF_TPU_ATTN_QBLOCK=64`) and a two-pass softmax
    (`CLEF_TPU_ATTN_TWO_PASS=1`).
- Prefix reuse no longer depends on this: a snapshot is taken in the same
  chunking a fresh run uses, so hits are exact.

## P1–P3 under #11459 (2026-10-13)

**P1 passed: Clef-Flash in BF16 answers correctly on a v6e-1.** The 40 e2e
fixture requests (104 questions) run through the Rust door
(`psionic-openai-server --decision-worker`, using Psionic's own encoder)
in front of the JAX worker. Against the HF f32 reference
(`crates/psionic/fixtures/clef/e2e/reference-hf-f32.jsonl`) the top answer
agrees on 104/104, with median |Δp| 0.0004 and max 0.012. That is closer
to the reference than our Q4_K_M GPU lanes, which agree on 98/104 against
the same goldens. With f32 matmuls the agreement is still 104/104, median
0.0001 and max 0.0024.

Chunk invariance (gate 5) is not met yet. Comparing chunks 2048, 512 and
64, f32 matmuls differ by up to 0.0022 in logits (no argmax flips) and
bf16 by up to 0.053 (one near-tie flip), against a target of 1e-3. A
server runs one fixed chunk (2048) and repeats are deterministic, so
answers are unaffected; prefix reuse (P3) needs the gate closed first.

**P2: about 20k tokens/s per v6e chip.** Batch 1, fresh prompts, median
of 5 runs, with the stable solver described below. Request time includes
the head:

| Prompt tokens | Prefill chunk | Prefill tok/s | Request tok/s | Request |
| ---: | ---: | ---: | ---: | ---: |
| 1,536 | 768 | 20,134 | 19,789 | 78 ms |
| 4,096 | 512 | 20,757 | 20,517 | 200 ms |
| 16,384 | 512 | 20,185 | 20,016 | 819 ms |

At 20k tok/s a v6e serves Clef-Flash for $0.0081 per million tokens on
spot ($0.016 at 50 % use) and $0.0375 on-demand. Measured spot costs on our
GPUs are $0.11 (H100) and $0.072 (A100); Jev sells at $0.042. Against the
HF f32 reference the e2e set gives 103/104 top answers (one near tie),
median |Δp| 0.0005.

How P2 got from 10.7k to 20k tok/s:

- A matmul-only solve for the UT transform.
- Block-causal online-softmax attention.
- A prefill chunk of 512, or 768 when that pads less.
- The embedding table on the device.
- The cumulative decay computed as a matmul.
- No `split` copies.

The UT solve must stay stable. Inverting the whole 64-wide matrix by
Newton-Schulz (as in tpu-inference's GDN solver) is 2.6× faster than
XLA's triangular solve. On near-parallel keys, though, that inverse has
entries near 2^62, and the delta state diverged to NaN on random prompts
longer than about 1.5k tokens. The e2e set (all prompts ≤ 1,100 tokens)
missed this. The worker now inverts 16-wide diagonal blocks by
Newton-Schulz and substitutes block by block (`CLEF_TPU_SOLVE=block`). On
adversarial keys that is as accurate as the triangular solve, at under
half its cost.

What did not help:

- Cross-request batching (vmap over 2 to 8 requests): prefill is already
  compute-bound.
- A Pallas scan: no faster than `lax.scan`.
- Delta sub-chunks other than 64.
- Any matmul inside the inverse below HIGHEST precision.

**P3 (not passed).** Its gate is ≥ 1.6× P2 with the e2e gates still
holding.

- *int8 (w8a8):* per-channel int8 weights and per-row dynamic int8
  activations, int8 × int8 on the MXU. Measured on the e2e set against
  the reference:

  | int8 projections | Top answer | median \|Δp\| | 1.5k / 16k tok/s |
  | --- | ---: | ---: | ---: |
  | all | 84.6 % | 0.021 | — |
  | gate_up, in_qkv, in_z, qkv | 88.5 % | 0.017 | 23.1k / 24.8k |
  | gate_up, down | 94.2 % | 0.013 | 21.0k / 23.4k |
  | gate_up | 95.2 % | 0.010 | 20.6k / 22.3k |

  None meets the ≥ 97 % accuracy gate, and the fastest is 1.2×, not
  1.6×. Naive per-row activation quantization loses too much on this
  model. The next step is calibrated activation scales (SmoothQuant-style
  migration) or per-group int8, judged by the same gate.
- *Cross-request batching:* no throughput gain (above).
- *Prefix reuse:* works and is exact. The worker snapshots the backbone
  state, K/V and final rows at the door's prefix boundary (system text
  plus state) and restarts from them. A restored run is bitwise identical
  to the run that took the snapshot, and within 0.001–0.003 in logits of
  an unsplit run (chunk-boundary numerics). On a repeated state:

  | Prompt / repeated prefix | Fresh | Restored | Speedup |
  | --- | ---: | ---: | ---: |
  | 1,800 / 1,200 | 103 ms | 64 ms | 1.6× |
  | 4,096 / 3,500 | 200 ms | 69 ms | 2.9× |
  | 8,192 / 7,600 | 401 ms | 77 ms | 5.2× |
  | 16,000 / 15,000 | 818 ms | 76 ms | 10.7× |

  The door forwards `prefix_tokens` and reports the restored tokens as
  `usage.cached_input_tokens`. The worker keeps up to 4 GB of snapshots
  on the device, least recently used first out (`--prefix-budget`).

Chunk invariance (gate 5) is still above 1e-3: up to 0.036 in logits
across chunks 2048, 512 and 64, with no argmax flips.

| Measured on the v6e-1, earlier P1 code (plain JAX, batch 1) | Value |
| --- | --- |
| Request latency through the door, 155 to 1,090 token prompts | 0.18 s median, every prompt one padded 2,048 chunk |
| Prefill tokens/s at 1.5k / 4k / 16k | 9.0k / 10.8k / 10.7k |
| Cold start (load 18 GB to HBM, hash the artifact, compile two shapes) | 71 s worker, then 5 s door |

**Where it runs.** `crates/psionic/clef-tpu/`:

- `clef_tpu/model.py`: the Qwen3.5 backbone with chunked prefill, carrying
  the DeltaNet state, conv tail and K/V cache, plus the joint head on padded
  buckets.
- `clef_tpu/delta.py`: the chunked gated delta rule, with a blocked
  Newton-Schulz UT solve and a JAX or Pallas scan.
- `clef_tpu/server.py`: the localhost worker.
- `bench/`: the P0 bench, the e2e harness, `p2.py` (tokens/s by prompt
  length), `p2_batch.py`, `layer_profile.py` and `prefix_check.py`.
- `scripts/vm.sh`: create and drive TPU VMs.
- `scripts/install-units.sh`: installs the systemd units `clef-tpu-worker`
  and `clef-tpu-door`.

The door is `psionic-openai-server --decision-worker
http://127.0.0.1:8091` (`clef/remote.rs`). It parses, encodes, admits and
answers, and puts the worker's identity in `psionic.worker` (`backend: "tpu"`,
`artifact_family: "bf16-safetensors"`, artifact and head digests).

**Live check through the door (2026-10-13 05:10 UTC).** The 40 e2e
requests give 103/104 top answers, 46 ms median latency and 84 ms p90. A
repeated 8.4k-token state answered a second question with 8,369 tokens
restored from the prefix snapshot, in 0.19 s against 1.23 s fresh.

**Gateway.** `X-Decision-Order: tpu` asks only the pool
`clef_models["clef-flash-tpu"]`. The launcher's default for that pool is
`tpu=http://10.146.0.2:18096`, and the firewall rule `clef-tpu-from-gateway`
plus the network tag `clef-tpu` let the gateway subnets and IAP reach it.
The TPU pool is never part of the default order until P5's shadow passes.
Live on openagents.com since 2026-10-13 05:21 UTC: revision
`coder-web-80f11ebd7b-20261013051852` (gateway image `openagents-stack:312421833e`;
staging smoke 92/0, candidate 51/0, production 59/0). Rollback:
`scripts/deploy/web.sh rollback coder-web-19a2dcd41e-20261012224256`.

```sh
curl -s https://api.openagents.com/v1/systemone -H 'content-type: application/json' -H 'X-Decision-Order: tpu' \
  -d '{"model":"clef-flash","state":{"ticket":"Checkout returns a 500 for every card payment."},"questions":{"urgent":{"type":"noul","instructions":"Is this urgent?"}}}'
# X-Decision-Backend: tpu, psionic.backend "tpu"
```

Five calls after the shift: all answered by the TPU, 0.26–0.33 s end to end
from this Mac (Tokyo round trip included), same answers as the GPU pool on
the sample ticket (urgent 0.969, team `technical`).

### TPU machines we hold

Owner policy (2026-10-12): keep held TPUs. Never delete these, and list
every machine here so none is orphaned.

| Name | Zone | Type | $/h | Purpose |
| --- | --- | --- | ---: | --- |
| `p6-asia-northeast1-b` | asia-northeast1-b | v6e-1, on-demand | 2.70 | Clef-Flash BF16 lane: `clef-tpu-worker` + `clef-tpu-door` on :18096 (internal 10.146.0.2, tag `clef-tpu`); permanent |

Not held: a queued on-demand v6e-4 request (`clef-qr-v6e4`, for 27B in
BF16) was withdrawn when this work stopped. The data agent's
`cd-v6e1-ane1b-d` is recorded in its own brief. #11502 developed on it and
on a second on-demand v6e-1, `tp-dev-ane1b`, which it created and deleted
(2026-10-13 05:58–08:31 UTC, about $8).

On `p6-asia-northeast1-b` the worker runs from `~/oa/clef-tpu` (commit
`60aed54d70`). Earlier trees are kept for rollback: `~/oa/clef-tpu-prev`
(the #11459 code), `~/oa/clef-tpu-1c19` and `~/oa/clef-tpu-d66b`. To roll
back: `sudo systemctl stop clef-tpu-door clef-tpu-worker`, swap the
directory, then `sudo systemctl start clef-tpu-worker clef-tpu-door`.

Capacity on 2026-10-12: no v6e anywhere in the US, spot or on-demand. The
one spot v6e-1 we got (us-east1-d) was preempted within 5 minutes. The
automation account creates TPU VMs only with
`--service-account oa-mvp-automation@…`, and us-east4-a has a v6e quota of 0.

### Remaining work

From #11502 (measured leads, not started):

- **Attention kernel.** Attention is still XLA's online softmax, which is
  f32 elementwise over every (query, key block) pair. That is why prefill
  chunks above 768 lose speed (2k chunks: 24.1k at 4k, 22.0k at 16k), even
  though larger chunks would read the 18 GB of weights fewer times and give
  the MXU taller matmuls. A splash/flash Pallas kernel (head_dim 256, GQA
  16/4) should make 1–2k chunks pay. It would also fix the attention part of
  chunk invariance with a fixed query tile.
- **int8 speed.** An rms-norm + smoothing + per-token-quantize pass fused
  into one kernel (today ~10 % of int8 time). Rotations folded into the
  residual stream (QuaRot-style) instead of online, to get rotation
  accuracy at int8 speed. A shared smoothing for `in_qkv` and `in_z` so they
  quantize once and run as one matmul.
- **Chunk invariance to 1e-3:** a fixed-tile attention kernel (above), then
  the remaining epilogue fusions.
- **The head in one dispatch with the last chunk.** It is 1–3 % of a
  request (request minus prefill: 1.4 ms at 1.1k, 5 ms at 16k), so it is
  left for later.
- **P4 Clef 27B.** The int8 path now works end to end (GPTQ builder,
  artifact loader), so 27B int8 on one v6e (~27 GB) is the next thing to try
  if no v6e-4 appears.
- **P5.** The 24 h shadow beside the H100 and A100, and a spot restarter.
- **Run time in Rust (path D):** StableHLO plus PJRT from the door.

## Why look at TPUs

Per token, our best cloud GPU today is a spot A100 at $0.072 per million
Clef-Flash tokens, and a spot H100 at $0.112 ([gpu-options.md](gpu-options.md)).
Jev sells at $0.042. A TPU v6e chip costs $0.58 an hour on spot in the
Americas with 918 bf16 TFLOPS — about a tenth of an H100's spot price for
roughly the same dense bf16 peak — so TPUs are the one cloud option whose
arithmetic could reach Jev's price with margin. The catch is software:
there is no Psionic, no Q4 kernels, and no Gated DeltaNet kernel on TPU.

## What we have (checked 2026-10-12)

- **Project access.** The Cloud TPU API was off in `openagentsgemini`; it
  is now enabled, and the automation account (`oa-mvp-automation@`) has
  `roles/tpu.admin`.
- **Zones** (`gcloud compute tpus accelerator-types list`):

  | Zone | Types offered |
  | --- | --- |
  | us-east5-a, us-east5-b, us-central1-a, us-south1-a | v5e (`v5litepod`), v5p, v6e |
  | us-central1-b, us-east4-a | v6e |
  | us-east1-d | v5p, v6e |
  | us-west4-a | v5e |

- **Quotas.** The TPU API's per-region chip quotas for v5e serving, v6e and
  v5p (on-demand and preemptible) read `-1` (no project limit set);
  Compute Engine shows `PREEMPTIBLE_TPU_LITE_PODSLICE_V5` 1,536 in us-east5.
  Capacity, not quota, is the practical limit, as with spot GPUs.

## The chips

Specifications from Google's v5e and v6e pages; prices per chip-hour from
the Cloud Billing catalog (Compute Engine SKUs, read 2026-10-12; Americas
multi-region `us-central1,us-east1,us-west1` unless noted).

| | v5e | v6e (Trillium) | v5p |
| --- | ---: | ---: | ---: |
| bf16 peak per chip | 197 TFLOPS | 918 TFLOPS | 459 TFLOPS |
| int8 peak per chip | 393 TOPS | 1,836 TOPS | not listed (FP8 459) |
| HBM per chip | 16 GB, 800 GiB/s | 32 GB, 1,638 GB/s | 95 GiB, 2,765 GB/s |
| Single-host slices | 1, 4, 8 chips | 1, 4, 8 chips | 4+ chips |
| On-demand $/chip-h | 1.20 | 2.70 | 4.20 |
| Spot $/chip-h | 0.36 (us-east5 0.57) | 0.58 (us-east5 1.62, us-east4 0.24) | 0.93 |
| 1-yr / 3-yr commit $/chip-h | 0.84 / 0.54 | 1.89 / 1.22 | 2.94 / 1.89 |

For comparison, one H100 has about 990 dense bf16 TFLOPS (spec) at $6.08
spot, and one A100 40GB about 312 at $2.20 spot ([gpu-options.md](gpu-options.md)).

## What fits where

| Model | bf16 weights | int8 weights | Fits on |
| --- | ---: | ---: | --- |
| Clef-Flash (9B) | ~18 GB | ~9 GB | v6e-1 in bf16 (32 GB, room for 16k-token state); v5e-1 only in int8; v5e-4 in bf16 with tensor parallelism |
| Clef 27B | ~54 GB | ~27 GB | v6e-4 in bf16 (128 GB, tensor parallel 4); v6e-1 in int8 is tight; v5p-4 in bf16 |

The per-request state is small: f16 K/V for the attention layers (Flash:
8 layers × 4 KV heads × 256 × 2 × 2 bytes ≈ 32 KB a token, 0.5 GB at 16k),
about 50 MB of DeltaNet state, and the head's memory rows (32 MB at 16k).
The joint head is 122 M parameters. The LM head is never multiplied; only
option-token rows of `lm_head` are gathered.

Q4_K_M, which every GPU runtime we use serves, has no TPU equivalent. A TPU
port serves the official BF16 checkpoints (`Cloudflare/clef-flash`,
`Cloudflare/clef`) directly, or int8 weight-only quantized from them. That
is a different artifact from our GPU lanes: answers will move slightly
against Q4 (and BF16 is the reference; see #11306 for how far Q4 27B sits
from it).

## Expected cost per token (estimates)

Clef-Flash costs about 2 × 8.5 B ≈ 1.7e10 FLOP per prompt token (the LM
head is skipped); Clef 27B about 5.4e10. Prefill is compute-bound. Taking
40 % of peak as a reachable utilization for a well-written port (our CUDA
lane reaches about 33 % of an H100 today; a mature TPU stack reaches more on
plain transformers, but the DeltaNet scan is the uncertain part):

| Chip, precision | Flash tokens/s per chip | Flash $/M spot | Flash $/M on-demand | 27B tokens/s per chip | 27B $/M spot |
| --- | ---: | ---: | ---: | ---: | ---: |
| v6e, bf16 | ~21,600 | ~$0.0075 | ~$0.035 | ~6,800 | ~$0.024 |
| v6e, int8 weights and activations | ~43,000 | ~$0.0037 | ~$0.017 | ~13,600 | ~$0.012 |
| v5e, int8 | ~9,200 | ~$0.011 | ~$0.036 | ~2,900 | ~$0.034 |
| **Measured today:** H100 spot, Psionic Q4 | 15,000–18,000 | $0.10–0.11 | $0.20 | 5,100 | $0.33 |
| **Measured today:** A100 spot, Psionic Q4 | 8,500 | $0.072 | $0.120 | 2,900 | $0.21 |

At 50 % utilization double each figure. Even at half the assumed
efficiency, a spot v6e serves Flash at about $0.015–0.03 per million, under
Jev's $0.042; on-demand v6e reaches Jev's price only with int8 or high
utilization. 27B on v6e would undercut Workers AI's $0.24 by an order of
magnitude. These are the numbers P2 must measure before anything is
committed.

## Software paths

| Path | What it is | Effort | Risk | Ceiling | Fits the Rust rule? |
| --- | --- | --- | --- | --- | --- |
| **A. JAX + Pallas** | The backbone in JAX, custom Pallas kernels for the chunked Gated DeltaNet scan and for attention (GQA 16/4, head_dim 256, partial interleaved MRoPE, sigmoid output gate), the head in JAX, behind an HTTP `/v1/systemone` server | High: a new engine, 6–10 weeks to a correct, fast Flash | The DeltaNet scan on TPU: a chunked (WY/UT) formulation turns most of it into matmuls; the sequential part must not dominate. Pallas maturity for this shape is the open question | Highest; Google's own serving stacks use this path | No: Python at run time. Needs an owner exception for a sealed sidecar, or path D |
| B. PyTorch/XLA | Port the reference `joint_schema_model.py` and the HF Qwen3.5 model to PyTorch/XLA | Medium to start, high to make fast | Lazy-tensor recompiles, weaker custom-kernel story than Pallas | Lower than A | No |
| C. Reuse a TPU serving stack (vLLM's TPU backend, JetStream, MaxText) | Add Clef's architecture and head to an existing engine | Depends entirely on whether the stack already runs the Qwen3.5/Qwen3-Next hybrid (Gated DeltaNet + gated attention). Not verified here; P0 checks it | If supported, lowest effort; if not, we inherit an engine we do not own | High | No (Python), same exception |
| D. Rust over PJRT | Author the graph in JAX once, export StableHLO, and execute it from a Rust server through the PJRT C API, with Psionic's encoder and head in Rust | Highest | Rust PJRT bindings are thin; custom Pallas kernels export as custom calls that must be carried along | Same as A for the compiled graph | Yes, run time is Rust; JAX only at build time |

**Update (2026-10-12, [clef-tpu-survey.md](clef-tpu-survey.md)):** path C's
open question is answered. vLLM TPU (tpu-inference) ships a fused, chunked
Gated DeltaNet Pallas kernel, SGL-JAX runs the Qwen3.5 hybrid natively
(Qwen3.8-27B validated on v6e-4), and both load the Clef checkpoints'
`Qwen3_5ForConditionalGeneration` class. Neither serves a prefill-only
custom-head workload, so the survey recommends path A built from those
parts (SGL-JAX backbone code, the tpu-inference `gdn/v3` kernel, JAX flash
attention, MaxText as the reference). Ironwood is not cheaper per Flash token
than v6e, and spot v6e is $0.243/chip-h in us-east4.

**Recommendation:** A for P0–P2 (fastest way to learn whether the
throughput is real), with the server split so the request edge is Rust from
the start: Psionic's byte-exact encoder in front (token ids and spans), the
TPU worker computing hidden rows, memory rows, span sums and the head, and a
Rust `/v1/systemone` door the gateway pool already speaks. If P2 hits its
target, decide between keeping a sealed Python worker (owner exception) and
moving the run time to D.

## The port, piece by piece

1. **Weights.** Load the BF16 safetensors from the HF checkpoints and the
   `joint_head.safetensors`; int8 weight-only (per-channel) quantization as
   a build step in P3, with the same correctness gates.
2. **Encoder.** Keep Psionic's byte-exact `encode_record` port in Rust (it
   is zero-difference against the reference on 200 records); the worker
   receives token ids, question and option spans, and option-token ids.
3. **Backbone.** Qwen3.5 hybrid: Flash 32 layers (24 Gated DeltaNet + 8
   gated full attention), 27B 64 layers with the same 3:1 pattern. Chunked
   prefill with a fixed chunk (2,048) so XLA compiles one program per
   (chunk, batch) shape; the last chunk is padded and the pad rows are
   masked out of the memory rows and span sums. Causal attention makes the
   real rows independent of trailing pads.
4. **Head inputs streamed.** As on CUDA: per chunk, `M = LN(H) W_mem`
   (1,024 wide) kept on the device, span sums of `LN(H)` accumulated, the
   last row kept; the 4,096/5,120-wide rows dropped.
5. **Head.** Two evidence-routing layers and four field layers of 16-head
   attention, small matmuls; batched over options and questions.
6. **Batching.** A batch dimension over concurrent requests at each chunk
   (TPUs want it); per-request state stays separate, as Psionic's Metal
   batch worker does.
7. **Prefix reuse.** Snapshot DeltaNet state, K/V and memory rows at a
   prefix boundary, as Psionic M4 does on Metal (exact, 6.8–9.4× on a
   repeated state there).
8. **Images** (later). The vision tower on the TPU or on the CPU front.

## Correctness gates (our existing ladder)

1. Encoder: zero differences (reuse the Rust encoder, so this holds).
2. Head: max |Δlogit| ≤ 1e-3 against the torch f32 head on dumped hidden
   rows.
3. Backbone: per-layer and final hidden-row cosine against the HF f32
   reference on the parity records (`crates/psionic/fixtures/clef/`).
4. End to end: the 40 e2e requests (104 questions) against the BF16
   reference goldens — top answer ≥ 97 % (near ties excluded), median |Δp|
   ≤ 0.02 — and against our GPU Q4 lanes for the record.
5. Bucket and chunk invariance: the same answers for every chunk size and
   pad length, to 1e-3 in logits.
6. Product: the file-relevance lab (`scripts/clef-lab/eval_closed.py`) and
   the router goldens, so a TPU pool member is judged on what we serve.

## Serving integration

- The TPU worker sits behind a Rust `/v1/systemone` door and joins the
  gateway's hosted Clef pool beside the H100 and the A100 (pooled
  backends with least-in-flight balancing are being added now). Answers
  carry an identity block like Psionic's (`artifact_digest`,
  `head_digest`, `backend: "tpu"`, precision) so calibration maps and
  receipts can key on it.
- Spot TPU VMs (`gcloud compute tpus tpu-vm create --spot`) behind the same
  restarter pattern as `clef-decisions-h100`; queued resources for
  capacity.
- **No sealed decisions on TPU.** TPUs have no confidential-computing mode;
  sealed decisions stay on the H100 in CC mode.

## Phases and gates

| Phase | Work | Gate to pass | Cost to run |
| --- | --- | --- | --- |
| P0 Feasibility (days) | One spot v6e-1: measure a bf16 matmul at Clef's shapes, try a chunked DeltaNet scan in Pallas at Flash's sizes, check whether vLLM-TPU / MaxText run Qwen3-Next-style hybrids | Matmul ≥ 50 % of peak at our shapes; a DeltaNet chunk runs at ≥ 25 % of peak; a go/no-go on path C | ~$5 |
| P1 Flash correct (2–4 weeks) | Path A: backbone + head on v6e-1 in bf16, Rust encoder in front | Ladder steps 2–5 | ~$50 |
| P2 Flash fast (2–4 weeks) | Fixed-chunk compile, batching, profile | ≥ 20,000 tokens/s per chip at 1.5k and 16k prompts; projected ≤ $0.02/M on spot at 50 % use | ~$100 |
| P3 int8 and batching | Weight-only then activation int8, cross-request batching, prefix reuse | Gates 4–6 still pass; ≥ 1.6× P2 | ~$100 |
| P4 Clef 27B | v6e-4 bf16 (tensor parallel) or v6e-1 int8 | Ladder on 27B; ≤ $0.05/M spot | ~$200 |
| P5 Production pool member | Rust door, identity block, spot restarter, gateway pool entry | 24 h shadow beside the H100 and A100 with ≥ 99 % top-answer agreement on live traffic and no errors | spot hours |

## Held TPU machines

Owner policy (2026-10-13): hold useful TPU capacity rather than release
it, up to $500/day across all TPU work. Measured results:
[clef-tpu-data.md](clef-tpu-data.md). Final state of the measurement
campaign, 2026-10-13 04:55 UTC (catalog list prices):

| Name | Zone | Type | Mode | $/h | $/day | Purpose |
| --- | --- | --- | --- | ---: | ---: | --- |
| p6-asia-northeast1-b | asia-northeast1-b | v6e-1 | on-demand | 3.24 | 78 | P1 correctness/serving worker (other agent) |
| cd-v6e1-ane1b-d | asia-northeast1-b | v6e-1 | on-demand | 3.24 | 78 | measurement campaign: held v6e capacity (int8 sweeps done) |
| cd-v5p8-ue5a-s | us-east5-a | v5p-8 | spot | 3.44 | 83 | v5p measurements (done); held |
| cd-v5e8-ue1c-s2 | us-east1-c | v5e-8 | spot | 2.87 | 69 | 27B int8 TP 8 run; held |
| **Total** | | | | **12.80** | **307** | |

Queued spot requests ($0 until granted; a granted one starts the standard
job automatically): v6e-8 and v6e-4 in us-central1-a, us-central1-b,
us-east1-d and asia-southeast1-b.

Unattended automation runs **only on the owner's Mac** (it stops if that
machine sleeps or the session ends), from `scripts/bench/clef-tpu/`:
`guard.py` (holds the project at or under $500/day by releasing v5e, then
v5p, then our v6e-1s, then extra v6e-4/-8 beyond one of each; never the P1
VM), `keeper.sh` (re-tries spot v6e), `requeue.sh` (refiles preempted
queued requests), `autorun.sh` (starts jobs on new VMs), `watch.sh` and
`syncall.sh`. If they stop, nothing enforces the cap: queued requests that
are granted later would bill unchecked (worst case about $30/h if every
request lands).

## Owner decisions

1. **Language at run time.** Path A runs Python (JAX) on the TPU worker.
   Allow it as a sealed sidecar behind a Rust door (the repo's
   documented-exception route), or require path D before production.
2. **Artifact.** TPUs serve BF16 (or our own int8) rather than the Q4_K_M
   GGUF the GPU lanes run; accept a second artifact family with its own
   calibration maps.
3. **Budget.** About $5 for P0 and a few hundred dollars of TPU time for
   P1–P4, plus the engineering time.

## Sources

- Google Cloud TPU v5e, v6e and v5p documentation (per-chip specifications,
  slice shapes), read 2026-10-12.
- Cloud Billing Catalog API, Compute Engine SKUs for `TpuV5e`, `TpuV6e`,
  `TpuV5p` (on-demand, spot, commitment), read 2026-10-12.
- [clef-tpu-survey.md](clef-tpu-survey.md): stacks, kernels, quantization,
  prices and quotas, 2026-10-12.
- Our measurements: [gpu-options.md](gpu-options.md),
  [decision-throughput.md](decision-throughput.md),
  [clef-native.md](clef-native.md).
