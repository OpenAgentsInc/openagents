# Psionic Framework-Core Acceptance Matrix

> Status: canonical `#3609` closure doc, updated 2026-03-14 after landing the
> runnable matrix hook in
> `scripts/release/check-psionic-framework-core-acceptance.sh` and the
> reusable-optimizer closure for `#3603` plus the reverse-mode autodiff closure
> for `#3602`.

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

The runner is intentionally honest about remaining boundaries:

- it executes the current validation hooks for each category
- it prints explicit refusal or implemented-early notes when a category is real
  but still intentionally bounded
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

Cross-library refusal truth is now explicit too: `psionic-core` owns the
canonical `PsionicRefusal` taxonomy, and graph, autodiff, runtime, topology,
and sandbox seams now adapt into that shared type instead of inventing new
shadow refusal records for the same unsupported families.

## Matrix

| Category | Current status | What a green category would mean | Current repo truth | Canonical hooks | Open gap / refusal discipline |
| --- | --- | --- | --- | --- | --- |
| Tensor semantics | `implemented_early` | typed tensor identity, shape/layout transforms, alias-preserving view rules, storage identity and view posture contracts, broadcast-compatible binary shape semantics, reduction shape rules, dtype promotion, and quantized payload containers behave deterministically enough to anchor compiler and IO layers | `psionic-core` now owns explicit broadcast, dtype-promotion, dtype-class, quantized-logical-storage, layout storage-span, and alias/view-semantic rules, `psionic-runtime` now exposes a backend-visible `BufferStorageContract`, and `psionic-ir` lowers broadcasted binary ops through explicit `expand` views instead of backend-only coincidence | `psionic-core`, `psionic-runtime`, `psionic-ir`, and `psionic-backend-cpu` tensor-semantics tests | Keep the category honest: the shared semantics are now explicit, but this is still not a blanket claim that every backend already executes every promoted dtype or advanced storage family |
| Autodiff and optimizer behavior | `implemented_early` | reverse-mode autodiff, detach semantics, training-mode gradient rules, broad current primitive-family coverage, and reusable optimizer families are all machine-checkable and not hidden inside one training loop | `psionic-ir` now owns autodiff-aware graph construction, an explicit gradient-support matrix, symbolic backward plans, dense reference materialization, full current primitive-family gradient regression tests, and typed refusal across current backend-extension families, while `psionic-train` owns the reusable optimizer and distributed-optimizer contracts layered above it | `psionic-ir` autodiff tests plus `psionic-train` integration, optimizer, and distributed-optimizer tests | Keep the category honest: the current primitive-family surface is broadly covered, but backend-extension gradients still refuse explicitly and later operator families remain outside this runner |
| Model and state IO | `implemented_early` | model weights, optimizer state, adapter deltas, tokenizer bindings, and manifest receipts roundtrip through stable formats without losing role or spec truth | `psionic-train::model_io` already owns safetensors export/import, GGUF import, tensor-role manifests, and typed artifact receipts | `psionic-train` model-IO roundtrip and GGUF inventory tests | Do not treat a serving-family loader alone as full model-state IO closure; state-dict and optimizer-state roundtrip must stay in scope |
| Compiler lowering and realize path | `implemented_early` | compile lowering is deterministic, topology-sensitive, extension-aware, schema-backed, fake- or meta-executable, and replayable from named fixtures instead of being only an internal implementation detail | `psionic-ir` now publishes a built-in plus extensible operator registry with explicit built-in/custom schema registration, backend-kernel registration, dispatch resolution, meta-execution contracts, shape-only graph/plan execution, and capability-gated refusal behavior, while `psionic-compiler` now owns deterministic lowering plus explicit schedule formation, fusion-policy realization, alias-aware memory planning, plan-cache identity, compile-cache evidence, and fixture-backed replay above that IR surface | `psionic-ir` operator-registry, extensible-registry, and meta-execution tests plus `psionic-compiler` compile-graph, compiler-artifact, plan-cache, and `process_replay` tests | Do not claim framework-core closure if lowering stays green only on one happy-path fixture while operator schemas, extension registration, dispatch resolution, meta validation, fake execution, schedule/memory/cache identity, graph identity, or topology sensitivity drift |
| Memory planning and cache behavior | `implemented_early` | model admission, compiler memory planning, allocator/cache budgets, KV/prefix cache state, and runtime resource reports stay explicit and bounded instead of being hidden behind backend heuristics | `psionic-compiler` now owns compile-time tensor lifetime intervals, alias-aware slot reuse, stable plan-cache identity, and cold-vs-warm compile-cache evidence, while `psionic-runtime` still owns model-admission planning, runtime resource reports, and prefix/KV cache contracts | `psionic-compiler` compiler-artifact and plan-cache tests plus `psionic-runtime` admission, budget, KV cache, and prefix-cache tests | Do not collapse compile-time or runtime cache truth into product throughput headlines; framework-core acceptance cares about explicit policy and refusal behavior too |
| Process replay and program identity | `implemented_early` | reviewers can tell whether a compiled or trained path is the same program, the same replay contract, and the same tool/environment posture | `psionic-compiler` fixture replay and `psionic-train` replay-truth receipts already exist | `psionic-compiler` replay fixtures plus `psionic-train` replay-truth tests | Do not treat request receipts or serving-route provenance alone as framework replay closure |
| Same-type local multi-device behavior | `implemented_early` | one same-host same-backend runner can realize a plan across multiple devices with explicit topology, sharding policy, and refusal reasons | `psionic-runtime` now owns a same-type local multi-device runner plus explicit refusal taxonomy, and `psionic-models` now publishes one representative decoder-family tensor-parallel sharding contract | representative decoder-family sharding-contract test, local multi-device runner execution/refusal tests, plus topology-sensitive compiler digest test | Keep local serving acceptance honest: the lower-level runtime runner is real, but `TOPOLOGY_ACCEPTANCE_MATRIX.md` still keeps local tensor/pipeline/layer/replica serving unsupported until a served lane adopts it |

## Category Mapping

### 1. Tensor semantics

This category is about the tensor and layout substrate itself, not about model
quality or serving throughput.

Current shipped foundation:

- `TensorSpec` keeps device and dtype explicit
- layout expand/permute/slice/select rules are deterministic and can be checked
  as alias-preserving transforms over the source storage span
- layout storage spans, view posture, and alias relations are machine-legible
  rather than backend-only convention
- broadcast-compatible binary shape rules are explicit in `psionic-core` and
  realized in `psionic-ir` through inserted `expand` views
- mixed `I8`/`F16`/`BF16`/`F32` binary ops use an explicit small promotion table
- dtype class and quantized logical-storage eligibility are explicit in
  `psionic-core`
- quantized tensor payload containers exist as typed data, not log-only blobs
- backend-visible storage identity is explicit in `psionic-runtime` and the CPU
  reference backend preserves it through dense views and allocator reuse
- graph/autodiff/runtime/topology/sandbox seams now share one canonical refusal
  taxonomy for unsupported op, gradient, layout, capability, serialization, and
  policy-denial boundaries

Canonical hooks:

- `cargo test -p psionic-core tests::tensor_spec_retains_device_and_dtype -- --exact`
- `cargo test -p psionic-core tests::shape_broadcast_merges_trailing_singleton_axes -- --exact`
- `cargo test -p psionic-core tests::dtype_promotion_prefers_widest_supported_representation -- --exact`
- `cargo test -p psionic-core tests::dtype_contracts_mark_current_quantized_and_dense_surface -- --exact`
- `cargo test -p psionic-core tests::derived_views_remain_alias_preserving_transforms -- --exact`
- `cargo test -p psionic-core tests::layout_alias_relation_tracks_dense_and_broadcast_views -- --exact`
- `cargo test -p psionic-core tests::psionic_refusal_builder_keeps_code_scope_and_subject -- --exact`
- `cargo test -p psionic-core tests::layout_expand_uses_zero_strides -- --exact`
- `cargo test -p psionic-core tests::layout_permute_updates_shape_and_strides -- --exact`
- `cargo test -p psionic-backend-cpu --lib tests::cpu_buffer_views_preserve_storage_identity_and_view_semantics -- --exact`
- `cargo test -p psionic-backend-cpu --lib tests::cpu_allocator_pool_reuses_dense_storage_identity -- --exact`
- `cargo test -p psionic-ir --lib tests::graph_error_refusal_taxonomy_maps_layout_capability_and_serialization_boundaries -- --exact`
- `cargo test -p psionic-ir --lib autodiff::tests::autodiff_refusal_taxonomy_maps_unsupported_gradient_family -- --exact`
- `cargo test -p psionic-ir --lib tests::binary_ops_broadcast_inputs_through_explicit_expand_views -- --exact`
- `cargo test -p psionic-backend-cpu --lib tests::cpu_backend_executes_broadcast_add_over_index_views -- --exact`
- `cargo test -p psionic-runtime --lib tests::runtime_refusal_taxonomy_maps_capability_and_serialization_boundaries -- --exact`
- `cargo test -p psionic-runtime --lib local_multi_device::tests::local_sharding_contract_refusal_taxonomy_surfaces_topology_mismatch -- --exact`
- `cargo test -p psionic-sandbox --lib execution::tests::policy_rejection_receipt_maps_into_refusal_taxonomy -- --exact`

### 2. Autodiff and optimizer behavior

This category is now `implemented_early`.

Current shipped foundation:

- autodiff-aware IR graph construction with explicit `detach`
- training/evaluation plus no-grad gradient semantics
- symbolic backward plans and dense reference gradient materialization for the
  full current primitive op family
- trainer-step integration proof that autodiff gradients feed the fixed-budget training core
- explicit-gradient trainer steps with typed telemetry
- reusable SGD, Adam, AdamW, LARS, and LAMB primitives outside one trainer loop
- typed per-group optimizer state plus distributed optimizer contracts

Implemented-early boundary:

- unsupported backend-extension gradients must still refuse through typed paths
- later operator families outside the current primitive/core extension surface
  remain outside the current hook set

Canonical hooks:

- `cargo test -p psionic-ir --lib autodiff::tests::reverse_mode_autodiff_materializes_matmul_chain_gradients -- --exact`
- `cargo test -p psionic-ir --lib autodiff::tests::reverse_mode_autodiff_accumulates_shared_paths_and_honors_detach -- --exact`
- `cargo test -p psionic-ir --lib autodiff::tests::reverse_mode_autodiff_covers_select_concat_and_reshape_primitives -- --exact`
- `cargo test -p psionic-ir --lib autodiff::tests::reverse_mode_autodiff_accepts_non_scalar_axis_seed -- --exact`
- `cargo test -p psionic-ir --lib autodiff::tests::autodiff_context_makes_training_and_no_grad_behavior_explicit -- --exact`
- `cargo test -p psionic-ir --lib autodiff::tests::autodiff_support_matrix_marks_primitives_and_backend_extensions_explicitly -- --exact`
- `cargo test -p psionic-ir --lib autodiff::tests::unsupported_gradient_backend_extensions_refuse_per_op_label -- --exact`
- `cargo test -p psionic-ir --lib autodiff::tests::unsupported_gradient_ops_refuse_through_typed_error -- --exact`
- `cargo test -p psionic-train --lib core_loop::tests::autodiff_gradients_compose_with_fixed_budget_training_core -- --exact`
- `cargo test -p psionic-train --lib core_loop::tests::fixed_budget_training_loop_applies_updates_and_tracks_telemetry -- --exact`
- `cargo test -p psionic-train --lib optimizer::tests::reusable_optimizer_surface_advances_small_model_with_sgd_and_adam -- --exact`
- `cargo test -p psionic-train --lib optimizer::tests::reusable_optimizer_surface_supports_all_declared_optimizer_families -- --exact`
- `cargo test -p psionic-train --lib optimizer::tests::reusable_optimizer_surface_refuses_state_kind_mismatch -- --exact`
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

Current shipped foundation:

- `psionic-ir` now exposes one built-in operator registry with stable schemas
  for source, composite, backend-kernel, and backend-extension ops
- the registry carries a visible split between implementation family and
  meta-execution posture rather than leaving that boundary implicit in one
  backend
- `psionic-ir` now also exposes an extensible registry contract seeded from
  the built-ins so custom schemas, backend-kernel registrations, and dispatch
  resolution stay on one typed registry surface instead of forking into
  per-backend shadow maps
- execution plans can be revalidated against the built-in registry, and graphs
  or plans can now run through fake/meta execution without material tensor data
- fake execution can refuse explicit backend-kernel capability gaps, which lets
  compatibility harnesses stay honest about what a target surface claims to
  support
- `psionic-compiler` now publishes deterministic schedule formation, explicit
  fusion groups, alias-aware memory intervals and slot reuse, stable
  plan-cache identity, and cold-versus-warm compile-cache evidence rather than
  leaving those compiler surfaces implicit inside one backend
- compiler replay fixtures now snapshot the richer compiler artifact contract,
  not only the lowered plan signature

Canonical hooks:

- `cargo test -p psionic-ir --lib tests::builtin_operator_registry_exposes_kernel_composite_and_meta_surfaces -- --exact`
- `cargo test -p psionic-ir --lib tests::extensible_operator_registry_seeds_builtin_dispatch_contracts -- --exact`
- `cargo test -p psionic-ir --lib tests::extensible_operator_registry_accepts_custom_schema_and_backend_dispatch -- --exact`
- `cargo test -p psionic-ir --lib tests::extensible_operator_registry_refuses_shadowing_duplicates_and_missing_output -- --exact`
- `cargo test -p psionic-ir --lib tests::operator_registry_validates_execution_plan_specs -- --exact`
- `cargo test -p psionic-ir --lib tests::meta_executor_runs_graph_without_real_tensor_data -- --exact`
- `cargo test -p psionic-ir --lib tests::meta_executor_refuses_missing_backend_kernel_capability -- --exact`
- `cargo test -p psionic-compiler --lib tests::compile_graph_lists_expected_steps -- --exact`
- `cargo test -p psionic-compiler --lib tests::compile_graph_plan_can_run_through_meta_execution_without_tensor_data -- --exact`
- `cargo test -p psionic-compiler --lib tests::compile_graph_preserves_deterministic_digest -- --exact`
- `cargo test -p psionic-compiler --lib tests::compile_graph_artifacts_surface_schedule_fusion_and_memory_contracts -- --exact`
- `cargo test -p psionic-compiler --lib tests::compile_graph_artifacts_cache_identity_tracks_topology_and_contract_changes -- --exact`
- `cargo test -p psionic-compiler --lib tests::compiler_plan_cache_emits_cold_compile_then_warm_reuse_evidence -- --exact`
- `cargo test -p psionic-compiler --test process_replay matmul_add_replay_fixture_matches -- --exact`
- `cargo test -p psionic-compiler --test process_replay attention_backend_extension_tensor_sharded_replay_fixture_matches -- --exact`

### 5. Memory planning and cache behavior

This category is about explicit planning, budget, and reuse truth.

Current shipped foundation:

- `psionic-compiler` now exposes compile-time tensor lifetime intervals,
  alias-aware slot reuse, and stable plan-cache identity/evidence instead of
  leaving those decisions implicit inside a backend-specific compile path
- `psionic-runtime` still owns runtime admission budgets, runtime resource
  reports, and prefix/KV cache contracts for realized execution paths

Canonical hooks:

- `cargo test -p psionic-compiler --lib tests::compile_graph_artifacts_surface_schedule_fusion_and_memory_contracts -- --exact`
- `cargo test -p psionic-compiler --lib tests::compile_graph_artifacts_cache_identity_tracks_topology_and_contract_changes -- --exact`
- `cargo test -p psionic-compiler --lib tests::compiler_plan_cache_emits_cold_compile_then_warm_reuse_evidence -- --exact`
- `cargo test -p psionic-runtime --lib tests::model_admission_can_evict_oldest_idle_model_to_fit_budget -- --exact`
- `cargo test -p psionic-runtime --lib tests::model_admission_refuses_when_only_active_models_block_the_budget -- --exact`
- `cargo test -p psionic-runtime --lib tests::prefix_cache_identity_and_policy_serialize_stably -- --exact`
- `cargo test -p psionic-runtime --lib tests::kv_cache_state_and_growth_serialize_stably -- --exact`

### 6. Process replay and program identity

This category is about proving that "the same thing ran again" is a typed claim
with stable receipts and fixture identity.

Current shipped foundation:

- `psionic-runtime` now owns a serializable determinism contract with explicit
  mode and deterministic-algorithm posture rather than treating seeded replay
  as one ad hoc sampler flag
- replayable generator state can now be exported from `TokenSampler`,
  checkpointed alongside runtime state, restored later, and resumed without
  silently resetting the RNG stream
- local-device and distributed-rank generator derivation are stable and
  machine-checkable instead of being left to lane-local seed math

Canonical hooks:

- `cargo test -p psionic-train --lib replay_truth::tests::replay_truth_receipt_is_machine_legible_and_verifiable -- --exact`
- `cargo test -p psionic-train --lib replay_truth::tests::replay_truth_verification_detects_seed_tool_and_order_drift -- --exact`
- `cargo test -p psionic-compiler --test process_replay matmul_add_replay_fixture_matches -- --exact`
- `cargo test -p psionic-runtime --lib tests::strict_determinism_contract_refuses_missing_generator_state -- --exact`
- `cargo test -p psionic-runtime --lib tests::runtime_determinism_contract_derives_stable_local_and_distributed_generators -- --exact`
- `cargo test -p psionic-runtime --lib tests::token_sampler_generator_state_restores_after_checkpoint -- --exact`

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
  autodiff or optimizer behavior, model-state IO, compiler identity,
  memory/cache policy, replay truth, and same-type local multi-device
  execution contracts.
- Psionic still keeps representative-coverage boundaries explicit: unsupported
  gradient families refuse cleanly, and the current hooks are not a claim that
  every future op family already has full reverse-mode coverage.
- A green serving or train acceptance result must not be cited as evidence that
  those framework-core gaps are closed.

That is the minimum honesty bar for any future Tinygrad-parity claim.
