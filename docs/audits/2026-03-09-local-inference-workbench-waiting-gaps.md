# Local Inference Workbench Waiting Gaps

Date: 2026-03-09
Branch: `metal-gptoss`
Scope: `apps/autopilot-desktop` workbench pane only

## What Landed In This Slice

- Added a new singleton `Local Inference` pane in `apps/autopilot-desktop`.
- The pane is app-owned and uses the current `local_inference_runtime` seam instead of reviving the older direct `ollama` UI approach.
- It exposes:
  - runtime refresh / warm / unload controls
  - a prompt playground with model + sampling inputs
  - pane-owned request tracking
  - output preview and runtime metadata visibility

## Waiting Gaps

1. Real GPT-OSS model loading is still constrained by the current runtime seam.

- The pane can only call `Refresh`, `WarmConfiguredModel`, `UnloadConfiguredModel`, and `Generate`.
- There is no pane-wired command yet for:
  - selecting a local model artifact path
  - enumerating on-disk GPT-OSS bundles
  - switching between multiple configured local models
  - choosing a backend implementation at run time

2. The desktop runtime is still backed by the in-process Psionic reference adapter.

- `default_local_inference_runtime()` still resolves to `PsionicRuntimeAdapter::new_reference()`.
- That is enough for the workbench shell, but it is not yet the final Metal-backed GPT-OSS loading path.

3. Provider/runtime internals still use older `ollama_*` names.

- The new user-facing pane is `Local Inference`.
- Internally, desktop runtime snapshots and provider mirrors still flow through fields like `state.ollama_execution` and `provider_runtime.ollama`.
- I intentionally did not rename those internals in this slice to avoid broad churn while other branch work is active.

4. Backend/model-selection UX should wait for the runtime contract to grow first.

- Once the runtime seam can accept explicit model descriptors or load targets, the next pane step should be:
  - model catalog / discovered artifact list
  - selected backend badge
  - active loaded model slot
  - load progress / memory pressure / load failure detail

5. This slice intentionally avoided the other agent’s active Psionic/Metal files.

- The branch already had concurrent edits in `crates/psionic/psionic-backend-metal/src/lib.rs`.
- To avoid collisions, this work stayed in `apps/autopilot-desktop` plus docs only.
- Any follow-up that depends on the other agent’s Metal-serving work should start by rebasing this pane onto their runtime/API shape instead of guessing it now.

## Recommended Next Step

Extend the app-owned `local_inference_runtime` contract before adding more UI:

- add explicit load/select commands for GPT-OSS model targets
- return structured load-progress and backend-capability updates
- then extend this pane from "playground over configured runtime" to "full GPT-OSS local model workbench"
