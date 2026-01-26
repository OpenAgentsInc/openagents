# DSPy in OpenAgents: Why It Matters and How We Use It

- Status: Draft
- Last verified: (see commit)
- Source of truth: code + root docs (GLOSSARY, SYNTHESIS_EXECUTION, ROADMAP)
- If docs conflict with code, code wins. If terms conflict, GLOSSARY wins.

## Purpose

Explain why DSPy (implemented as `dsrs`) is central to OpenAgents, where it is
wired today, and how we plan to use MIPROv2/GEPA to improve agent behavior
without retraining base models.

## Why we care (OpenAgents framing)

- **Compiler layer for behavior:** DSPy lets us express agent decisions as typed
  signatures and modules. This decouples policy from execution.
- **Outcome-coupled learning:** We optimize using real session outcomes
  (`step_utility`, `verification_delta`, cost, repetition), not prompt vibes.
- **Auditability + rollback:** Compiled modules produce manifests and policy
  bundles that can be versioned, shadow-tested, and rolled back.
- **Portability across lanes:** Signatures + adapters make decisions independent
  of the LM provider or lane (local, cloud, swarm).
- **UI visibility:** Signature metadata supports deterministic UI rendering and
  replay, not opaque chat logs.

## How DSPy maps to OpenAgents systems

- **dsrs (compiler layer):** Signatures, predictors, optimizers, metrics,
  tracing, compiled manifests. See `crates/dsrs/docs/README.md`.
- **Adjutant (execution engine):** DSPy decision pipelines + session tracking
  + auto-optimization hooks. See `crates/adjutant/docs/DSPY-INTEGRATION.md`.
- **Autopilot core:** DSPy-driven planning/execution/verification flow. See
  `crates/autopilot-core/docs/EXECUTION_FLOW.md`.
- **Runtime:** Enforces tool schemas, retries, receipts; DSPy should never own
  validation or retries (ADR-0007). See `crates/dsrs/docs/TOOLS.md`.
- **RLM/FRLM:** Recursive inference modes are routed to by DSPy decisions and
  can be optimized like other modules.
- **UI/Effuse:** Signature-driven UI generation plan is described in
  `docs/dsrs-effuse-ui-plan.md`.

## Where it is wired today (high-signal touchpoints)

- **Decision pipelines in Adjutant:** Complexity, delegation, and RLM trigger
  pipelines are DSPy-first with confidence gating and legacy fallback.
- **Autopilot planning/execution:** Current v1 chain uses Planning/Execution
  signatures, plus verification signatures for build/test/requirements.
- **Training data collection:** Successful DSPy calls are recorded to
  `~/.openagents/adjutant/training/dataset.json` for optimization.
- **Artifacts + replay targets:** Verified Patch Bundle (`PR_SUMMARY.md`,
  `RECEIPT.json`, `REPLAY.jsonl`) is the target output for sessions.

## How we use MIPROv2 and GEPA (practical plan)

1. **Define the decision surface**
   - Every decision that gates action is a signature with structured outputs and
     confidence.
   - Keep prompts out of orchestration code; signatures and modules own them.

2. **Collect training data from real runs**
   - Sessions record decisions, tool calls, outcomes, and verification results.
   - Target labels include `step_utility`, `verification_delta`, repetition, and
     cost; some are spec-only until ToolResultSignature is wired.

3. **Compile with MIPROv2 by default**
   - Use MIPROv2 for baseline improvements to instructions and demos.
   - Optimize worst-performing signatures first (rolling accuracy).

4. **Use GEPA for multi-objective tradeoffs**
   - Apply GEPA where trace-level feedback or Pareto tradeoffs matter (quality
     vs cost/latency/tool calls).

5. **Promote via policy bundles**
   - Compiled artifacts should carry manifests and `policy_bundle_id` for
     auditability; emission is partial until bundle wiring lands.
   - Use shadow/canary evaluation before promoting to production paths.

## Known gaps (from ROADMAP + EXECUTION_FLOW)

- **ToolCallSignature/ToolResultSignature not wired:** execution still uses
  redundant per-step calls; step-level learning signals are limited.
- **PlanIR is split:** Adjutant and Autopilot emit different plan formats, which
  fragments training data.
- **REPLAY.jsonl emission is a target:** ReplayBundle exists, exporter pending.
- **Policy pin/rollback is not shipped:** bundles exist as a concept but need
  CLI wiring and UX.
- **Tool schema validation in runtime:** required for deterministic enforcement.

## Archived references (backroom)

Archived code in backroom shows earlier DSPy experiments:

- A PHP DSPy router and signature-based tool flow (pre-dsrs era).
- OANIX DSPy signatures and pipelines for situation assessment and issue
  selection (early integration patterns).

These are historical references only and are not wired into the current runtime.

## Related docs

- `GLOSSARY.md`
- `SYNTHESIS_EXECUTION.md`
- `ROADMAP.md`
- `crates/dsrs/docs/README.md`
- `crates/dsrs/docs/DSPY_ROADMAP.md`
- `crates/dsrs/docs/CODING_AGENT_LOOP.md`
- `docs/dsrs-effuse-ui-plan.md`
