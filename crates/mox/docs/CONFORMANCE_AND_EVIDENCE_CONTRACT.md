# Mox Conformance And Evidence Contract

> Status: active cutover contract for `MOX-117`, `MOX-126B`, `MOX-162`,
> `MOX-171` through `MOX-175`, `OA-201`, and `OA-202`.

## Purpose

This document defines the minimum behavioral contract Mox must satisfy before
the desktop can honestly replace the external Ollama dependency.

It has two halves:

- a conformance harness that proves Mox matches or intentionally and explicitly
  redefines the subset of Ollama behavior OpenAgents depends on
- a runtime evidence schema that ensures receipts, diagnostics, and
  compute-market capability truth stay machine-checkable after cutover

This is a cutover contract, not a UI or orchestration spec. It stays inside
Mox-owned reusable runtime behavior plus the app-owned adapter seam.

## Conformance Harness Scope

`MOX-117` should produce a repeatable harness that can run the same scenarios
against the current Ollama-backed path and the Mox path and compare results.
See `crates/mox/docs/CONFORMANCE_HARNESS.md` for the concrete harness/report
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

`MOX-118` should maintain a small but real fixture corpus sourced from supported
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

`MOX-162` should define a first-class identity for the full served artifact set
instead of treating "same model" as a display name plus plan digest.

At minimum the identity tuple must cover:

- `served_artifact_digest`
- `model_blob_digest`
- `tokenizer_digest`
- `chat_template_digest`
- `generation_defaults_digest`
- `weight_format`
- `quantization_family`
- `backend_toolchain_version`

If any element of that tuple changes, cache reuse, comparability claims, and
receipt equivalence must be treated as changed unless a narrower reuse rule is
explicitly documented.

## Local Serving Isolation Policy

`MOX-160` should make the crash/reset boundary explicit instead of leaving the
desktop to infer it from implementation details.

At minimum the reusable runtime contract should surface:

- whether the active path is `in_process` or `subprocess`
- the smallest process boundary that contains a backend/runtime crash
- what happens on request-local failure versus backend error versus crash
- which state scopes are discarded on an isolation reset

For the current Mox path, that policy is intentionally `in_process`: request
failures are refused directly, backend execution errors require an explicit
runtime-state reset, and an outright backend/runtime crash implies host-process
restart because there is no smaller subprocess boundary yet.

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
- `effective_backend`
- `fallback_state`

Required execution-plan and cache fields:

- `execution_plan_digest`
- `compile_digest`
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

`MOX-161` should define the allowed fallback lattice used by this contract.

Until that lands, no implementation may silently:

- switch from accelerated backend to CPU
- switch from quantized execution to hidden eager dequantized execution while
  still advertising quantized parity
- change prompt or stop semantics to salvage a request

Allowed temporary behavior during development is:

- explicit refusal
- explicit degraded state
- explicit same-backend slow path when surfaced in evidence
- explicit replan or rewarm when surfaced in evidence
