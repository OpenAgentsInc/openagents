# After the open era: what to build if NVIDIA's ecosystem closes

**Date:** November 15, 2026. **Status:** speculation. This document qualifies
nothing, opens no issues and changes no priority. It supplements the
[robotics report](robotics-report-2026-11-15.md), the
[open robotics platform proposal](open-robotics-platform-2026-11-15.md) and
the [Microduck capstone](microduck-pipeline-2026-11-15.md), and widens them
from robotics to the whole stack that [Psionic](../../psionic/README.md) and
Coax are already rewriting.

The platform proposal assumed, as an exercise, that NVIDIA stops operating.
This document takes a likelier and harder case: NVIDIA keeps operating and
keeps selling hardware, but stops being the main sponsor of open models and
open tooling.

## 1. What happened, and what is assumed

Reported, as of this date:

- NVIDIA announced on November 12, 2026 that Jensen Huang had resigned as
  CEO, effective immediately. CNBC reported that the board had decided the
  night before to remove him, after an investigation into his personal
  conduct that the board says is continuing.
  ([CNBC](https://www.cnbc.com/2026/11/12/nvidia-jensen-huang-ceo-live-updates.html),
  [NBC News](https://www.nbcnews.com/tech/tech-news/nvidia-ceo-jensen-huang-resigns-reports-extramarital-relationships-rcna608402),
  [board statement](https://nvidianews.nvidia.com/news/statement-from-nvidia-board-of-directors))
- CFO Colette Kress is interim CEO while the board searches for a successor.
- The Hugging Face acquisition, signed in September and reported at about
  $13 billion, has not closed. Press coverage says its handling under the
  interim CEO is unclear.
  ([TechCrunch](https://techcrunch.com/2026/11/12/jensen-huangs-exit-puts-nvidias-support-for-open-source-in-question/))
- Commentators disagree about the effect on open work. Some call it a major
  setback for an American open-model ecosystem
  ([Interconnects](https://www.interconnects.ai/p/open-source-ai-without-jensen));
  others argue NVIDIA's commercial reasons to support open models outlast any
  one executive, while its investments in model labs get more scrutiny.
  A claim that open-source support itself motivated the removal is
  unsupported speculation and is not used here.

These facts come from search summaries of the linked pages. The pages
themselves could not be fetched from this session, so check them before
quoting any detail.

**Assumed for planning:** openness narrows by degrees over one to two years
rather than ending in one announcement. Nothing in this document depends on
knowing who the next CEO is.

## 2. How an ecosystem closes

Published source and weights under Apache, MIT or BSD terms stay usable.
Closure works through everything around them:

| Mechanism | What it looks like | What it breaks |
| --- | --- | --- |
| Releases stop | GR00T 2 code (promised for December), the next Nemotron, Cosmos or Newton release slips or never ships | The frontier moves behind a door while the last open version ages |
| New versions change terms | A successor model ships under a narrower licence, or as a hosted service only | Plans that assumed "the next one will be open too" |
| Existing terms have exits | OpenMDW-1.1 ends a licensee's grant if it brings patent or copyright claims over the model materials; reviewers on the OSI list said the clause reaches too far, and the licence is not OSI-approved ([license-review thread](https://lists.opensource.org/pipermail/license-review_lists.opensource.org/2026-August/006213.html)) | Certainty about weights held under it |
| Hosting disappears | Model repositories, NGC containers, asset servers and SimReady libraries go private or move | Any build that downloads at install time |
| The hub changes hands or policy | The Hugging Face deal closes under different leadership, is abandoned, or is completed and then restricted | Distribution for nearly every open model, and LeRobot |
| Maintainers leave | Newton, Warp, RAPIDS, Isaac Lab and cuOpt lose their paid engineers | Projects that are open but have one employer |
| Funding for open labs stops | Investments in labs that publish weights are cut back | The supply of new open weights from outside NVIDIA |
| The platform licence tightens | The CUDA EULA already restricts what may be done with the SDK ([EULA](https://docs.nvidia.com/cuda/eula/contents.html)); a stricter reading reaches translation layers and redistribution | Anyone whose independence strategy is "run CUDA elsewhere" |

Two conclusions follow. Retention is urgent and cheap, and it is a matter of
days. Replacement is slow and expensive, so it has to be chosen carefully.

## 3. Retain now

The platform proposal's rule stands: keep source, model artifacts, build
inputs and qualification evidence, each separately. Add urgency and an order:

1. **Weights and datasets we already execute or plan to:** GR00T N1.7 and its
   LIBERO head, Cosmos3-Edge, Parakeet, C-RADIO, MotionBricks, the Nemotron
   and Minitron checkpoints in the
   [breadth survey](../../psionic/docs/targets/nvidia-breadth.md), DoMINO,
   FourCastNet 3, the Human-1M public subset. File hashes, licence text as
   published on the day of download, and the model card.
2. **Reference source at pinned revisions:** Newton, Warp, Isaac Lab,
   Isaac-GR00T, the Microduck training and runtime repositories, MuJoCo,
   PhysicsNeMo, cuOpt, RAPIDS (cuDF, cuML, cuVS), Triton and Dynamo.
3. **Reference outputs while the references still run.** This is the item
   people skip. A frozen repository we can no longer build is a text to read.
   A repository plus recorded inputs, intermediate tensors and outputs is an
   oracle a rewrite can be tested against for years. Record them now, on
   hardware that still has working drivers.
4. **Build inputs:** driver and toolkit installers for the machines we own,
   container images, and the exact versions every reference needs.

Retaining a copy grants no right to redistribute it. Keep the proposal's
distinction between local retention, redistribution and modification, and
publish hashes with acquisition instructions where the bytes must stay
private.

A model hub with one owner is a single point of failure whoever owns it. The
useful public contribution is a mirror that needs no hub: artifacts named by
content hash, announced under Nostr identities, served by any Pylon that
chooses to carry them and paid for over Lightning. Psionic's artifact
admission already refuses bytes that do not match a pinned hash, so the
client half exists. The mirror, its discovery records and its payment rule
are new work, and the payment rule belongs in `pay/` as an `#authority` file.

## 4. The wider stack, and what a rewrite is worth

Robotics is one branch. The table ranks NVIDIA's library families by how
much an open rewrite would help people outside this repository, and names
where Psionic or Coax already has a foothold. "Foothold" means a bounded,
recorded profile, never a general replacement.

| Family | What it does | Foothold today | Worth of a rewrite |
| --- | --- | --- | --- |
| cuBLAS, cuDNN, cuSPARSE, cuSOLVER, cuFFT | The kernels under every framework | Coax writes its own arithmetic for CPU, CUDA and Metal; cuBLAS is allowed only as a benchmark comparator | **Highest.** Everything else sits on this, and section 5 argues it is one compiler problem |
| TensorRT | Compiles a trained graph for one device | Device plans and fused kernels in `psionic/lib/device/`; bounded ONNX import and export | **High.** A portable, checkable model compiler is what lets a model leave the datacentre |
| Triton, Dynamo, NIM | Serving, batching, routing, packaging | The resident decision service, the Responses server, the Pylon mesh | **High**, and already ours. Missing: multi-model scheduling and a published packaging format |
| Training at scale (the PyTorch-on-CUDA path) | Multi-GPU optimisation of large models | Autodiff, compiled backward, one useful-scale CUDA decoder training profile, data-parallel and sharded studies | **High.** [Chapter 250](../../psionic/book/ch250-can-someone-use-psionic-instead-of-pytorch.md) lists the gaps |
| RAPIDS: cuDF, cuML, cuGraph, cuVS | Dataframes, classical ML, graphs, vector search | Book search uses embeddings; no columnar engine | **High** for cuDF and cuVS. Every dataset pipeline needs a columnar reader, and LeRobot data is Parquet plus video |
| DALI, video codecs | Decoding and augmenting images and video | Image admission for C-RADIO and GR00T | **Medium.** Needed as soon as policies train from video |
| NeMo, Riva, Parakeet | Speech recognition and synthesis | Original Parakeet on Metal and CUDA; the voice engine | **Medium.** Recognition exists; learned synthesis is the gap |
| Warp, Newton, PhysX | Simulation kernels and solvers | Engine rigid-body and collision modules; the owned Panda components | **High**, and already planned in the robotics documents |
| Omniverse, OpenUSD, RTX | Scene description and rendering | The Metaverse Engine and its glTF path | **Medium.** A strict USD subset reader is worth having; a renderer clone is not |
| PhysicsNeMo, Earth-2 | Learned surrogates for physics and weather | Darcy, DoMINO and Fourier studies | **Medium.** Good teaching branches with real scientific users |
| cuOpt | Routing and linear or mixed-integer optimisation | None | **Medium.** Small, self-contained, and useful to fleets and maker networks |
| BioNeMo, Clara, MONAI, Parabricks | Biology, medical imaging, genomics | None | **Low for us.** Others are better placed; offer the kernel layer to them |
| cuQuantum, CUDA-Q, cuLitho, Sionna, Aerial | Quantum simulation, lithography, radio | None | **Not ours.** No consumer here |

Every row from TensorRT down depends on the first one. A rewrite that starts
at the kernels and the compiler makes the rest cheaper. A rewrite that
starts in the middle inherits a dependency on someone else's kernels.

## 5. Purpose-built languages and frameworks

The Python ecosystem is a scripting language holding together C++ kernels, a
vendor's GPU libraries, YAML configuration, pickled checkpoints and an
interchange format that each tool reads differently. It was assembled
incrementally and nobody would design it that way. Psionic's premise is that
one language with the right guarantees can replace the whole arrangement.
The opportunities below extend that premise. Each is a candidate. None is a
commitment.

**One kernel source, every device.** Coax already emits C11, Wasm, and bounded
CUDA and Metal compute, plus checked WGSL and SPIR-V for graphics. The
largest single step toward vendor independence is to carry compute onto
those last two: Vulkan SPIR-V compute, which
reaches AMD, Intel, Arm and NVIDIA through drivers that vendors ship anyway,
and WebGPU compute, which reaches browsers. A Phoronix test in October 2026
found llama.cpp's Vulkan backend still ahead of ROCm on AMD hardware for
most throughput measures
([Phoronix](https://www.phoronix.com/review/lemonade-vulkan-rocm/2)), which
suggests the portable path is not the slow path. Each target needs its own
measured profile; emitting a shader qualifies nothing.

**Do not clone CUDA.** ZLUDA translates compiled CUDA binaries and has lost
its commercial funding; SCALE recompiles CUDA source and is closed
([HPCwire](https://hpcwire.com/2026/07/09/spectral-compute-aims-to-set-cuda-free-will-it-succeed),
[Phoronix](https://phoronix.com/news/zluda-cuda-13.1-compatibility)). Both
keep the world writing CUDA and keep it exposed to one company's licence. A
language that never names a vendor in the source avoids the question.

**Shapes, dtypes, units and budgets in the types.** Most failures in a
Python model appear at run time, several minutes in: a shape mismatch, a
silent broadcast, an out-of-memory error. Coax has quantities and affine
point and vector types, and Psionic admits memory before it runs. Extending
this to tensor shapes and device residency as checked facts would make a
whole class of failures compile errors. That matters most to an agent
writing the code, which learns from a compiler message at once and from a
crashed training run slowly.

**Differentiation in the compiler.** Psionic has function transforms and a
compiled backward pass. Promoting them from library to language feature
means a simulator, a renderer and a model can be differentiated by the same
mechanism. That is Warp's central idea without Warp's Python host or its
NVIDIA-only GPU path.

**Models as data.** A checkpoint should be inert bytes plus a typed
description, with one identity covering both. Psionic has no general pickle
reader by design, reads original weight files and GGUF headers, exports
Safetensors, and pins artifacts by hash. The missing
piece is a published model-description format: enough to state the
architecture, preprocessing, tokenizer, normalisation and supported
requests, so that "does this runtime support this model" has a mechanical
answer. This is what ONNX and NIM each half-provide.

**A columnar data language.** Dataframes are the largest part of the Python
ecosystem that Psionic has not touched. A small columnar engine in Coax
(Arrow memory layout, Parquet reading, typed queries that compile to the
same kernels as tensors) would serve dataset preparation, evaluation tables
and the exposure ledger, and would give robotics its LeRobot interchange
without a Python runtime. Start from one real consumer, never from pandas
parity.

**A simulation language shared with the engine.** Solvers, collision,
articulation and sensors written once, running batched on any device and
differentiable where the mathematics allows. The robotics documents already
plan the solvers. The speculation here is only that they should be written
against the portable kernel layer from the start, so that "4,096 parallel
environments" stops being an NVIDIA-only sentence.

**Robot and controller descriptions with clocks and deadlines.** A strict
description format with frames, units, joint conventions and calibration,
and a real-time profile of Coax with bounded loops, no allocation and a
stated worst-case time, targeting a microcontroller. The Coax roadmap's
physical-world items point this way. Nothing here exists as a qualified
target.

**Experiments as checked values.** A training run, an evaluation and a
simulation batch described as typed jobs with inputs, budgets, seeds and
expected outputs, runnable on one machine or sent across Pylons, with a
receipt that says what ran. This replaces OSMO, most of what people use
Kubernetes for in research, and the shell scripts in between.

**Verification as the differentiator.** Mutation testing, `#authority`
files, replay classes, noise floors and support ledgers are what the Python
ecosystem lacks and cannot easily add. A rewritten library that states
exactly which models, operators, devices and precisions it supports, with
evidence, is more useful than a larger one that says "should work".

### Why a rewrite of this size is plausible now

Two things changed. Agents write most of the code, so the cost of porting a
library is dominated by knowing whether the port is right, not by typing it.
And the references still run today, so each port can be tested against
recorded outputs (section 3, item 3). The window in which both are true may
be short. That is the argument for recording oracles before writing code.

## 6. What stays hard

- **Hardware does not open.** A leadership change at NVIDIA does not make
  its GPUs less necessary for large training runs next year. Independence
  here means a floor that works without NVIDIA and acceleration that uses
  NVIDIA where it is installed, which is the repository's existing CUDA rule.
- **Weights are the scarce input.** No language produces a 64-billion-
  parameter world model. The supply of open weights will come from many
  labs, several of them outside the United States. Psionic's habit of
  admitting each architecture on its own evidence, whoever published it, is
  the right preparation.
- **Breadth is a trap.** The table in section 4 could absorb any number of
  engineers. The owner's order still holds: Elephant, Mammoth, then the
  selected branches. Each addition below needs a consumer.
- **Closed is not gone.** If NVIDIA keeps shipping good closed tools, many
  teams will use them. The open stack has to be better on the things it
  claims: portability, inspection, repair, and evidence.
- **Licences need a reader.** Several conclusions above turn on licence
  text. A person with legal training should read OpenMDW-1.1, the NVIDIA
  Open Model License and the CUDA EULA before anything is redistributed.

## 7. Proposed additions to current plans

Ordered by cost, cheapest first. Each is a proposal for the owner.

| # | Addition | First deliverable | Gate |
| --- | --- | --- | --- |
| 1 | Retention sweep | Pinned copies, hashes and day-of-download licence text for the section 3 list | A second machine rebuilds each retained reference offline |
| 2 | Oracle recording | Inputs, intermediates and outputs from each reference we intend to port | Recorded on a named machine and driver; replayable without the reference |
| 3 | Dependency ledger | One table of every NVIDIA-controlled input in the repository: toolkit, driver, hosted file, licence | Each row has an owner and a fallback, or says "none" |
| 4 | Vulkan compute target for Coax kernels | One Psionic kernel family running on a non-NVIDIA, non-Apple GPU | Matches the CPU reference; measured against CUDA on the same model |
| 5 | Model-description format | A published, versioned description for the models Psionic already runs | A second implementation can answer "supported or refused" from the file alone; registered in `proto/` |
| 6 | Hub-independent mirror | Content-addressed artifact announcements and fetch over existing identities | A clean machine fetches a pinned model with no model hub reachable |
| 7 | Columnar reader | Arrow layout and bounded Parquet reading, driven by the LeRobot interchange profile | Independent comparison against a reference reader on a fixed corpus |
| 8 | Portable batched simulation | The Microduck collector running on the Vulkan target | Solo-versus-batch invariance holds on two devices |
| 9 | Training across Pylons | One distributed training job type with admission, cancellation and a checker | Reproduces a single-machine result within its noise floor |
| 10 | Real-time Coax profile | Bounded-time subset and one microcontroller target | Measured worst case on the named board; no allocation proven |

Items 1 to 3 need no new code and lose value every week. Item 4 changes the
most for the least: it turns "CPU and Metal are the escape path" into "any
GPU is". Items 5 and 6 are the public goods other projects could adopt
without adopting Coax.

## 8. What to watch

Each of these would move the assumption in section 1 one way or the other:

- Whether GR00T 2 code ships in December as announced, and under what terms.
- The licence on the next Nemotron and Cosmos releases.
- Whether the Hugging Face acquisition closes, is renegotiated or is
  abandoned, and what the hub's terms say afterwards.
- Staffing and release cadence on Newton, Warp, Isaac Lab and RAPIDS.
- Any change to the CUDA EULA or to driver redistribution terms.
- Whether NVIDIA's investments in labs that publish weights continue.
- The successor's first statement about open models.

If most of these stay unchanged for two quarters, the urgency in sections 3
and 7 falls and the ordinary roadmap order applies. Items 1 to 3 are worth
doing in either case.

## Sources and limits

News and licence links above were found by web search on November 15, 2026.
Direct page fetches were blocked in the session that wrote this, so
statements about them rest on search summaries; the NVIDIA library
descriptions in section 4 are from general knowledge of those products and
were not rechecked against current documentation. Statements about this
repository follow the [Psionic README](../../psionic/README.md), the
[breadth survey](../../psionic/docs/targets/nvidia-breadth.md),
[chapter 250](../../psionic/book/ch250-can-someone-use-psionic-instead-of-pytorch.md),
the [Coax roadmap](../../coax/ROADMAP.md) and the two companion documents in
this folder. Where this document and a chapter's recorded evidence disagree,
the chapter is right.
