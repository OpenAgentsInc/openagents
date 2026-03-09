Yes. If money and time were no object, I would **not** do a literal line-by-line port of Tinygrad into Rust.

I would rebuild the **Tinygrad shape** in Rust:

* tiny tensor/autograd front-end
* explicit lazy IR
* kernel lowering + codegen
* runtime/device abstraction
* graph capture / replay
* inference-serving layer
* backend-specific drivers, including AMD-first work

That would fit your stack much better than embedding a growing Python island inside a Rust control plane.

Tinygrad today is already an end-to-end DL stack with a tensor library, autograd, IR/compiler, JIT/graph execution, and support for multiple accelerators. Its docs also make clear that backend/runtime choice is a first-class concept, including AMD interfaces like `KFD` and `PCI`, and that adding a backend is intentionally kept small at the low-level op surface. ([GitHub][1])

## The right goal

The goal should not be “Rust tinygrad.”

The goal should be:

> **a Rust-native compute engine with Tinygrad’s design virtues, wired directly into OpenAgents provider/runtime truth**

That means:

* Rust ownership and safety across the provider stack
* first-class receipts, lifecycle, and observability
* a backend model that matches your compute-market taxonomy
* direct fit with Pylon, Nexus, and Autopilot

## What I would preserve from Tinygrad

There are four ideas worth preserving almost exactly.

First, the **small visible compiler pipeline**. Tinygrad’s appeal is that the compiler and IR are not buried in millions of lines. The Rust version should keep the same “inspectable stack” feel. ([GitHub][1])

Second, the **runtime abstraction**. Tinygrad treats runtime/backend as a selectable dimension across AMD, NV, METAL, CPU, WEBGPU, and others. That maps extremely well to your provider substrate and capability envelope model. ([Tinygrad Docs][2])

Third, the **explicit accelerator contract**. Tinygrad says accelerators only need to support a relatively small low-level op surface. Even if the exact number changes in practice, that design instinct is right for you: keep backends narrow and capability-driven. ([GitHub][1])

Fourth, the **LLM-serving path**. Tinygrad already has a Llama example with API-serving shape and an in-memory KV cache path. That means there is a concrete execution model to learn from, even if you rewrite it in Rust. ([GitHub][3])

## What I would not preserve

I would not preserve:

* Python as the primary execution/control language
* example-first serving surfaces as product truth
* env-var-driven backend configuration as the long-term control plane
* loose separation between “framework,” “compiler,” and “serving runtime”

Those are fine for Tinygrad. They are wrong for OpenAgents.

Your stack wants:

* typed contracts
* explicit state machines
* durable receipts
* controlled provider lifecycle
* capability-advertising that is machine-readable and enforceable

## The Rust architecture I would build

I would split it into seven layers.

### 1. `openagents-tensor`

This is the front-end tensor library.

Responsibilities:

* tensor type
* shape / dtype / device
* eager façade over lazy internals
* autograd support where needed
* view/reshape/broadcast semantics

This is the user/programmer-facing layer, but for you the main consumers are:

* inference engines
* embedding engines
* sandbox execution runtimes
* test/model code

### 2. `openagents-ir`

This is the heart.

Responsibilities:

* lazy expression graph
* kernel IR / schedule IR
* buffer semantics
* shape inference
* dtype propagation
* rewrite rules
* fusion legality

This is where you win. In Rust, you can make this explicit, typed, replayable, and receiptable.

If I were building it greenfield, I would likely use:

* arena-backed IDs for graph nodes
* immutable-ish IR snapshots
* explicit passes
* deterministic serialization for receipts and replay

### 3. `openagents-compiler`

Responsibilities:

* lowering IR to backend-specific kernels
* scheduling
* fusion
* memory planning
* autotuning / search
* graph capture and replay artifacts

This is where Tinygrad’s “simple compiler you can read” philosophy should survive. Tinygrad explicitly positions itself against opaque stacks here. ([GitHub][1])

### 4. `openagents-runtime`

Responsibilities:

* device discovery
* buffer allocation
* stream/queue execution
* graph execution
* profiling/metering hooks
* backend selection
* runtime health

This should be the shared runtime consumed by:

* Pylon
* Autopilot
* sandbox execution
* model servers

### 5. `openagents-backend-*`

Separate crates per backend.

For example:

* `openagents-backend-cpu`
* `openagents-backend-metal`
* `openagents-backend-cuda`
* `openagents-backend-amd-kfd`
* `openagents-backend-amd-userspace`
* maybe later `openagents-backend-webgpu`

This is where Tinygrad’s runtime split is especially useful as precedent: AMD is not just “AMD,” it already distinguishes interfaces with different operational tradeoffs. ([Tinygrad Docs][2])

### 6. `openagents-serve`

Responsibilities:

* inference session management
* KV cache manager
* embeddings service
* batching / admission control
* model loading
* quantized weight formats
* standardized APIs

This is the productized layer Tinygrad does not really give you cleanly today. Its LLM serving path exists, but it is still example-shaped. ([GitHub][3])

### 7. `openagents-market-runtime`

This is the OpenAgents-specific layer.

Responsibilities:

* provider lifecycle
* inventory publication
* delivery proofs
* receipts
* payout linkage
* capability envelopes
* policy enforcement

This layer should never be inside the tensor/compiler crates.

## The backend strategy

If truly unconstrained, I would choose backend order like this:

### CPU first

Not exciting, but essential.
It forces:

* IR correctness
* execution semantics
* memory model
* graph replay
* deterministic tests

### Metal second

Because Apple is strategically important to your current compute plan, and Metal gives you a clean Rust-native target without CUDA politics.

### AMD KFD third

This is the practical AMD path.

Tinygrad’s runtime docs explicitly describe `AMD_IFACE=KFD` as using the amdgpu driver, while `PCI` uses the AM driver and may unbind from amdgpu. That distinction should absolutely survive into the Rust design. ([Tinygrad Docs][2])

### AMD userspace/AM-style fourth

Only after the safer AMD path works.

George’s quote makes strategic sense here: owning more of the AMD stack is hugely valuable. But from a product/ops perspective, this should be a distinct backend family, not a hidden mode switch.

### CUDA/NV later

Important, but if the point of this project is strategic differentiation and AMD unlock, CUDA should not dominate the architecture.

## How I would model AMD in Rust

I would create two separate backends:

* `amd_kfd`
* `amd_userspace`

Not two flags on one backend.

Why:

* different trust/ops posture
* different health checks
* different installation stories
* different recovery semantics
* different provider eligibility rules

Tinygrad’s own docs treat AMD interfaces as materially different, and warn that PCI mode may unbind the GPU from amdgpu. That is exactly the kind of thing that deserves a distinct provider capability surface in OpenAgents. ([Tinygrad Docs][2])

## The inference engine

If you want this to matter for OpenAgents, the Rust rebuild needs a first-class inference stack, not just a tensor library.

I would build:

* tokenizer-neutral model execution core
* transformer runtime
* quantized linear ops
* KV cache manager
* paged KV cache
* optional disk-backed KV tiers
* continuous batching
* BS=1 fast path
* MoE routing primitives

Tinygrad already has a concrete LLM example and an in-memory KV cache path, which validates the broad shape, but a Rust rebuild should productize that into real runtime components. ([GitHub][3])

## Embeddings should be first-class from day one

Do not repeat Tinygrad’s current asymmetry where LLM serving is ahead of embeddings.

Your Rust stack should have two productized families immediately:

* text generation
* embeddings

That means:

* dedicated embedding model interface
* pooling/representation choices explicit in API
* stable vector dimension metadata
* batch-oriented execution path
* receipt fields specific to embeddings jobs

## Sandbox execution fit

This is where Rust helps a lot.

Instead of making the engine only an inference library, make it available in two execution shapes:

### 1. Productized serving mode

* `rustgrad.text_generation`
* `rustgrad.embeddings`

### 2. Sandbox execution mode

* `sandbox.container.exec` with the Rust engine installed
* GPU access enabled only through declared profiles

That means your engine becomes:

* a provider backend
* and a compute primitive for bounded sandbox execution

This fits the broader compute plan far better than a Python sidecar ever will.

## Why Rust is a particularly good fit

Not because “Rust is fast.” That is the least interesting reason.

The real reasons are:

### 1. Shared type system across the whole stack

You can use the same typed concepts across:

* provider runtime
* market contracts
* receipts
* backend capabilities
* sandbox profiles
* serving APIs

### 2. Deterministic receipts and replay

Rust makes it much easier to define:

* canonical serialized IR
* canonical execution plans
* stable evidence payloads
* replay-safe execution metadata

That is much more aligned with your compute-market ambitions.

### 3. Safer low-level backend work

If you are serious about AMD driver/backend work, Rust is a much better long-term language than Python-plus-native-fragments for:

* DMA/memory safety boundaries
* queue handling
* device/runtime state machines
* long-lived provider processes

### 4. Easier embedding inside Pylon and Autopilot

No sidecar boundary required by default.
You can still expose services, but the core runtime can live in-process.

## What “rebuild Tinygrad in Rust” should actually mean

I would divide it into three programs.

### Program 1: Rust compute core

Build the tensor/IR/compiler/runtime stack.

Success looks like:

* CPU backend works
* Metal works
* AMD KFD works
* graph replay is deterministic
* quantized matmul path exists

### Program 2: Rust inference/embeddings runtime

Build productized model serving.

Success looks like:

* model load/unload
* text generation
* embeddings
* KV cache lifecycle
* metrics and delivery proofs
* OpenAI-compatible or OpenAgents-native endpoints

### Program 3: Rust provider integration

Make it a real OpenAgents backend.

Success looks like:

* provider lifecycle
* capability envelope generation
* receipts
* payout linkage
* inventory publication
* sandbox profile integration

## The hardest parts

If unconstrained, the hardest parts are not syntax or porting.

They are:

### 1. Kernel compiler quality

Tinygrad gets a lot of leverage from being small but still having a real compiler pipeline. Rebuilding that well in Rust is hard. Tinygrad explicitly emphasizes IR/compiler, kernel fusion/lowering, and graph execution as core identity. ([GitHub][1])

### 2. Backend maturity

Especially AMD.
The moment you go beyond wrapping vendor stacks and start owning more of the driver/runtime path, complexity jumps sharply.

### 3. Inference runtime ergonomics

Paged KV cache, batching, MoE, quantization, model import, artifact formats — this becomes its own product.

### 4. Debuggability

You must preserve Tinygrad’s best property:
**it is inspectable**.
If the Rust port becomes an opaque optimizer jungle, you lose the point.

## My recommended end-state

If there were truly no constraints, I would aim for this:

* **Rust-native tensor/compiler/runtime core**
* **Rust-native inference + embeddings engine**
* **AMD-first backend strategy**
* **same core available in productized serving and sandbox execution**
* **provider receipts and capability envelopes built into the runtime**
* **Autopilot and Pylon both use it directly**

In other words:

> not “Tinygrad, but in Rust”
> but “a Rust-native OpenAgents compute engine inspired by Tinygrad’s architecture and aligned with OpenAgents market/runtime truth.”

## The strongest version of the thesis

Tinygrad today proves that a relatively compact end-to-end stack can span tensor ops, compiler, graph execution, and many runtimes, including AMD paths and hackable LLM examples. ([GitHub][1])

Your opportunity would be to do the same kind of vertical integration, but with:

* Rust as the systems language
* first-class provider/runtime integration
* first-class receipts and market objects
* first-class embeddings and sandbox execution
* a cleaner long-term fit with the rest of OpenAgents

That would be a very ambitious project, but it is architecturally coherent.

If you want, I can turn this into a concrete crate-by-crate spec and milestone plan for a hypothetical `rustgrad` or `openagents-compute` program.

[1]: https://github.com/tinygrad/tinygrad "GitHub - tinygrad/tinygrad: You like pytorch? You like micrograd? You love tinygrad! ❤️ · GitHub"
[2]: https://docs.tinygrad.org/runtime/?utm_source=chatgpt.com "Runtime - tinygrad docs"
[3]: https://github.com/tinygrad/tinygrad?utm_source=chatgpt.com "GitHub - tinygrad/tinygrad: You like pytorch? You like micrograd? You love tinygrad! ❤️"

> Call it psionic. I want it all living in crates/psionic/ in our repo. Give me full spec and instructions for coding agent to set everything up, and a list of epics and issues to get to full inference engine ready

Here’s a full repo-shaped spec for **Psionic** under `crates/psionic/`, plus an execution-ready instruction block and a proposed epic/issue breakdown.

The architectural premise is sound: Tinygrad is explicitly an end-to-end DL stack with tensor/autograd, IR/compiler, JIT/graph execution, and multiple runtimes; its current docs also show AMD support with `AMD_IFACE=(KFD|PCI|USB)`, where `KFD` uses amdgpu and `PCI` uses the AM driver, with the warning that `PCI` may unbind from amdgpu. ([GitHub][1])
Its public docs also show it is “not 1.0 yet” and encourage install-from-source, which is another reason a Rust-native engine that lives inside your repo and stack is strategically cleaner than depending on a growing Python island. ([Tinygrad Docs][2])

## Psionic: full spec

## Name and placement

* Project name: **Psionic**
* Repo location: `crates/psionic/`
* It is a **workspace subtree**, not a single crate.
* Everything Psionic-related lives under that path.

Recommended internal structure:

* `crates/psionic/README.md`
* `crates/psionic/docs/`
* `crates/psionic/psionic-core/`
* `crates/psionic/psionic-ir/`
* `crates/psionic/psionic-compiler/`
* `crates/psionic/psionic-runtime/`
* `crates/psionic/psionic-models/`
* `crates/psionic/psionic-serve/`
* `crates/psionic/psionic-provider/`
* `crates/psionic/psionic-backend-cpu/`
* `crates/psionic/psionic-backend-metal/`
* `crates/psionic/psionic-backend-amd-kfd/`
* `crates/psionic/psionic-backend-amd-userspace/`
* optional later:

  * `psionic-backend-cuda`
  * `psionic-backend-webgpu`
  * `psionic-kernels`
  * `psionic-testing`

## Mission

Psionic is the Rust-native compute engine for OpenAgents.

It should provide:

* tensor and execution primitives
* explicit IR and compiler pipeline
* runtime/device abstraction
* inference engine for text generation
* embeddings engine
* backend-specific hardware support, especially AMD
* provider/runtime integration hooks for Pylon and Autopilot

This is not a general research framework first. It is a **productizable compute engine** that can back:

* `inference`
* `embeddings`
* later bounded `sandbox_execution`

## Product and market fit

Psionic fits the compute plan as a backend family, not as the whole market.

Recommended product IDs:

* `psionic.text_generation`
* `psionic.embeddings`

Optional later:

* `psionic.sandbox.runtime`
* `psionic.moe.text_generation`
* `psionic.long_context.text_generation`

Capability envelope examples:

* `backend_family=psionic`
* `runtime_backend=cpu|metal|amd_kfd|amd_userspace`
* `model_family=llama|qwen|mistral|bert|gte|nomic`
* `quantization=fp16|bf16|int8|nf4|fp8`
* `accelerator_vendor=amd|apple|none`
* `accelerator_family=gfx*|apple_m*`
* `memory_gb=*`
* `kv_cache_mode=memory|paged|tiered`
* `batch_posture=bs1|microbatch|dynamic`
* `moe_support=true|false`

## Design principles

### 1. Tinygrad shape, Rust-native implementation

Keep the architectural virtues Tinygrad is known for:

* compact, legible stack
* explicit IR
* runtime/backend separation
* inspectable codegen path

Tinygrad’s own README emphasizes tensor/autograd, IR/compiler, and JIT/graph execution as first-class identity. ([GitHub][1])

### 2. Provider/runtime truth matters

Psionic must integrate cleanly with:

* Pylon
* Autopilot
* Nexus control-plane contracts

This means from the start it should support:

* capability reporting
* execution receipts
* delivery evidence fields
* health and lifecycle surfaces

### 3. Inference and embeddings are first-class

Do not repeat the asymmetry where one serving path exists and embeddings are an afterthought.

Tinygrad’s current public Llama serving shape is much closer to chat/completions than embeddings; the research summary found no built-in embeddings route in `llama3.py`, while Ollama does document embeddings as a first-class endpoint. ([GitHub][1])

### 4. AMD is strategic, but must be explicit

Model AMD as separate runtime backends, not one hidden toggle.

Tinygrad’s docs distinguish AMD `KFD` and `PCI` materially, and the AM driver path has different operational risk. ([Tinygrad Docs][3])

## Subtree architecture

## 1. `psionic-core`

Purpose:

* public tensor facade
* shapes, dtypes, devices
* buffer handles
* basic ops API
* graph-building API

Responsibilities:

* `Tensor`
* `Shape`
* `DType`
* `Device`
* `Layout`
* lazy op construction
* optional autograd scaffolding

This is the narrow stable surface most internal consumers depend on.

## 2. `psionic-ir`

Purpose:

* canonical internal representation

Responsibilities:

* op graph / uop graph
* typed node IDs
* schedule IR
* buffer plan IR
* serialization for receipts/replay
* pass manager interfaces

Key property:

* deterministic
* hashable
* inspectable
* stable enough for debug snapshots

## 3. `psionic-compiler`

Purpose:

* lower IR into executable kernels/plans

Responsibilities:

* legalizations
* fusion passes
* schedule construction
* memory planning
* backend-specific lowering hooks
* codegen interfaces
* autotune/search hooks

Keep this readable. The point is not maximal cleverness; it is strong, inspectable lowering.

## 4. `psionic-runtime`

Purpose:

* runtime orchestration

Responsibilities:

* device discovery
* queue/stream execution
* allocators
* graph execution
* synchronization
* metrics hooks
* runtime health
* backend selection policy

This maps closely to Tinygrad’s runtime split between compiled device, allocator, program, compiler. ([Tinygrad Docs][4])

## 5. `psionic-models`

Purpose:

* reusable model definitions and weight loading

Responsibilities:

* transformer blocks
* attention
* KV cache types
* embedding models
* tokenizer/model metadata integration
* model config parsing
* weight format adapters

Initial support targets:

* Llama-family text generation
* one embedding-family model
* MoE-aware abstractions from the start, even if basic

## 6. `psionic-serve`

Purpose:

* serving/runtime layer

Responsibilities:

* model lifecycle
* inference sessions
* prompt execution
* batching
* KV cache lifecycle
* embeddings API
* server-facing request/response types

This is where “inference engine ready” actually becomes true.

## 7. `psionic-provider`

Purpose:

* OpenAgents-facing provider integration

Responsibilities:

* capability envelope generation
* receipts/delivery evidence generation
* inventory-facing metadata
* health/readiness status
* Pylon/Autopilot adapter interfaces

This crate should depend on runtime/serve, but not vice versa.

## 8. backend crates

### `psionic-backend-cpu`

Required first backend.
Use it to prove:

* IR correctness
* kernel semantics
* deterministic tests
* reference behavior

### `psionic-backend-metal`

Important because Apple is already in your compute plan, and Tinygrad docs confirm METAL is a first-class runtime on M1+ Macs. ([Tinygrad Docs][3])

### `psionic-backend-amd-kfd`

This is the lower-risk AMD path.
Map to the normal amdgpu/KFD operational posture analogous to Tinygrad’s `AMD_IFACE=KFD`. ([Tinygrad Docs][3])

### `psionic-backend-amd-userspace`

This is the “sovereign AMD stack” path.
Treat it as distinct from KFD, because Tinygrad’s docs make clear `PCI`/AM-driver mode has different host-driver behavior and risk. ([Tinygrad Docs][3])

Optional later:

* `psionic-backend-cuda`
* `psionic-backend-webgpu`

## Inference engine spec

Psionic is not “inference-ready” until all of these exist:

### Model execution

* transformer forward path
* rotary embeddings where needed
* grouped query attention / KV heads support
* quantized linear layers
* sampler layer
* tokenizer integration boundary

### KV cache

George’s note about “K/V cache to disk” is exactly the sort of missing serving/runtime piece that makes sense in this layer.

Required KV cache modes:

* in-memory KV cache
* paged KV cache
* optional tiered KV cache
* later disk-backed spill/tiering

The importance of KV cache lifecycle is consistent with the Tinygrad research summary, which found a current in-memory KV approach in its Llama model path. ([GitHub][1])

### Serving posture

Required:

* BS=1 optimized path
* microbatch path
* continuous batching later
* streaming tokens
* cancellation
* timeout support

### MoE support

Required from architecture day one:

* routing abstraction
* expert dispatch abstraction
* BS=1 fast path priority
* later batch improvements

### Weight formats

Required:

* safetensors first
* GGUF import adapter if desired
* internal normalized weight layout

## Embeddings engine spec

Embeddings is not “just inference without decoding.”

Required:

* embedding-model interface
* pooling strategy explicit
* vector dimension metadata
* deterministic output shape
* batch embedding path
* normalization policy explicit
* model metadata surfaced to provider receipts

Recommended first product:

* one high-quality text embedding model family
* later multimodal embeddings

## Runtime/device model

Every runtime backend must expose:

* backend name
* device discovery
* health state
* memory capacity
* supported dtypes
* supported quantization modes
* kernel feature flags
* graph support
* failure modes / reason codes

AMD-specific backends must also expose:

* runtime mode: `kfd` vs `userspace`
* device family / gfx target
* memory / topology metadata
* reset/recovery posture

That separation is justified by Tinygrad’s own runtime docs and AMD interface distinctions. ([Tinygrad Docs][3])

## Receipt and delivery evidence spec

Psionic executions should be receiptable from the start.

Minimum evidence fields:

* provider identity
* backend family = `psionic`
* runtime backend
* device IDs / topology summary
* model ID / weights digest
* quantization mode
* request digest
* execution plan digest
* start time
* end time
* tokens in / tokens out for generation
* vector dimension / count for embeddings
* resource usage summary
* KV cache mode
* termination reason
* payout linkage fields

This is what makes Psionic fit the compute market rather than being a black-box model runner.

## Integration with Pylon and Autopilot

Pylon should treat Psionic as a provider backend family.

Autopilot should treat Psionic as:

* a local compute backend option
* a capability source
* a status/earnings source through the shared provider substrate

Recommended product IDs:

* `psionic.text_generation`
* `psionic.embeddings`

Pylon status should surface:

* runtime backend
* model loaded
* quantization
* batch posture
* KV cache posture
* health
* recent jobs
* earnings

## Coding-agent instructions

Use this block directly.

```text
Create a new Rust-native compute engine called Psionic under `crates/psionic/` in this repo.

Goal

Psionic is the Rust-native compute engine for OpenAgents. It should eventually provide:
- tensor/core execution primitives
- explicit IR and compiler pipeline
- runtime/device abstraction
- inference engine for text generation
- embeddings engine
- AMD-first backend support
- provider/runtime integration hooks for Pylon and Autopilot

This is not a port of Tinygrad source line by line.
It is a Rust-native compute engine inspired by Tinygrad’s architectural shape.

Authoritative framing to preserve

- Tinygrad is an end-to-end deep learning stack with tensor/autograd, IR/compiler, and JIT/graph execution.
- Tinygrad runtime docs distinguish AMD `KFD` and `PCI` interfaces and warn that `AMD_IFACE=PCI` may unbind the GPU from amdgpu.
- Psionic should preserve the architectural virtues (small visible stack, explicit IR, runtime/backend separation) while fitting our Rust provider/runtime stack.

Repo placement

Everything must live under `crates/psionic/`.

Create this subtree structure:

- crates/psionic/README.md
- crates/psionic/docs/
- crates/psionic/psionic-core/
- crates/psionic/psionic-ir/
- crates/psionic/psionic-compiler/
- crates/psionic/psionic-runtime/
- crates/psionic/psionic-models/
- crates/psionic/psionic-serve/
- crates/psionic/psionic-provider/
- crates/psionic/psionic-backend-cpu/
- crates/psionic/psionic-backend-metal/
- crates/psionic/psionic-backend-amd-kfd/
- crates/psionic/psionic-backend-amd-userspace/

Optional later backends should not be created unless needed now:
- psionic-backend-cuda
- psionic-backend-webgpu

Implementation priorities

Phase 1: set up the workspace subtree and crate boundaries
- add Cargo manifests
- wire the workspace correctly
- create narrow public APIs
- add docs that explain crate responsibilities
- ensure clean dependency direction

Phase 2: core compute foundation
- `psionic-core`: Tensor, Shape, DType, Device, basic lazy op facade
- `psionic-ir`: graph/IR node model, typed IDs, deterministic serialization
- `psionic-compiler`: pass interfaces, lowering skeleton, scheduling skeleton
- `psionic-runtime`: device/runtime traits, allocator traits, executable graph traits

Phase 3: backend skeletons
- CPU backend as reference backend
- Metal backend skeleton
- AMD KFD backend skeleton
- AMD userspace backend skeleton
- backend capability reporting must be explicit

Phase 4: model and serving skeletons
- `psionic-models`: transformer abstractions, attention/KV cache abstractions, embedding model abstractions
- `psionic-serve`: generation session types, embeddings request types, batching/KV cache interfaces
- do not build a full engine immediately; set up the architecture so it is obvious how the inference engine lands

Phase 5: provider integration skeleton
- `psionic-provider`: capability envelope generation
- provider-side status/readiness types
- receipt/delivery-evidence data structures
- Pylon-facing adapter traits

Crate responsibilities

1. `psionic-core`
- public tensor facade
- shapes/dtypes/devices
- buffer handles
- lazy op construction
- no backend-specific code here

2. `psionic-ir`
- canonical IR
- op graph / schedule graph types
- deterministic hashing/serialization
- debug-printable forms

3. `psionic-compiler`
- legalizations
- fusion/scheduling scaffolding
- backend-lowering interfaces
- plan/codegen boundaries

4. `psionic-runtime`
- runtime traits
- device discovery interfaces
- allocator traits
- execution context / graph execution traits
- metrics hooks

5. `psionic-models`
- transformer blocks
- attention abstractions
- KV cache abstractions
- embedding model abstractions
- weight metadata structures

6. `psionic-serve`
- generation request/response types
- embeddings request/response types
- session model
- batching model
- streaming abstractions
- model lifecycle interfaces

7. `psionic-provider`
- capability envelope structures
- execution evidence structures
- receipt structures
- Pylon/Autopilot integration traits

8. backend crates
- backend-specific device/runtime implementations only
- AMD KFD and AMD userspace must remain distinct backends, not a hidden toggle

Design constraints

- Keep the dependency graph clean and narrow.
- Do not put product/business logic into the core/compiler/runtime crates.
- Do not put Pylon/Autopilot-specific UX logic inside Psionic.
- Do not hide AMD KFD vs AMD userspace behind one generic “AMD” runtime.
- Keep inference and embeddings first-class in architecture from day one.
- Keep receipt/evidence needs visible in the provider crate from day one.

Required docs to create

- `crates/psionic/README.md`
  - what Psionic is
  - what it is not
  - crate map
  - design principles
- `crates/psionic/docs/ARCHITECTURE.md`
  - layering
  - crate boundaries
  - dependency direction
- `crates/psionic/docs/BACKENDS.md`
  - backend model
  - CPU / Metal / AMD KFD / AMD userspace
- `crates/psionic/docs/INFERENCE_ENGINE.md`
  - what “inference-ready” means
  - text generation requirements
  - embeddings requirements
  - KV cache requirements
- `crates/psionic/docs/PROVIDER_INTEGRATION.md`
  - capability envelopes
  - receipts / delivery evidence
  - mapping to Pylon and Autopilot

Required initial code deliverables

- compile-clean workspace subtree
- placeholder public APIs with doc comments
- test harness for crate wiring
- one minimal CPU execution path proving the architecture is real
  - can be very small (e.g. simple tensor add/matmul path)
- capability-reporting trait and example implementation
- generation/embeddings request/response structs
- provider receipt/evidence structs

Do not overreach in the first setup pass

Do not try to finish:
- full LLM inference
- full embeddings runtime
- full AMD kernels
- full graph compiler
in the first setup pass.

Instead:
- establish the exact architecture
- make the crate boundaries real
- make the first execution path real enough to prove the design

Deliverables expected back from the coding agent

1. Summary of the Psionic subtree created
2. Exact crate list and dependency direction
3. Files created
4. Current minimal working execution path
5. Remaining gaps to reach:
   - text generation engine
   - embeddings engine
   - AMD KFD backend
   - AMD userspace backend
6. Any places where repo-wide workspace changes were required
```

## Epics and issues

Here’s a clean roadmap from “set up subtree” to “full inference engine ready.”

## Epic 1 — Psionic workspace foundation

Goal: create `crates/psionic/` as a real workspace subtree.

Issues:

1. Create Psionic subtree and workspace manifests
2. Add initial crate skeletons and compile-clean dependency graph
3. Write top-level README and architecture docs
4. Add workspace tests/lints for Psionic subtree

## Epic 2 — Core tensor and IR foundation

Goal: establish the internal language of the engine.

Issues:

1. Implement `Tensor`, `Shape`, `DType`, `Device` in `psionic-core`
2. Implement canonical IR node graph and typed IDs in `psionic-ir`
3. Add deterministic IR serialization and hashing
4. Add op registry and minimal lazy execution graph
5. Add debug-printable IR inspection tools

## Epic 3 — Compiler and execution-plan skeleton

Goal: prove the compiler/runtime split.

Issues:

1. Add compiler pass manager and lowering pipeline skeleton
2. Add schedule IR and execution plan types
3. Add memory-planning abstraction
4. Add fusion/legalization scaffolding
5. Add backend-lowering traits

## Epic 4 — Runtime and device model

Goal: make execution infrastructure real.

Issues:

1. Add runtime traits for device discovery and synchronization
2. Add allocator and buffer abstractions
3. Add executable graph/runtime context abstractions
4. Add runtime metrics and profiling hooks
5. Add health/readiness/error-state model

## Epic 5 — CPU reference backend

Goal: make the architecture real with a working backend.

Issues:

1. Implement CPU device discovery and allocator
2. Implement minimal CPU kernel execution path
3. Support basic ops: add, mul, matmul, reshape
4. Add CPU reference tests
5. Add deterministic correctness benchmarks

## Epic 6 — Metal backend

Goal: first accelerated backend for Apple hardware.

Issues:

1. Create Metal backend skeleton and device discovery
2. Implement Metal allocator and command queue abstractions
3. Add minimal Metal kernels for core ops
4. Add backend capability reporting
5. Add integration tests on supported Apple hardware

Tinygrad’s runtime docs confirm METAL is already a meaningful backend category on Apple devices, which makes it a sensible early target here too. ([Tinygrad Docs][3])

## Epic 7 — AMD backend architecture

Goal: explicitly support two AMD paths.

Issues:

1. Define AMD backend capability model
2. Implement `psionic-backend-amd-kfd` skeleton
3. Implement `psionic-backend-amd-userspace` skeleton
4. Add backend-specific health/readiness state
5. Add provider-facing AMD capability envelope fields

This separation is justified by Tinygrad’s runtime docs distinguishing `KFD` and `PCI`, with different operational behavior. ([Tinygrad Docs][3])

## Epic 8 — Transformer model foundation

Goal: make LLM execution architecture explicit.

Issues:

1. Add transformer config/model abstractions
2. Add attention abstraction and rotary embedding support
3. Add KV cache trait and in-memory implementation
4. Add sampler abstraction
5. Add weight metadata and model loading interfaces

## Epic 9 — Quantization support

Goal: inference-ready weight/runtime posture.

Issues:

1. Add quantized linear abstraction
2. Add FP16/BF16 baseline execution
3. Add INT8 execution path
4. Add NF4 abstraction and metadata model
5. Add FP8 abstraction and backend capability gating

The Tinygrad research summary identified int8, NF4, and FP8 as meaningful inference-related modes in its Llama path; that makes this a sensible target posture for Psionic too. ([GitHub][1])

## Epic 10 — Text generation engine

Goal: first real inference product.

Issues:

1. Implement single-request text generation path
2. Add tokenizer/model boundary
3. Add streaming token output
4. Add cancellation/timeouts
5. Add BS=1 optimized execution path
6. Add microbatch architecture
7. Add model load/unload lifecycle
8. Add text-generation benchmark and correctness suite

## Epic 11 — KV cache and long-context engine

Goal: production-grade serving memory model.

Issues:

1. Implement per-session in-memory KV cache
2. Add paged KV cache design and implementation
3. Add KV accounting and limits
4. Add optional host-tiered KV cache
5. Add optional disk-tiered KV design
6. Add concurrency-safe session isolation tests

The relevance of KV lifecycle is reinforced by your George Hotz quote and by the current Tinygrad research path finding an in-memory KV approach. ([GitHub][1])

## Epic 12 — Embeddings engine

Goal: make embeddings a first-class compute family.

Issues:

1. Add embedding-model abstraction
2. Add pooling strategy abstraction
3. Implement first embedding inference path
4. Add batch embeddings path
5. Add vector metadata and dimension reporting
6. Add embeddings correctness and performance tests
7. Add provider receipt/evidence fields for embeddings jobs

This is particularly important because Tinygrad’s current public Llama serving shape is closer to chat/completions than embeddings, while your compute plan wants embeddings first-class. ([GitHub][1])

## Epic 13 — MoE and BS=1 fast path

Goal: align with your desired inference posture.

Issues:

1. Add expert-routing abstractions
2. Add MoE-aware linear/expert dispatch interface
3. Optimize BS=1 MoE path
4. Add MoE capability reporting
5. Add MoE benchmark suite

## Epic 14 — Serving layer and API contracts

Goal: productize the engine.

Issues:

1. Implement generation request/response contracts
2. Implement embeddings request/response contracts
3. Add model registry/lifecycle layer
4. Add local HTTP/gRPC serving shell if desired
5. Add metrics and request tracing
6. Add compatibility layer for OpenAgents/Pylon adapters

## Epic 15 — Provider integration

Goal: make Psionic fit the compute market.

Issues:

1. Add capability envelope generator
2. Add backend/product derivation
3. Add execution evidence schema
4. Add receipt schema for text generation
5. Add receipt schema for embeddings
6. Add Pylon-facing provider adapter
7. Add Autopilot-facing local backend adapter

## Epic 16 — Testing, determinism, and replay

Goal: make the engine trustworthy.

Issues:

1. Add IR determinism tests
2. Add reference-vs-accelerated backend correctness tests
3. Add generation golden tests
4. Add embeddings golden tests
5. Add replay-safe execution-plan tests
6. Add receipt/evidence determinism tests

## Epic 17 — Full inference engine ready

Goal: the point at which you can honestly say Psionic is inference-engine ready.

Definition:

* can load a supported LLM
* can serve text generation end-to-end
* has KV cache lifecycle
* has quantization support
* has capability reporting
* emits receipts/evidence
* has at least one accelerated backend beyond CPU
* has embedding support
* has provider integration hooks

Issues:

1. Ship first supported generation model family
2. Ship first supported embedding model family
3. Ship first accelerated production backend
4. Ship provider integration in Pylon
5. Publish readiness docs and verification matrix

## Recommended order

If you actually want the shortest path to “real engine exists,” the order should be:

1. Epic 1 — workspace foundation
2. Epic 2 — core/IR
3. Epic 4 — runtime
4. Epic 5 — CPU backend
5. Epic 8 — transformer model foundation
6. Epic 10 — text generation engine
7. Epic 11 — KV cache
8. Epic 12 — embeddings engine
9. Epic 6 — Metal backend
10. Epic 7 — AMD backend architecture
11. Epic 9 — quantization
12. Epic 15 — provider integration
13. Epic 16 — determinism/replay
14. Epic 13 — MoE fast path
15. Epic 17 — readiness ship

## Bottom line

The right way to do this is:

* **Psionic is a Rust-native compute engine**
* **it lives entirely under `crates/psionic/`**
* **it is inspired by Tinygrad’s architecture, not a literal port**
* **it is built from day one to fit Pylon, Autopilot, and the compute market**

And the first meaningful milestone is not “AMD beats NVIDIA.”
It is:

> **Psionic compiles cleanly, has a real CPU execution path, has a real crate architecture, and has explicit text-generation / embeddings / provider integration surfaces.**

If you want, I can also turn this into a repo-ready `RUSTYGRAD_PLAN.md` and a GitHub master issue with numbered child issues.

[1]: https://github.com/tinygrad/tinygrad?utm_source=chatgpt.com "GitHub - tinygrad/tinygrad: You like pytorch? You like micrograd? You love tinygrad! ❤️"
[2]: https://docs.tinygrad.org/?utm_source=chatgpt.com "tinygrad documentation - tinygrad docs"
[3]: https://docs.tinygrad.org/runtime/?utm_source=chatgpt.com "Runtime - tinygrad docs"
[4]: https://docs.tinygrad.org/developer/runtime/?utm_source=chatgpt.com "Runtime Overview - tinygrad docs"

