# 2026-03-15 Tinygrad Test Suite Port Audit

## Intent

This audit answers a narrower, execution-oriented question than the earlier
Tinygrad parity note:

> what test coverage from `~/code/tinygrad` is actually worth pulling into this
> repo, where should that coverage live in Rust, and what missing functionality
> must be built first?

The right goal is not:

- port every Tinygrad test line by line
- recreate Tinygrad's Python API in Rust just to keep test names familiar
- drag hardware-lab and benchmark lanes into default CI

The right goal is:

- identify the Tinygrad suites that defend real framework behavior
- port those scenarios into the owning Rust crates in `crates/psionic/*`
- build only the missing functionality required to make those tests honest
- keep default coverage deterministic, replay-safe, and local-fixture driven

This audit also corrects one important assumption from
`docs/audits/2026-03-14-tinygrad-parity-target-for-psionic-audit.md`:
Psionic already has materially more GGUF, tokenizer, KV-cache, and serving
coverage than that earlier architecture-focused snapshot gave it credit for.

## Scope

Tinygrad sources reviewed from `~/code/tinygrad`:

- `test/README`
- directory inventory under `test/`
- `test/backend/test_ops.py`
- `test/backend/test_nn.py`
- `test/backend/test_optim.py`
- `test/backend/test_schedule.py`
- `test/backend/test_jit.py`
- `test/backend/test_kernel_cache.py`
- `test/null/test_process_replay.py`
- `test/null/test_schedule_cache.py`
- `test/null/test_llm_tokenizer.py`
- `test/unit/test_schedule_cache.py`
- `test/unit/test_llm_server.py`
- `test/unit/test_gguf.py`
- `test/models/test_train.py`
- `test/models/test_onnx.py`

OpenAgents sources reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/audits/2026-03-14-tinygrad-parity-target-for-psionic-audit.md`
- `crates/psionic/docs/ARCHITECTURE.md`
- `crates/psionic/docs/TRAIN_SYSTEM.md`
- `crates/psionic/docs/deep-research-tinygrad.md`
- `crates/psionic/psionic-models/src/lib.rs`
- `crates/psionic/psionic-models/src/harmony.rs`
- `crates/psionic/psionic-train/src/model_io.rs`
- `crates/psionic/psionic-train/src/optimizer.rs`
- `crates/psionic/psionic-train/src/core_loop.rs`
- `crates/psionic/psionic-ir/src/autodiff.rs`
- `crates/psionic/psionic-compiler/tests/process_replay.rs`
- `crates/psionic/psionic-serve/src/lib.rs`
- `crates/psionic/psionic-serve/tests/*.rs`

## Executive Summary

Tinygrad's test tree is large, but only part of it should be ported.

The highest-value Rust port target is:

- most of `test/backend`
- selected `test/null`
- selected `test/unit`
- a small, selective slice of `test/models`

The low-value or deferred target is:

- `test/amd`
- `test/device`
- `test/speed`
- most of `test/external`
- `test/web`
- most of `test/testextra`

The biggest gap is not "Psionic has no Tinygrad-like coverage." That is no
longer true.

The real gap is more specific:

- GGUF quant formats are incomplete, especially `Q4_K`, `Q5_K`, and `Q6_K`
- compiler schedule and JIT conformance is much thinner than Tinygrad's matrix
- tensor-op and NN-module numerical parity coverage is still sparse
- optimizer parity is narrower than Tinygrad's torch-backed suite
- model-zoo and ONNX smoke coverage is thinner, but that is lower priority

So the right plan is:

1. finish the missing GGUF quantization surface
2. deepen compiler replay, schedule-cache, and JIT invariants
3. port LLM cache and tokenizer scenarios into existing serving/model crates
4. add a selective numerical parity suite for autodiff, ops, modules, and
   optimizers
5. defer hardware-lab and benchmark-style Tinygrad lanes

## What Tinygrad's Test Tree Looks Like

Tinygrad's `test/README` describes three main CI groups:

- `backend`: tests that run on each backend
- `null`: tests that do not require any backend
- `unit`: tests that only run on a single backend in CI

The current `test/` tree contains 316 Python or JS files total, including
helpers and harness modules. Counting runnable test-entry files gives a smaller
but still broad suite:

| Lane | Approx. test entry files | What it mainly covers | Port priority |
| --- | ---: | --- | --- |
| `backend` | 40 | tensor ops, modules, optimizer behavior, schedule, JIT, cache, dtype, linearizer | highest |
| `null` | 55 | replay, schedule cache, tokenizer, pure logic | highest |
| `unit` | 34 | GGUF, LLM cache semantics, backend-specific single-lane checks | highest |
| `models` | 9 | train smoke and ONNX smoke | medium |
| `amd` | 28 | AMD-driver and emulator specific behavior | defer |
| `external` | 73 | fuzz, benchmark, network/model fetch, non-hermetic cases | mostly defer |
| `device` | 4 | backend-device specific lanes | defer |
| `speed` | 4 | performance benchmarks | defer |
| `opt` | 3 | optimization experiments | defer |
| `testextra` | 10 | extra model/data coverage | defer |
| `web` | 1 | web path | defer |

The broad lesson is that Tinygrad uses tests to defend both framework semantics
and lab-side experimentation. Only the framework-semantic portion should be
ported into Rust by default.

## The Tinygrad Suites That Matter Most

### 1. Core framework conformance

These are the suites that most directly define whether Tinygrad behaves like a
real framework:

- `test/backend/test_ops.py`
  Broad tensor-op forward and backward parity, usually against PyTorch.
- `test/backend/test_nn.py`
  Module-level parity for `Linear`, `Conv`, `ConvTranspose`, `BatchNorm`,
  `LayerNorm`, `GroupNorm`, `Embedding`, `LSTM`, and state loading.
- `test/backend/test_optim.py`
  Optimizer behavior for `SGD`, `Adam`, `AdamW`, `LARS`, `LAMB`, mixed
  precision, duplicated weights, offload, and `Muon`.
- `test/backend/test_schedule.py`
  Large lowering and scheduling matrix: fusion, kernel-count expectations,
  reduce behavior, movement ops, pad/index/setitem interactions, and schedule
  shape sensitivity.
- `test/backend/test_jit.py`
  `TinyJit` capture, reset, shape mismatch, graph reuse, batch split, prune,
  free, and multi-device cases.
- `test/backend/test_kernel_cache.py`
  Cache reuse semantics after compile.
- `test/null/test_process_replay.py`
  Replay determinism around program generation and optimization state.
- `test/null/test_schedule_cache.py`
  Schedule-cache enable/disable/read/write behavior.
- `test/unit/test_schedule_cache.py`
  Bound-variable reuse and custom-kernel cache reuse.

If the Rust port wants "Tinygrad-relevant" confidence, these files are the core
of that claim.

### 2. LLM and model-IO behavior

These are the most relevant Tinygrad tests for Psionic's current model-serving
work:

- `test/unit/test_gguf.py`
  GGUF loading plus dequantization and GEMV checks for `Q4_0`, `Q4_1`,
  `Q8_0`, `Q4_K`, `Q5_K`, `Q6_K`, and `MXFP4`, plus unknown-type refusal.
- `test/null/test_llm_tokenizer.py`
  Tokenizer edge cases including early tokenize, control chars, raw bytes,
  special tokens, repeats, and pattern handling.
- `test/unit/test_llm_server.py`
  KV-cache reuse, cache invalidation, two-prompt schedule cache reuse, and
  chunked prefill.

These are much closer to Psionic's present needs than Tinygrad's backend-lab
tests.

### 3. Model smoke lanes

These are useful, but lower priority:

- `test/models/test_train.py`
  One-step train smoke for `ConvNeXt`, `EfficientNet`, `ViT`, `Transformer`,
  and `ResNet`; `bert` is still a TODO there.
- `test/models/test_onnx.py`
  ONNX smoke for selected image and transformer models.

They are helpful once the substrate is stable, but they should not come before
GGUF, schedule, replay, and serving-cache parity.

## What Psionic Already Covers

The repo already contains more Tinygrad-adjacent coverage than a top-level doc
scan suggests.

### `psionic-models`

`crates/psionic/psionic-models/src/lib.rs` already tests:

- GGUF metadata and tensor-info parsing
- tokenizer metadata loading for sentencepiece, GPT-style BPE, and BERT
- tokenizer ID validation failures
- chat-template metadata loading and prompt rendering fixtures
- GGUF weight-bundle loading for dense and quantized tensors
- paged blob artifact access and memory-mapped open path
- Ollama blob and manifest loading
- decoder adapter loading for Llama, Qwen, and GPT-OSS
- embedding adapter loading for BERT and Nomic-BERT
- reference block decode for:
  - `Q8_0`
  - `Q4_0`
  - `Q4_1`
  - `MXFP4`
- digest stability and block-alignment refusal cases

That means the repo is not starting from zero on GGUF or quantized weights.

The important limitation is more precise:

- `GgufTensorType` recognizes many GGUF tensor kinds
- the actual supported decode and storage path currently only maps
  `MXFP4`, `Q4_0`, `Q4_1`, and `Q8_0`
- formats still rejected include:
  - `Q5_0`
  - `Q5_1`
  - `Q8_1`
  - `Q2_K`
  - `Q3_K`
  - `Q4_K`
  - `Q5_K`
  - `Q6_K`
  - `Q8_K`

That is the clearest "missing functionality" surfaced by the Tinygrad suite.

### `psionic-models::harmony`

`crates/psionic/psionic-models/src/harmony.rs` already covers:

- GPT-OSS tokenizer special-token encoding
- token-to-byte recovery behavior
- local-oracle checks against a GPT-OSS GGUF when present

This is already close in spirit to `test/null/test_llm_tokenizer.py`, but the
Tinygrad edge cases are broader and should still be imported selectively.

### `psionic-train`

`crates/psionic/psionic-train/src/model_io.rs` already includes GGUF import
tests that surface tokenizer binding and tensor inventory.

`crates/psionic/psionic-train/src/optimizer.rs` already covers:

- optimizer-surface advancement with `SGD` and `Adam`
- support for declared optimizer families
- state-kind mismatch refusal

`crates/psionic/psionic-train/src/core_loop.rs` already covers:

- fixed-budget training-loop updates
- autodiff integration into the training loop
- durable checkpoint restore
- missing-gradient refusal

That is real training substrate coverage, but it is not yet a Tinygrad-scale
numerical parity matrix.

### `psionic-ir`

`crates/psionic/psionic-ir/src/autodiff.rs` already contains real autodiff
machinery. What is still missing is broad test coverage proving numerical
behavior across the same kind of op matrix that Tinygrad exercises.

### `psionic-compiler`

`crates/psionic/psionic-compiler/tests/process_replay.rs` already provides
deterministic replay coverage for two fixtures:

- `matmul_add_replay_fixture_matches`
- `attention_backend_extension_tensor_sharded_replay_fixture_matches`

This is a good start and a direct analogue to Tinygrad replay tests, but it is
far smaller than Tinygrad's schedule, cache, and JIT matrix.

### `psionic-serve`

This is where the earlier parity discussion most clearly undercounted current
coverage.

`crates/psionic/psionic-serve/src/lib.rs` already includes internal tests for:

- session isolation and KV-cache reset
- paged KV growth, refill, refusal, eviction, and reclaim
- host/device KV residency transitions
- shared-prefix store hit, miss, rebuild, and refusal semantics
- continuous batching that mixes prefill and decode
- prefix-hit and bypass reporting
- session reuse, reset, and unknown-session refusal

The integration tests in `crates/psionic/psionic-serve/tests/*.rs` also cover
reference generation, model-backed generation, embeddings, and hardware-parity
paths.

So the right conclusion is not "build LLM cache semantics from scratch." The
right conclusion is:

> import Tinygrad's LLM cache scenarios into `psionic-serve` to tighten and
> extend an existing feature surface.

## Gap Assessment

### Gap 1: GGUF quantization support is incomplete

This is the most direct functional gap exposed by Tinygrad's tests.

Tinygrad's `test/unit/test_gguf.py` explicitly covers:

- dequantization for `Q4_0`, `Q4_1`, `Q8_0`, `Q4_K`, `Q5_K`, `Q6_K`, `MXFP4`
- hardcoded decode checks
- GGUF GEMV behavior on loaded quantized tensors
- refusal on unknown types

Psionic already covers the first four implemented formats well enough to call
them real. It does not yet cover the K-family formats that Tinygrad exercises,
and those formats are materially relevant to local model interchange.

### Gap 2: schedule and JIT conformance is much thinner than Tinygrad's

Tinygrad's schedule and JIT suites are not small feature checks. They are a
conformance matrix for:

- fusion boundaries
- kernel-count stability
- symbolic shape behavior
- cache reuse
- graph capture and invalidation
- multi-device and graph-split semantics

Psionic has replay fixtures, but it does not yet have a comparable schedule or
JIT scenario matrix.

### Gap 3: tensor-op and NN numerical parity coverage is sparse

There is real autodiff and train-loop machinery in this repo, but there is not
yet a broad Rust-native equivalent of:

- `test/backend/test_ops.py`
- `test/backend/test_nn.py`

Without that, the implementation can be structurally impressive while still
missing subtle numerical or gradient regressions.

### Gap 4: optimizer parity is narrower

The optimizer surface exists and is tested, but it is still narrower than
Tinygrad's optimizer suite in two ways:

- there is no broad numeric parity grid like Tinygrad's torch-backed checks
- `Muon` does not appear to exist anywhere in the repo today

That does not automatically mean Psionic should add `Muon`. It means the repo
needs an explicit decision rather than silent omission.

### Gap 5: model-zoo and ONNX smoke is thinner but lower priority

Tinygrad uses model smoke to catch large integration failures. Psionic can add
selective equivalents later, but those should not outrank the core substrate
and serving tests above.

## Port Strategy

The right port strategy is crate-owned and scenario-owned, not file-owned.

Do not create a generic "tinygrad compatibility" crate. Land each imported
scenario in the crate that owns the behavior.

### Stage 1: finish GGUF and quantized tensor coverage

Primary target:

- port or adapt the core cases from `test/unit/test_gguf.py` into
  `psionic-models`

Build first:

- `Q4_K`
- `Q5_K`
- `Q6_K`

Build next:

- `Q5_0`
- `Q5_1`
- `Q8_1`
- `Q2_K`
- `Q3_K`
- `Q8_K`

Required work:

- implement block decode for the missing formats
- add local fixture bytes and reference outputs
- add GEMV-style behavior checks on loaded quantized tensors
- keep unsupported-type refusal tests for the remaining not-yet-built formats

This is the cleanest high-value tranche because the missing functionality is
obvious and the crate ownership is already clear.

### Stage 2: deepen compiler replay, schedule, and cache tests

Primary target:

- port or adapt the core cases from:
  - `test/null/test_process_replay.py`
  - `test/null/test_schedule_cache.py`
  - `test/unit/test_schedule_cache.py`
  - `test/backend/test_kernel_cache.py`
  - selected scenarios from `test/backend/test_schedule.py`
  - selected scenarios from `test/backend/test_jit.py`

Landing zone:

- `psionic-compiler`

Required work:

- add more replay fixtures beyond the current two examples
- add explicit schedule-cache and plan-cache hit/miss tests
- add shape-substitution and invalidation tests
- add graph-capture reset and reuse tests
- add kernel-count or plan-count invariants where the compiler surface exposes
  them cleanly

This is the second-highest priority because `docs/MVP.md` and Psionic's own
architecture emphasize deterministic and replay-safe behavior.

### Stage 3: import LLM cache semantics into `psionic-serve`

Primary target:

- port or adapt `test/unit/test_llm_server.py`

Landing zone:

- `psionic-serve`

Required work:

- keep the existing session and prefix-store tests
- add explicit Tinygrad-style scenarios for:
  - KV-cache reuse across prompts
  - cache invalidation after changed context
  - two-prompt or three-prompt cache stability
  - chunked prefill behavior
  - schedule or plan cache reuse across prompt extension

This is not a greenfield build. It is a refinement of an already-tested serving
surface.

### Stage 4: import tokenizer edge cases into `psionic-models`

Primary target:

- port or adapt `test/null/test_llm_tokenizer.py`

Landing zone:

- `psionic-models`

Required work:

- add fixture coverage for byte-level edge cases
- add special-token and repeated-pattern checks
- keep model-family specific prompt-oracle tests where they already exist

This is high leverage because tokenizer regressions silently poison both serve
and train flows.

### Stage 5: add selective numerical parity for ops, modules, and optimizers

Primary target:

- selected coverage from:
  - `test/backend/test_ops.py`
  - `test/backend/test_nn.py`
  - `test/backend/test_optim.py`

Landing zones:

- `psionic-ir`
- `psionic-train`
- any owning crate for module implementations if they split out later

Required work:

- define a compact parity matrix instead of importing Tinygrad wholesale
- cover the canonical cases first:
  - elementwise ops
  - reductions
  - matmul
  - broadcast semantics
  - gradient propagation
  - `Linear`
  - `Conv`
  - normalization layers
  - `Embedding`
  - `SGD`
  - `Adam`
  - `AdamW`
  - `LARS`
  - `LAMB`
- decide explicitly whether `Muon` belongs in Psionic's training surface

This stage matters, but it should follow GGUF and compiler conformance rather
than lead them.

### Stage 6: defer model-zoo, hardware, and benchmark lanes

Defer for now:

- most of `test/models`
- `test/amd`
- `test/device`
- `test/external`
- `test/speed`
- `test/web`

These lanes are valuable for a mature runtime program, but they are the wrong
first use of engineering time if the goal is "pull all relevant Tinygrad test
coverage into this repo."

## How To Port The Tests Without Importing Tinygrad's Problems

The port should preserve Tinygrad's behavioral intent, not its repo shape.

Recommended method:

1. Treat Tinygrad and, where needed, PyTorch as oracle generators.
2. Convert the scenario into a Rust-native unit or integration test in the
   owning crate.
3. Check in local deterministic fixtures or golden outputs.
4. Keep network fetches, heavyweight Python oracles, and hardware-specific
   comparisons out of default CI.
5. Provide optional refresh scripts only if the fixture-generation path needs
   to be reproducible later.

This avoids the two worst outcomes:

- a fake port that only renames tests without defending behavior
- a brittle CI lane that now depends on Python, Hugging Face downloads, or
  special hardware

## Recommended Backlog Shape

If this work starts now, the backlog should be ordered like this:

1. `psionic-models`: complete K-family and adjacent GGUF quant decode support,
   then port `test_gguf.py` scenarios.
2. `psionic-compiler`: expand replay fixtures and add schedule-cache, kernel
   cache, and JIT invalidation tests.
3. `psionic-serve`: import Tinygrad LLM cache and chunked-prefill scenarios.
4. `psionic-models`: port tokenizer edge-case coverage.
5. `psionic-ir` and `psionic-train`: add compact numerical parity suites for
   ops, modules, autodiff, and optimizers.
6. Revisit selective model and ONNX smoke only after the substrate lanes above
   are stable.

## Bottom Line

Tinygrad has a lot of tests, but the Rust port target is not "all of Tinygrad."

The correct target is:

- finish the GGUF and quantization functionality Tinygrad already proves useful
- import Tinygrad's strongest replay, schedule, JIT, tokenizer, and LLM cache
  scenarios into the crates that already own those behaviors
- add a selective numerical parity matrix for ops, modules, and optimizers
- explicitly defer hardware-lab, benchmark, and non-hermetic model-fetch lanes

If that plan is followed, Psionic can inherit the parts of Tinygrad's test
discipline that actually matter, without inheriting Tinygrad's repo shape or
its least-hermetic CI burdens.
