# After the open era: what to build if NVIDIA's ecosystem closes

**Date:** November 15, 2026. **Status:** speculation. This document qualifies
nothing, opens no issues and changes no priority. It supplements the
[robotics report](robotics-report-2026-11-15.md) (and its longer
[PDF edition](NVIDIA_Robotics_Report_2026-11-15.pdf)), the
[open robotics platform proposal](open-robotics-platform-2026-11-15.md) and
the [Microduck capstone](microduck-pipeline-2026-11-15.md), and widens them
from robotics to the whole stack that [Psionic](../../psionic/README.md) and
Coax are already rewriting.

The platform proposal assumed, as an exercise, that NVIDIA stops operating.
This document takes a different planning case: NVIDIA keeps operating and
keeps selling hardware, but stops being the main sponsor of open models and
open tooling.

## 1. What happened, and what is assumed

A narrow factual baseline, checked against primary documents on November 15:

- NVIDIA's [November 12 transition announcement](https://nvidianews.nvidia.com/news/nvidia-announces-ceo-transition)
  says Jensen Huang resigned and Colette Kress became interim CEO while
  remaining CFO. The board also says it expects operations, customer
  commitments and the product roadmap to continue. That is evidence of a
  leadership transition and the board's stated intentions; it is not an
  announcement that open work is ending.
- The [September 2 SEC filing](https://www.sec.gov/Archives/edgar/data/1045810/000104581026000078/nvda-20260902.htm)
  describes an agreement to acquire Hugging Face, approximately $11.9 billion
  payable to stockholders and up to $1 billion in employee retention, with
  closing expected in the first half of 2027 subject to conditions. It records
  commitments to an open platform and support for other silicon vendors.
  It is a dated account of a proposed transaction and forward-looking plans,
  not proof of what distribution will look like after closing.
- Commentary about the consequences remains commentary. The earlier
  [Interconnects essay](https://www.interconnects.ai/p/open-source-ai-without-jensen)
  and [TechCrunch coverage](https://techcrunch.com/2026/11/12/jensen-huangs-exit-puts-nvidias-support-for-open-source-in-question/)
  are leads to inspect, not the basis for predicting a shutdown. Their
  contents were available only as search summaries in the original revision.

The engineering argument does not require a theory about why a board acted,
who replaces the interim CEO, or whether a transaction closes. Our ability to
maintain a robot should survive any of those outcomes. The closure scenario
below remains an assumption to test our dependency choices against.

The PDF report in this folder has the same November 15 cutoff and does not
mention the change of CEO. Its account of products, licences and partners
is unaffected, but its strategic sections describe NVIDIA's direction under
the previous leadership.

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

Parts of the stack are already closed in the sense that matters here. The
PDF report's licence table (its section 16) lists them per artifact: cuMotion
may only be used on systems with NVIDIA GPUs, the cuVSLAM library has an
NVIDIA-platform restriction, GR00T N1.7's weights carry the NVIDIA Open
Model License while its code is Apache-2.0, Isaac Sim's Apache-2.0 source
sits beside separately licensed Kit components and assets, parts of Isaac
Capture have their own terms, and some healthcare weights are
noncommercial. The report also notes that permissive source does not make a
system hardware independent: CUDA dependencies, optimised operators, driver
support, rendering and test coverage all create practical dependence. A
narrowing would extend terms that already exist. It would not need to invent
new ones.

Two conclusions follow. Retain the selected inputs before their availability
changes, and choose replacements against actual consumers. Large checkpoints,
video corpora and old build environments have real storage, transfer and
maintenance costs. An indiscriminate archive can consume the same resources
needed to make one retained route work.

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
4. **Evaluation suites.** The report says evaluation is becoming a larger
   part of the platform (Isaac Lab-Arena, Cosmos Evaluator, the industrial
   dexterity tasks announced at CoRL). Task definitions and scoring code are
   small, and they are what makes a later claim comparable with published
   results. Retain them with the references.
5. **Build inputs:** driver and toolkit installers for the machines we own,
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

Shared kernels and compiler support can reduce duplication across these
rows. They do not eliminate the different algorithms, data formats, device
layouts, distributed protocols and maintenance each family requires. Extend
the lower layer when a selected workload exposes a missing operation; the
portfolio table is a map of possible reuse, not a prerequisite list for
finishing either robot.

## 5. Purpose-built languages and frameworks

Existing workflows span Python, C++ kernels, vendor libraries, configuration,
checkpoints and interchange formats. Each boundary is an integration job.
Coax and Psionic let us build shared arithmetic and contracts across more of
that path, while retained external tools supply independent comparisons.
The opportunities below extend that premise. Each is a candidate. A common
language still has to earn correctness, speed and usable deployment.

**One kernel source, every device.** Coax already emits C11, Wasm, and bounded
CUDA and Metal compute, plus checked WGSL and SPIR-V for graphics. The
largest single step toward vendor independence is to carry compute onto
those last two: Vulkan SPIR-V compute, which
reaches AMD, Intel, Arm and NVIDIA through drivers that vendors ship anyway,
and WebGPU compute, which reaches browsers. A Phoronix test in October 2026
found llama.cpp's Vulkan backend still ahead of ROCm on AMD hardware for
most throughput measures
([Phoronix](https://www.phoronix.com/review/lemonade-vulkan-rocm/2)), which
is one reported workload comparison, not a performance forecast for our
kernels or simulator. Each target needs its own measured profile; emitting a
shader qualifies nothing. Memory layout, transfers, synchronization and solver
structure can dominate a workload even when its arithmetic is portable.

**Do not clone CUDA.** ZLUDA translates compiled CUDA binaries and has lost
its commercial funding; SCALE recompiles CUDA source and is closed
([HPCwire](https://hpcwire.com/2026/07/09/spectral-compute-aims-to-set-cuda-free-will-it-succeed),
[Phoronix](https://phoronix.com/news/zluda-cuda-13.1-compatibility)). Both
keep the world writing CUDA and keep it exposed to one company's licence. A
language that never names a vendor in the source avoids the question.

**Shapes, dtypes, units and budgets in the types.** Shape mismatches,
unintended broadcasts and exhausted capacity can surface late in a run.
Coax has quantities and affine
point and vector types, and Psionic admits memory before it runs. Extending
this to tensor shapes and device residency as checked facts would make a
whole class of statically known failures compile errors; dynamic requests
still need runtime admission. That matters to an agent
writing the code, which learns from a compiler message at once and from a
crashed training run slowly.

**Differentiation in the compiler.** Psionic has function transforms and a
compiled backward pass. Promoting them from library to language feature
means a simulator, a renderer and a model can be differentiated by the same
mechanism. That is Warp's central idea without Warp's Python host or its
NVIDIA-only GPU path.

Contact switches, resets and discrete choices still need explicit derivative
or refusal conventions. Putting differentiation in a compiler does not make
every simulated operation smooth or choose a physically meaningful gradient.

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
receipt that says what ran. This could supply a common job contract for our
local and remote runners.
Scheduling, isolation, data movement, cancellation, retries and recovery still
need concrete implementations; typed jobs alone do not replace an orchestration
platform.

**Verification as the differentiator.** Mutation testing, `#authority`
files, replay classes, noise floors and support ledgers can be a consistent
contract across our compiler, simulator and models. Other ecosystems also
have testing and numerical verification tools; our useful distinction would
be making these records part of every supported release. A library that states
exactly which models, operators, devices and precisions it supports, with
evidence, is more useful than a larger one that says "should work".

### Why a rewrite of this size is plausible now

Agents reduce the cost of drafting and revising implementations in our
workflow. That makes independent references more valuable: correctness,
performance, integration and long-term maintenance still have to be earned.
Where a selected reference still runs, record its outputs before replacing
it (section 3, item 3). Treat the expected reduction in total porting cost as
a hypothesis to measure on complete workloads, rather than a reason to widen
the rewrite in advance.

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
- **A worker boundary is still a dependency.** The PDF report recommends
  (its section 18) keeping our own interface and connecting to an Isaac or
  `ovphysx` worker across a process boundary. That is sound advice while the
  worker stays available on today's terms. Under this document's assumption,
  use such a worker as an independent comparator for the owned
  implementation, which is the role MuJoCo already has, and never as the
  shipped runtime.
- **Their churn is our cost too.** The report lists renamed workflows and
  fast version changes as a risk for NVIDIA's own users. A reference we
  retain is one exact version; recorded outputs belong to that version and
  no other.
- **Licences need a reader.** Several conclusions above turn on licence
  text. A person with legal training should read OpenMDW-1.1, the NVIDIA
  Open Model License and the CUDA EULA before anything is redistributed.

## 7. Proposed additions to current plans

Candidate additions for the owner. The numbers identify proposals; they are
not cost estimates or a new implementation order.

| # | Addition | First deliverable | Gate |
| --- | --- | --- | --- |
| 1 | Retention sweep | Pinned copies, hashes and day-of-download licence text for the section 3 list | A second machine rebuilds each retained reference offline |
| 2 | Oracle recording | Inputs, intermediates and outputs from each reference we intend to port | Recorded on a named machine and driver; replayable without the reference |
| 3 | Dependency ledger | One table of every NVIDIA-controlled input in the repository: toolkit, driver, hosted file, licence | Each row has an owner and a fallback, or says "none" |
| 4 | Vulkan compute target for Coax kernels | One Psionic kernel family needed by a selected model or solver, on a named non-NVIDIA, non-Apple GPU | Matches declared CPU error bounds; complete-workload time and memory measured on that device |
| 5 | Model-description format | A published, versioned description for the models Psionic already runs | A second implementation can answer "supported or refused" from the file alone; registered in `proto/` |
| 6 | Hub-independent mirror | Content-addressed artifact announcements and fetch over existing identities | A clean machine fetches a pinned model with no model hub reachable |
| 7 | Columnar reader | Arrow layout and bounded Parquet reading, driven by the LeRobot interchange profile | Independent comparison against a reference reader on a fixed corpus |
| 8 | Portable batched simulation | The Microduck collector running on the Vulkan target | Solo-versus-batch invariance within each qualified device profile; separate numerical bounds across devices |
| 9 | Training across Pylons | One distributed training job type with admission, cancellation and a checker | Reproduces a single-machine result within its noise floor |
| 10 | Real-time Coax profile | Bounded-time subset and one microcontroller target | Allocation/work bounds checked; deadline and fault measurements on the named board/load; any formal worst-case claim needs its own proof |

Item 3 can start from the PDF report's licence table and its 92-entry
source directory, which already name most of the artifacts and their terms.

Items 1 to 3 can begin with existing tools, but retaining, rebuilding and
recording references still takes work. Item 4 qualifies one device and one
workload at a time; it cannot turn a single kernel result into “any GPU”
support. Items 5 and 6 could be public goods other projects adopt without
adopting Coax.

### Engineering judgment: make the next route deeper before making it wider

The immediate missing loop is still a collector feeding a real neural update,
then a reproducible evaluation. Chapter 257's experiment record and chapter
258's illustrative servo are implemented CPU components. Chapter 259 now has
shared statistics, while collection and its freeze/version lifecycle remain
open. None of that is a learned biped or a closed robotics pipeline. The
[implementation roadmap](../../psionic/docs/robotics/microduck-implementation-roadmap.md)
and its receipts are the completion ledger.

For any proposed rewrite above, first name the consumer, its exact missing
operation, an independent answer, a resource budget and the result that would
make us stop or keep the existing path. A video reader earns work when a
selected dataset needs it; a new device target earns work when a measured
model or solver bottleneck needs it. General dataframe, USD, fleet and model
hub ambitions need not stand between the current small collector and its
first neural training run. The wider table remains optional scope.

The first continuity result should be a second builder running one complete
retained route with vendor services unavailable. Start with ordinary permitted
copies and a manifest. A distributed paid mirror can follow a demonstrated
availability problem; it needs discovery, rights, delivery checking and payment
recovery in addition to a content hash. Model identity detects changed bytes.
Availability, permitted distribution and numerical usefulness are separate
properties to establish.

If NVIDIA keeps its open commitments, this work should still pay for itself
in reproducibility, repair and a second maintained execution path. That is a
stronger reason to do it than needing the worst-case forecast to come true.

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

The [PDF report](NVIDIA_Robotics_Report_2026-11-15.pdf) is the retained longer
research edition, with a numbered source directory, a history from 2014 and
more detail on licences, evaluation and deployment evidence. This follow-up
particularly uses sections 16–18 on artifact terms, practical limits and a
custom engine. Its product map and our proposed implementation choices have
different purposes; the PDF itself is unchanged.

The transition announcement and acquisition filing in section 1 were read
directly for the November 15 follow-up. Other news/commentary and licence
links retain the original limitation: their earlier assessment used search
summaries when direct fetches were blocked. The NVIDIA library descriptions
in section 4 were not comprehensively rechecked in this follow-up. The new
engineering judgments are proposals, not additional research results or
implemented capabilities. Statements about this repository follow the
[Psionic README](../../psionic/README.md), the
[breadth survey](../../psionic/docs/targets/nvidia-breadth.md),
[chapter 250](../../psionic/book/ch250-can-someone-use-psionic-instead-of-pytorch.md),
the [Coax roadmap](../../coax/ROADMAP.md) and the two companion documents in
this folder. Where this document and a chapter's recorded evidence disagree,
the chapter is right.
