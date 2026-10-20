# Native DeepSeek-V4.1-Flash in Psionic

Status: proposal, 2026-10-20. Nothing in this document is implemented. It
extends the [DeepSeek support plan](deepseek-support-plan.md) (DS0–DS6) and
the [implementation roadmap](implementation-roadmap.md) with one concrete
goal: serve the current DeepSeek release, V4.1-Flash, with Psionic as the only
inference engine, on Pylons, so that the Grid can show it. The product
roadmap that consumes this is
[Enter the Verse](../roadmap/2026-10-20-enter-the-verse.md).

Owner direction (2026-10-20): Psionic only, no external inference engines in
the serving path. Adapt DeepSeek's open-source code where it helps. Split the
model across devices or machines if one GPU can't hold it.

## Summary

- V4.1-Flash is about 510 GB of weights: FP4 routed experts, FP8 dense
  weights, and two large FP8 Engram lookup tables. No single GPU holds it.
- Serving one request at a time is bound by memory bandwidth, not by
  tensor-core math. Portable Psionic dequantize-and-multiply kernels work on
  any recent NVIDIA GPU, so the datacenter-only FlashMLA, DeepGEMM, and
  DeepEP kernels aren't required for the first native result.
- DeepSeek's checkpoint ships a readable PyTorch reference (`inference/`,
  about 2,400 lines). Run it once on rented GPUs to produce golden
  activations. It's a test oracle only and never serves a request.
- Build V4.1 directly. Don't detour through V2-Lite: V4.1's attention, cache,
  residual, and Engram design differ enough that V2-Lite would mostly prove
  a different model. Share the MoE and FP8/FP4 primitives between them.
- Split by pipeline stages (contiguous layers) first: within one machine
  across its GPUs, then across machines as stage Pylons. Each token sends
  only a small residual state across a boundary, so ordinary datacenter
  networking is enough for one request at a time. Expert parallelism
  (DeepEP-style all-to-all) is a later throughput optimization inside one
  machine.

## The checkpoint

Pinned revision: [`deepseek-ai/DeepSeek-V4.1-Flash-1017`][H4] at
`1bae0e1088993b9b82de08f59de5be5a34e42170`. We read the `config.json`,
`inference/config.json`, `model.safetensors.index.json`, the file listing, and
the `inference/` reference sources at that revision on 2026-10-20. We didn't
download the weights.

| Property | Value | Source |
| --- | --- | --- |
| Weights on disk | 510.3 GB in 48 safetensors shards | Revision file listing |
| Layers | 40 decoder layers, plus 3 MTP/DSpark layers | `config.json` |
| Hidden size | 5,120; vocabulary 129,280 | `config.json` |
| Attention | 64 query heads, one shared KV head of dimension 512 with a 64-dimensional RoPE part; query LoRA rank 1,280; grouped output LoRA (8 groups, rank 1,024); attention sink | `config.json`, tensor names |
| Attention per layer | `compress_ratios`: layers 0–1 sliding window only; layers 2–19 ratio 2; layers 20–39 ratio 1; window 128 tokens | `inference/config.json` |
| Cross-layer sharing | KV source layers 2, 8, 14, 20; indexer source layers 2, 8, 14, 20, 24, 28, 32, 36; candidate source layer 20 (top 2,048 blocks of 8 tokens); indexer top-k 512 | `inference/config.json` |
| Residual stream | Hyper-connections with `hc_mult=4` and 20 Sinkhorn iterations, so the residual is four streams of 5,120 | `config.json` |
| MoE | 384 routed experts, 6 active, 1 shared expert; expert width 2,304; `sqrtsoftplus` scores, `noaux_tc` selection, route scale 1.5, SwiGLU clamp 10 | `config.json` |
| Engram | Layers 1 and 14; n-grams up to 4 over a normalized, compressed vocabulary (99,092 ids); about 384 million rows per table; 8 heads of 256 | `inference/config.json`, `inference/engram.py` |
| Weight formats | FP8 E4M3 with one UE8M0 scale per 32×32 block; experts FP4 E2M1 with one UE8M0 scale per 32 elements along K | `inference/model.py` lines 26–30, 211–231 |
| Context | 1,048,576 positions allocated; YaRN factor 16 over 65,536 | `config.json` |
| Optional parts | DSpark/MTP draft (block 5, 128 experts, 3 active); 32-layer vision encoder | `config.json` |

Memory split, by our arithmetic from the config and the safetensors element
counts. These are estimates, not measurements.

| Part | Approximate size | Notes |
| --- | --- | --- |
| Routed experts, 40 layers × 384 | 280 GB | About 544 billion FP4 parameters plus scales |
| Engram tables, 2 | 196 GB | FP8; a few rows are read per token |
| Everything else | 34 GB | Attention, shared experts, gates, embeddings, head, MTP, vision |

Each token reads 6 routed experts and 1 shared expert in each of the 40
layers: about 10 billion expert parameters, or 5 GB at FP4. Attention and the
shared weights add a few GB. Because the attention keeps one 512-wide KV
head with a 128-token window and compressed history, the cache for a long
context is small compared with the weights.

Engram is a lookup table, not a matrix product. Only a few rows are touched
for each token, so the 196 GB can live in host memory or on local SSD with
the touched rows copied to the GPU. That brings the GPU-resident part down to
about 314 GB.

## What we reuse and what we write

| Need | Reuse | Write |
| --- | --- | --- |
| Prompt, tokens, reasoning, tool-call parsing | [`deepseek-recipe`][Recipe] (official Rust), as DS1 in the support plan | A Psionic adapter that keeps special-token ranges and token-boundary streaming |
| Exact math for each block | The checkpoint's `inference/model.py`, `kernel.py`, `engram.py` | Rust CPU references for each component, checked against golden activations |
| Weight loading | Psionic's paged artifact readers and identity rules | A safetensors reader that keeps FP8/FP4 blocks and UE8M0 scales byte-exact; no conversion to GGUF |
| GPU kernels | Psionic CUDA backend, existing quantized matrix-vector kernels and CUDA graphs | FP4 E2M1 and FP8 E4M3 block-scaled matrix-vector kernels, MQA attention with sink and window, the indexer, Sinkhorn mixing, the Engram gather |
| Single-request serving | `--single-generation`, the R1 reasoning contract, managed Pylon recovery | A V4.1 template family and reasoning parser through DS1 |
| Splitting the model | The Gemma 4 step/reset pipeline (`psionic-serve/src/gguf.rs` and `openai_http.rs`), Mesh-LLM lessons, `psionic-net` | Typed stage state for V4.1, stage packages, multi-GPU stages in one process, stage readiness |
| Load balancing experts (later) | [EPLB][EPLB] policy ideas | Real expert routing in `psionic-cluster`, which today derives routing from a hash ([Psionic gaps](#psionic-gaps-this-plan-must-close)) |

Leave out of the serving path: DeepInfer, the PyTorch reference, FlashMLA,
DeepGEMM, DeepEP, TileLang, and llama.cpp. FlashMLA and DeepGEMM stay
optional accelerators for a later, pinned SM90/SM100 profile. Their kernels
can guide ours, but they aren't dependencies.

## Build order

Each step ends with retained evidence. A component is enabled only after its
check passes, and no public capability names V4.1 until the full model
passes.

| Step | Work | Done when |
| --- | --- | --- |
| V0: Oracle | Pin the reference sources. On a rented 8-GPU machine, run `inference/generate.py` (TP8, FP4 experts) on a fixed prompt set and dump per-layer inputs and outputs for selected layers and tokens: embeddings, Engram output, each attention variant, indexer selections, Sinkhorn mixing, router ids and weights, expert outputs, logits. Store digests and small slices in the repository and full dumps in a bucket. | Golden fixtures cover layers 0, 1, 2, 14, 20, 21, and 39; the run's environment, revision, and costs are recorded; the VM is deleted. |
| V1: Protocol | DS1: integrate `deepseek-recipe` for V4.1 rendering, token encoding, reasoning, and tool-call parsing, with mock-backend tests. | The checkpoint's own `encoding/tests` cases pass byte for byte. |
| V2: Loader | Safetensors reader with exact FP8 and FP4 block layouts, tensor naming, and the 48-shard index. Stream-mode loading for machines that hold only part of the model. | Every tensor in the index loads with its shape and dtype; random-block dequantization matches the oracle's dequantized values. |
| V3: CPU components | Rust references for RMSNorm, hyper-connection mixing (Sinkhorn), the three attention variants, the indexer and candidate selection, Engram hashing and lookup, the gate, routed and shared experts, the head. | Each matches its golden slice within a stated tolerance; router ids match exactly. |
| V4: CPU full model | Compose the 40 layers with a streamed or memory-mapped weight source. Greedy decode on the oracle prompts. | Logits match the oracle within tolerance for the first 32 generated tokens; the greedy text matches. Slow is fine. |
| V5: CUDA kernels | FP4 and FP8 block-scaled matrix-vector kernels, attention, indexer, Sinkhorn, Engram gather; each with a CPU-parity test on the RTX 4080 using real tensors from one layer. | Per-kernel parity passes on SM89 and on the target GPU. |
| V6: One machine, many GPUs | Layer stages across all GPUs of one VM in one Psionic process, with typed stage state (below). Serial decode only. | Full-model parity with V4 on the oracle prompts; DS0's six runtime checks pass; tokens per second, time to first token, and memory per GPU are recorded. |
| V7: Pylon serving | Serve through `psionic-openai-server --single-generation` and a managed native worker in Pylon (generalizing #12364's llama.cpp recovery). | The serial Pylon and recovery checks pass against the native worker. |
| V8: Stage Pylons | Run the stages as separate Pylons on separate machines, joined over the cloud network. | Same parity and checks across machines; a stage crash fails the request visibly and the group recovers. |
| V9: Throughput (later) | Batching, expert parallelism inside a machine, DSpark speculation, prefix reuse. | Measured against V6 by committed output tokens per second. |

V0 and V1 can start now and in parallel. V2 and V3 can start against the
oracle fixtures before any big GPU is rented again.

## Splitting the model

### Stage state

A stage boundary must carry everything later layers read, not just the
hidden state:

- The four hyper-connection residual streams (`hc_mult=4`), 4 × 5,120 values.
- KV from the source layers 2, 8, 14, and 20, which later layers reuse.
- Indexer state from the source layers 2, 8, 14, 20, 24, 28, 32, and 36.
- Candidate blocks chosen at layer 20, which layers 21–39 read.
- Engram output for layers 1 and 14, which depends on token history.
- Position, request identity, and a cache generation number, as the Gemma 4
  step does today.

Choose boundaries so that shared KV and indexer state stays inside the stage
that produced it where possible. Where it can't, send it once per token with
the residual. For one request at a time, the residual is about 40 KB per
token in BF16, so a datacenter network adds well under a millisecond per
hop. The [Mesh-LLM review](mesh-llm-lessons.md) applies: qualify a split
against the unsplit model, give each stage only its own weights, publish a
stage only after it's loaded and checked, and fence a group so a restarted
stage can't join a stale request.

### Inside one machine first

Psionic's CUDA backend tracks a device ordinal, but the server runs one
device per process today. V6 adds several devices to one process, with
stages copying residual state device to device. This is the simplest
multi-GPU path and needs no collective library. Tensor and expert
parallelism need all-to-all and all-reduce, which Psionic doesn't execute
yet; leave them for V9.

### Then across machines

V8 makes each stage its own Pylon. Each stage Pylon publishes a beacon with
its stage role and the model's group identity. A front stage takes the
NIP-CJ job and calls the next stage over an authenticated private channel,
as the Gemma 4 pipeline does with its peer. Run all stages of one group in
one cloud zone. Ordinary home connections aren't a target for one group: the
docs already rule out token-level expert traffic over wide-area links, and a
pipeline over home links would be slow. Contributors' machines can still
serve whole smaller models, or join as stages once the group protocol
measures their latency.

## Hardware

Serial decode reads about 5–8 GB of weights per token, so speed depends on
memory bandwidth and on keeping the model resident. All candidates are spot
or quota-limited; existing production GPUs aren't spare capacity.

| Option | Memory | Fits | Price we have on record | Use |
| --- | --- | --- | --- | --- |
| `g4-standard-384`, 8× RTX PRO 6000 Blackwell, PCIe | 768 GB | Everything, with about 250 GB to spare | $36.04/h on demand ([Muse serving notes][Muse]) | Recommended first target: one machine, stages over PCIe, the same card a contributor could own. Spot quota is 16 GPUs per region. |
| `a4-highgpu-8g`, 8× B200, NVLink | about 1,440 GB usable | Everything | $39.63/h spot ([Muse serving notes][Muse]) | Throughput work (V9) and the oracle run. Spot only; quota 64 B200 per region. |
| `a3-ultragpu-8g`, 8× H200 | 1,128 GB | Everything | $83.37/h on demand ([Muse serving notes][Muse]) | Fallback if G4 and A4 spot capacity is unavailable. |
| 4× RTX PRO 6000 in one or more G4 VMs | 384 GB | Only with Engram in host memory | Not recorded | Stage-Pylon demo with fewer GPUs. |
| RTX 4080 (coderos-4080) | 16 GB | One layer's tensors at a time | Owned | Kernel parity tests (V5). |

Before renting, check the current spot price and capacity, and set a cost cap
and an expiry. Delete or stop the machine when the bounded run ends, and
label it with the run (`agent-run.py label-vm`).

The full CPU reference (V4) needs about 510 GB of readable weights. It can
memory-map them from local SSD on a smaller machine, more slowly, or run on a
host with enough RAM: a `g4-standard-384` host has 1,440 GiB and an A4 host about 3.9 TiB, so the GPU machine can also run the CPU reference.

## Psionic gaps this plan must close

- **No DeepSeek decoder family.** `GgufDecoderFamily` has no DeepSeek case,
  and loading is GGUF-centric. V2 adds a safetensors path.
- **No FP4 E2M1 or FP8 E4M3 block layouts with UE8M0 scales.**
  `QuantizationMode` has MXFP4 and GGUF block formats. MXFP4 also uses E2M1
  values with E8M0 scales over 32 elements, so check whether its layout
  matches before reusing it.
- **One device per process.** V6 adds multi-device stages.
- **No collective execution.** Planning types exist in `psionic-collectives`
  and `psionic-cluster`, but nothing runs all-to-all or all-reduce, and the
  sparse expert "realization" derives routing from a SHA-256 of the request
  seed rather than from a router. Don't cite it as expert-parallel support.
- **No NVLink or in-machine link class.** `ClusterLinkClass` models links
  between machines only.
- **The Gemma 4 pipeline is two stages, unqualified (R4), and sends f32
  hidden state.** Generalize it with a typed stage-state message rather than
  adding a second pipeline protocol.

## Pylon gaps for this model

These are owned by the [Enter the Verse roadmap](../roadmap/2026-10-20-enter-the-verse.md)
but block V7:

- NIP-CJ input is capped at 16 KiB and 32 turns (`crates/pylon/src/job.rs`).
  Agent work needs far more context.
- Results aren't streamed, and the provider deadline is 90 seconds. A
  V4.1 reasoning answer can run longer. Any longer deadline is a transport
  contract change with its own tests, not a client timeout edit.
- Tool calls and structured output are refused end to end. Agent use needs
  DS1's tool-call parsing, R2's constrained decoding, or both.

## Open questions

- The precise active parameter count and the KV bytes per token at each
  compression ratio. Measure both in V4 instead of trusting our estimates.
- Whether MXFP4 in Psionic is byte-compatible with this FP4 layout.
- G4 spot price and capacity on the day of the run.
- Whether to qualify vision. It's out of scope for the first result; image
  requests are refused.
- License terms for the checkpoint's `LICENSE` file, which we haven't
  reviewed for redistribution of converted stage packages.

[H4]: https://huggingface.co/deepseek-ai/DeepSeek-V4.1-Flash-1017/tree/1bae0e1088993b9b82de08f59de5be5a34e42170
[Recipe]: https://github.com/deepseek-ai/deepseek-recipe
[EPLB]: https://github.com/deepseek-ai/EPLB
[Muse]: ../research/muse/2026-10-15-muse-spark-serving.md
