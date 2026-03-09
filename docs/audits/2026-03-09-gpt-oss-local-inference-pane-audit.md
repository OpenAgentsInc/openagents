# 2026-03-09 GPT-OSS Local Inference Pane Audit

> Historical note: this audit is a point-in-time snapshot from 2026-03-09 on branch `metal-gptoss`. Current product and architecture authority lives in `docs/MVP.md`, `docs/OWNERSHIP.md`, and `crates/psionic/docs/ROADMAP.md`.

## Scope

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/PANES.md`
- `crates/psionic/docs/ROADMAP.md`
- `crates/psionic/docs/METAL_GPT_OSS_UNIFIED_PLAN.md`
- `crates/psionic/docs/HARDWARE_VALIDATION_MATRIX.md`
- `docs/audits/2026-03-09-psionic-gpt-oss-metal-gap-audit.md`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/pane_registry.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/pane_system.rs`
- `apps/autopilot-desktop/src/local_inference_runtime.rs`
- `apps/autopilot-desktop/src/input/reducers/jobs.rs`
- `apps/autopilot-desktop/src/state/provider_runtime.rs`
- `apps/autopilot-desktop/src/kernel_control.rs`
- `crates/openagents-provider-substrate/src/lib.rs`
- `crates/psionic/psionic-serve/src/lib.rs`
- `crates/psionic/psionic-serve/src/gpt_oss.rs`
- `crates/psionic/psionic-serve/src/openai_http.rs`
- `crates/psionic/psionic-serve/src/bin/psionic-gpt-oss-server.rs`
- GitHub issues reviewed on 2026-03-09:
  - closed: `#3217`, `#3218`, `#3219`, `#3250` through `#3260`
  - open: `#3249`, `#3247`, `#3248`, `#3261`, `#3262`

## Executive Summary

Psionic is now far enough along that a desktop GPT-OSS local-inference pane is worth starting immediately. The March 9, 2026 Metal issue sequence closed the core engine/bootstrap work: the shared GPT-OSS runtime seam exists, `MetalGgufGptOssTextGenerationService` exists, the OpenAI-compatible GPT-OSS server can run with `--backend metal`, and the hardware validation matrix now has an explicit `metal.gpt_oss.text_generation.apple_silicon` row.

The blocker has moved from "there is no real Metal GPT-OSS lane" to "the desktop does not expose it honestly." `apps/autopilot-desktop` still has no local-inference pane, still stores the local Psionic runtime snapshot under `ollama_execution`, still copies that into `provider_runtime.ollama`, and still maps launch inventory and metering to `ollama.*` product IDs. The app seam exists, but it is too thin and too stale to support a truthful GPT-OSS workbench.

The right path is:

1. clean up the app-owned local-inference seam and naming debt,
2. add a singleton `Local Inference` pane in `apps/autopilot-desktop`,
3. expand the app seam to surface catalog, load, unload, observability, and workbench execution state, and
4. then swap the desktop from the current CPU reference adapter to a real model-backed Psionic GPT-OSS adapter without routing through loopback HTTP as the primary product path.

## Current GitHub State

### What closed on March 8, 2026

- `#3217` / `OA-201`: app-owned `LocalInferenceRuntime` seam landed.
- `#3218` / `OA-202`: desktop default switched to in-process Psionic.
- `#3219` / `OA-203`: external Ollama dependency stopped being the default product path and user-facing wording moved toward Psionic/local inference.

Those issues matter because the pane should be built on that app seam, not by adding a new direct dependency from pane code into random Psionic internals.

### What closed on March 9, 2026

The Metal GPT-OSS foundation sequence is now mostly closed:

- `#3250`: split dense Metal text-generation truth from GPT-OSS readiness
- `#3251`: move GPT-OSS graph/runtime contract out of the CUDA-specific serve path
- `#3252`: backend-native quantized tensor storage and upload
- `#3253`: Metal RMSNorm, RoPE, argmax, and top-k primitives
- `#3254`: grouped expert dispatch
- `#3255`: device-resident KV cache and shared-prefix residency
- `#3256`: Metal decode attention with a flash-attention path
- `#3257`: backend-side greedy sampling and bounded output readback
- `#3258`: graph reserve, graph reuse, and command-buffer reuse
- `#3259`: allocator, buffer-pool, and kernel-cache policy
- `#3260`: ship `MetalGgufGptOssTextGenerationService` through the shared runtime

That means the earlier same-day audit in `docs/audits/2026-03-09-psionic-gpt-oss-metal-gap-audit.md` is now partially stale. It still contains claims that were true earlier on March 9, 2026 but are no longer true by the end of the current issue sequence.

### What is still open on March 9, 2026

- `#3261`: CPU-vs-Metal GPT-OSS parity, validation, and benchmark evidence
- `#3262`: same-host `llama.cpp`-class GPT-OSS throughput on Apple Silicon
- `#3249`: mirror `llama.cpp` GPT-OSS graph and CUDA fusion architecture
- `#3247`: port `llama.cpp` GPT-OSS CUDA kernels and dispatch policy
- `#3248`: reach `llama.cpp`-class throughput on the real Psionic HTTP path

These open issues do not block starting the pane. They do block presenting Metal GPT-OSS as fully validated or performance-closed.

## What Is Actually Landed In Code

### 1. The Psionic GPT-OSS Metal path is real enough for UI work

Current code now includes:

- `MetalGgufGptOssTextGenerationService` in `crates/psionic/psionic-serve/src/gpt_oss.rs`
- `ManagedTextGenerationRuntime` support for that service:
  - `loaded_models`
  - `observability`
  - `warm_model`
  - `unload_model`
  - `generate`
  - `generate_stream`
- `GptOssOpenAiCompatServer` in `crates/psionic/psionic-serve/src/openai_http.rs` with `metal` backend support
- `psionic-gpt-oss-server` CLI in `crates/psionic/psionic-serve/src/bin/psionic-gpt-oss-server.rs` with `--backend metal`
- automatic `Auto -> Metal` backend resolution on macOS in the OpenAI-compatible server config

So "Metal GPT-OSS exists only on paper" is no longer accurate.

### 2. The validation story moved forward

`crates/psionic/docs/HARDWARE_VALIDATION_MATRIX.md` now includes:

- `metal.gpt_oss.text_generation.apple_silicon`

That is a meaningful change. The lane still has open evidence and throughput work, but the repo no longer treats Metal GPT-OSS as an unnamed future placeholder.

### 3. Psionic already has the reusable runtime concepts a pane wants

`crates/psionic/psionic-serve/src/lib.rs` already exposes the reusable local-serving concepts a workbench should surface:

- managed runtime lifecycle
- loaded-model views
- model residency policy and residency snapshot
- generation provenance
- cache observations
- GPT-OSS performance metrics
- aggregate local runtime surface from `PSI-125`:
  - `list_models`
  - `show_model`
  - `loaded_models`
  - `observability`
  - `warm_model`
  - `unload_model`
  - `generate`
  - `generate_stream`
  - `embed`

This is the right substrate. The desktop just is not exposing it yet.

## Desktop Reality Today

### 1. There is no local-inference pane

`apps/autopilot-desktop` currently has no pane kind or registry entry for local inference:

- no `PaneKind::LocalInference`
- no `PaneSpec` entry in `pane_registry.rs`
- no corresponding entry in `docs/PANES.md`

The only visible local-inference UI is a small status summary embedded inside `Mission Control` / `Provider Status`.

### 2. The app seam exists, but only as a narrow job-execution slot

`apps/autopilot-desktop/src/local_inference_runtime.rs` currently gives the app:

- `Refresh`
- `WarmConfiguredModel`
- `UnloadConfiguredModel`
- `Generate`

That is enough for the provider loop, but it is not enough for a workbench. Missing app-level operations include:

- list installed models
- inspect one model
- load a model from a selected GGUF path
- select backend preference (`cpu`, `metal`, `cuda`, `auto`)
- read explicit runtime observability
- stream generation events
- expose model residency and memory state
- surface GPT-OSS-specific perf counters

### 3. The default desktop adapter is still the CPU reference service

`default_local_inference_runtime()` still constructs `PsionicRuntimeAdapter::new_reference()`, which wraps `CpuReferenceTextGenerationService`.

That means:

- the desktop default runtime is still not a real GPT-OSS model-backed service
- there is no UI-facing model path selection
- there is no backend selection in the app seam
- there is no current app path to instantiate `MetalGgufGptOssTextGenerationService`

This is the single biggest functional gap for the proposed pane.

### 4. Desktop state still carries stale `ollama` names everywhere

The biggest code-health problem is not missing buttons. It is stale product identity:

- `RenderState` stores the local runtime snapshot as `ollama_execution`
- reducers copy local runtime updates into `provider_runtime.ollama`
- `ProviderRuntimeState` still owns `ProviderOllamaRuntimeState`
- `kernel_control.rs` still maps local inference to:
  - `ollama.text_generation`
  - `ollama.embeddings`
  - `meter.ollama.inference.v1`
  - `meter.ollama.embeddings.v1`
- `openagents-provider-substrate` still uses:
  - `ProviderBackendKind::Ollama`
  - `ProviderComputeProduct::OllamaInference`
  - `ProviderBlocker::OllamaUnavailable`

This is more than cosmetic. A GPT-OSS workbench would be visibly dishonest if it loads a Psionic GGUF model through a pane while the same app still describes the backend/product lineage as `ollama.*`.

### 5. The desktop does not yet use the richer Psionic local-runtime surface

Even though `psionic-serve` exposes a broader local runtime API, the desktop adapter does not consume it. The current app-owned adapter is effectively "single configured model + synchronous generate + warm/unload."

So the pane cannot simply be "render what Psionic already knows." The seam must grow first.

### 6. The existing same-day Metal audit is partially obsolete

`docs/audits/2026-03-09-psionic-gpt-oss-metal-gap-audit.md` still says, among other things:

- the GPT-OSS server surface is CUDA-only
- the Metal GPT-OSS lane is not yet a real service
- the validation row does not yet exist

Those claims no longer match the current branch state after `#3260` and the later March 9, 2026 changes. That older audit is still useful as architectural reasoning, but not as current implementation truth.

## What The New Pane Should Actually Be

The right product shape is not "one more provider debug box." It should be a singleton `Local Inference` pane with a GPT-OSS workbench inside it.

### Minimum useful sections

- Runtime
  - requested backend
  - effective backend
  - readiness / refusal state
  - validation claim id and coverage
  - last action / last error
- Model
  - selected GGUF path
  - model id
  - family / architecture
  - quantization
  - max context
  - revision / digest if available
- Residency
  - loaded models
  - keepalive state
  - resident host/device bytes
  - memory budget and admission result
- Playground
  - prompt editor
  - generation parameters
  - run / stream / cancel
  - output text
- Evidence
  - warm vs cold load state
  - prompt tokens / output tokens
  - prompt eval / decode timings
  - prefix-cache reuse
  - execution plan digest
  - cache observations
  - GPT-OSS perf counters when present

### What it should not be

- not a thin wrapper around loopback HTTP just because the OpenAI-compatible server exists
- not a WGPUI-owned product workflow
- not a provider-only surface that disappears when offline
- not a place that silently overclaims Metal GPT-OSS readiness while `#3261` and `#3262` are still open

## Ownership-Correct Path To Get There

### Phase 1: fix the app seam and naming debt first

This is the first required slice before any real pane work.

In `apps/autopilot-desktop`:

- rename the app-owned snapshot/state fields away from `ollama_*`
- add app-owned `LocalInferencePaneState` and `LocalInferencePaneInputs`
- keep the pane orchestration entirely in the app layer

In `crates/openagents-provider-substrate`:

- decide whether to:
  - do a real `Ollama -> LocalInference/Psionic` rename now, or
  - add explicit alias/deprecation posture and keep the old IDs temporarily only for provider-lane compatibility

My recommendation: do the rename or alias work before shipping the pane. The alternative is a pane that visibly says "Psionic GPT-OSS" while the rest of the product still publishes `ollama.text_generation`.

### Phase 2: expand the app-owned local runtime contract

Do not expose raw Psionic internals directly to the pane. Keep the app seam app-owned, but make it large enough for a workbench.

The app seam should gain app-owned DTOs and commands for at least:

- `RefreshRuntime`
- `ListModels`
- `InspectModel`
- `LoadModel { path, backend, ctx, gpu_layers, reasoning_budget }`
- `WarmModel { model_id, keep_alive_ms }`
- `UnloadModel { model_id }`
- `Generate`
- `GenerateStream`
- `ReadObservability`

The DTOs can be derived from Psionic types, but the seam itself should stay owned by `apps/autopilot-desktop`.

### Phase 3: add the pane skeleton in the desktop app

This is pure app work and should stay out of reusable crates.

Expected future edit surface:

- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/pane_registry.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/pane_system.rs`
- `apps/autopilot-desktop/src/input.rs`
- `docs/PANES.md`

The pane should follow the existing singleton-pane pattern used by the Codex panes.

### Phase 4: swap the desktop adapter from reference-only to model-backed GPT-OSS

This is where the pane becomes genuinely useful.

The current adapter should evolve from:

- `CpuReferenceTextGenerationService`

to a selectable model-backed runtime that can instantiate:

- `MetalGgufGptOssTextGenerationService` on Apple Silicon
- `CudaGgufGptOssTextGenerationService` on NVIDIA hosts
- CPU GPT-OSS or refusal path where appropriate

This should call Psionic directly in-process. The HTTP server may remain useful for parity testing, but it should not be the primary desktop product path.

### Phase 5: reconnect provider truth after the pane is honest

Only after the pane can truthfully load and run GPT-OSS should we reconnect provider inventory and launch-product truth around it.

That follow-on includes:

- launch-product identity cleanup away from `ollama.*`
- metering-rule cleanup
- inventory/product derivation updates
- provider-status pane cleanup

That work crosses into `openagents-provider-substrate`, so it should be treated as a separate ownership-respecting slice, not hidden inside the first pane PR.

## Recommended Implementation Order

1. App rename and state cleanup:
   - remove `ollama_execution` / `provider_runtime.ollama` naming from app-owned state
   - add explicit "legacy provider product ids still mapped to ollama" note only if substrate rename is deferred
2. Runtime seam expansion:
   - add catalog/load/observability/streaming commands and DTOs
3. Pane shell:
   - add `PaneKind::LocalInference`
   - render runtime/model/residency/playground/evidence sections
4. Model-backed adapter:
   - instantiate real GPT-OSS runtime services
   - support backend selection and refusal reporting
5. Provider/product identity cleanup:
   - stop mapping Psionic local inference to `ollama.*`

## Recommendation

Start the pane work now, but do not start with rendering. Start with the seam and naming cleanup that makes the pane worth trusting.

The engine side is ready enough. The desktop side is not. The correct first implementation target is a truthful app-owned `Local Inference` workbench that can eventually front the real Metal GPT-OSS path, not another provider-status subsection that happens to mention a model name.
