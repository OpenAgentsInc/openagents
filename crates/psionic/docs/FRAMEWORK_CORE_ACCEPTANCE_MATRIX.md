# Psionic Framework-Core Acceptance Matrix

> Status: canonical `#3609` closure doc, updated 2026-03-14 after landing the
> runnable matrix hook in
> `scripts/release/check-psionic-framework-core-acceptance.sh`.

Psionic can now make several higher-level claims:

- it has local and clustered serving acceptance matrices
- it has typed train benchmark acceptance
- it has substantial runtime, cluster, datastream, sandbox, and train substrate

Those claims are real, but they are not the same thing as saying Psionic is
close to Tinygrad-class ML framework completion on the actual framework core.

This document defines that narrower bar.

It is about `crates/psionic/*` only. It does not widen MVP product scope in
`docs/MVP.md`, and it does not move ownership boundaries out of
`docs/OWNERSHIP.md`.

## Canonical Runner

Run the framework-core matrix from the repo root:

```bash
scripts/release/check-psionic-framework-core-acceptance.sh
```

Targeted entrypoints:

```bash
scripts/release/check-psionic-framework-core-acceptance.sh --only tensor-semantics
scripts/release/check-psionic-framework-core-acceptance.sh --only autodiff-optimizer
scripts/release/check-psionic-framework-core-acceptance.sh --only model-state-io
scripts/release/check-psionic-framework-core-acceptance.sh --only compiler-realize
scripts/release/check-psionic-framework-core-acceptance.sh --only memory-cache
scripts/release/check-psionic-framework-core-acceptance.sh --only replay-identity
scripts/release/check-psionic-framework-core-acceptance.sh --only local-multi-device
```

The runner is intentionally honest about open gaps:

- it executes the current validation hooks for each category
- it prints explicit open-gap notes for rows that are still partial
- it does not treat serving or train acceptance as a substitute for
  framework-core closure

## Why This Matrix Exists

The current acceptance program already distinguishes:

- product-class serving acceptance
- serving-topology acceptance
- train benchmark acceptance

What was missing was a separate answer to:

> what does Psionic need before reviewers can claim it is approaching
> Tinygrad-class completeness on the core tensor/compiler/runtime framework,
> independent of product serving or train orchestration?

That answer has to name both the shipped foundations and the still-open holes.

## Claim Split

| Claim family | What it answers | What it does not answer |
| --- | --- | --- |
| Framework-core acceptance | whether Psionic has the core tensor/compiler/IO/replay/runtime substrate needed to be called a serious ML framework | it does not prove serving product closure or train product closure |
| Product-class serving acceptance | whether Psionic can honestly claim portability, throughput-serving, or structured-agent serving envelopes | it does not prove framework-core autodiff, reusable optimizer, or local multi-device closure |
| Train benchmark acceptance | whether the current train substrate meets typed performance thresholds | it does not prove general framework-core completeness or full autodiff closure |

If a future issue tries to close one of these claim families with evidence from
another, that issue is wrong by definition and should update this document
first.

## Status Legend

- `implemented_early`: there is a real foundation with runnable validation
  hooks, but the category is not broad enough yet to justify a sweeping parity
  claim.
- `partial`: some of the category is real, but one or more central framework
  pieces remain open and must stay explicit.

## Matrix

| Category | Current status | What a green category would mean | Current repo truth | Canonical hooks | Open gap / refusal discipline |
| --- | --- | --- | --- | --- | --- |
| Tensor semantics | `implemented_early` | typed tensor identity, shape/layout transforms, dtype and device semantics, and quantized payload containers behave deterministically enough to anchor compiler and IO layers | `psionic-core` owns `TensorSpec`, layout transforms, typed tensor payload containers, and stable device/dtype semantics | `psionic-core` tests for tensor spec, expand, and permute semantics | Do not claim eager full-framework tensor execution breadth from these layout or metadata tests alone |
| Autodiff and optimizer behavior | `partial` | reverse-mode autodiff, detach semantics, training-mode gradient rules, and reusable optimizer families are all machine-checkable and not hidden inside one training loop | `psionic-train` already has an explicit-gradient fixed-budget core plus typed optimizer and distributed-optimizer contracts | `psionic-train` training-core and distributed-optimizer tests | `#3602` and `#3603` remain open. Current hooks only validate explicit-gradient training and train-owned optimizer contracts, not general reverse-mode autodiff or reusable optimizer primitives |
| Model and state IO | `implemented_early` | model weights, optimizer state, adapter deltas, tokenizer bindings, and manifest receipts roundtrip through stable formats without losing role or spec truth | `psionic-train::model_io` already owns safetensors export/import, GGUF import, tensor-role manifests, and typed artifact receipts | `psionic-train` model-IO roundtrip and GGUF inventory tests | Do not treat a serving-family loader alone as full model-state IO closure; state-dict and optimizer-state roundtrip must stay in scope |
| Compiler lowering and realize path | `implemented_early` | compile lowering is deterministic, topology-sensitive, extension-aware, and replayable from named fixtures instead of being only an internal implementation detail | `psionic-compiler` already owns deterministic graph compilation, topology-sensitive digests, and fixture-backed replay tests | `psionic-compiler` compile-graph and `process_replay` tests | Do not claim framework-core closure if lowering stays green only on one happy-path fixture while graph identity or topology sensitivity drifts |
| Memory planning and cache behavior | `implemented_early` | model admission, allocator/cache budgets, KV/prefix cache state, and runtime resource reports stay explicit and bounded instead of being hidden behind backend heuristics | `psionic-runtime` already owns model-admission planning, runtime resource reports, prefix/KV cache contracts, and cache observations | `psionic-runtime` admission, budget, KV cache, and prefix-cache tests | Do not collapse runtime cache truth into product throughput headlines; framework-core acceptance cares about explicit policy and refusal behavior too |
| Process replay and program identity | `implemented_early` | reviewers can tell whether a compiled or trained path is the same program, the same replay contract, and the same tool/environment posture | `psionic-compiler` fixture replay and `psionic-train` replay-truth receipts already exist | `psionic-compiler` replay fixtures plus `psionic-train` replay-truth tests | Do not treat request receipts or serving-route provenance alone as framework replay closure |
| Same-type local multi-device behavior | `implemented_early` | one same-host same-backend runner can realize a plan across multiple devices with explicit topology, sharding policy, and refusal reasons | `psionic-runtime` now owns a same-type local multi-device runner plus explicit refusal taxonomy, and `psionic-models` now publishes one representative decoder-family tensor-parallel sharding contract | representative decoder-family sharding-contract test, local multi-device runner execution/refusal tests, plus topology-sensitive compiler digest test | Keep local serving acceptance honest: the lower-level runtime runner is real, but `TOPOLOGY_ACCEPTANCE_MATRIX.md` still keeps local tensor/pipeline/layer/replica serving unsupported until a served lane adopts it |

## Category Mapping

### 1. Tensor semantics

This category is about the tensor and layout substrate itself, not about model
quality or serving throughput.

Current shipped foundation:

- `TensorSpec` keeps device and dtype explicit
- layout expand/permute rules are deterministic
- quantized tensor payload containers exist as typed data, not log-only blobs

Canonical hooks:

- `cargo test -p psionic-core tests::tensor_spec_retains_device_and_dtype -- --exact`
- `cargo test -p psionic-core tests::layout_expand_uses_zero_strides -- --exact`
- `cargo test -p psionic-core tests::layout_permute_updates_shape_and_strides -- --exact`

### 2. Autodiff and optimizer behavior

This category remains intentionally partial.

Current shipped foundation:

- explicit-gradient trainer steps with typed telemetry
- typed per-group optimizer state and distributed optimizer contracts

Open core gaps:

- reverse-mode autodiff
- detach semantics
- training-mode gradient semantics
- reusable SGD, Adam, AdamW, LARS, and LAMB primitives outside one train loop

Canonical hooks:

- `cargo test -p psionic-train --lib core_loop::tests::fixed_budget_training_loop_applies_updates_and_tracks_telemetry -- --exact`
- `cargo test -p psionic-train --lib distributed_optimizer::tests::distributed_optimizer_contract_surfaces_precision_and_memory_truth -- --exact`
- `cargo test -p psionic-train --lib distributed_optimizer::tests::distributed_optimizer_contract_refuses_incomplete_shard_coverage -- --exact`

### 3. Model and state IO

This category is about whether framework state can be exported, imported,
verified, and replayed without losing semantic meaning.

Canonical hooks:

- `cargo test -p psionic-train --lib model_io::tests::portable_model_bundle_roundtrips_through_safetensors_manifest -- --exact`
- `cargo test -p psionic-train --lib model_io::tests::gguf_import_surfaces_tokenizer_binding_and_tensor_inventory -- --exact`

### 4. Compiler lowering and realize path

This category is about deterministic lowering and identity, not only about one
successful runtime execution.

Canonical hooks:

- `cargo test -p psionic-compiler --lib tests::compile_graph_lists_expected_steps -- --exact`
- `cargo test -p psionic-compiler --lib tests::compile_graph_preserves_deterministic_digest -- --exact`
- `cargo test -p psionic-compiler --test process_replay matmul_add_replay_fixture_matches -- --exact`
- `cargo test -p psionic-compiler --test process_replay attention_backend_extension_tensor_sharded_replay_fixture_matches -- --exact`

### 5. Memory planning and cache behavior

This category is about explicit planning, budget, and reuse truth.

Canonical hooks:

- `cargo test -p psionic-runtime --lib tests::model_admission_can_evict_oldest_idle_model_to_fit_budget -- --exact`
- `cargo test -p psionic-runtime --lib tests::model_admission_refuses_when_only_active_models_block_the_budget -- --exact`
- `cargo test -p psionic-runtime --lib tests::prefix_cache_identity_and_policy_serialize_stably -- --exact`
- `cargo test -p psionic-runtime --lib tests::kv_cache_state_and_growth_serialize_stably -- --exact`

### 6. Process replay and program identity

This category is about proving that "the same thing ran again" is a typed claim
with stable receipts and fixture identity.

Canonical hooks:

- `cargo test -p psionic-train --lib replay_truth::tests::replay_truth_receipt_is_machine_legible_and_verifiable -- --exact`
- `cargo test -p psionic-train --lib replay_truth::tests::replay_truth_verification_detects_seed_tool_and_order_drift -- --exact`
- `cargo test -p psionic-compiler --test process_replay matmul_add_replay_fixture_matches -- --exact`

### 7. Same-type local multi-device behavior

This category is intentionally not green yet.

Current shipped foundation:

- multi-device topology identity and inventory qualifiers
- topology-sensitive compile digests
- one same-host same-backend local plan runner with explicit runtime refusal reasons
- one representative GGUF decoder-family tensor-parallel sharding contract with inspectable weight-class rules
- explicit local-serving unsupported-scope language in the serving-topology matrix so the framework runner is not mistaken for a served product lane

Canonical hooks:

- `cargo test -p psionic-models --lib sharding::tests::gguf_decoder_family_tensor_parallel_contract_is_declarative_and_inspectable -- --exact`
- `cargo test -p psionic-runtime --lib local_multi_device::tests::local_multi_device_plan_runner_executes_tensor_sharded_workload_without_cluster_truth -- --exact`
- `cargo test -p psionic-runtime --lib local_multi_device::tests::local_sharding_contract_refuses_backend_memory_and_device_count_mismatches -- --exact`
- `cargo test -p psionic-compiler --lib tests::compile_graph_with_topology_changes_digest_when_sharding_changes -- --exact`

## Current Closure Summary

As of this matrix:

- Psionic has real framework-core foundations for tensor/layout semantics,
  model-state IO, compiler identity, memory/cache policy, replay truth, and
  same-type local multi-device execution contracts.
- Psionic does not yet have a closed framework-core story for reverse-mode
  autodiff or reusable optimizer primitives.
- A green serving or train acceptance result must not be cited as evidence that
  those framework-core gaps are closed.

That is the minimum honesty bar for any future Tinygrad-parity claim.
