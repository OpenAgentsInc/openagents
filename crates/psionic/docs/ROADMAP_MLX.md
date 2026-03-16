# Psionic MLX Roadmap

> Status: created 2026-03-16 after reviewing `ROADMAP.md`,
> `ARCHITECTURE.md`, `FRAMEWORK_CORE_ACCEPTANCE_MATRIX.md`,
> `TRAIN_SYSTEM.md`,
> `deep-research-mlx.md`, and
> `../../../docs/audits/2026-03-16-mlx-full-rust-port-into-psionic-audit.md`,
> and after reviewing the local upstream MLX checkout at `~/code/mlx` and the
> local MLX ecosystem checkout at `~/code/ivanf`.
>
> This is the lane-specific roadmap for building an MLX-class framework surface
> inside `crates/psionic/*`. It is intentionally narrower than
> `crates/psionic/docs/ROADMAP.md`: it is about the MLX-shaped array,
> transform, module, export, distributed-semantics, and package-ecosystem
> program in Rust, not the whole Psionic library roadmap.

Agent execution instruction: implement this roadmap in dependency order, not by
whichever backend or compatibility surface looks most tempting first. The first
goal is a Rust-native MLX-class substrate above current Psionic core crates.
Compatibility shells, bindings, and `.mlxfn`-style interop come later.

Reference-first instruction: MLX work must not be implemented from memory.
Choose the reference that owns the layer being changed:

- start with `~/code/mlx` for array semantics, lazy evaluation, function
  transforms, compile behavior, export behavior, distributed helper semantics,
  module trees, optimizer APIs, memory-management behavior, and upstream tests
- start with `crates/psionic/docs/ROADMAP.md` for the canonical full-program
  owner split and dependency order
- start with `crates/psionic/docs/ARCHITECTURE.md` for Psionic-wide runtime,
  artifact, receipt, cluster, sandbox, and security truth that MLX work must
  not bypass
- start with `crates/psionic/docs/FRAMEWORK_CORE_ACCEPTANCE_MATRIX.md` for the
  CPU-reference-first acceptance discipline
- start with `crates/psionic/docs/TRAIN_SYSTEM.md` for reusable optimizer,
  checkpoint, and distributed-train substrate that MLX-class framework surfaces
  should reuse rather than bypass
- start with `../../../docs/audits/2026-03-16-mlx-full-rust-port-into-psionic-audit.md`
  for the adaptation logic that explains what should be ported directly versus
  redesigned deliberately

Psionic-only execution rule: these references are semantic and test oracles
only. Do not shell out to, proxy through, FFI-wrap, or otherwise delegate
runtime behavior to MLX or Python when closing issues in this roadmap. The
shipped lane must remain Rust-native and Psionic-owned end to end.

## Decision

MLX belongs in a separate `ROADMAP_MLX.md`, not as a direct expansion of
`ROADMAP.md`.

That decision is deliberate for four reasons:

1. `ROADMAP.md` is already the canonical full-library roadmap and should remain
   the answer to "what is Psionic overall?"
2. MLX work cuts across Epic 1, Epic 2, Epic 4, Epic 5, and Epic 7 in the main
   roadmap, so adding the full issue program there would bloat the canonical
   doc and duplicate dependencies already tracked elsewhere.
3. The repo already uses lane-specific companion roadmaps such as
   `ROADMAP_METAL.md` and `ROADMAP_CLUSTER.md` for dependency-ordered
   deep dives.
4. The MLX lane needs its own issue program, compatibility boundaries, and
   parity-harness plan without implying that MLX is the only or canonical
   surface Psionic will expose.

So the structural rule is:

- `ROADMAP.md` remains canonical
- `ROADMAP_MLX.md` is the MLX-specific dependency-ordered deep dive

## Objective

Build an MLX-class framework surface inside Psionic with:

- a Rust-native lazy array API above the current framework core
- first-class function transforms including `grad`, `value_and_grad`, `jvp`,
  `vjp`, `vmap`, `checkpoint`, and compile-as-transform
- MLX-class module, parameter-tree, optimizer, scheduler, and quantized-layer
  semantics
- general array and model IO plus native function-export contracts
- framework-level distributed helpers above Psionic collectives and cluster
  truth
- backend-visible memory, cache, profiling, and debug tooling
- an `mlx-lm`-class text package and CLI above the native framework core,
  including prompt-cache and model-catalog workflows
- OpenAI-compatible text and multimodal serving surfaces with tool calling,
  structured output, logprobs, and explicit model-lifecycle truth
- `mlx-vlm`-class multimodal and `mlx-audio`-class speech or audio package
  families where the underlying Psionic substrate is real
- training-recipe and benchmark packages above `psionic-train` and
  `psionic-eval`, not sidecar Python-only ecosystems
- a bounded MLX compatibility shell only after the native substrate is real

This is not a plan to:

- add an MLX runtime dependency
- rewrite Psionic around Python
- turn Apple unified memory assumptions into cross-backend lies
- use MLX compatibility as a substitute for native semantics
- port every notebook, Gradio demo, or desktop shell from the MLX ecosystem
  into Psionic crates

## Relationship To The Main Roadmap

This roadmap is subordinate to `ROADMAP.md`.

It depends on and refines work already named there:

- Epic 1: framework-core completion
- Epic 2: semantics and compatibility
- Epic 4: backend truth and performance
- Epic 5: cluster, sandbox, and execution truth
- Epic 7: interop and adoption

This roadmap does not widen product scope in `docs/MVP.md`, and it does not
move ownership boundaries out of `docs/OWNERSHIP.md`.

## Ownership Rules

This roadmap must continue to respect `docs/OWNERSHIP.md`:

- `crates/psionic/*` owns the reusable MLX-class array, transform, module,
  export, distributed, backend, and parity-harness surfaces
- `apps/*` remain responsible for product UX and product control flows
- kernel and Nexus remain authority owners, not framework-execution owners

More specifically:

- `psionic-core`, `psionic-ir`, `psionic-compiler`, and `psionic-runtime`
  remain the compact lower substrate
- MLX-class public semantics should land above those crates rather than bloating
  them directly
- model-family code in `psionic-models` and request or response code in
  `psionic-serve` should consume the MLX-class framework layer rather than
  pretending to be that layer

## Why This Roadmap Exists

The MLX audit established a clear architectural answer:

> Psionic should not become Apple's MLX repo in Rust.
>
> Psionic should become MLX-class as a Rust-native framework while keeping
> receipts, manifests, replay, cluster truth, sandbox truth, and train or eval
> evidence explicitly Psionic-native.

That creates a distinct issue program that does not fit cleanly into one main
roadmap epic.

The biggest current gaps are:

- no public lazy array facade with explicit `eval` semantics
- no first-class public transform API above the current autodiff substrate
- no MLX-class `nn.Module` or parameter-tree system
- no general array save/load surface
- no function-export/import layer above the current IR
- no MLX-class framework-distributed API above collectives
- no dedicated MLX parity harness or version window
- no MLX-class package ecosystem above the framework surface for text,
  multimodal, audio, serving, training-recipe, or benchmark workflows

## Current Position

Psionic already has strong lower-layer fit for this work:

- `psionic-core` owns tensor metadata, dtype, layout, quantization primitives,
  and refusal taxonomy
- `psionic-ir` owns graph and reverse-mode autodiff substrate
- `psionic-compiler` owns deterministic lowering, replay identity, and cache
  identity
- `psionic-runtime` owns runtime truth, caches, residency, and backend
  diagnostics
- `psionic-train` already owns reusable optimizer math, model IO, and
  distributed optimizer contracts
- `psionic-collectives` and `psionic-cluster` already own lower-layer
  distributed substrate

What is missing is the MLX-shaped public semantics layer above that substrate.

## Ecosystem Evidence From `~/code/ivanf`

Reviewing `~/code/ivanf` clarified that "full MLX port" means more than the
upstream `mlx` framework repo.

The local ecosystem check shows concrete package layers Psionic does not yet
name explicitly in this roadmap:

- `mlx-lm` adds the text-model package expectations: `load`, `generate`,
  `stream_generate`, chat or batch helpers, Hugging Face conversion flows,
  prompt-cache artifacts, rotating KV caches, quantized KV caches, and a broad
  architecture-registry model family
- `maclocal-api` adds the serving expectations: OpenAI-compatible endpoints,
  model hot-load or unload, prompt-prefix caching, tool-call extraction,
  guided or JSON-shaped output, stop-sequence handling, logprobs, and
  reasoning-content extraction
- `mlx-vlm` shows that the practical MLX ecosystem now includes image, audio,
  and video-capable package and server surfaces, not only text LLM helpers
- `mlx-audio` shows a real speech and audio layer above MLX, including TTS,
  speech-to-speech, codec-style models, quantized checkpoint flows, and server
  packaging
- `mlx-lm-lora` and `unsloth-mlx` show a recipe layer above the framework and
  train substrate: LoRA, DoRA, QLoRA, DPO, ORPO, GRPO-family methods,
  synthetic-data generation, reward or judge stages, and GGUF or HF export
- `mlx-openbench` shows the benchmark or eval package discipline expected from
  a serious local model ecosystem
- `vllm-metal` is not a direct port target for Psionic, but it does confirm
  that scheduler-aware serving, cache policy, and long-context inference
  ergonomics are first-class ecosystem requirements rather than optional demos

The right implication is not "clone these repos in Rust."

The right implication is:

- keep the framework-native port first
- then add the package and service layer that makes the MLX lane actually
  usable as an ecosystem inside `crates/psionic/*`

## Success Bar

The MLX lane should be judged against five progressively stronger claims.

### Claim 1: `mlx-core-surface`

Psionic can expose a Rust-native lazy array, eval, device, stream, and transform
surface that feels like a real framework rather than just reusable internals.

### Claim 2: `mlx-library-usable`

Psionic can run common MLX-shaped inference and training code with real module,
optimizer, scheduler, loss, init, serialization, and export behavior.

### Claim 3: `mlx-distributed-usable`

Psionic can expose MLX-class framework-distributed semantics above its own
collectives and cluster truth, including gradient averaging, tensor-parallel
helpers, and FSDP-class update helpers.

### Claim 4: `mlx-ecosystem-usable`

Psionic can expose a real MLX-class package ecosystem above the native core for
text, multimodal, audio, serving, training recipes, and benchmark workflows.

### Claim 5: `mlx-compatible-bounded`

Psionic can make explicit bounded compatibility claims against a named upstream
MLX version window, backed by parity harnesses and explicit supported,
convertible, and unsupported matrices.

## Roadmap Shape

This roadmap is organized into eight epics.

| Epic | Theme | Outcome |
| --- | --- | --- |
| Epic 0 | Governance and parity discipline | one bounded MLX issue program and one explicit compatibility target |
| Epic 1 | Array and runtime surface | public lazy arrays, eval semantics, device or stream behavior, and host interop |
| Epic 2 | Transforms and compile | MLX-class transforms, custom transform hooks, and compile-as-transform |
| Epic 3 | `nn` and optimizers | module trees, state semantics, layers, losses, initializers, optimizers, schedulers, and quantized modules |
| Epic 4 | Export, serialization, and tooling | general array IO, native function export, optional `.mlxfn` interop, memory tools, and debug tools |
| Epic 5 | Framework distributed semantics | MLX-class distributed groups and helpers on top of Psionic collectives |
| Epic 6 | Backend closure and compatibility shell | CPU, Metal, CUDA, test closure, and bounded compatibility or bindings |
| Epic 7 | Ecosystem packages and service surfaces | `mlx-lm`-class text tooling, multimodal and audio packages, serving lanes, recipe APIs, and benchmark packages |

There is no GitHub issue queue opened from this block yet. This document is the
source of truth for opening that queue.

## Epic 0: Governance And Parity Discipline

### Goal

Define the bounded MLX target before implementation sprawls.

### Exit Criteria

- one named upstream MLX version window is frozen
- one MLX acceptance matrix exists
- one parity harness entrypoint exists
- compatibility language is explicit and bounded

### Issues

| ID | Status | Proposed GitHub issue title | Description |
| --- | --- | --- | --- |
| `PMLX-001` | landed | `Psionic MLX: create the lane-specific roadmap and issue program` | This document closes the issue. It records the decision to use `ROADMAP_MLX.md`, names the owner split, and seeds the full dependency-ordered issue queue. |
| `PMLX-002` | planned | `Psionic MLX: freeze the upstream MLX version window and compatibility scope` | Pick the exact upstream MLX version window Psionic will target for parity claims, document what "MLX-class" means versus "MLX-compatible", and prevent versionless drift. |
| `PMLX-003` | planned | `Psionic MLX: add an acceptance matrix for array, transform, nn, export, distributed, and backend closure` | Create the MLX-lane acceptance doc and machine-readable report contract so closure is measured against explicit capability families rather than one-off demos. |
| `PMLX-004` | planned | `Psionic MLX: build a parity harness runner seeded from upstream MLX test families` | Create one Rust-native parity runner that can import, mirror, or port upstream MLX test categories and emit a comparable pass, refusal, or unsupported report. |
| `PMLX-005` | planned | `Psionic MLX: publish a supported-convertible-unsupported compatibility matrix` | Define one matrix covering native support, compatibility shims, and intentionally unsupported MLX behaviors so later adoption claims stay bounded and reviewable. |

## Epic 1: Array And Runtime Surface

### Goal

Expose a real MLX-class lazy array surface above current Psionic core crates.

### Exit Criteria

- one public lazy array type exists
- `eval` and `async_eval` semantics are explicit
- device and stream behavior is public and typed
- indexing, views, broadcasting, creation, random, and host-materialization
  boundaries are real enough to support later transforms and `nn`

### Issues

| ID | Status | Proposed GitHub issue title | Description |
| --- | --- | --- | --- |
| `PMLX-101` | planned | `Psionic MLX: add a public lazy array facade above psionic-core and psionic-ir` | Introduce the public Rust-native array type and its graph-backed arithmetic surface without bloating `psionic-core` directly. |
| `PMLX-102` | planned | `Psionic MLX: define explicit eval and async_eval semantics plus materialization boundaries` | Expose MLX-class lazy evaluation rules, including explicit evaluation, implicit materialization triggers, and replay-safe execution boundaries. |
| `PMLX-103` | planned | `Psionic MLX: publish device and stream APIs with unified-memory capability flags` | Add public device and stream handles, dependency scheduling, and a capability-based unified-memory model that is honest on Apple, CUDA, and future backends. |
| `PMLX-104` | planned | `Psionic MLX: widen array creation, indexing, view, reshape, slice, concat, and broadcast families` | Close the public array-surface gap for standard construction and view families needed for MLX-style code and for later transform or module layers. |
| `PMLX-105` | planned | `Psionic MLX: add random, dtype-cast, and common creation families with deterministic policy` | Expose MLX-class random and creation APIs above the current deterministic RNG substrate, keeping seeded replay and device-scoped determinism explicit. |
| `PMLX-106` | planned | `Psionic MLX: define host interop, scalar item access, and tree utility boundaries` | Add safe host-materialization, scalar extraction, and tree utility contracts so higher layers can preserve lazy semantics without hidden eager fallbacks. |

## Epic 2: Transforms And Compile

### Goal

Turn current graph and autodiff substrate into MLX-class public transforms.

### Exit Criteria

- `grad`, `value_and_grad`, `vjp`, `jvp`, `vmap`, and `checkpoint` are public
- custom transform hooks exist
- compile is exposed as a callable transform with purity and debug rules
- shapeless or symbolic compile posture is explicit and tested

### Issues

| ID | Status | Proposed GitHub issue title | Description |
| --- | --- | --- | --- |
| `PMLX-201` | planned | `Psionic MLX: expose grad, value_and_grad, and vjp as first-class public transforms` | Lift current reverse-mode substrate into a user-facing transform layer with stable signatures, typed refusals, and test coverage against MLX behavior. |
| `PMLX-202` | planned | `Psionic MLX: add jvp and forward-mode autodiff to the public transform surface` | Extend the current autodiff substrate beyond reverse mode so the MLX lane can honestly support `jvp`-class use cases and transform composition. |
| `PMLX-203` | planned | `Psionic MLX: add vmap with explicit unsupported-op refusals and parity fixtures` | Add automatic vectorization with a public refusal matrix so unsupported cases fail honestly instead of hiding gaps behind fallback behavior. |
| `PMLX-204` | planned | `Psionic MLX: add checkpoint, custom_vjp, and custom transform registration hooks` | Expose gradient checkpointing and user-defined transform overrides in a reusable contract above IR internals. |
| `PMLX-205` | planned | `Psionic MLX: expose compile as a transform with purity, cache, and debug controls` | Turn current compiler substrate into MLX-class compiled-function behavior with explicit disable, trace, purity, and cache invalidation rules. |
| `PMLX-206` | planned | `Psionic MLX: add shapeless or symbolic compile behavior and trace-family cache identity` | Make shape-polymorphic compile and export rules explicit so later export or compatibility claims do not overpromise what compiled traces actually support. |

## Epic 3: `nn` And Optimizers

### Goal

Add the MLX-class module, layer, loss, init, optimizer, scheduler, and
quantized-module surface above current train primitives.

### Exit Criteria

- one public `Module` tree exists
- parameters, buffers, and frozen state are explicit
- core layers, losses, and initializers are real
- optimizer and scheduler APIs are public and reusable
- module-level quantization semantics are exposed

### Issues

| ID | Status | Proposed GitHub issue title | Description |
| --- | --- | --- | --- |
| `PMLX-301` | planned | `Psionic MLX: add a public Module tree with parameter, buffer, and freeze semantics` | Implement the MLX-class module container model, including trainable versus frozen posture and recursive parameter discovery. |
| `PMLX-302` | planned | `Psionic MLX: add save_weights and load_weights with strict and non-strict module-state behavior` | Build module-level state naming, load, and save semantics above existing model IO and refusal taxonomy rather than reusing serving-model loaders directly. |
| `PMLX-303` | planned | `Psionic MLX: land the core layer surface for linear, embedding, norms, activations, conv, pooling, and dropout` | Provide the minimum MLX-class layer families needed for common models and for parity coverage against upstream MLX tests. |
| `PMLX-304` | planned | `Psionic MLX: add losses, init families, and nn utility helpers` | Expose canonical loss functions, initialization utilities, and `nn` helper functions needed for practical training loops. |
| `PMLX-305` | planned | `Psionic MLX: build the public optimizer API on top of psionic-train optimizer primitives` | Reuse existing optimizer math, but publish a framework-native optimizer surface suitable for MLX-shaped model updates. |
| `PMLX-306` | planned | `Psionic MLX: add scheduler families, parameter-group behavior, and multi-optimizer composition` | Extend the optimizer shell to cover learning-rate schedules, parameter groups, and MLX-style multi-optimizer behavior. |
| `PMLX-307` | planned | `Psionic MLX: add quantized module families and module-level quantize behavior` | Expose module quantization and quantized layer wrappers above the existing quantization substrate without pretending file-format decode alone is quantization closure. |

## Epic 4: Export, Serialization, And Tooling

### Goal

Finish the general array, function-export, memory, and debug surfaces expected
from an MLX-class framework.

### Exit Criteria

- general array save/load exists
- one Psionic-native function export/import artifact exists
- optional `.mlxfn` compatibility is explicitly bounded
- memory counters, cache controls, and debug tools are public

### Issues

| ID | Status | Proposed GitHub issue title | Description |
| --- | --- | --- | --- |
| `PMLX-401` | planned | `Psionic MLX: add general array save and load APIs for npy, npz, safetensors, and gguf families` | Expose general array serialization above the current model- and checkpoint-focused IO surfaces, keeping dtype and layout truth explicit. |
| `PMLX-402` | planned | `Psionic MLX: add a Psionic-native function export and import artifact above the IR` | Build the native graph or function export story first, with stable signatures, trace families, and replay identity. |
| `PMLX-403` | planned | `Psionic MLX: add bounded mlxfn import and export compatibility on top of the native function artifact` | Only after the native function artifact exists, add optional `.mlxfn` interoperability with explicit supported and unsupported boundaries. |
| `PMLX-404` | planned | `Psionic MLX: expose memory-reporting and cache-control APIs to the public framework surface` | Publish active, peak, and cache memory counters plus cache-limit and reset controls above the current runtime diagnostics. |
| `PMLX-405` | planned | `Psionic MLX: expose backend debug, logging, and capture hooks for Metal and CUDA lanes` | Turn internal backend debug and profiling substrate into public framework tooling rather than leaving it as lane-local implementation detail. |
| `PMLX-406` | planned | `Psionic MLX: add a custom kernel authoring and extension surface for accelerated backends` | Publish the user-facing hook layer for MLX-class custom kernels and extensions while preserving Psionic's explicit backend capability and refusal truth. |

## Epic 5: Framework Distributed Semantics

### Goal

Expose MLX-class framework-distributed behavior above existing Psionic
collectives, cluster, and train substrate.

### Exit Criteria

- distributed groups and singleton no-op behavior are public
- collective helpers are exposed above lower-level runtime substrate
- gradient averaging and tensor-parallel helpers are public
- FSDP-class update helpers exist above distributed optimizer primitives
- launch and topology tooling maps onto Psionic control and evidence seams

### Issues

| ID | Status | Proposed GitHub issue title | Description |
| --- | --- | --- | --- |
| `PMLX-501` | planned | `Psionic MLX: add a public distributed group API with init, split, rank, and size semantics` | Expose framework-visible process-group semantics above current collectives and cluster layers, including explicit singleton behavior. |
| `PMLX-502` | planned | `Psionic MLX: expose all_sum, all_gather, reduce_scatter, send, and recv above the distributed group surface` | Publish the core collective helper layer with typed refusal and backend-capability reporting rather than forcing callers into low-level cluster APIs. |
| `PMLX-503` | planned | `Psionic MLX: add a framework launch and distributed-config surface mapped onto Psionic cluster truth` | Build the MLX-analogue of launch and hostfile tooling without bypassing Psionic cluster, sandbox, or evidence contracts. |
| `PMLX-504` | planned | `Psionic MLX: add average_gradients and grouped all-reduce helpers` | Expose tree-aware gradient reduction helpers for data-parallel training above the lower-level collectives layer. |
| `PMLX-505` | planned | `Psionic MLX: add tensor-parallel sharded linear helpers and module wrappers` | Publish the framework-level tensor-parallel helper family needed for MLX-class distributed module semantics. |
| `PMLX-506` | planned | `Psionic MLX: add an fsdp_apply_gradients-style helper on top of distributed optimizer contracts` | Reuse current train distributed-optimizer substrate to expose an MLX-style FSDP-class update helper above it. |
| `PMLX-507` | planned | `Psionic MLX: map ring, mpi, jaccl, and nccl capability families onto Psionic collectives and topology profiles` | Make backend-specific distributed capability explicit without treating MLX's backend names as global truth divorced from Psionic cluster semantics. |

## Epic 6: Backend Closure And Compatibility Shell

### Goal

Close enough CPU, Metal, CUDA, test, and compatibility work to make bounded
MLX claims honest.

### Exit Criteria

- CPU reference closure is broad enough to anchor parity claims
- Metal and CUDA lanes support the declared MLX-class surface honestly
- upstream MLX test families are ported or mirrored through one parity harness
- any MLX-facing compatibility or binding shells are explicit and bounded

### Issues

| ID | Status | Proposed GitHub issue title | Description |
| --- | --- | --- | --- |
| `PMLX-601` | planned | `Psionic MLX: close CPU reference operator coverage against imported MLX parity categories` | Use the CPU lane as the canonical semantic oracle for array, transform, compile, `nn`, and serialization behavior. |
| `PMLX-602` | planned | `Psionic MLX: close Metal backend coverage for the declared MLX-class surface` | Widen the Metal backend only through explicit capability contracts, stream semantics, and parity evidence rather than by inference from one serving lane. |
| `PMLX-603` | planned | `Psionic MLX: close CUDA backend coverage for the declared MLX-class surface` | Widen the CUDA backend with the same honesty rules, including explicit distributed capability and numerics behavior. |
| `PMLX-604` | planned | `Psionic MLX: add advanced operator-family closure for linalg, fft, fast kernels, and attention helpers` | Land the operator families that MLX users expect beyond basic dense tensor math, keeping unsupported surfaces explicit until real coverage exists. |
| `PMLX-605` | planned | `Psionic MLX: port the upstream MLX C++ and Python test categories into the parity harness` | Build the real evidence base for bounded compatibility by porting or mirroring the highest-value upstream test families. |
| `PMLX-606` | planned | `Psionic MLX: add an optional MLX naming facade and module-layout compatibility crate` | Only after native closure is real, add a thin compatibility shell that helps MLX users map concepts and API names without claiming full upstream closure. |
| `PMLX-607` | planned | `Psionic MLX: add optional Python, C, or Swift binding layers above the Rust-native core` | Bindings are explicitly late-surface work and must depend on the native substrate rather than freezing a Python-first architecture into the core. |
| `PMLX-608` | planned | `Psionic MLX: publish an MLX-to-Psionic migration guide, example suite, and bounded compatibility matrix` | Finish the adoption story with examples, migration steps, and explicit supported, convertible, and unsupported tables. |

## Epic 7: Ecosystem Packages And Service Surfaces

### Goal

Close the package and service layer that makes the MLX lane usable as an
ecosystem inside Psionic rather than only as a low-level framework port.

### Exit Criteria

- one `mlx-lm`-class text package and CLI exists above the native framework
- one reusable OpenAI-compatible served text lane exists with structured-output
  and tool-calling behavior
- one multimodal package family exists above the same core
- one audio package family exists above the same core
- one recipe layer exists above `psionic-train`
- one benchmark package exists above `psionic-eval`

### Issues

| ID | Status | Proposed GitHub issue title | Description |
| --- | --- | --- | --- |
| `PMLX-701` | planned | `Psionic MLX: add an mlx-lm-class text package with load, generate, stream, batch, and prompt-cache workflows` | Build the first text-model package and CLI above `psionic-models` and the MLX-class framework core, including chat templating, sampler composition, batch generation, rotating and quantized KV-cache behavior, and persisted prompt-cache artifacts. |
| `PMLX-702` | planned | `Psionic MLX: build model-catalog, Hugging Face cache, and architecture-registry workflows for MLX-class packages` | Add model-id resolution, local cache discovery, architecture-specific loader registration, conversion entrypoints, and explicit trust or refusal policy for remote processor or template metadata so the ecosystem can support `mlx-community`-style catalogs honestly. |
| `PMLX-703` | planned | `Psionic MLX: expose an OpenAI-compatible text-serving surface with tool calling, structured output, logprobs, and prefix caching` | Build a reusable served text lane in `psionic-serve` with chat or responses endpoints, streaming and non-streaming behavior, tool-call extraction, JSON-schema or guided-output posture, logprobs, stop-sequence handling, model hot-load or unload, reasoning-content extraction, and prefix-cache reuse. |
| `PMLX-704` | planned | `Psionic MLX: add a multimodal package and served surface for image, audio, and video inputs` | Land the `mlx-vlm` analogue above `psionic-models` and `psionic-serve`, including processor registries, multimodal prompt shaping, OpenAI-compatible image or audio request shapes, and bounded model-family coverage for VLM and omni models. |
| `PMLX-705` | planned | `Psionic MLX: add an audio package for TTS, speech-to-speech, codecs, and speech model IO` | Add the `mlx-audio` analogue for reusable audio generation and codec models, keeping UI shells out of Psionic while supporting library, CLI, and server surfaces, streaming audio outputs, quantized checkpoints, and honest voice or reference-conditioning posture. |
| `PMLX-706` | planned | `Psionic MLX: add a training-recipe layer for LoRA, DoRA, QLoRA, and preference or RL methods above psionic-train` | Build ergonomic MLX-class recipe APIs and CLIs on top of `psionic-train` for SFT, LoRA or DoRA or QLoRA, DPO or CPO or ORPO, GRPO-family methods, online DPO or XPO, PPO, and related bounded methods without creating a second trainer architecture outside Psionic. |
| `PMLX-707` | planned | `Psionic MLX: add synthetic-data, judge or reward-model, adapter-merge, and publish workflows` | Add synthetic SFT and preference dataset generation, reward or judge model training helpers, adapter merge or export, GGUF or Hugging Face publish pipeline, and lineage-bound dataset or output manifests so recipe work produces reusable artifacts rather than notebook-only side effects. |
| `PMLX-708` | planned | `Psionic MLX: add an openbench-class evaluation and benchmark package for local MLX-class providers` | Build the benchmark and eval package plus provider adapter layer that makes local MLX-text, multimodal, and served lanes easy to score across standardized tasks and local or private eval suites while reusing `psionic-eval` and receipt truth. |
| `PMLX-709` | planned | `Psionic MLX: publish ecosystem CLIs, examples, and migration guides without leaking product UX into app code` | Ship the package-facing CLI and example layer for text, multimodal, audio, serving, training recipes, and evaluation so the MLX lane is usable as an ecosystem in this repo, while keeping Gradio demos, desktop pickers, and product UX out of `crates/psionic/*`. |

## Current Execution Order

### Phase 1: freeze the MLX parity contract

- `PMLX-001`
- `PMLX-002`
- `PMLX-003`
- `PMLX-004`
- `PMLX-005`

### Phase 2: land the public lazy array and runtime surface

- `PMLX-101`
- `PMLX-102`
- `PMLX-103`
- `PMLX-104`
- `PMLX-105`
- `PMLX-106`

### Phase 3: land public transforms and compile

- `PMLX-201`
- `PMLX-202`
- `PMLX-203`
- `PMLX-204`
- `PMLX-205`
- `PMLX-206`

### Phase 4: land `nn`, optimizers, and quantized modules

- `PMLX-301`
- `PMLX-302`
- `PMLX-303`
- `PMLX-304`
- `PMLX-305`
- `PMLX-306`
- `PMLX-307`

### Phase 5: finish export, serialization, memory, and debug tooling

- `PMLX-401`
- `PMLX-402`
- `PMLX-404`
- `PMLX-405`
- `PMLX-406`
- `PMLX-403`

### Phase 6: land framework-distributed semantics above collectives and cluster truth

- `PMLX-501`
- `PMLX-502`
- `PMLX-503`
- `PMLX-504`
- `PMLX-505`
- `PMLX-506`
- `PMLX-507`

### Phase 7: close backend breadth and parity evidence

- `PMLX-601`
- `PMLX-602`
- `PMLX-603`
- `PMLX-604`
- `PMLX-605`

### Phase 8: close the package and service ecosystem above the native substrate

- `PMLX-701`
- `PMLX-702`
- `PMLX-703`
- `PMLX-704`
- `PMLX-705`
- `PMLX-706`
- `PMLX-707`
- `PMLX-708`
- `PMLX-709`

### Phase 9: only then add bounded compatibility shells, bindings, and migration facades

- `PMLX-606`
- `PMLX-607`
- `PMLX-608`

## Roadmap Rules

### 1. Do not put the full MLX issue program back into `ROADMAP.md`

This document exists so the canonical roadmap stays canonical.

### 2. Do not open compatibility shells before the Rust-native substrate is real

`PMLX-606` through `PMLX-608` are explicitly late.

### 3. Do not let Apple-specific unified-memory assumptions become cross-backend lies

Unified memory must be modeled as a capability, not a universal law.

### 4. Do not bypass Psionic system-truth layers

Function export, launch, distributed, and debug tooling must remain compatible
with Psionic manifests, receipts, replay identity, and cluster truth.

### 5. Do not confuse format compatibility with semantic closure

`.mlxfn`, `.npy`, `.npz`, or binding work do not substitute for actual array,
transform, `nn`, or distributed semantics.

### 6. CPU reference truth still gates backend claims

MLX parity claims are only honest if CPU reference semantics are green first.

### 7. Do not port notebook or web-UI convenience layers into Psionic crates

Package-facing CLIs, libraries, and service surfaces belong in Psionic.
Gradio apps, desktop pickers, and product UX do not.

## Bottom Line

The right structural decision is:

- keep `ROADMAP.md` as the canonical full-library roadmap
- put the MLX lane in `ROADMAP_MLX.md`

The right programmatic decision is:

- build a Rust-native MLX-class framework surface above Psionic's current core
- extend that core into a Psionic-owned MLX package ecosystem for text,
  multimodal, audio, serving, training recipes, and evaluation
- reuse current optimizer, IO, collectives, cluster, runtime, serve, and eval
  substrate
- delay compatibility shells and bindings until the native semantics are honest

This document is now the source of truth for opening the MLX issue program on
GitHub.
