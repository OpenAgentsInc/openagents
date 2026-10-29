# Beyond language: proposed NVIDIA studies

Read-only source survey, 2026-10-28. The supplied ChatGPT Pro recommendations
are research input. This document adopts their useful capability progression
while preserving the owner's earlier priority: **native Elephant, then
Mammoth, before the broader model program**. Chapters 1–3 currently
run scalar learning, bounded storage and CPU row/matrix numerics in Psionic. All targets here are proposals; no NVIDIA implementation or
original-training reproduction is claimed.

The [proposed contents](../../psionic/book/PLAN.md) gives each study a place.
The [roadmap](roadmap.md) distinguishes teaching order from delivery priority.
No new project issues are published by this update.

## What to select first

| Order of investment | Concrete payoff | Reason and boundary |
| --- | --- | --- |
| Shared core → Elephant → Mammoth | Native text decisions using original weights | Owner priority; training and all NVIDIA studies remain independent of this delivery gate. |
| First NVIDIA application | Search the Coax book with Nemotron embeddings | Reuses artifact/tokenizer/attention work and adds pooling plus a small meaningful retrieval evaluation. |
| First NVIDIA text generator | Minitron base-model continuation | Reuses dense inference, with its own precise configuration and tokenizer. |
| First vision study | C-RADIO features, asset retrieval, frozen-feature classifier | Adds image admission/preprocessing and feature conventions; the trained head uses shared autodiff. |
| Preferred first major Verse demonstration | MotionBricks SOMA motion → explicit Verse skeleton interface | Introduces motion representations, codebooks and masked generation with a visible consumer. |
| Small learning branch, as soon as autodiff exists | Train our own physical surrogate | Owned solver-generated data; no external checkpoint, huge corpus or image model prerequisite. |
| Product-selected breadth branches | Parakeet speech, Darcy FNO / DoMINO physics | Start when the necessary operators and a consumer exist; speech and physics need not wait for motion. |
| Advanced architectural studies | Nano recurrence → tiny MoE → Lightning; optional Bolt | Different model architectures earn separate qualification; names and active parameter counts are insufficient. |
| Later composite studies | One Cosmos3-Edge generation path; one simulated GR00T embodiment | Compose mature vision/language, codecs, generative methods and explicit task interfaces. |

A small imported SmolLM2 lab remains optional. It can make learning the artifact
reader practical on a CPU without introducing a prerequisite for urgent
decision-model delivery. Likewise, nanochat supplies a complete learning
pipeline; forward-only retrieval or motion need not wait for its RL chapter.

## Candidate checkpoints and the new work they teach

Links pin repository revisions observed in this survey. They are research
anchors, not qualified artifact manifests: implementation must still pin
individual file hashes, tensor coverage, reference-code revision, preprocessing,
licenses/notices and supported execution limits. Code referenced by a model
card is read as untrusted source; none of its scripts was executed here.

| Target and immutable revision | Added capability and first bounded scope | Qualification emphasis |
| --- | --- | --- |
| [Nemotron-3-Embed-1B-BF16](https://huggingface.co/nvidia/Nemotron-3-Embed-1B-BF16/tree/c0c9fea93ea424587517f2c59e20db9f1d6bf615) | Bidirectional text embeddings, average pooling and normalization; retrieve from a fixed Coax documentation collection. | Exact query/document rendering, padding masks, intermediate tensors, embeddings and held-out ranking. Card/config identify OpenMDW-1.1. |
| [Llama-3.1-Minitron-4B-Width-Base](https://huggingface.co/nvidia/Llama-3.1-Minitron-4B-Width-Base/tree/5205ef7d36204947e3b973cb8b147a816ccd7e6a) | Dense autoregressive continuation, GQA, RoPE, gated FFN and KV state. | Base model; exact full/cached logits and deterministic continuation. Its config specifies hidden size 3072, 32 query heads and head dimension 128: do not infer head dimension by division. Preserve NVIDIA model terms and Llama ancestry/notices. |
| [C-RADIOv3-B](https://huggingface.co/nvidia/C-RADIOv3-B/tree/44653a0482cf460bb4f12595fc3cc3dfecc403d1) | Image/spatial features, visual retrieval and a small trained downstream head. | Exact preprocessing, patches, position handling and feature-output conventions. Features alone do not qualify a depth or segmentation model. Weight and reference-code terms differ; see below. |
| [Nemotron-Mini-4B-Instruct](https://huggingface.co/nvidia/Nemotron-Mini-4B-Instruct/tree/791833e92ebddb0bc2c1007f6d2b6764f886a2ae) | Optional NPC/tool-format study with squared-ReLU and partial rotary embeddings. | Original `.bin` release requires a controlled inert export, not a general pickle reader. Separate chat formatting, architecture and NVIDIA model terms. |
| [NVIDIA-Nemotron-3-Nano-4B-BF16](https://huggingface.co/nvidia/NVIDIA-Nemotron-3-Nano-4B-BF16/tree/dfaf35de3e30f1867dd8dbc38a7fc9fb52d3914f) | Mamba-2/Transformer hybrid; begin with a tiny state-space recurrence. | Whole/chunked/token-step comparisons, convolution state, request isolation and complete checkpoint execution. Its terms are the NVIDIA Nemotron Open Model License. Qwen DeltaNet compatibility does not establish Mamba-2 compatibility. |
| [NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16](https://huggingface.co/nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16/tree/a9904d24bcc1d289a1950fa9d2b978c47cf903b9) | Hybrid recurrence/attention plus MoE after a tiny checkable router/expert example. | Admit total expert residency and scratch, not only active-per-token work. Establish floating-point correctness before NVFP4/speculation. Card identifies OpenMDW-1.1. |
| [NVIDIA-Nemotron-3.5-Bolt-E4B-BF16](https://huggingface.co/nvidia/NVIDIA-Nemotron-3.5-Bolt-E4B-BF16/tree/49843eed8db1f1c616c47af75cfd36e07aec0667) | Optional assistant study with a distinct Gemma-4-derived architecture. | Inventory the actual tensors and modalities; the effective-size name is not total storage. Exact profile needed independently of Nano/Lightning. Card identifies OpenMDW-1.1. |
| [MotionBricks](https://huggingface.co/nvidia/MotionBricks/tree/3cb362d9b95aa40462abcb4d8cfae9ffb6d97410) | SOMA first: root duration/trajectory → masked motion-token transformer → VQ-VAE pose decoder. | Full fixed-input trajectories, rotation/skeleton/time conventions and explicit Verse handoff. G1 is a distinct package. Model, software and sample-data terms are separate. |
| [Cosmos-0.1-Tokenizer-CI8x8](https://huggingface.co/nvidia/Cosmos-0.1-Tokenizer-CI8x8/tree/04792f8b318a26d9e54319887124a8848c3cd3ca) | Older image autoencoder teaching study: convolution, down/up-sampling, wavelets, attention and latent reconstruction. | Inspect intermediate/latent/reconstruction values and image boundaries. NVIDIA model terms. This exact tokenizer does not establish compatibility with current Cosmos3 codecs. |
| [domino_drivaerml](https://huggingface.co/nvidia/domino_drivaerml/tree/35b1bf1edafdaa2600d16182825890cd51c07427) | Geometry-to-field prediction: point neighborhoods/convolutions, signed distances, grids and local aggregation. | Complete preprocessing and geometry operators as well as surface/volume outputs; held-out physical errors on a small DrivAerML subset. Model card identifies NVIDIA Open Model terms. A small weight count does not imply a small operator port. |
| [fourcastnet3](https://huggingface.co/nvidia/fourcastnet3/tree/df5d8d005f2334c16502581abf0a510c634c295a) | Optional spherical weather branch with its own spherical operators and field conventions. | Model inputs include 72 variables on a 721×1440 grid; budget activations and geography, not only weights. Multi-step, geographically weighted forecast evaluation. Apache-2.0 model release; ERA5 data/processing terms remain a separate input. |
| [Cosmos3-Edge](https://huggingface.co/nvidia/Cosmos3-Edge/tree/344d602b128d1bbdacb43b08d0a3626f46343e29) | One bounded conditioned generation path using its autoregressive/diffusion towers, exact codecs and scheduler. | State which path/modalities work; reference tensors and task evaluation. OpenMDW-1.1 model release; the card includes NVIDIA-owned data, so complete original corpus reproduction is not established. |
| [GR00T-N1.7-3B](https://huggingface.co/nvidia/GR00T-N1.7-3B/tree/2fc962b973bccdd5d8ce4f67cc63b264d6886495) | Candidate VLA/flow-matching action model after a tiny simulated imitation policy. | Exact embodiment normalization, observations, action chunks and closed-loop simulation. Included license/card inconsistency remains unresolved; see below. |
| [parakeet-tdt-0.6b-v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3/tree/541d1f99c6b0c3cd0b11a95167540bb8edefd82b) | Audio features → FastConformer → token-duration-transducer transcription. | Sample/rate/window/mel/normalization identity, length propagation, decoder rules, WER/CER and selected-language evaluation. CC BY 4.0 weights. Streaming and TTS are separate capabilities. |
| [SmolLM2-135M](https://huggingface.co/HuggingFaceTB/SmolLM2-135M/tree/93efa2f097d58c2a74874c7e644dbc9b0cee75a2) | Optional non-NVIDIA small dense decoder import lab. | Exact tokenizer, tensor coverage, full/cached logits and continuation; Apache-2.0. Useful for bounded CPU teaching, without changing product priority. |

## Physics begins with our own model

After first-order autodiff, train a small supervised cooling/oscillator model
from a checked deterministic solver. Keep initial conditions and physical
parameters disjoint across training and evaluation. Measure one-step error,
longer rollout behavior and stability against that solver. This can ship
without a language model, vision model or pretrained scientific checkpoint.

Next build complex arithmetic and tiny DFT checks, then FFT/inverse and a
spectral layer. Reimplement the [PhysicsNeMo Darcy FNO example](https://github.com/NVIDIA/physicsnemo/tree/57466f08242eb4806cc202905b6393b60bc07c13/examples/cfd/darcy_fno)
as an architecture/training-recipe study, using bounded generated fields and
held-out solver comparisons. The inspected recipe generates its data; a recipe
is not a pretrained weight artifact. Speech and FNO share spectral primitives,
so neither needs completion of the other's application chapters.

DoMINO adds geometry-specific execution. FourCastNet3 is a separate spherical
specialty; it need not wait for DoMINO. A true PINN chapter requires an explicit
extension for input/higher-order derivatives and derivative checks. The initial
reverse-mode parameter tape does not automatically provide those operations.

## Keep weights, code, recipes and data distinct

These are four separate properties in every target manifest. Available weights
can enable inference without the original corpus; available inference code does
not establish a complete, distributable training recipe or dataset.

- **C-RADIO:** the [model card](https://huggingface.co/nvidia/C-RADIOv3-B/blob/44653a0482cf460bb4f12595fc3cc3dfecc403d1/README.md)
  gives NVIDIA model terms. The inspected [RADIO source license](https://github.com/NVlabs/RADIO/blob/4ffaf15d0ff48197c194e306772bdb5fb92db230/LICENSE)
  limits use of that code and derivatives to noncommercial research/evaluation.
  Do not copy its implementation into a commercial runtime under the weight
  license; record reference use and author Coax numerical work separately.
- **MotionBricks:** the [package card](https://huggingface.co/nvidia/MotionBricks/blob/3cb362d9b95aa40462abcb4d8cfae9ffb6d97410/README.md)
  distinguishes NVIDIA model terms from Apache-2.0 software and the separately
  restricted BONES motion samples. The [source license](https://github.com/NVlabs/motionbricks/blob/54fc563aeaaf819a753d72cc9fdc6713989cdd43/LICENSE)
  also distinguishes code from documentation/skills/assets. Check the specific
  asset terms before selecting fixtures. Full original training reproduction
  has not been established by inspecting inference packages.
- **GR00T:** the [card](https://huggingface.co/nvidia/GR00T-N1.7-3B/blob/2fc962b973bccdd5d8ce4f67cc63b264d6886495/README.md)
  describes commercial availability, while the included [LICENSE §3.3](https://huggingface.co/nvidia/GR00T-N1.7-3B/blob/2fc962b973bccdd5d8ce4f67cc63b264d6886495/LICENSE)
  restricts the work and derivatives to noncommercial research/evaluation.
  Keep this a conditional study and resolve applicable terms for the exact
  artifact before adopting it as a commercial default.
- **Cosmos:** the older tokenizer is one teaching artifact; the modern composite
  model gets its own complete manifest, codec and evaluation scope. Neither
  usable inference weights nor visually plausible output proves reproduction
  of original training or physically accurate simulation.

## Proposed data studies

Use owned generated data first. For later public examples, select small
versioned subsets with file hashes, original IDs, attribution, permitted
redistribution, split rules, transformation code and evaluation leakage checks.
Do not embed huge datasets in ordinary chapter checks.

| Dataset and inspected revision | Proposed use | Source terms / qualification |
| --- | --- | --- |
| [HelpSteer3](https://huggingface.co/datasets/nvidia/HelpSteer3/tree/f6d145777bcbde96137596340fab89793acd1031) | Pairwise ranking, a small reward model and preference experiments. | CC BY 4.0 card; preserve attribution and split IDs. |
| [Nemotron-Agentic-v1](https://huggingface.co/datasets/nvidia/Nemotron-Agentic-v1/tree/650d590978ca35c8f1ecea2faf136e5fac421b62) | Tool trajectories, loss masks and structured-output evaluation. | CC BY 4.0 card; dataset tool text remains data, not permission to execute tools. |
| [OpenCodeReasoning](https://huggingface.co/datasets/nvidia/OpenCodeReasoning/tree/20a1ca19c0d050fe9057fc08339d6b370ec1c67a) | Code SFT and bounded answer verification. | CC BY 4.0 card; retain underlying source provenance and terms. |
| [PhysicalAI-Robotics-Manipulation-SingleArm](https://huggingface.co/datasets/nvidia/PhysicalAI-Robotics-Manipulation-SingleArm/tree/73c00c18e7ca108e91846294b20a1d7cd5de9340) | A bounded imitation/data-handling exercise before a large VLA. | CC BY 4.0 card; specify embodiment, units, timing and train/test tasks. |
| [DrivAerML](https://huggingface.co/datasets/neashton/drivaerml/tree/5d448b209bf654503c64ce7261c34fa125f46392) | Geometry/flow preprocessing and DoMINO evaluation. | CC BY-SA 4.0; approximately 31 TB across 500 variants. Use a bounded subset and respect published splits. |
| [Granary](https://huggingface.co/datasets/nvidia/Granary/tree/0fe23a860e3570b111e79a05ebeefc53822161e2) | Transcription training/evaluation. | Published materials identify CC BY 4.0; original audio/source-corpus terms are separate. |
| [Nemotron-CC-v2](https://huggingface.co/datasets/nvidia/Nemotron-CC-v2/tree/2669787c66d18601c0e91167fd520be6c1245865), [v2.1](https://huggingface.co/datasets/nvidia/Nemotron-CC-v2.1/tree/ba6f2aaef7ada865bb08fc08640ca292150097db) | Possible later pretraining input, outside public book fixtures. | Cards point to the NVIDIA data agreement; availability is not permission to republish examples. |

The [inspected data agreement](https://huggingface.co/datasets/nvidia/Nemotron-Pretraining-Dataset-sample/blob/3ad096e6394e487bb4f778733300da85275bb449/LICENSE.md)
limits use to internal training and prohibits making the datasets available to
others. Keep that distinct from CC-licensed public fixtures. NVIDIA's
[available-data Nano recipe](https://github.com/NVIDIA-NeMo/Nemotron/blob/99a7e45302f486c7e104647c128c2962ae7695d6/docs/nemotron/nano3/pretrain.md)
explicitly uses a subset of the released model's original data and expects
different results. Rebuilding a methodology is a different claim from
reproducing its published checkpoint or original training run.

## Shared architecture and evidence

The [architecture](architecture.md) remains Coax-first. Native adapters handle
files, storage and device effects; Coax owns parsing/admission, preprocessing,
operators, model state and numerical execution. A Python/PyTorch reference is
development tooling, not the shipped implementation. Choosing NVIDIA weights
does not make TensorRT, Triton, CUDA or a hidden BLAS implementation part of
Psionic. A CUDA backend needs its own compiler/device milestone alongside the
proposed Metal work.

Broader models motivate explicit operator families: convolution and resampling;
complex/spectral and spherical operations; geometry neighborhoods; codebooks,
rotations and motion; state-space recurrence and MoE; diffusion/flow schedules;
audio and embodiment normalization. Add them from tiny independently checkable
examples when selected consumers require them. Preserve the simple dense CPU
path and its tests.

Measure memory from the actual tensor inventory/dtypes plus conversion buffers,
activations, state, scratch and device copies. A BF16 artifact expanded to f32
needs twice its weight bytes before these other costs. Expert activation counts
do not remove inactive weights from storage admission. Name the target machine
and peak budget before a full-model run; tiny fixture checks remain available
on ordinary development machines.

Every selected checkpoint chapter needs five forms of evidence: independent
operator checks, malformed-artifact refusals, pinned-reference intermediate and
output comparisons, state/lifetime/mask invariants, and task-level evaluation.
For each backend/profile, complete full-weight execution is required before
claiming model support. Retrieval rankings, motion constraints, transcription
errors and physical rollout errors provide application evidence beyond a
successful import or a plausible sample.
