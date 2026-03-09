# Psionic Conformance And Evidence Contract

> Status: active cutover contract for `PSI-117`, `PSI-126B`, `PSI-162`,
> `PSI-171` through `PSI-175`, `OA-201`, and `OA-202`.

## Purpose

This document defines the minimum behavioral contract Psionic must satisfy before
the desktop can honestly replace the external Ollama dependency.

It has two halves:

- a conformance harness that proves Psionic matches or intentionally and explicitly
  redefines the subset of Ollama behavior OpenAgents depends on
- a runtime evidence schema that ensures receipts, diagnostics, and
  compute-market capability truth stay machine-checkable after cutover

This is a cutover contract, not a UI or orchestration spec. It stays inside
Psionic-owned reusable runtime behavior plus the app-owned adapter seam.

## Conformance Harness Scope

`PSI-117` should produce a repeatable harness that can run the same scenarios
against the current Ollama-backed path and the Psionic path and compare results.
See `crates/psionic/docs/CONFORMANCE_HARNESS.md` for the concrete harness/report
shape that landed for this contract.

The harness must cover:

- `tags` / `list_models`
  - installed-model discovery
  - missing, corrupted, and unsupported-model behavior
- `show` / `show_model`
  - model identity, format, tokenizer facts, quantization facts, and supported
    capability metadata
- `ps` / `loaded_models`
  - loaded-model registry, warm state, unload state, and backend state
- `generate`
  - prompt rendering
  - BOS/EOS handling
  - truncation and over-limit refusal behavior
  - stop handling
  - seed determinism
  - streaming chunk semantics
  - slow-reader backpressure and dropped-client behavior
  - cancellation semantics
  - error-before-stream and error-after-stream behavior
- `embed`
  - single-input and batched behavior
  - normalization behavior
  - vector-dimension reporting
  - truncation and over-limit refusal behavior
  - failure semantics

## Golden Fixtures

`PSI-118` should maintain a small but real fixture corpus sourced from supported
GGUF or Ollama installs.

At minimum the fixture corpus must include:

- tokenizer metadata for supported families
- BOS/EOS and add-bos/add-eos behavior
- named and default chat-template variants
- stop-token and stop-sequence defaults
- prompt rendering edge cases with system and user message retention
- truncation edge cases for generation and embeddings

The harness should treat those fixtures as the truth source for supported model
families rather than hand-built synthetic strings alone.

## Served Artifact Identity Tuple

`PSI-162` defines a first-class `ServedArtifactIdentity` for the full served
artifact set instead of treating "same model" as a display name plus plan
digest.

At minimum the identity tuple must cover:

- `served_artifact_digest`
- `model_id`
- `model_revision`
- `weight_bundle_digest`
- `model_blob_digest`
- `tokenizer_digest`
- `chat_template_digest`
- `generation_defaults_digest`
- `weight_format`
- `quantization_family`
- `backend_toolchain_version`
- `effective_backend`

If any element of that tuple changes, cache reuse, comparability claims, and
receipt equivalence must be treated as changed unless a narrower reuse rule is
explicitly documented.

The same `ServedArtifactIdentity` must be surfaced without app-local
reconstruction through:

- generation provenance
- provider capability envelopes
- provider execution receipts
- any cache or persisted-state identity that claims reuse across requests

For the current implementation, session KV ownership and shared prefix-cache
reuse both key off the served-artifact digest. Descriptor drift and
served-artifact drift are separate invalidation cases and both must refuse
cross-run reuse.

## Local Serving Isolation Policy

`PSI-160` should make the crash/reset boundary explicit instead of leaving the
desktop to infer it from implementation details.

At minimum the reusable runtime contract should surface:

- whether the active path is `in_process` or `subprocess`
- the smallest process boundary that contains a backend/runtime crash
- what happens on request-local failure versus backend error versus crash
- which state scopes are discarded on an isolation reset

For the current Psionic path, that policy is intentionally `in_process`: request
failures are refused directly, backend execution errors require an explicit
runtime-state reset, and an outright backend/runtime crash implies host-process
restart because there is no smaller subprocess boundary yet.

## Cache Invalidation Policy

`PSI-163` defines a reusable `CacheInvalidationPolicy` plus per-request
`CacheObservation` records so cache and persisted-state behavior stops depending
on implicit process details.

The policy must cover at least:

- execution-plan caches
- backend kernel caches
- artifact-backed paged tensor storage
- shared prefix caches
- session KV state

For each cache family, the runtime must make explicit:

- the reuse scope
  - `process_local`, `shared_across_requests`, `session_bound`, or
    `artifact_backed`
- the current format version
- the compatible action
  - usually `reuse`
- the incompatible action
  - `rebuild`, `invalidate`, or `restore`
- the triggers that force invalidation

Those triggers must include the relevant subset of:

- `binary_upgrade`
- `backend_toolchain_upgrade`
- `model_metadata_change`
- `tokenizer_drift`
- `chat_template_drift`
- `generation_defaults_drift`
- `quantization_change`
- the cache-family format upgrade trigger
- `explicit_reset` where caller policy can discard state directly

Capability and observability surfaces should carry the full invalidation policy.
Generation provenance and generation receipts should carry the realized cache
observations for the request path so the runtime can explain whether state was
reused, rebuilt, bypassed, invalidated, or restored.

## Artifact Provenance And License Gating

`PSI-164` defines a separate governance contract for whether a local artifact
may be advertised or served into compute-market supply. This is not the same as
blob integrity:

- integrity answers "are these bytes present and uncorrupted?"
- provenance answers "what local source did this artifact come from?"
- license facts answer "what, if anything, was declared about its usage terms?"

At minimum the reusable Psionic surfaces must carry, when known:

- artifact provenance kind
  - `fixture`, `local_path`, `ollama_blob`, `ollama_manifest`, or
    `ollama_remote_alias`
- a source label
  - file path, blob name, or canonical Ollama model name
- manifest digest when the artifact came from a resolved Ollama manifest
- declared upstream alias facts
  - `remote_host`, `remote_model`, and `base_model` when present
- declared license payloads in source order
  - including stable per-license digests

Provider capability envelopes must also carry:

- the explicit compute-market supply policy being applied
- the resulting machine-checkable advertise/serve decision
- explicit refusal reasons when the policy denies supply

The default compute-market supply policy for Psionic must refuse external artifacts
that lack declared license facts, and it must distinguish at least:

- refusal because provenance class is disallowed
- refusal because governance metadata is missing entirely
- refusal because no license was declared
- refusal because licenses fail an allowlist or hit a denylist

Those policy-driven refusals must remain distinct from:

- integrity failures
- unsupported-format failures
- backend-readiness failures

So a caller can tell whether a model was rejected because it is corrupt,
unsupported, or policy-disallowed even when all three surfaces are exercised in
the same workflow.

## Ollama-Compat Migration Boundary

`PSI-170` defines the explicit boundary between Ollama compatibility support and
the long-term Psionic-owned model/runtime format.

The reusable Psionic contract must distinguish four separate questions:

- which catalog surface discovered the model
- how the model entered Psionic descriptor/runtime space
- which request/inspection surface is exposing it right now
- which runtime format actually owns execution

For the current implementation:

- `psionic-catalog`'s `OllamaModelCatalog` is an `ollama_compat_migration` surface
  only; it is not the long-term Psionic-native catalog contract
- importing weights from a resolved Ollama manifest is
  `ollama_compat_manifest_import`
- importing a raw Ollama blob by digest is `ollama_compat_blob_import`
- direct local GGUF or safetensors paths are `direct_artifact_import`
- programmatic fixtures are `fixture`
- loaded Psionic descriptors and the served runtime path remain `psionic_native` even
  when their source came from Ollama compatibility inputs

At minimum, `show`-style or descriptor-facing Psionic surfaces must make explicit:

- `psionic.catalog_surface` when discovery came through a catalog
- `psionic.model_ingress_surface`
- `psionic.serving_surface`
- `psionic.runtime_surface`

This keeps Ollama compatibility support honest as migration substrate instead of
letting the compatibility layer silently become the architectural source of
truth for Psionic execution.

## Capability Inventory Schema

`PSI-171` defines the minimum compute-market inventory and qualifier truth that
provider capability envelopes and execution receipts must expose.

At minimum those surfaces must carry:

- `backend_toolchain`
  - `effective_backend`
  - `toolchain_version`
  - `compiled_backend_features`
  - `probe_state`
    - `compiled_only` or `compiled_and_probed`
  - `probed_backend_features`
- `selected_device_inventory` when a concrete device was chosen
  - `stable_device_id`
  - `topology_key` when available
  - `performance_class`
    - `reference`, `integrated_accelerator`, `discrete_accelerator`, or
      `partitioned_accelerator`
  - `memory_class`
    - `host_only`, `shared_host_device`, or `dedicated_device`
  - `total_memory_bytes` when known
  - `free_memory_bytes` when the runtime can surface current budget truth

Those qualifier fields complement, rather than replace, the backend-specific
topology and risk metadata already surfaced through the AMD and NVIDIA context
objects. The generic qualifiers are for reusable inventory comparison and
compute-market filtering; the backend-specific contexts remain the source of
truth for vendor detail such as PCI topology, MIG state, or recovery posture.

## Batch And Queueing Schema

`PSI-172` defines the minimum batching, queueing, and throughput truth that
capability and observability surfaces must expose.

At minimum, served-product capability envelopes must carry an
`execution_profile` with:

- `batch_posture`
  - `single_request_only`
  - `caller_static_batch`
  - `scheduler_static_batch`
  - `continuous_batch`
- `queue_policy`
  - `discipline`
    - at least `direct_caller_backpressure` or `fifo`
  - `max_active_requests`
  - `max_queued_requests`
  - `per_model_serialization`
- `throughput_class`
  - at least `latency_optimized`, `balanced`, or `throughput_optimized`

The current Psionic implementation is intentionally explicit rather than
aspirational:

- local text generation is `single_request_only` with direct caller-owned
  backpressure, one active request, and no internal queue
- local embeddings are `caller_static_batch` with the same direct-caller queue
  posture, because callers may submit bounded input batches even though the
  runtime does not yet claim shared scheduler batching

Local-runtime observability must stay aligned with that capability truth. If it
surfaces queue depth or queue capacity, it must also carry the same
`execution_profile`/queue-policy context so callers can tell whether "queue
depth = 0" means "no queue currently" or "no internal queue exists at all."

## Multi-Device And Sharding Schema

`PSI-173` defines the minimum topology truth for same-backend multi-device and
sharded execution paths.

At minimum, capability and receipt surfaces must carry:

- `selected_devices`
  - one reusable inventory entry per concrete participating device
  - each entry should reuse the same qualifier schema as
    `selected_device_inventory`
- `execution_topology` when the runtime has a concrete placement plan
  - `effective_backend`
  - `kind`
    - `single_device`
    - `replicated`
    - `layer_sharded`
    - `tensor_sharded`
  - `assignments`
    - `shard_id`
    - `device`
      - `stable_device_id`
      - `topology_key` when available
      - `placement_index`
    - `partition`
      - `whole_model`
      - `replica`
      - `layer_range`
      - `tensor_range`

The current Psionic implementation is still operationally single-device for the
shipped product paths, but that single-device fact must now be explicit rather
than implied from a lone `selected_device` field. When future paths attach more
than one device, Psionic must not silently imply replication or sharding from a
device count alone; the topology kind and assignments must be surfaced
explicitly.

## Runtime Evidence Schema

Every `generate` and `embed` execution path that can feed receipts or
provider-facing diagnostics should emit a structured evidence record.

Required identity and backend fields:

- `request_id`
- `product_id`
- `model_id`
- `model_revision`
- `served_artifact_digest`
- `model_blob_digest`
- `tokenizer_digest`
- `chat_template_digest`
- `generation_defaults_digest`
- `weight_format`
- `quantization_family`
- `backend_family`
- `backend_interface_mode`
- `isolation_policy`
- `backend_toolchain_version`
- `compiled_backend_features`
- `selected_devices`
- `execution_topology`
- `effective_backend`
- `fallback_state`

Capability and receipt surfaces may carry the whole served-artifact object
instead of flattening every identity field, but they must still preserve the
same information content.

Required execution-plan and cache fields:

- `execution_plan_digest`
- `compile_digest`
- `runtime_resources.execution_plan_cache`
  - `policy.enabled`
  - `policy.max_cached_entries`
  - `policy.max_cached_bytes`
  - `state.cached_entries`
  - `state.cached_bytes`
- `compile_path`
  - `temperature`
    - `cold_compile` or `warm_reuse`
  - `execution_plan_cache`
  - `kernel_cache`
- `plan_cache_state`
  - `hit`, `miss`, or `rebuilt`
- `prefix_cache_state`
  - `none`, `hit`, `miss`, `bypassed`, or `rebuilt`
- `warm_cold_load_state`
  - `cold`, `warm`, `rewarm`, or `restored`

Required timing and movement fields:

- `queue_wait_ms`
- `load_ms`
- `first_token_ms` for generation
- `total_ms`
- `kernel_count`
- `bytes_moved`
  - total bytes minimum
  - split fields are allowed if the backend can expose them cleanly

Required delivery-proof and settlement-linkage fields:

- `delivery_proof.execution_plan_digest`
- `delivery_proof.kernel_count`
- `delivery_proof.bytes_moved`
- `delivery_proof.plan_cache_hits`
- `delivery_proof.plan_cache_misses`
- `delivery_proof.kv_growth`
  - `tokens`
  - `bytes`
  - `pages`
- `settlement_linkage.request_digest`
- `settlement_linkage.product_id`
- `settlement_linkage.model_id`
- `settlement_linkage.served_artifact_digest`
- `settlement_linkage.execution_plan_digest`
- `settlement_linkage.runtime_backend`
- `settlement_linkage.kernel_count`
- `settlement_linkage.bytes_moved`
- `settlement_linkage.plan_cache_hits`
- `settlement_linkage.plan_cache_misses`
- `settlement_linkage.kv_growth`
- `settlement_linkage.output_tokens` where applicable

Required token and KV fields:

- `prompt_tokens`
- `prefix_tokens_reused`
- `output_tokens` where applicable
- `context_tokens_used`
- `kv_bytes`
- `kv_growth_bytes`
- `kv_pages` when paged KV is active

Required outcome and refusal fields:

- `termination_reason`
- `refusal_reason_code`
- `degraded_reason_code`
- `error_taxonomy_code`

## Cutover Rules

`OA-201` and `OA-202` must not merge unless all of the following are true:

- the app-owned `LocalInferenceRuntime` adapter can surface the conformance and
  evidence fields without reaching back into Ollama-specific types
- the conformance harness passes for the supported model families and product
  paths
- failure, fallback, and degraded behavior remain truthful through the app seam
- receipts and diagnostics can carry the runtime evidence schema without
  app-local reconstruction
- served-artifact identity remains stable enough to compare runs and honest
  enough to invalidate caches and comparability claims when artifacts drift

## Fallback Policy Boundary

`PSI-161` defines the backend-neutral fallback lattice used by this contract.

The allowed fallback triggers are:

- `requested_backend_unavailable`
- `requested_backend_degraded`
- `numerical_safety_risk`
- `memory_pressure`
- `plan_unavailable`
- `transient_backend_failure`

The allowed fallback actions are:

- `refuse`
- `degrade`
- `replan`
- `retry`
- `same_backend_slow_path`

The current lattice rules are:

- `requested_backend_unavailable` may `refuse` or `replan`, depending on the
  served-product backend policy.
- `requested_backend_degraded` may `refuse` or `degrade`, but must remain on
  the requested backend when degraded execution is allowed.
- `numerical_safety_risk` must `refuse`; correctness wins over speed or
  convenience.
- `memory_pressure` may follow the explicit backend policy for that product
  path, but it must never silently substitute a backend or alter request
  semantics.
- `plan_unavailable` may take an explicit `same_backend_slow_path` or explicit
  `replan`, but only when the resulting path preserves request semantics.
- `transient_backend_failure` may `retry` explicitly; retry count must remain
  surfaced in runtime/provider truth.

Capability, receipt, and runtime-observability surfaces must carry fallback
state explicitly through `backend_selection`. That state must include:

- the requested backend
- the effective backend
- the realized selection state
- the active fallback lattice
- the fallback trigger and action when the request left the direct path
- the fallback or degraded reason
- the retry attempt count when a retry occurred

No implementation may silently:

- switch from accelerated backend to CPU
- switch from quantized execution to hidden eager dequantized execution while
  still advertising quantized parity
- change prompt or stop semantics to salvage a request
- relabel a refusal, retry, or same-backend slow path as a direct success
- publish capability or receipt evidence that omits the effective backend or
  the realized fallback state
