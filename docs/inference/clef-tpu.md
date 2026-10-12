# Clef on Cloud TPU: spec and phased plan

2026-10-12. Proposed. What it takes to serve our Clef decision models
(Clef-Flash 9B, Clef 27B) on Google Cloud TPUs, what it would cost, and the
plan with gates. Nothing here is built; every throughput figure below is an
estimate from published chip specifications unless it says measured.
Tracking: [#11446](https://github.com/OpenAgentsInc/openagents/issues/11446).
Context: [decision-profitability.md](decision-profitability.md),
[gpu-options.md](gpu-options.md), [clef-native.md](clef-native.md).

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
- Our measurements: [gpu-options.md](gpu-options.md),
  [decision-throughput.md](decision-throughput.md),
  [clef-native.md](clef-native.md).
