# QVAC Edge Stack Analysis: What Tether's Local-AI Platform Means For The Psion Pipeline

Date: 2026-06-10

Status: reference analysis of the `tetherto` QVAC repos tracked in the
workspace `projects/tether/` lane, read against the Psion full-pipeline
buildout plan (`2026-06-10-psion-full-pipeline-buildout-plan.md`) and the
product-promises registry (`2026-06-10.9`). Per workspace policy these
repos are **read-only reference: study and port ideas, never vendor.**
Nothing here moves any promise.

Repos reviewed in depth: `qvac-examples` (with the `qvac` SDK monorepo and
`qvac-fabric-llm.cpp` skimmed for context), `qvac-ext-ggml`,
`qvac-ext-stable-diffusion.cpp`, `qvac-rnd-fabric-llm-bitnet` (with
`qvac-rnd-fabric-llm-finetune` skimmed). Licenses: Apache-2.0 (SDK, R&D
repos) and MIT (ggml and stable-diffusion forks, fabric-llm.cpp) — all
permissive, so the constraint on borrowing is our own Rust-substrate
policy, not legal.

## 1. What QVAC is, in one paragraph

QVAC is Tether's open-source, local-first edge AI platform: a TypeScript
SDK over a Bare-runtime worker with native C++ plugin addons (llama.cpp,
whisper.cpp, stable-diffusion.cpp, ONNX), running on Linux/macOS/Windows/
Android/iOS with serious mobile GPU work (Vulkan on Adreno, Metal on
Apple). Models are distributed peer-to-peer over the Holepunch stack
(Hyperdrive/Hyperswarm), inference can be *delegated* to a peer provider
identified by public key with local fallback, and — the part that matters
most to us — it ships **on-device LoRA fine-tuning** (checkpoint/resume,
assistant-only masked loss, LR schedules) and a **BitNet b1.58 ternary
lane** whose GPU kernels are demonstrated bit-exact against the CPU
reference. What it conspicuously does not ship: payments, verification,
receipts, reputation, settlement, or any market layer. Tether built the
machine and left out the economy.

## 2. The headline findings, ranked by relevance to our plan

**2.1 Ternary determinism is the big one.** The BitNet lane (TQ1_0 ~1.0
bit/weight, TQ2_0 ~2.0; ternary {-1,0,1} forward) publishes lossless-
verification evidence across heterogeneous GPUs: 99.04% same-top-token,
mean KL divergence 0.000295 vs CPU, claimed bit-exact equivalence between
Vulkan kernels and the CPU reference. Read against our plan's §9 numerics
problem — cross-device float non-determinism is *the* obstacle to cheap
verification of inference work on untrusted consumer hardware — this is
an existence proof that a quantization format can make **inference a
deterministic-recompute work class across device vendors**. Today our
verification map routes inference work to seeded_replication +
statistical checks; a ternary/integer serving lane would let it ride the
cheapest rung (digest spot-checks) instead. That directly serves
`training.verification_classes.v1` and the rollout-generation rows of the
verification map, and it complements (without touching) the Tassadar
exact lane — see the boundary in §5.

**2.2 On-device LoRA is an existence proof for our remote-finetune
promises.** QVAC fine-tunes 1B-class models on phones: ~1h18m for a 1B
BitNet LoRA on an Adreno 830, ~1h45m on an iPhone 16, with checkpoint/
resume, cosine-warmup schedules, and assistant-only masked loss — the
exact feature set our plan's post-training arc (§8) specifies for the
instruct SFT lane, already running on the exact device class our funnel
counts as dark capacity. The bounded two-device LoRA run that
`pylon.first_real_model_training_run.v1` needs (#4670 shape) is not
speculative hardware-wise; a Tether R&D team has run its single-device
version on consumer phones. Their dynamic-tiling trick (tiling matmuls
under Adreno's 128 MiB SSBO limit) is the kind of unglamorous kernel work
our plan's §9 budgeted for.

**2.3 The mobile-GPU hardening is a free taxonomy for our device
dataset.** The qvac-ext-ggml fork's delta over upstream is mostly
production Android pain: per-GPU-generation backend selection (Mali →
Vulkan only; Adreno ≤700 → CPU; Adreno >700 → OpenCL/Vulkan), Adreno 740
workarounds, descriptor-pool grow-on-demand, hybrid static-CPU +
dynamic-GPU backend packaging for app distribution. For
`training.device_capability_dataset.v1` (#4681), this is a ready-made
quirks taxonomy: when phones join the funnel, the dark-capacity reason
codes and benchmark dimensions should cover exactly the failure modes
this fork patches. It is also the clearest empirical answer to a backend
question our plan deferred: on non-Apple edge GPUs, **Vulkan with
per-device fallbacks is the durable path**, and that should shape
psionic's backend roadmap (currently CPU/Metal/CUDA).

**2.4 The diffusion fork's durable lesson is the RNG, not the work
class.** The stable-diffusion fork uses a Philox counter-based RNG
(PyTorch-CUDA compatible) that makes generation reproducible from a seed
*across backends*, plus an abort-callback API for mid-sampling
cancellation — the general shape any seeded paid work kind needs
(dispatch with pinned seed and digests, verify by seeded replication,
cancel on timeout). **Owner decision 2026-06-10: image generation is not
pursued at this time, at least not locally via this infrastructure** —
no image/video work kind gets filed from this review. The Philox pattern
survives that decision on its own merits, because our existing seeded
work classes (ablation cells, sweep cells, rollouts) want the same
cross-device reproducibility; it is ported as psionic#1116.

**2.5 The platform itself validates our thesis by what it omits.** QVAC
solves discovery (model registry), packaging (plugins, GGUF), and
operability (caching, streaming, lifecycle) for edge AI — three of the
five missing-market-plumbing items from the compute-fracking frame in our
plan §3.2. It has **no trust layer and no settlement layer**: delegated
inference is voluntary, unmetered, unverified, and unpaid (the WDK wallet
kit exists as a sibling and is not integrated). Tether, of all companies,
shipped the supply machinery without the economy. That is the gap our
entire promise board exists to fill, and it means QVAC is better read as
complementary infrastructure — and its users as a future supply class —
than as a competitor to the clearing layer.

## 3. Repo-by-repo, condensed

| Repo | What it is | Delta vs upstream / notable | License |
|---|---|---|---|
| `qvac` + `qvac-examples` | TS SDK, plugin worker, 94 examples (completion, embeddings, STT/TTS, translation, OCR, RAG, diffusion, VLA, finetune, delegated inference) | Holepunch P2P model delivery; pubkey-firewalled provider/consumer delegation with local fallback; `finetune()` in the public API | Apache-2.0 |
| `qvac-fabric-llm.cpp` | llama.cpp fork — "fabric" = single-device hardware-abstraction/training layer, **not** distributed | `llama-finetune-lora` CLI (checkpoint/resume, masked loss, schedulers, Jinja templates); KV-cache quantization (TurboQuant/PolarQuant); BitNet TQ2_0 on Vulkan/Metal | MIT |
| `qvac-ext-ggml` | ggml fork | Android/Adreno/Mali hardening; per-GPU-gen backend policy; hybrid static/dynamic backend packaging; AdamW/SGD training ops present; TQ1_0/TQ2_0/Q1_0 quant types | MIT |
| `qvac-ext-stable-diffusion.cpp` | stable-diffusion.cpp fork | SD1.x→SDXL→SD3→Flux→LTX-2.3 video; Philox seeded reproducibility; abort callbacks; vcpkg/system-ggml packaging; q4_0 SD1.5 in ~1.5 GB | MIT |
| `qvac-rnd-fabric-llm-bitnet` | R&D docs/benchmarks for ternary lane | b1.58 models 125M–13B; 2.1–11.3× edge-GPU speedups; lossless-verification metrics; LoRA-on-frozen-ternary with FP16/32 backward; explicitly pre-production | Apache-2.0 |
| `qvac-rnd-fabric-llm-finetune` | R&D LoRA framework + recipes | GGML-based (no PyTorch) cross-platform finetune; biomedical-QA and style-transfer recipes with eval scripts; multi-GPU explicitly experimental | Apache-2.0 |

One naming caution recorded so nobody over-reads it: **"fabric" is not a
distributed-training fabric.** It is a per-device heterogeneous-GPU
abstraction. Multi-device coordination is exactly the layer they do not
have — and the layer our plan owns (windows, verification, settlement).

## 4. Synergy map against the promise board

| Promise | What QVAC contributes | Nature |
|---|---|---|
| `training.verification_classes.v1` | Ternary bit-exact inference → a deterministic-recompute inference class; their KL/top-token lossless methodology as an external conformance bar | Pattern + methodology |
| `training.device_capability_dataset.v1` | GPU-gen quirks taxonomy, mobile thermal/memory constraints (SSBO limits), backend-selection policy as reason-code vocabulary | Taxonomy |
| `training.post_training_arc.v1` | Working reference for the instruct SFT lane: masked-loss LoRA, checkpoint/resume, schedulers, chat-template handling, plus dataset/eval recipe shapes | Pattern |
| `pylon.first_real_model_training_run.v1` | Existence proof + timing envelopes for bounded LoRA on consumer/mobile devices (sizes #4670's two-device run honestly) | Evidence-shaping |
| `pylon.compute_revenue_modes.v1` | BitNet perf (13B in 2.8 GiB, 15–386 tok/s on phones/Macs) strengthens the sellable-local-inference lane | Lane-strengthening |
| `compute.tassadar_executor_poc.v1` | Complementary determinism culture; **distinct lane** (see §5) | Boundary to keep |
| `training.model_ladder.v1` | BitNet-b1.58-style QAT as a derisking-ledger candidate for a future rung (1.58-bit pretraining is a published recipe; ours only via ablation) | Ablation candidate |
| `provider.compliant_usage_labor.v1` / five streams | QVAC's unpaid voluntary provider network is a future supply class whose missing economy is our product | Strategic adjacency |

## 5. A boundary worth stating once: BitNet ≠ Tassadar

Both lanes produce computation that is cheap to verify on weak devices,
and that is where the similarity ends. BitNet is a *trained, statistical*
model whose integer-quantized forward pass happens to be reproducible —
its outputs are as right or wrong as its training made them, and "99.04%
same top token" is a quantization-fidelity metric, not a correctness
proof. Tassadar is *exact by construction* — compiled, not trained, with
a claim boundary the disclosure flow guards. The registry must never let
a ternary-determinism claim borrow the exact lane's language. What the
lanes share productively is the verification economics: both make the
cheapest seats in the network competent auditors, which is move 3
(downshift) and move 6 (eliminate) of the plan's §3.4 operating on the
same supply.

## 6. Borrow code or adapt patterns? The recommendation, in tiers

Workspace policy (read-only reference, port ideas, never vendor) and
psionic's founding thesis (rebuild the relevant ML infrastructure in
Rust) decide most of this. Four tiers:

**Tier 1 — adopt the idea, port the spec, prove parity (do this).**
- **TQ1_0/TQ2_0 ternary formats** into psionic's quantization ladder as
  Rust implementations with fixture-backed parity against published QVAC/
  upstream numbers — the same external-conformance pattern the CS336
  ports use. The deliverable that matters is not the format; it is the
  receipt: *a quantized serving lane whose cross-backend determinism is
  proven well enough to register as a deterministic-recompute
  verification class.*
- **Philox counter-based RNG** as psionic's standard for seeded work. It
  is a small, fully-specified algorithm; one Rust port gives every
  seeded work class (ablation cells, sweep cells, rollouts) cross-device
  replayability by construction. Filed: psionic#1116.
- **Masked-loss LoRA SFT loop shape** (assistant-only loss, checkpoint/
  resume granularity, warmup-cosine schedules, template-aware
  tokenization) into the planned `psionic-train` instruct lane. We were
  going to build exactly this from the smol playbook's description;
  `llama-finetune-lora` is a working reference implementation to read
  while doing it.

**Tier 2 — adopt the taxonomy, not the code.**
- The **per-GPU-generation backend policy and quirks list** feeds the
  device-capability dataset schema and the funnel's dark-capacity reason
  codes (thermal throttle, driver class, memory-limit class).
- **Dynamic tiling under mobile memory limits** and **KV-cache
  quantization** go on the psionic backend roadmap as named techniques
  with their QVAC provenance, picked up when the ladder reaches the
  scale that needs them.
- **Vulkan as the cross-vendor edge backend** becomes the stated
  direction for psionic's fourth backend when contributor phones/Android
  matter — an entry for the derisking ledger, not a current work item.

**Tier 3 — a real decision, flagged but not made here: QVAC SDK inside
Pylon.** Pylon is Bun/Effect; the QVAC SDK is Apache-2.0 TypeScript with
a plugin contract. Wrapping QVAC plugins as Pylon runtime adapters (the
way Pylon already wraps runtimes) would buy STT/TTS/OCR/diffusion/
finetune capabilities for contributor devices quickly *without*
violating the Rust-substrate policy, since Pylon's adapter layer is
already TS. The costs are real: a large dependency surface, a C++ addon
supply chain we don't control, and capability claims we would have to
verify rather than inherit. Recommendation: do **not** adopt now — and
with image generation declined (owner decision, 2026-06-10) no current
work kind triggers the evaluation, so no issue is filed; the gate is "a
paid work kind that would actually use one of its capabilities (e.g.,
transcription) reaches dispatch readiness." Until then, use QVAC as a
**conformance and benchmark comparator** — same model, same device,
QVAC runtime vs psionic serving — which produces useful receipts either
way (recorded as spec input on openagents#4681).

**Tier 4 — do not adopt.**
- **Holepunch/Hyperswarm transport.** Our protocol substrate is
  Nostr/NIP-90 through the shared `nostr-effect` package by workspace
  mandate; the delegated-inference *API ergonomics* (delegate-with-
  fallback, pubkey allow-lists) are worth imitating on our rails, the
  transport is not.
- **The trust model.** QVAC delegation assumes benevolent peers; nothing
  in it survives contact with paid, adversarial supply. Our verification
  registry is not optional plumbing to bolt onto their pattern — it is
  the product.
- **BitNet QAT pretraining for Psion rungs, today.** It is a real
  candidate with a published recipe, but it is an architecture delta,
  and the plan's own discipline applies: it enters through the ablation
  manifest on the derisking ledger, not through enthusiasm. Ledger
  entry: *deferred — revisit at R2+ when the ablation system can price
  it.*

## 7. Follow-ups — filed 2026-06-10

The Tier 1/2 items do **not** become new product promises: they are
implementation work inside existing planned `training.*` promises, so
the tight connection is issues in the owning trackers plus evidence refs
on the touched promises (registry `2026-06-10.10`).

Filed in the psionic tracker (the owning repo for execution-truth work):

- **psionic#1115** — ternary TQ-class formats with cross-backend
  determinism receipts (→ `training.verification_classes.v1`,
  `pylon.compute_revenue_modes.v1`)
- **psionic#1116** — Philox counter-based RNG as the standard seeded-work
  RNG (→ `training.ablation_system.v1`,
  `training.verification_classes.v1`)
- **psionic#1117** — general instruct SFT lane with the fabric finetune
  CLI as external reference (→ `training.post_training_arc.v1`)
- **psionic#1118** — derisking-ledger entries for the deferred
  techniques: Vulkan backend, dynamic tiling, KV-cache quantization,
  BitNet QAT (→ `training.ablation_system.v1`,
  `training.model_ladder.v1`)

Filed in this monorepo: the device-taxonomy and QVAC-comparator
benchmark inputs are recorded as spec input on **#4681**
(`training.device_capability_dataset.v1`).

Explicitly **not** filed: any image/video-generation work kind (owner
decision above) and the Tier-3 Pylon-adapter evaluation (gate not
triggered). None of this moves a promise; all of it is the plan's
existing workstreams absorbing a well-built external reference — which
is what the `projects/` lanes are for. The unified roadmap lives in the
buildout plan's sequencing section.

## 8. Bottom line

QVAC is the best evidence yet that the edge-first supply thesis is
mainstream enough for a major company to build infrastructure for — and
that the infrastructure everyone builds stops exactly where our product
begins. They shipped packaging, runtimes, mobile GPU hardening, on-device
finetuning, and P2P distribution, with no way to pay, price, verify, or
trust any of it. We should take their determinism formats, their SFT loop
shape, their RNG, and their hardware taxonomy — through the Rust door,
with parity receipts — and keep building the layer they left out.
