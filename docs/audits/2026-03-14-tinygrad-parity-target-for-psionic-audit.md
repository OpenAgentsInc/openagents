# 2026-03-14 Tinygrad Parity Target For Psionic Audit

## Intent

This audit answers a narrower question than the older Tinygrad research notes:

> how close to 100% Tinygrad parity should Psionic actually try to get, given
> that Psionic is now a wider Rust-native execution program and not just a
> local-inference replacement project?

The useful answer is not:

- "port Tinygrad line by line"
- or "copy every Tinygrad surface until the repos look similar"

The useful answer is:

- get very close to Tinygrad on the parts that define a real ML framework
- diverge deliberately on the parts where Psionic must own receipts, manifests,
  session claims, cluster truth, sandbox truth, and operator-grade control
- do not spend time chasing parity with Python ergonomics, example scripts, or
  risky runtime defaults just to say "we match Tinygrad"

## Scope

Sources reviewed from `~/code/tinygrad`:

- `README.md`
- `docs/runtime.md`
- `docs/developer/runtime.md`
- `tinygrad/tensor.py`
- `tinygrad/device.py`
- `tinygrad/engine/schedule.py`
- `tinygrad/engine/realize.py`
- `tinygrad/engine/jit.py`
- `tinygrad/nn/state.py`
- `tinygrad/nn/optim.py`
- `tinygrad/apps/llm.py`
- `test/null/test_process_replay.py`

OpenAgents sources reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `crates/psionic/README.md`
- `crates/psionic/docs/ARCHITECTURE.md`
- `crates/psionic/docs/TRAIN_SYSTEM.md`
- `crates/psionic/docs/LLAMA_VLLM_SGLANG_INFERENCE_SPEC.md`
- `crates/psionic/docs/plan.md`
- `crates/psionic/docs/deep-research-tinygrad.md`

## Executive Summary

Psionic should not aim for 100% Tinygrad parity overall.

It should aim for near-complete parity with Tinygrad's core ML-framework
substrate, while intentionally diverging on product, control-plane, and
provider-safety surfaces.

The right target is roughly:

- `85-95% parity` on core ML-framework shape
- `60-80% parity` on selected serving and model-runtime patterns
- `0-40% parity` on Python-first ergonomics, example-app surfaces, and risky
  runtime behavior

In plain terms:

- Psionic should become as complete as Tinygrad in tensor, autodiff, optimizer,
  IR, lowering, runtime, graph replay, model IO, and local multi-device
  execution.
- Psionic should borrow a lot from Tinygrad's LLM path, but should not inherit
  Tinygrad's example-shaped server, Python control plane, or env-var-driven
  product contract.
- Psionic should exceed Tinygrad in the things Tinygrad is not trying to be:
  receipt-bearing execution truth, runtime manifests, session claims, cluster
  topology truth, sandbox policy, operator surfaces, and compute-market-grade
  evidence.

So the right goal is:

> Psionic should become Tinygrad-complete as an ML framework, but not
> Tinygrad-shaped as a product runtime.

## What Tinygrad Proves Today

Tinygrad still matters because it is not just "some inference demo."

The current repo still proves one compact codebase can span:

- a real `Tensor` type with autograd and training mode
- a real optimizer library in `tinygrad/nn/optim.py`
- real model and state IO in `tinygrad/nn/state.py`
- a visible runtime split in `tinygrad/device.py`
  - `Compiled`
  - `Allocator`
  - `Program`
  - `Compiler`
- a visible lowering pipeline in:
  - `tinygrad/engine/schedule.py`
  - `tinygrad/engine/realize.py`
- graph capture and replay in `tinygrad/engine/jit.py`
  - `TinyJit`
  - `GraphRunner`
  - `MultiGraphRunner`
- concrete LLM execution in `tinygrad/apps/llm.py`
  - tokenizer
  - GGUF loading
  - prefill and rollout split
  - KV cache lifecycle
  - model-family loading
- backend breadth across CPU, Metal, CUDA, AMD, NV, QCOM, OpenCL, and WebGPU
- conformance discipline like `test/null/test_process_replay.py`

That is why Tinygrad is still the best compact "full ML stack" reference for
Psionic, even though Psionic should not copy it literally.

## Where Psionic Already Differs In Good Ways

Psionic is already broader than Tinygrad in several directions that should stay
distinct:

- `psionic-cluster` owns ordered clustered execution and topology truth
- `psionic-datastream` owns resumable artifact movement and delivery receipts
- `psionic-sandbox` owns bounded execution profiles and runtime policy
- `psionic-net` owns session identity, rendezvous, and session-bound transport
  claims
- `psionic-runtime` already cares about proof bundles and execution evidence
- the broader OpenAgents stack already cares about authority truth, policy
  truth, and market settlement

Tinygrad is not trying to solve those problems.

So parity should not mean "make Psionic look more like a Python research repo."
It should mean "make Psionic equally complete on the framework core, while
keeping Psionic-native truth surfaces."

## The Parity Decision

### Area 1: Core ML framework substrate

This is where Psionic should get very close to Tinygrad.

| Area | Tinygrad today | Psionic target |
| --- | --- | --- |
| Tensor semantics | real `Tensor` front end with views, broadcasting, movement, and autograd | near-parity required |
| Optimizers and train loop primitives | real `SGD`, `Adam`, `AdamW`, `LARS`, `LAMB`, fused and device-aware behavior | near-parity required |
| Model and state IO | `safe_load`, `safe_save`, `torch_load`, `gguf_load`, quant decode | near-parity required |
| IR and lowering | visible UOp graph, schedule creation, linearization, realize pipeline | near-parity required |
| Runtime contract | `Compiled`, `Allocator`, `Program`, `Compiler`, `LRUAllocator` | near-parity required |
| Graph replay | `TinyJit`, `GraphRunner`, `MultiGraphRunner`, plan reuse | near-parity required |
| Memory planning and method cache | explicit memory planner, schedule cache, method cache | near-parity required |
| Multi-device local execution | same-type multi-device execution and sharding hooks | strong parity required |
| Conformance and replay tests | process replay and kernel-identity checks | near-parity required |

If Psionic does not eventually reach this level, it is not honest to call it a
full-fledged ML framework. It would still be an execution substrate with some
ML runtime pieces, but not a complete framework core.

### Area 2: Serving and model-runtime patterns

This is where Psionic should borrow aggressively, but not copy blindly.

Tinygrad has strong patterns in:

- GGUF and quantized weight ingest
- tokenizer-from-GGUF behavior
- explicit KV-cache allocation and update
- distinct prefill and rollout execution paths
- shape-aware JIT reuse
- model-family loading from one generic runtime path

Psionic should adopt most of that shape.

But Psionic should turn it into:

- a productized `psionic-models` contract
- a productized `psionic-serve` runtime
- typed session and KV truth
- explicit request, response, and evidence contracts

So this is not "build Tinygrad's `apps/llm.py` in Rust." It is:

> take the engine truths from `apps/llm.py` and land them under Psionic-owned
> serving and evidence contracts.

### Area 3: Product runtime and control plane

This is where Psionic should diverge deliberately.

Tinygrad uses:

- example-first serving
- environment variables as the primary runtime switchboard
- Python-side composition
- research-friendly but sharp low-level interop and driver behavior

Psionic should not chase parity here.

Psionic should prefer:

- typed manifests and runtime policies
- operator-visible state machines
- explicit backend capability and risk posture
- bounded runner and sandbox contracts
- proof-bearing execution receipts
- app-independent but product-grade control-plane seams

## Recommended Parity Targets

### 1. Areas where Psionic should reach `90%+` parity

These are the areas where "full-fledged ML framework" is real or not.

#### Tensor and autodiff core

Tinygrad's `Tensor` is not just a convenience type. It is the front door to the
entire framework.

Psionic should reach near-parity on:

- tensor creation and movement semantics
- view and reshape semantics
- dtype and device propagation
- backward or autodiff graph semantics
- training-mode context and gradient ownership

This is one of the biggest remaining Psionic gaps.

#### Optimizer and trainer primitives

Tinygrad already has real optimizer surfaces. Psionic train docs still describe
the full training core as planned.

So if Psionic wants Tinygrad-level framework completeness, it must land:

- optimizer state ownership
- trainer-step execution
- device-aware optimizer behavior
- fused or batch-friendly update paths where appropriate
- reproducible train-loop primitives

#### IR, scheduler, realize, and memory planning

Tinygrad is still strong because the execution path is inspectable:

- schedule creation
- linearization
- runtime lowering
- memory planning
- method cache
- JIT graph reuse

Psionic should seek near-parity here, because this is the part that makes a
framework feel real rather than "a set of backend adapters."

#### Model and state IO

Tinygrad's `nn/state.py` is broader than many people realize. It covers:

- safetensors
- torch checkpoint loading
- GGUF loading
- quantized decode
- state-dict traversal and assignment

Psionic should get very close here. A serious ML framework cannot strand its
artifacts.

#### Process replay and compiler conformance

Tinygrad's `test_process_replay.py` is one of the most important cultural cues
in the repo.

Psionic should match this discipline closely:

- replayable program generation
- kernel or plan identity comparison
- regression tests for compiler changes
- reproducible schedule or lowering expectations

This matters even more for Psionic because Psionic also wants receipt-bearing
execution truth.

### 2. Areas where Psionic should seek `70-85%` parity

These matter a lot, but Psionic should still reshape them around its own
product and evidence model.

#### LLM runtime shape

Tinygrad's `apps/llm.py` has the right kind of patterns:

- GGUF-derived tokenizer behavior
- model-family dispatch
- prefill-vs-rollout split
- explicit KV-cache lifecycle
- JIT specialization on decode posture

Psionic should borrow most of that, but re-home it under:

- `psionic-models`
- `psionic-runtime`
- `psionic-serve`

#### Multi-device execution patterns

Tinygrad has concrete same-type multi-device graph replay and model-sharding
hooks.

Psionic should borrow the design pattern, but go further by making:

- topology truth explicit
- sharding policy explicit
- capability envelopes explicit
- clustered and provider-visible refusal reasons explicit

#### Backend surface breadth

Tinygrad shows the value of many backends, but Psionic does not need immediate
parity with every Tinygrad runtime to be successful.

Near-term Psionic should prioritize:

- CPU
- Metal
- CUDA
- AMD KFD

with lower priority on:

- userspace AMD experimental modes
- WebGPU
- QCOM
- full NV-vs-CUDA split

### 3. Areas where Psionic should seek low or zero parity

These are not strategic targets.

#### Python ergonomics and example-app parity

Psionic does not need parity with:

- Tinygrad's Python API feel
- example script count
- model-zoo breadth in example files
- notebook or script ergonomics

Those are useful for research and adoption, but they are not the right parity
target for OpenAgents.

#### Env-var-as-control-plane parity

Tinygrad uses environment variables heavily for backend and runtime posture.

That is fine for Tinygrad.

Psionic should not make env vars its long-term control plane. It should prefer:

- typed runtime policy
- backend manifests
- operator-visible readiness and risk posture
- explicit config surfaces

#### Risky driver posture as default

Tinygrad documents backend modes like `AMD_IFACE=PCI` that may unbind the GPU
from the system driver.

Psionic should not seek parity with that as a default capability surface.

At most, Psionic should preserve the idea that:

- interface mode is explicit truth
- risk posture is explicit truth

while keeping risky modes behind strong gates or outside default product paths.

#### Raw pointer and zero-copy interop as a product primitive

Tinygrad's `Tensor.from_blob` interop is powerful, but it is not the right
default substrate for untrusted or provider-facing product paths.

Psionic should keep that sort of thing heavily gated.

## What Psionic Should Exceed Tinygrad On

Psionic should not only match Tinygrad's ML core. It should exceed Tinygrad in
several ways that Tinygrad is not trying to own.

### Execution truth and receipts

Psionic should exceed Tinygrad on:

- execution-plan digests
- runtime manifests
- session claims
- receipt families
- operator-visible evidence
- promotion and reproducibility records

### Cluster and distributed truth

Tinygrad has useful multi-device patterns, but Psionic should exceed it on:

- ordered cluster state
- topology revisions
- catch-up and recovery truth
- cluster-scoped evidence and refusal reasons

### Sandbox and safety posture

Psionic should exceed Tinygrad on:

- bounded sandbox profiles
- filesystem and network policy
- explicit runtime admissibility
- safe-by-default backend posture

### Training control-plane clarity

Tinygrad already has a more complete local training framework than Psionic does
today.

But when Psionic catches up on the trainer core, Psionic should exceed Tinygrad
on:

- run identity
- stage identity
- policy-revision lineage
- validator and verdict attachment
- durable receipts for train and eval

## Current Parity Reading For Psionic

Today the honest reading is:

- Psionic is already ahead of Tinygrad in control-plane and execution-truth
  architecture.
- Tinygrad is still ahead of Psionic in the actual ML-framework core.

More specifically:

| Domain | Who is ahead today | Why |
| --- | --- | --- |
| Tensor and autograd completeness | Tinygrad | Psionic still does not claim a full trainer or autodiff core |
| Optimizer library and local training loop | Tinygrad | Tinygrad already has usable optimizers and training examples |
| Model and state IO breadth | Tinygrad | Tinygrad already handles safetensors, torch load, GGUF, and quant decode in one place |
| JIT, graph replay, and method cache | Tinygrad | Psionic has plan and runtime concepts, but Tinygrad still has the more complete shipped framework path |
| Cluster, sandbox, and provider-safe execution truth | Psionic | Tinygrad is not trying to be a provider substrate |
| Receipts, manifests, and proof-bearing execution identity | Psionic | Tinygrad does not own these concerns |
| Product-grade serving semantics | neither fully | Tinygrad is example-shaped; Psionic is still completing the serving engine |
| Full Rust-native training system | neither fully for Psionic terms | Tinygrad has local training completeness; Psionic wants a broader receipt-bearing train system that is still unfinished |

So the parity goal should not be "be just like Tinygrad."

It should be:

> become Tinygrad-complete on ML framework internals, then keep going into the
> Psionic-specific execution, evidence, and market-facing layers.

## Backlog Implications

The current Psionic issue programs already cover a lot of the right direction:

- `PSI-232` through `PSI-258` for inference-engine completion
- `#3564` through `#3593` for the train system

But Tinygrad parity suggests several things must stay explicit in the backlog,
whether as new issues or widened scope in the current ones.

### Highest-priority parity checks

1. `Tensor and autodiff core`
   - Psionic needs an explicit all-Rust tensor and autodiff completion bar, not
     only serving and train orchestration.
2. `Scheduler, memory planner, and plan cache`
   - Tinygrad's visible schedule-to-realize path should have an explicit
     Psionic analog.
3. `Process replay and compiler conformance`
   - Psionic should have a compiler replay gate as explicit as Tinygrad's
     process replay discipline.
4. `State and weight IO parity`
   - GGUF is not enough; safetensors, checkpoint compatibility, and model-state
     portability also matter.
5. `Optimizer library and local trainer-step engine`
   - the train backlog should be read as including Tinygrad-class local train
     completeness, not only distributed train control flow.
6. `LLM runtime shape`
   - prefill versus rollout specialization, tokenizer-from-model metadata, and
     first-class KV cache lifecycle should stay explicit.

### Lower-priority or selective checks

1. `example zoo parity`
   - nice to have, not strategic
2. `backend breadth to every Tinygrad target`
   - useful, but not required for first framework completeness
3. `research-script ergonomics`
   - useful for experimentation, not a product truth surface

## Recommendation

The right Tinygrad goal for Psionic is:

- do not chase 100% repo parity
- do chase near-complete framework-core parity
- do exceed Tinygrad on execution truth, safety, and operator-grade runtime
  ownership

If Psionic wants to honestly become a full-fledged ML framework, it should use
Tinygrad as the architectural oracle for:

- tensor and autodiff completeness
- optimizer and trainer-step completeness
- IR, lowering, and runtime visibility
- graph replay and plan reuse
- model and checkpoint IO breadth
- multi-device local execution
- compiler replay discipline

And it should intentionally not use Tinygrad as the oracle for:

- Python API shape
- example-server product semantics
- env-var-driven control plane
- risky driver defaults
- provider-safety policy
- receipt and manifest truth

## Bottom Line

Psionic should not be a literal Rust port of Tinygrad.

But if Psionic does not eventually match Tinygrad on most of the actual
framework core, it will remain an impressive execution substrate without being a
fully complete ML framework.

The right ambition is:

> approximately `90%` Tinygrad parity on the framework core, and clear
> Psionic-native divergence everywhere that execution truth, safety, clustering,
> sandboxing, and market evidence matter.
