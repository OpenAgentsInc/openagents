# Non-GPT-OSS Qwen Pilot

> Status: canonical `PSI-257` / `#3562` pilot record, updated 2026-03-14 after
> landing the runnable pilot harness in
> `scripts/release/check-psionic-qwen-pilot.sh`.

This document records the first explicit non-GPT-OSS family pilot through the
generic Psionic server.

The chosen family is `Qwen` using the `qwen2` GGUF architecture metadata path.

## Why Qwen

Qwen is the cleanest first pilot in the current repo because Psionic already
has:

- a real `qwen2` tokenizer fixture in `psionic-models`
- real `qwen2` prompt-template fixtures and conformance coverage
- a real executed CPU GGUF decoder path for the Qwen family
- a generic server path that already routes Qwen through the same multi-family
  serving surface used by non-pilot decoder families

This keeps the pilot honest: it is not a new special-case server and not a
GPT-OSS alias.

## Pilot Artifact

The pilot artifact is a tiny GGUF generated inside the test harness with:

- real Qwen-family `qwen2` architecture metadata
- real Qwen tokenizer-family metadata and prompt-render behavior
- a deterministic tiny decoder tensor set exercised by the Psionic-owned CPU
  GGUF runtime

This is a correctness pilot, not a production-size quality or throughput claim.
The artifact is intentionally small so the pilot is repeatable in CI and on
ordinary developer machines while still traversing the real Psionic loader,
runtime, scheduler, generic server, and evidence surfaces.

## Canonical Runner

Run the pilot from the repo root:

```bash
scripts/release/check-psionic-qwen-pilot.sh
```

## What The Runner Proves

The runner executes three layers of evidence:

1. real prompt-fixture and tokenizer-family evidence
   - `generate_case_builder_uses_real_qwen2_fixture`
2. real executed runtime evidence
   - `cpu_gguf_service_executes_qwen_family`
3. real generic-server end-to-end evidence
   - `generic_server_qwen_pilot_is_end_to_end_machine_checkable`

## Pass Criteria

The pilot is green only if all of the following are true:

- the real Qwen prompt fixture still renders through the shared fixture path
- the Psionic-owned CPU GGUF runtime still executes the Qwen family directly
- the generic server advertises the model as `psionic_model_family = "qwen"`
- the generic server advertises `"/v1/chat/completions"` and `"/v1/responses"`
  for the Qwen pilot model
- the Qwen model card carries an execution profile and scheduler policy
- a Qwen `/v1/chat/completions` request returns the expected deterministic pilot
  output
- the response carries machine-checkable scheduler headers:
  - `x-psionic-batch-posture = continuous_batch`
  - `x-psionic-scheduling-class = mixed_prefill_decode`
  - `x-psionic-prefill-decode-mode = disaggregated_colocated`
- the response JSON carries the scheduler receipt instead of hiding the runtime
  path behind a model-family exception

## Expected Pilot Signals

The current tiny-Qwen pilot should produce these signals:

- health or model inventory reports:
  - `psionic_model_family = "qwen"`
  - `psionic_residency_mode = "cpu_only"`
  - `psionic_supported_endpoints = ["/v1/chat/completions", "/v1/responses"]`
  - `psionic_execution_profile.batch_posture = "continuous_batch"`
- the deterministic pilot request:
  - prompt: `hello`
  - output text: `world`
- scheduler truth:
  - `batch_posture = continuous_batch`
  - `scheduling_class = mixed_prefill_decode`
  - `prefill_decode_mode = disaggregated_colocated`

## Current Limitations

The pilot is intentionally bounded:

- it is a CPU-lane pilot, not a GPU throughput claim
- it does not claim that all non-GPT-OSS families are equally validated
- it does not claim reasoning-parser support for Qwen
- it does not claim tool-loop or structured-agent completion; that is the next
  pilot issue
- it uses a tiny deterministic GGUF fixture to prove end-to-end path ownership,
  not to prove production-scale quality

## Claim Rule

This pilot is sufficient to prove that Psionic is no longer GPT-OSS-only at the
generic-server correctness layer.

It is not sufficient to claim:

- Qwen throughput parity
- full multi-backend Qwen portability
- all-family pilot completion
- structured-agent completion for non-GPT-OSS families

Future non-GPT-OSS pilot work should add separate rows or documents rather than
quietly widening the scope of this Qwen pilot.
