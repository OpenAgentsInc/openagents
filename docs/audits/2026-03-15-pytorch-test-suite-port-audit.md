# 2026-03-15 PyTorch Test Suite Port Audit

## Intent

This audit repeats the earlier Tinygrad test-port exercise, but uses
`~/code/pytorch` as the comparison source:

> what test coverage from local PyTorch is actually worth pulling into this
> repo, where should that coverage live in Rust, and what missing functionality
> would have to exist first?

The right goal is not:

- port PyTorch wholesale
- recreate Python-facing PyTorch APIs in Rust
- import PyTorch's hardware matrix, legacy subsystems, or backend-specific CI
  burdens just to say "we have parity"

The right goal is:

- use PyTorch as the strongest local oracle for tensor semantics, autograd,
  module behavior, optimizer correctness, serialization discipline, and
  compiler invariants
- port only the scenarios that defend framework behavior relevant to Psionic
- keep those scenarios inside the Rust crates that own the behavior
- remain explicit about the parts of PyTorch that are intentionally out of
  scope for this repo

One assumption note:

- the user request included the trailing phrase `0 cinnut abd oysg`
- no matching local repo or recognizable target was found under `~/code`
- this audit therefore compares only against `~/code/pytorch`

## Scope

PyTorch sources reviewed from `~/code/pytorch`:

- test-tree inventory under `test/`
- `test/test_ops.py`
- `test/test_nn.py`
- `test/test_autograd.py`
- `test/test_optim.py`
- `test/test_serialization.py`
- `test/test_fx.py`
- `test/test_jit.py`
- `test/test_modules.py`
- `test/nn/test_load_state_dict.py`
- `test/export/test_export.py`
- `test/dynamo/test_dynamic_shapes.py`
- `test/inductor/test_cache.py`
- `test/inductor/test_memory_planning.py`
- `test/onnx/test_models.py`

OpenAgents sources reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/audits/2026-03-15-tinygrad-test-suite-port-audit.md`
- `crates/psionic/docs/ARCHITECTURE.md`
- `crates/psionic/docs/TRAIN_SYSTEM.md`
- `crates/psionic/psionic-compiler/src/lib.rs`
- `crates/psionic/psionic-compiler/tests/process_replay.rs`
- `crates/psionic/psionic-ir/src/autodiff.rs`
- `crates/psionic/psionic-models/src/lib.rs`
- `crates/psionic/psionic-models/src/harmony.rs`
- `crates/psionic/psionic-serve/src/lib.rs`
- `crates/psionic/psionic-serve/tests/*.rs`
- `crates/psionic/psionic-train/src/core_loop.rs`
- `crates/psionic/psionic-train/src/distributed_optimizer.rs`
- `crates/psionic/psionic-train/src/model_io.rs`
- `crates/psionic/psionic-train/src/optimizer.rs`

## Executive Summary

PyTorch's test corpus is much larger and much more specialized than Tinygrad's.

The useful port target is not "PyTorch parity." It is:

- strong parity with PyTorch on core tensor, autodiff, optimizer, and
  serialization semantics
- selective borrowing from PyTorch's compiler stack around symbolic shapes,
  caching, and memory planning
- deliberate non-parity on Python frontend APIs, pickle-based artifact formats,
  TorchScript legacy surfaces, process-group APIs, and backend lab coverage

PyTorch is the better oracle than Tinygrad for:

- operator correctness
- autograd edge cases
- module behavior and `state_dict` discipline
- optimizer correctness
- serialization and portability boundaries
- compiler and shape-system invariants

PyTorch is the worse oracle than Tinygrad for:

- GGUF parsing and quant decode
- tokenizer-from-model behavior
- KV cache and prefix cache semantics
- local LLM-serving behavior

That means the two audits are complementary:

- Tinygrad should drive GGUF, tokenizer, JIT-replay, and serving-cache imports
- PyTorch should drive numerical parity, state semantics, and compiler-hygiene
  imports

The biggest PyTorch-relative gaps in this repo are:

- no `OpInfo`-scale operator conformance matrix
- no `module_db`-style module conformance matrix
- no `optim_db`-style optimizer correctness matrix
- no `fx` / `export` / `dynamo` / `inductor` style compiler frontend or test
  harness
- no ONNX surface at all
- intentionally narrower serialization surface than PyTorch

The biggest PyTorch-relative strengths in this repo are:

- stronger GGUF and local-serving coverage than PyTorch has
- explicit receipt and replay truth that PyTorch is not designed to own
- typed distributed optimizer and cluster contracts that defend a different
  class of truth than PyTorch's DDP/process-group tests

## What PyTorch's Test Tree Looks Like

PyTorch's `test/` tree currently contains 1,376 Python files total. Counting
only `test*.py` entry files still leaves 1,097 test-entry files.

Unlike Tinygrad, PyTorch does not organize the suite into three simple CI
groups. The tree is much more subsystem-oriented.

Approximate `test*.py` counts by top-level lane:

| Lane | Approx. test entry files | What it mainly covers | Port priority |
| --- | ---: | --- | --- |
| root `test/` | 167 | core tensor, ops, autograd, modules, serialization, dispatch, FX, JIT | highest |
| `distributed` | 252 | DDP, process groups, collectives, mesh, NCCL/Gloo/UCC | mostly defer |
| `inductor` | 154 | compile cache, codegen, memory planning, lowering, fusion, autotune | selective |
| `dynamo` | 145 | graph capture, guards, dynamic shapes, recompiles, export adjacency | selective |
| `jit` | 80 | TorchScript and legacy JIT stack | mostly defer |
| `quantization` | 37 | quantized module/export/runtime behavior | defer |
| `torch_np` | 36 | NumPy-compat layer | defer |
| `export` | 32 | graph export, shape contracts, serialization, verifier | selective |
| `onnx` | 33 | ONNX export and model validation | defer for now |
| `functorch` | 21 | functional transforms, vmap, AOT autograd | selective |
| `nn` | 15 | focused module and `load_state_dict` behavior | highest |
| `optim` | 3 | optimizer families, LR schedulers, SWA | highest |
| `autograd` | 3 | focused autograd functional behavior | highest |

The other major difference from Tinygrad is test infrastructure.

PyTorch leans heavily on declarative registries and reusable harnesses:

- `op_db` and `OpInfo`
- `module_db`
- `optim_db`
- fake/proxy tensor infrastructure
- symbolic shape and guard infrastructure

That matters because the best thing to port from PyTorch is not just test
cases. It is the shape of the test harness.

## The PyTorch Suites That Matter Most

### 1. Core operator and tensor semantics

The most relevant PyTorch root tests are the ones that define general tensor
behavior:

- `test/test_ops.py`
  Broad operator conformance driven by `op_db`, including device, dtype,
  reference, aliasing, metadata, and out-variant expectations.
- `test/test_reductions.py`
  Reduction behavior.
- `test/test_view_ops.py`
  View semantics and alias-sensitive operations.
- `test/test_shape_ops.py`
  Shape-manipulation correctness.
- `test/test_tensor_creation_ops.py`
  Tensor construction correctness.
- `test/test_type_promotion.py`
  Dtype and promotion rules.
- `test/test_dispatch.py`
  dispatch behavior and operator registration expectations.
- `test/test_fake_tensor.py`
  abstract execution and metadata-only correctness.
- `test/test_proxy_tensor.py`
  trace/proxy behavior.
- `test/test_dynamic_shapes.py`
  symbolic shape behavior.
- `test/test_prims.py`
  primitive operator layer behavior.

This is the strongest local oracle for "does the framework core behave like a
real tensor system?"

### 2. Autograd and functional transforms

Key suites:

- `test/test_autograd.py`
  Large reverse-mode/autograd edge-case matrix.
- `test/autograd/test_functional.py`
  functional autograd utilities.
- `test/functorch/test_*`
  transform-oriented behavior, AOT autograd, and vectorized semantics.
- `test/test_functionalization.py`
  mutation-to-functional transformation semantics.
- `test/test_functional_optim.py`
  functional optimizer behavior.

PyTorch is vastly stronger than the current repo here, especially once higher
order transforms and mutation semantics are considered.

### 3. Modules and `state_dict` semantics

Key suites:

- `test/test_nn.py`
  broad module behavior and module utilities.
- `test/test_modules.py`
  `module_db`-driven module forward/reference/device/dtype checks.
- `test/nn/test_convolution.py`
- `test/nn/test_embedding.py`
- `test/nn/test_pooling.py`
- `test/nn/test_multihead_attention.py`
- `test/nn/test_load_state_dict.py`
  strict/non-strict loads, missing keys, unexpected keys, size mismatch,
  prefix handling, backward-compat behavior, and typed errors.

This is relevant even if Psionic never builds a Python-like `nn.Module`, because
the module and state-tree semantics behind trainable models still matter.

### 4. Optimizers and scheduler behavior

Key suites:

- `test/test_optim.py`
  umbrella optimizer correctness.
- `test/optim/test_optim.py`
  `optim_db`-driven correctness, state-dict behavior, erroring, parameter
  groups, hook order, differentiable variants, and multiple implementations.
- `test/optim/test_lrscheduler.py`
  scheduler integration.
- `test/optim/test_swa_utils.py`
  SWA behavior.

PyTorch's optimizer tests are especially valuable because they check not just
loss descent, but state serialization, parameter-group semantics, and variant
equivalence.

### 5. Serialization and artifact behavior

Key suites:

- `test/test_serialization.py`
  save/load behavior, storage aliasing, zip/pickle paths, compatibility,
  metadata, and edge cases.
- `test/nn/test_load_state_dict.py`
  module-tree load behavior.
- selective `test/onnx/*`
  model export and interoperability.

PyTorch's serialization surface is much broader than this repo should copy, but
it is still a useful oracle for which invariants matter.

### 6. Compiler-stack behavior

Key suites:

- `test/test_fx.py`
  graph capture, tracing, graph rewrites, passes, and representation.
- `test/export/test_export.py`
  export contracts, graph signatures, retraceability, serdes, verifier, and
  dynamic shape boundaries.
- `test/dynamo/*`
  graph capture, guards, recompilation, module tracing, export adjacency.
- `test/inductor/test_cache.py`
  cache infrastructure.
- `test/inductor/test_memory_planning.py`
  pool allocation and memory-planning invariants.

This is the right PyTorch area to borrow from if Psionic wants stronger
compiler coverage.

### 7. The areas that matter less for now

Lower priority or mostly out of scope:

- `test/distributed/*`
  important for PyTorch, but mostly about DDP/process-group API parity
- `test/jit/*`
  large legacy surface; PyTorch itself has shifted future emphasis to
  `dynamo`/`inductor`/`export`
- `test/quantization/*`
  PyTorch-specific quantized module/runtime APIs
- `test/package/*`
- `test/mobile/*`
- `test/cpp_extensions/*`
- `test/custom_backend/*`
- most backend-specific CUDA/XPU/MPS lanes
- most ONNX exporter breadth

## What Psionic Already Covers

The repo already covers some PyTorch-adjacent concerns, but usually at a much
smaller and more typed scope.

### `psionic-ir`

`crates/psionic/psionic-ir/src/autodiff.rs` already has real reverse-mode
autodiff and tests for:

- matmul-chain gradient materialization
- shared-path accumulation with detach semantics
- explicit training/eval/no-grad context behavior
- typed refusal for unsupported gradient ops
- broadcast and view primitive gradients

That is real framework substrate, not a stub.

The problem is scope:

- the current builder surface is still small
- today it visibly covers:
  - `add`
  - `mul`
  - `matmul`
  - `reshape`
  - `permute`
  - `slice`
  - `select`
  - `concat`
  - `expand`
  - `reduce_sum`
  - `rms_norm`
  - `layer_norm`
  - `rope`
  - `scaled_dot_product_attention`
  - `quantized_matmul`

That is far from PyTorch's operator breadth.

### `psionic-train`

`crates/psionic/psionic-train/src/optimizer.rs` already tests:

- `SGD` and `Adam` advancement on a small model
- support for declared optimizer families
- state-kind mismatch refusal

`crates/psionic/psionic-train/src/core_loop.rs` already tests:

- fixed-budget training loop updates
- autodiff integration
- checkpoint restore
- missing-gradient refusal

`crates/psionic/psionic-train/src/distributed_optimizer.rs` already tests:

- distributed optimizer contract memory and precision truth
- microbatch accumulation and flush behavior
- shard-coverage refusal

This is stronger than a superficial scan suggests, but it is still not a
PyTorch-style optimizer matrix.

### `psionic-train::model_io`

`crates/psionic/psionic-train/src/model_io.rs` already supports and tests:

- torch-style JSON state-dict export/import
- safetensors export/import with embedded manifest metadata
- GGUF import
- tokenizer binding and chat-template digest carry-through
- adapter-delta derivation and application

This is a meaningful serialization surface.

The key difference from PyTorch is deliberate:

- Psionic currently supports typed JSON and safetensors paths
- it does not attempt PyTorch pickle/zip/`torch.save` / `torch.load` behavior

That is likely the correct safety posture for this repo.

### `psionic-compiler`

`crates/psionic/psionic-compiler/src/lib.rs` already provides:

- deterministic execution-plan assembly
- stable digests and signature lines
- optional topology attachment

`crates/psionic/psionic-compiler/tests/process_replay.rs` adds replay fixtures.

But the same file is explicit that `compile` currently produces a placeholder
execution plan. This is much smaller than the PyTorch compiler stack.

### `psionic-models` and `psionic-serve`

These crates are strong, but mostly in directions PyTorch is not the best
oracle for:

- GGUF parsing
- quantized block decode
- tokenizer metadata and prompt rendering
- session KV cache
- shared prefix cache
- text generation and embeddings serving
- hardware parity checks for served models

Those are real strengths, but they should still be calibrated primarily against
Tinygrad and Psionic's own serving contracts, not against PyTorch.

## Gap Assessment

### Gap 1: no data-driven conformance registries

PyTorch's greatest testing advantage is not a single file. It is the existence
of reusable registries:

- `op_db`
- `module_db`
- `optim_db`

This repo has point tests, but not a declarative parity harness of comparable
shape.

That means every new capability currently needs bespoke tests rather than
dropping into a reusable matrix.

### Gap 2: operator and autograd breadth is much smaller

The current `psionic-ir` surface is credible but narrow.

There is no evidence in the active crates of first-class support for broad
PyTorch-style module and op families such as:

- convolution
- pooling
- dropout
- relu/gelu/silu-style activation families
- generic softmax/log-softmax coverage
- recurrent module families such as `LSTM` or `GRU`
- broad dtype/promotion behavior

For present Psionic serving needs, some of that is fine. For "PyTorch-relevant
framework coverage," it is a real gap.

### Gap 3: no module-behavior matrix

PyTorch's `test_nn.py`, `test_modules.py`, and `nn/test_load_state_dict.py`
cover module semantics, parameter/buffer placement, forward reference checks,
and state loading edge cases.

This repo has:

- trainable parameter-group state
- model-bundle traversal
- typed portable state artifacts

It does not have a comparable module conformance layer.

That means it also does not have a true equivalent to `module_db`.

### Gap 4: optimizer coverage is narrower and less behavioral

PyTorch's optimizer suite covers:

- update-algorithm correctness
- parameter groups
- scheduler integration
- state-dict determinism
- device movement of optimizer state
- erroring and hooks
- alternate implementations like foreach/fused variants

Psionic today covers:

- core optimizer family math
- training-loop integration
- some distributed optimizer contracts

It does not cover:

- scheduler behavior
- hook ordering
- differentiable optimizer semantics
- variant-equivalence testing
- broader optimizer-state serialization behavior

### Gap 5: compiler frontend and symbolic-shape coverage is largely absent

PyTorch's modern compiler stack spans:

- `fx`
- `export`
- `dynamo`
- `inductor`

No equivalent surface was found in the active Psionic crates for:

- graph capture from a frontend language/API
- guard management
- fake or proxy tensor execution
- symbolic shape environments
- export verification
- compiler cache infrastructure at PyTorch depth

Psionic does have replay fixtures and stable-digest planning, but those are
closer to the bottom of the stack than the top.

### Gap 6: serialization is intentionally narrower than PyTorch

PyTorch's serialization tests defend:

- pickle/zip save-load behavior
- storage aliasing
- compatibility and legacy semantics
- `weights_only` and related load modes

Psionic intentionally avoids that surface and instead prefers:

- typed JSON
- safetensors
- GGUF

That is probably correct. The gap is real, but it should not automatically be
closed.

### Gap 7: ONNX is currently absent

No ONNX surface or ONNX tests were found in the active Psionic crates.

If ONNX import/export is a product goal later, PyTorch's ONNX tests are an
obvious oracle. If not, this should remain deferred.

### Gap 8: distributed parity is different, not simply missing

PyTorch's `distributed` suite is enormous because it defends a runtime API:

- process groups
- collectives
- DDP
- mesh and placement APIs

Psionic's distributed work instead emphasizes:

- cluster truth
- execution receipts
- optimizer sharding contracts
- memory and precision plans

So this is not a simple "missing" category. It is a different abstraction
layer.

## Port Strategy

The right import strategy is not file-by-file. It is harness-first and
crate-owned.

### Stage 1: build a reusable parity harness

Before importing many PyTorch scenarios, build the Rust-side equivalent of the
registries that make PyTorch's tests scalable.

Needed immediately:

- an op-case registry for `psionic-ir`
- a module-case registry for trainable reusable module surfaces
- an optimizer-case registry for `psionic-train`

Those registries should carry:

- input fixture definitions
- expected forward outputs
- expected gradients when applicable
- dtype and shape expectations
- allowed tolerances
- expected error cases

This is the single most valuable structural lesson to borrow from PyTorch.

### Stage 2: add a compact PyTorch-derived operator parity matrix

Primary sources:

- `test/test_ops.py`
- `test/test_reductions.py`
- `test/test_view_ops.py`
- `test/test_shape_ops.py`
- `test/test_type_promotion.py`

Landing zone:

- `psionic-ir`

Build first:

- transformer-relevant ops and semantics:
  - broadcast rules
  - reduction behavior
  - reshape/view semantics
  - batched matmul variants
  - softmax/log-softmax
  - activation families needed by current models
  - type-promotion rules for the dtypes Psionic actually supports

Do not start with:

- the full PyTorch operator surface
- obscure device-only overloads
- backend-specific dispatch behavior

### Stage 3: deepen autograd and functionalization coverage

Primary sources:

- `test/test_autograd.py`
- `autograd/test_functional.py`
- selected `functorch/test_*`
- `test/test_functionalization.py`

Landing zone:

- `psionic-ir`
- `psionic-train`

Add first:

- more gradient reference cases
- view/mutation edge cases
- higher-order gradient cases only where the runtime actually needs them
- explicit no-grad / detach / shared-path edge cases beyond the current five
  tests

This is the right PyTorch area to strengthen once the operator matrix exists.

### Stage 4: add selective module and state-tree conformance

Primary sources:

- `test/test_nn.py`
- `test/test_modules.py`
- `test/nn/test_load_state_dict.py`

Landing zones:

- `psionic-train`
- any future reusable model/module crate, if one becomes necessary

Add first:

- typed module-like parity for the surfaces Psionic actually trains or serves:
  - `Linear`
  - normalization layers
  - embeddings
  - attention-adjacent reusable blocks
- strict vs non-strict state loading semantics for portable state trees
- key mismatch, shape mismatch, and missing-key refusal cases

Do not import:

- Python module hooks and metaclass behavior
- `nn.Module` API parity for its own sake

### Stage 5: deepen optimizer correctness and serialization

Primary sources:

- `test/test_optim.py`
- `optim/test_optim.py`
- `optim/test_lrscheduler.py`

Landing zone:

- `psionic-train`

Add first:

- loss-descent and parameter-update reference fixtures
- parameter-group equivalence tests
- optimizer-state roundtrip tests
- scheduler integration only if schedulers become a real product need

Defer:

- foreach/fused implementation parity
- differentiable optimizer semantics
- full hook-order API parity

### Stage 6: import compiler invariants without importing PyTorch's frontend

Primary sources:

- `test/test_fx.py`
- `test/export/test_export.py`
- selected `dynamo/*`
- `inductor/test_cache.py`
- `inductor/test_memory_planning.py`

Landing zones:

- `psionic-compiler`
- `psionic-ir`
- parts of `psionic-serve` that own plan caches today

The right translation is not:

- build `torch.fx`
- build `torch.export`
- build `torch.compile`

The right translation is:

- port the invariants those suites defend:
  - stable graph forms
  - symbolic shape handling
  - cache determinism
  - recompilation or invalidation on shape changes
  - explicit memory-plan checks

This stage is valuable, but only after the operator and autodiff surfaces are
less sparse.

### Stage 7: keep serialization intentionally narrow

PyTorch's serialization breadth should not be copied blindly.

Recommended rule:

- keep typed JSON, safetensors, and GGUF as primary artifact surfaces
- do not add pickle-based `.pt` load/save semantics unless there is an explicit
  product requirement
- if PyTorch interoperability is needed, prefer safe conversion tooling over
  runtime pickle compatibility

### Stage 8: defer ONNX and most distributed parity

Defer for now:

- ONNX exporter or importer breadth
- TorchScript/JIT parity
- DDP/process-group parity
- mobile/package/cpp-extension/custom-backend lanes
- full quantization API parity

Those are large programs, not reasonable side effects of "repeat the Tinygrad
analysis for PyTorch."

## Recommended Backlog Shape

If this work starts now, the backlog should be ordered like this:

1. Build a reusable parity harness modeled on `op_db` / `module_db` / `optim_db`
   ideas.
2. Add a compact PyTorch-derived operator and autodiff matrix in `psionic-ir`.
3. Add selective module and `state_dict` conformance in `psionic-train`.
4. Expand optimizer correctness and optimizer-state roundtrip tests.
5. Add compiler-cache, symbolic-shape, and memory-plan invariants in
   `psionic-compiler`.
6. Revisit ONNX or broader compiler-frontend work only if product scope demands
   it.

## Bottom Line

PyTorch is not the right parity target for Psionic as a whole.

It is the right local oracle for:

- core operator semantics
- autograd
- module and state-tree discipline
- optimizer behavior
- compiler invariants

It is not the right oracle for:

- GGUF
- tokenizer behavior
- KV cache and prefix cache semantics
- local LLM serving

So the practical strategy is:

- keep using Tinygrad to drive model-serving and GGUF-related imports
- use PyTorch to drive a much stronger Rust-native parity harness for ops,
  autodiff, modules, optimizers, and compiler invariants
- borrow PyTorch's test-architecture ideas, especially declarative registries,
  rather than trying to mimic PyTorch's Python API or CI footprint

If that happens, this repo will gain the best part of PyTorch's testing
discipline without taking on PyTorch's full product surface.
