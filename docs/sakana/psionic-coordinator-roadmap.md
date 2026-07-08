# Psionic Coordinator: Primitives Audit & Roadmap

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


*Analysis — 2026-06-22. Exactly what Psionic is missing to host a Sakana-style
learned coordinator, the primitives to add, and the order to build them.
Symbol/path references verified against the current `psionic/` source on
2026-06-22; symbols marked **NEW** are proposed, everything else exists today.*

Companion to [`adapting-sakana-coordination.md`](adapting-sakana-coordination.md)
and [`coordinator-as-verified-work.md`](coordinator-as-verified-work.md). This
doc lives in the OpenAgents docs set but is entirely about Psionic internals.

## The target

A [TRINITY](trinity.md)-style coordinator: a frozen small LM produces a
hidden-state feature `h`; a tiny (~10K-param) head reads `h` and emits logits
over `(worker, role)`; trained by **separable CMA-ES** against a terminal reward
(verified-work / module-eval pass). The [Conductor](conductor.md) variant
(7B + GRPO emitting NL workflows) reuses primitives #5–#7 below but needs a
GRPO loop instead of ES; this roadmap targets TRINITY first because it's the
cheaper, higher-leverage build and lands on Psionic's existing route-model seam.

## What Psionic already has (reuse)

| Capability | Where (verified) |
|---|---|
| Optimizer config + training core loop | `crates/psionic-train/src/core_loop.rs:137` `TrainingOptimizerKind` (Sgd/Adam/AdamW/Lars/Lamb), `TrainingOptimizerConfig` |
| A *learned router* precedent (naive-Bayes) | `crates/psionic-eval/src/compiled_agent_route_model.rs:74` `train_compiled_agent_route_model()`; decision at `compiled_agent_module_eval.rs:761` `evaluate_compiled_agent_route()` |
| Adapter packaging (LoRA / residual) | `crates/psionic-adapters/src/lib.rs:27` `AdapterArtifactKind`, `:49` `AdapterTargetFamily`, `AdapterArtifactIdentity`, `AdapterPackageManifest` |
| Promoted/candidate + shadow governance | `crates/psionic-train/src/compiled_agent_artifact_contract.rs:68` `CompiledAgentArtifactLifecycleState` (Promoted/Candidate), `:134` `CompiledAgentPromotedArtifactContract`; confidence bands & rollback in `docs/COMPILED_AGENT_SHADOW_GOVERNANCE.md` |
| Module-eval pass/fail receipts | `crates/psionic-eval/src/compiled_agent_module_eval.rs:89` `CompiledAgentModuleEvalReport`; learning receipts `compiled_agent_receipts.rs` `CompiledAgentLearningReceipt` |
| Worker/compute-source registry | `crates/psionic-train/src/signed_node_identity_contract.rs`, `cross_provider_launch_contract.rs`: `admitted_execution_classes: Vec<CrossProviderExecutionClass>` |
| Rollout-driver harness (the loop to extend) | `crates/psionic-train/src/bin/compiled_agent_xtrain_loop.rs`; rollout coordinator `probe_gepa_rollout_coordinator.rs` |

The verification/governance half of the system is **strong and reusable**. The
learning half is gradient-only and text/logit-only. Every gap below is on the
learning side.

## The five missing primitives

### P1 — Hidden-state extraction from inference  *(blocking; foundational)*

**Gap (MISSING).** Model forward paths return logits/text only
(`crates/psionic-models/src/...:forward` / `forward_logits`). There is no API to
read an intermediate or penultimate-token hidden vector. (The `hidden_state_*`
hits in `psionic-provider`/`psionic-sandbox` are governance booleans, not
activation extraction.) TRINITY's entire thesis is that `h` carries the routing
signal — without it there is no coordinator feature.

**Primitive to add.** A forward variant that returns the penultimate-token
hidden state alongside (or instead of) logits:
- **NEW** `fn forward_with_hidden(&self, input) -> (NnTensor /*logits*/, NnTensor /*h: [batch, hidden_dim]*/)` on the chosen backbone (e.g. a Qwen3-0.6B stack in `psionic-models`).
- The paper only needs *one* token's hidden state and discards generated text, so this can short-circuit after the prompt forward pass — cheap, no full decode.
- Expose it through the inference surface as an opt-in "feature extraction" mode so callers that don't need `h` pay nothing.

**Effort:** medium. Localized to the model forward + a thin inference flag.
**De-risk:** validate on a frozen Qwen3-0.6B that `h` is reproducible
(deterministic given input) — required for stable ES fitness.

### P2 — Coordinator head + frozen-backbone pattern  *(blocking)*

**Gap (MISSING).** Adapters are LoRA/residual overlays
(`AdapterArtifactKind`); there is no "frozen backbone + small task head" type,
and no head that maps `h -> (L workers + 3 roles)` logits.

**Primitives to add.**
- **NEW** `AdapterArtifactKind::CoordinatorHead` and **NEW**
  `AdapterTargetFamily::CoordinatorRouter` in `psionic-adapters/src/lib.rs`, so a
  coordinator ships as a first-class, digest-pinned adapter artifact (reusing
  the existing identity/manifest/digest machinery).
- **NEW** head module: linear `W: [hidden_dim, L+3]` (≈10K params for
  `hidden_dim≈1024`, `L≈7`), softmax over the `L` worker logits and over the `3`
  role logits separately. Keep the linear head as the default (TRINITY's best);
  leave block-diagonal/low-rank as alternate configs.
- **Optional NEW** SVF (singular-value fine-tuning): learn only singular-value
  *scales* of one backbone layer. Psionic tracks LoRA rank but has **no SVF**;
  it added ~2.6 avg points for TRINITY and keeps total learnable params <20K.
  Ship as a second `AdapterArtifactKind` variant or a flag on CoordinatorHead.

**Effort:** small (the head) + small/medium (SVF). The artifact plumbing is reuse.

### P3 — Separable CMA-ES optimizer  *(blocking)*

**Gap (MISSING).** Confirmed zero references to CMA-ES, evolution strategies,
population methods, or random search anywhere in `crates`. The core loop is
deterministic gradient steps; `TrainingOptimizerKind` has no gradient-free
member. TRINITY's central empirical claim is that under our exact regime —
**tiny head, expensive per-eval, binary reward, weak parameter coupling** —
sep-CMA-ES beats RL/SFT/RS; we can't test that without the optimizer.

**Primitive to add.**
- **NEW** `crates/psionic-train/src/evolution_trainer.rs` implementing sep-CMA-ES
  (diagonal covariance): sample a population of perturbed parameter vectors,
  evaluate each via the fitness hook (P4), recombine by fitness-weighted mean,
  update the diagonal step sizes. ~200–300 lines; reference is the paper's
  Appendix A + the ML reference clones under `projects/repos/`.
- **NEW** `TrainingOptimizerKind::SepCmaEs` (+ config: population size `λ`,
  replication count `m`, per-generation eval budget) so it slots into the
  existing optimizer-config surface rather than a parallel path.
- Include a **random-search baseline** mode in the same file — the paper's
  control, and a cheap sanity gate before trusting ES.

**Effort:** medium. Self-contained; no autograd dependency (gradient-free).

### P4 — Scalar terminal-reward adapter + atomic-evaluation harness  *(blocking)*

**Gap (MISSING scalar reward).** Promotion today is *categorical* —
`CompiledAgentModuleValidation::stronger_than()` returns Promote/Hold by
comparing pass counts. ES/RL need a **scalar** `R(τ) ∈ {0,1}` (or `[0,1]`) per
full trajectory, and a function that *runs one end-to-end coordinated trajectory
and returns that reward* (the paper's "atomic evaluation / Bernoulli call").

**Primitives to add.**
- **NEW** reward adapter: derive a scalar from existing receipts — module-eval
  pass (`CompiledAgentModuleEvalReport`) for the offline lane, and the Tassadar
  verification-class verdict for the live lane (the `training.verification_classes.v1`
  registry: `exact_trace_replay` at sample rate 1.0 for deterministic/kernel
  work, `seeded_replication`/`statistical_cross_check` for stochastic LLM work;
  see [`tassadar-run-integration.md`](tassadar-run-integration.md) and the
  verified-work doc). For cost-aware training, return `verified ? 1.0 : 0.0` and
  separately log spend so fitness can be `reward − λ·cost`.
- **NEW** atomic-evaluation function: `fn evaluate_coordinator(params) -> f32`
  that (1) loads the head with `params`, (2) runs the multi-turn select→role→
  dispatch→verify loop over a sampled batch, (3) returns mean reward. This is the
  fitness hook P3 calls. Wire it into the existing rollout driver
  (`probe_gepa_rollout_coordinator.rs`) rather than building a new executor.
- **Budget accounting as a first-class config** — every eval may run real
  workers (and, on the live lane, move sats), so cap eval spend per generation
  and emit it as a receipt. The paper operates at 1.5k–40k evals for a ~10K-dim
  problem; our per-eval cost is higher, so the budget-tight regime where ES wins
  is exactly ours — but only if we meter it.

**Effort:** medium. Mostly glue over existing eval + rollout + verdict surfaces.

### P5 — Worker-pool binding for the action space  *(blocking but small)*

**Gap (partial).** The compute-source contract enumerates *execution classes*
(`dense_full_model_rank`, `validator`, `eval_worker`, …) and external-agent roles
(`replay_generation`, `ranking_labeling`, `validator_scoring`,
`bounded_module_training`), but there is no notion of "a pool of `L`
interchangeable answer-producing worker LLMs" that the coordinator's `L` logits
index, nor frontier-LLM endpoints as first-class pool members.

**Primitive to add.**
- **NEW** typed worker-pool view: a stable, ordered list of `L` eligible workers
  (open + frontier endpoints) the head's worker logits map onto, derived from the
  compute-source registry and **filtered by the receipted capability envelope**
  before the coordinator ever sees it. The coordinator selects *within* the
  capability-eligible set; it never overrides the receipt gate.
- Map TRINITY's three roles onto Psionic's reality: **Worker** → an answer/
  trace-producing endpoint; **Thinker** → a planner subtask; **Verifier** →
  bind to the replay validator / `validator_scoring`, **not** a prompted LLM
  (this is the key simplification from the verified-work analysis — the
  ACCEPT decision leaves the policy and becomes the digest verdict).

**Effort:** small. A typed projection over an existing registry + capability gate.

## What ships it: the candidate-artifact path (no new governance)

The coordinator rides Psionic's existing promotion machinery unchanged:
1. Train the head (P2) with sep-CMA-ES (P3) against the reward harness (P4)
   over the worker pool (P5), using hidden states (P1).
2. Emit it as a **Candidate** entry in `CompiledAgentPromotedArtifactContract`
   (`compiled_agent_artifact_contract.rs`), `candidate_label =
   "coordinator_sep_cmaes_v1"`, alongside the current promoted route model.
3. Shadow-run it against the baseline router; compare on verified-work-per-sat
   under the confidence bands in `COMPILED_AGENT_SHADOW_GOVERNANCE.md`
   (high ≥ 0.80 / watch ≥ 0.60 / review < 0.60).
4. Promote only on a clean win; keep the heuristic/NB router as the
   `rollback_artifact_id`. Any held-out regression ≥ 1 trips rollback.

No new authority surface is introduced — the coordinator is just another
artifact in the contract.

## Roadmap (build order)

**Phase 0 — Reward + harness (P4, partial).** Add the scalar reward adapter and
the `evaluate_coordinator` atomic-eval over the *offline* module-eval lane only
(no live workers yet). Cheapest possible loop; proves the fitness signal is
sane and reproducible.

**Phase 1 — Optimizer (P3).** Land `evolution_trainer.rs` with sep-CMA-ES +
random-search baseline, driven by the Phase-0 harness against a *trivial* head
(even a hand-built feature) so the optimizer is validated before P1/P2 exist.

**Phase 2 — Feature + head (P1, P2).** Add `forward_with_hidden`, the
CoordinatorHead adapter type, and the linear head. Now the real coordinator
trains end-to-end offline. Add SVF here if Phase-2 plateaus.

**Phase 3 — Pool binding + live reward (P5, P4 live lane).** Bind the worker
pool with capability filtering; switch the reward to the Tassadar verdict /
verification-command on a small, budgeted live batch. Verifier role = replay
validator.

**Phase 4 — Shadow ship.** Emit as a Candidate artifact, shadow vs the NB route
model, gate on verified-work-per-sat, promote on a clean win.

**Phase 5 (later) — Conductor lane.** Reuse P4/P5/governance; swap P1–P3 for a
GRPO loop over a 7B base (closest existing substrate:
`AGENTIC_SFT_RL_REFERENCE_PROGRAM` + the rollout-worker/validator-verdict loop)
emitting the NL workflow schema. Bigger, do it only after TRINITY proves out.

## Build/run anchor

Extend the existing harness rather than forking one:
`cargo run -q -p psionic-train --bin compiled_agent_xtrain_loop` is the loop to
add the coordinator module + ES optimizer into.

## Critical-path summary

```
P1 hidden-state  ─┐
P2 head + SVF    ─┼─> P4 reward/atomic-eval ─> P3 sep-CMA-ES ─> shadow candidate ─> promote
P5 pool binding  ─┘        (P0 offline first, P3 live second)
```

Five new primitives, all on the learning side; the entire verify/pay/govern half
is reuse. P1 (hidden-state extraction) and P3 (the ES optimizer) are the two
genuinely new pieces of capability — everything else is glue or a new enum
variant over machinery Psionic already runs.
