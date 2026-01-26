# Autopilot Desktop Self-Improver Plan (DSPy/DSRS)

Date: 2026-01-26

## Agent note (read before changes)
Before making any implementation related to this plan, review the **Agent Action Log** at the bottom of this document. After you complete relevant work, append a brief entry describing what you did.

## Purpose
Define a concrete, codebase-specific plan to keep Autopilot Desktop improving its DSPy signatures over time, with measured benchmarks and safe promotion of optimized instructions.

This plan adapts the crest self-improver concepts to the current OpenAgents codebase and uses the existing dsrs optimizer + evaluation primitives.

## Sources reviewed
- crest: `docs/autopilot/20260122/0930-autopilot-selfimprover-plan.md`
- crest: `docs/autopilot/directives/selfimprove.md`
- crest: `docs/autopilot/logs/improvement-log.md`
- openagents: `apps/autopilot-desktop/src-tauri/src/agent/adjutant/*` (plan mode pipeline, training, optimizer)
- openagents: `apps/autopilot-desktop/src-tauri/src/full_auto.rs`
- openagents: `crates/dsrs/docs/OPTIMIZERS.md`, `crates/dsrs/docs/EVALUATION.md`, `crates/dsrs/docs/SIGNATURES.md`

## Current state in OpenAgents
### Plan mode (already wired)
- Pipeline: `apps/autopilot-desktop/src-tauri/src/agent/adjutant/planning.rs` (topic decomposition → parallel exploration → synthesis → validation).
- Training data: `plan_mode_training.rs` writes to `~/.openagents/autopilot-desktop/training/plan_mode.json`.
- Optimizer: `plan_mode_optimizer.rs` runs MIPROv2/COPRO/GEPA, logs to `~/.openagents/autopilot-desktop/optimization/plan_mode.jsonl`.
- Metrics: `plan_mode_metrics.rs` provides proxy-style format/length checks per signature.
- Manifests: optimized instructions stored under `~/.openagents/autopilot-desktop/manifests/plan_mode/` and applied in `planning.rs`.
- Config: `PlanModeOptimizationConfig` (min examples, optimizer choice, cadence, etc.).

### Full Auto (partially wired)
- Decision signature: `FullAutoDecisionSignature` used in `apps/autopilot-desktop/src-tauri/src/full_auto.rs`.
- No training store, metrics, optimizer, or benchmark harness yet.

### DSRS capabilities available
- Optimizers: `crates/dsrs/src/optimizer/*` (MIPROv2, COPRO, GEPA).
- Evaluation & promotion gating: `crates/dsrs/src/evaluate/*` (proxy/truth metrics, scorecards, promotion manager).
- Manifests: `crates/dsrs/src/manifest/*` (CompiledModuleManifest + Scorecard).

## Gaps to close
1. **No formal benchmark harness** for plan mode or full auto; optimization only uses lightweight proxy metrics.
2. **No truth metrics** (LLM judge, tests, or structured scoring) and no promotion gates beyond “score >= baseline.”
3. **No data curation** (dedupe, quality filters, “gold” examples) in training store.
4. **No self-improver loop for FullAutoDecisionSignature**.
5. **Limited observability** in UI/CLI for optimization status, score trends, and manifest selection.

## Target outcomes
- A repeatable, benchmarked loop for plan-mode and full-auto signatures.
- A dataset pipeline that captures examples, filters for quality, and keeps a stable evaluation set.
- A promotion system that only adopts optimized instructions when they beat baseline on benchmarks.
- A clear, inspectable audit trail of optimization results and applied manifests.

## Implementation plan (OpenAgents-specific)

### Phase 0: Inventory + guardrails (short)
- Add a short “Self-improver status” doc section or command output that prints:
  - training example counts per signature
  - latest manifest IDs + scorecards
  - last optimization timestamp
- Surface paths used by the system (training, optimization logs, manifests) in one place.

### Phase 1: Benchmark harness for plan mode
**Goal:** Turn plan mode optimization into a measured loop.

1) **Add plan mode benchmark tasks**
- Add `apps/autopilot-desktop/src-tauri/src/agent/adjutant/plan_mode_bench.rs`.
- Build `EvalTaskSet` from `PlanModeTrainingStore` with a reserved “eval split” (e.g., last N examples per signature).
- Use dsrs `ScorerBuilder` with proxy metrics + one truth metric per signature:
  - TopicDecomposition: LLM judge on topic clarity + count.
  - PlanSynthesis: LLM judge for step quality + completeness.
  - ResultValidation: format + classification sanity.

2) **Persist scorecards**
- Write scorecards to `~/.openagents/autopilot-desktop/benchmarks/plan_mode.jsonl`.
- Include baseline vs optimized scores, config snapshot, and manifest IDs.

3) **Promotion gating**
- Replace “>= baseline” with dsrs `PromotionManager` gates (proxy + truth thresholds).
- Keep a “candidate” manifest until it passes gates, then mark as “promoted.”

4) **Config additions**
- Extend `PlanModeOptimizationConfig` with:
  - `eval_split_size`, `min_promotion_delta`, `min_proxy_score`, `min_truth_score`.

### Phase 2: Full Auto signature loop
**Goal:** bring FullAutoDecisionSignature into the same benchmark + optimization pipeline.

1) **Training capture**
- Create `apps/autopilot-desktop/src-tauri/src/full_auto_training.rs`.
- Log `FullAutoDecisionRequest` + model decision + outcome label (continue/pause/stop/review).
- Save to `~/.openagents/autopilot-desktop/training/full_auto.json`.

2) **Metrics & evaluator**
- Add `full_auto_metrics.rs` with proxy metrics:
  - action in allowed set
  - confidence >= min_confidence
  - reason length threshold
- Add truth metric using LLM judge for “sensible action given summary.”

3) **Optimizer & manifests**
- Implement `full_auto_optimizer.rs` using dsrs MIPROv2/COPRO/GEPA like plan mode.
- Store manifests under `~/.openagents/autopilot-desktop/manifests/full_auto/`.
- Apply optimized instructions in `run_full_auto_decision` before predicting.

### Phase 3: UI + diagnostics
**Goal:** Make optimization visible and controllable.

- Add Tauri commands to fetch:
  - training counts
  - last benchmark results
  - active manifest per signature
- Show in the right sidebar (or a diagnostics drawer) alongside existing status.
- Add “Reset to baseline” button that clears manifests for a signature.

## Benchmarking policy
- **Proxy metrics** run on every candidate (format/length/allowed values).
- **Truth metrics** on a fixed eval set (stable snapshot).
- Only promote if:
  - proxy >= threshold
  - truth score >= threshold
  - delta over baseline >= `min_promotion_delta`

## Data management
- Training data: `~/.openagents/autopilot-desktop/training/`
- Optimization logs: `~/.openagents/autopilot-desktop/optimization/`
- Benchmarks: `~/.openagents/autopilot-desktop/benchmarks/`
- Manifests: `~/.openagents/autopilot-desktop/manifests/`

## Acceptance criteria
- Plan mode optimization produces benchmark scorecards and only promotes when gates pass.
- Full auto decisions are trained, benchmarked, and optimized with the same loop.
- UI can show latest scores and active manifest IDs.

## Next concrete steps
1) Implement plan-mode benchmark harness + scorecard logging.
2) Add promotion gates to plan-mode optimizer.
3) Add full-auto training capture + metrics + optimizer.
4) Surface summary status in UI.

## Agent Action Log
- 2026-01-26: Added agent note requiring log review + append-only action entries. (doc update)
