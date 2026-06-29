# Adapting Sakana's Coordinators to OpenAgents

*Analysis — 2026-06-22. How to recreate/adapt TRINITY and the Conductor
([summaries](README.md)) inside our Tassadar + Pylon + Psionic stack. Grounded
in `docs/tassadar/`, `docs/launch/`, and the sibling `psionic/docs/`.*

## TL;DR

We already have most of what Sakana had to invent:
- a **pool of black-box workers** (the Gemini/Vertex/Fireworks inference fleet
  and the Pylon contributor fleet),
- a **dispatch loop** (Artanis admin-tick + the trace-contribution routes),
- and — the part Sakana *lacks* — a **cryptographic terminal reward** in
  Tassadar's exact-trace-replay (digest match, not a benchmark grader).

What we don't have is the **learned coordinator** in the middle. Today routing
is a hand-written `cheapest-viable` heuristic (claude→Vertex, open→Fireworks,
overflow→passthrough — `JUNE19_ROADMAP.md`, inference gateway EPIC #5474) and a
static naive-Bayes route model in Psionic (`COMPILED_AGENT_ROUTE_MODEL.md`).
Both Sakana papers are recipes for replacing that heuristic with a *learned*
one. This doc maps each onto a concrete seam.

## The two designs, restated for our stack

| | TRINITY here | Conductor here |
|---|---|---|
| Sits at | model/worker **selection** in the dispatch tick | **workflow planning** above the dispatch tick |
| Emits | logits → (pick worker, pick role) | natural-language workflow (subtasks + worker ids + access-list) |
| Trained by | evolution (sep-CMA-ES) on terminal reward | RL (GRPO) on terminal reward |
| Needs from us | hidden-state feature + a tiny head + an ES trainer (Psionic has neither yet) | a 7B base + GRPO (Psionic's `AGENTIC_SFT_RL_REFERENCE_PROGRAM.md` is close) |
| Best first target | trace/inference router with a measurable verified-work reward | Blueprint-chat / coding-agent workflow planner |

## What maps cleanly (reuse, don't rebuild)

**1. The worker pool already exists, multi-provider and ToS-clean.**
`docs/launch/2026-06-20-cloud-agent-fleet-audit.md` and the `vertex-fleet/`
bindings describe a metered, ADC-authed Vertex path serving Claude *and* Gemini
in one GCP project, plus Fireworks for open models. That is exactly Sakana's
"pool of open + closed models from diverse providers" — and our metering means
each coordinator decision has a real, attributable **cost** to feed into the
reward (Sakana only adjusted token budgets; we have per-call sats/credits).

**2. The terminal reward is stronger than Sakana's.**
Sakana's reward is benchmark correctness (`{0,1}` from MATH/LCB graders).
Ours is a **typed verification-class registry** (`training.verification_classes.v1`,
per-contribution sampling owner-approved 2026-06-20 —
`docs/promises/2026-06-20-verification-class-sampling-policy.md`): the reward
class is chosen by work type. The strongest is **`exact_trace_replay`** (sample
rate 1.0) — a worker submits a trace-commitment digest, an *independent device*
replays and compares byte-for-byte, verdict is `Verified`/`Rejected`
(`docs/tassadar/work-that-proves-itself.md`,
`2026-06-15-executor-trace-contributor-completion-design.md`). For executor /
kernel-parity work this is a non-gameable, dense binary reward — ideal for both
sep-CMA-ES (which *wants* clean Bernoulli rewards) and GRPO. For stochastic LLM
work the registry already defines `seeded_replication` (rollouts) and
`statistical_cross_check` (benchmarks); for open-ended coding work the analog is
a job's **verification command** that must pass
(`2026-06-11-autopilot-agentic-labor-market.md`, NIP-LBR kind-5934). The
coordinator's reward = "did the assembled trajectory produce a `Verified`
verdict under its verification class?" See
[`tassadar-run-integration.md`](tassadar-run-integration.md) for the
class-by-work-type mapping and the dense-vs-sparse / nondeterminism caveats.

**3. Dispatch + settlement rails exist.**
Artanis emits assignments and records decisions
(`2026-06-20-cloud-agent-fleet-audit.md`); the trace-contribution routes carry
submit→replay-verdict→auto-stream→settle
(`2026-06-19-autostream-settlement-visibility-capture.md`). A coordinator
doesn't need new plumbing — it needs to *author the assignment* the tick emits.

## Seam A — TRINITY-style logit router (recommended first)

**Where:** the model/worker-selection step in the dispatch tick. For inference,
the gateway router; for executor work, the assignment emission in the
trace-contribution routes; in Psionic, the static `route_model`
(`COMPILED_AGENT_ROUTE_MODEL.md`) it would replace.

**Coordinator:** keep it tiny, per the paper's whole thesis. A small LM (we have
Psion checkpoints; even a frozen Qwen3-0.6B) produces a hidden state over
`(task description + current transcript + candidate-worker metadata +
live cost/latency)`; a ~10K-param linear head emits `L+3` logits → pick one
worker from the live fleet, assign a role.

**Roles, mapped to what we already run:**
- **Thinker** → planner subtask (Blueprint signature step; or Gemini/Claude as
  high-level planner — exactly the LCB strategy Conductor discovered).
- **Worker** → the executor/coding agent that produces the artifact or trace.
- **Verifier** → the **independent replay validator** we already have. This is
  the cleanest fit in the whole exercise: Sakana's "Verifier ACCEPT halts" *is*
  our `exact_trace_replay` verdict. We don't prompt an LLM to self-judge — we
  let the digest decide. Verifier-ACCEPT = `Verified` verdict = settle.

**Training:** this is the open gap. Psionic documents SFT→RL only; **no
CMA-ES, no SVF** (`AGENTIC_SFT_RL_REFERENCE_PROGRAM.md`, `TRAIN_SYSTEM.md`,
confirmed in the psionic doc sweep). Two options:
1. Implement sep-CMA-ES as a thin trainer (it's ~200 lines; the reference is
   `projects/repos/` ML refs and the paper's Appendix A). Reward = verified-work
   rate / cost over a batch of replayed jobs. The paper's pitch is precisely
   that ES beats RL/SFT under our exact conditions: **tiny head, expensive
   per-eval (each eval runs real workers + real replay), binary reward**. Our
   per-eval cost is even higher than theirs (real Bitcoin settles), so the
   budget-tight regime where ES dominates is *our* regime.
2. Bootstrap with the cheaper SFT path first: Psionic's module-eval scores and
   shadow-disagreement receipts (`COMPILED_AGENT_MODULE_EVALS.md`,
   `COMPILED_AGENT_SHADOW_GOVERNANCE.md`) are a ready-made label/reward surface
   for a single-step "best worker for this task" classifier — Sakana's own SFT
   baseline. Use it to de-risk, then move to ES for the multi-turn policy (their
   Table 4 / Appendix A.2 shows SFT can't scale to multi-turn because labels
   blow up combinatorially — same will be true for us).

**SVF instead of full finetune:** Psionic tracks LoRA adapters but no SVF
(`APPLE_ADAPTER_LINEAGE_SPEC.md`). SVF (learn only singular-value scales of a
chosen layer) is a small add and keeps the whole coordinator < 20K learnable
params — worth porting since it materially helped TRINITY (ablation: −2.6 avg).

## Seam B — Conductor-style NL workflow planner

**Where:** above the dispatch tick, and inside Blueprint-chat delegation.
`docs/launch/2026-06-18-blueprint-tassadar-chat-delegation.md` (EPIC #5449)
already wants a Blueprint chat program that turns a chat turn into typed steps;
the missing pieces (#5450–#5456) are exactly a workflow planner + step binding.
A Conductor *is* that planner, expressed as a Blueprint signature: intent
"solve/optimize X" → Conductor emits `(model_id, subtasks, access_list)` →
Blueprint executes each step → Tassadar/verification-command grounds the reward.

**Why it fits our coding wedge:** the Conductor's headline behaviors are the
ones our agentic-labor-market doc is reaching for by hand — planner→coder→
format-checker pipelines, weak-model-as-validator, difficulty-adaptive step
counts (`2026-06-11-coding-agent-primitive-wedge.md`, the Rung-1/2 ladder in
`2026-06-11-autopilot-agentic-labor-market.md`). Instead of humans designing
those topologies per rung, the Conductor learns them from the verification
signal we already collect.

**Recursion = our test-time scaling knob.** The Conductor's self-recursion
(it sees its own prior output and revises) maps onto our retry/reassignment
logic in the Artanis tick. Cap recursion depth = cap spend; this gives Autopilot
a tunable "try harder" lever that bottoms out in real settled cost.

**Training:** GRPO on Qwen2.5-7B is the closest thing Psionic already supports
(`AGENTIC_SFT_RL_REFERENCE_PROGRAM.md` has the rollout-worker / validator-verdict
loop). Reward = format (parseable workflow) + correctness (Verified verdict or
passing verification command), exactly Sakana's two-tier reward. The
rollout-worker protocol and validator-aware adjudication Psionic documents are
the GRPO substrate; the new bit is the Conductor *output schema* and the parser.

## Why the order: A before B

- A is cheaper (tiny head, no 7B RL run) and lands on a seam with a hard reward
  *today* (executor replay), so we can measure whether a learned router actually
  beats `cheapest-viable` before investing in the 7B Conductor.
- A's coordinator can be trained on **logs we are already accumulating** (which
  worker got which job, which verified, at what cost) — `cloud-agent-fleet-audit`
  and the trace-contribution verdict stream. No new data collection.
- B is the bigger prize (general, language-grounded, recursive) but depends on
  the Blueprint-chat runtime (#5452) shipping and a real GRPO run.

## Open questions / risks

- **Reward latency & cost.** Each coordinator evaluation runs real workers and
  real replay, and (for executor work) moves real sats. ES's small population ×
  replication count is attractive, but we must cap eval spend per generation.
  Treat the per-generation budget as a first-class config, logged as a receipt.
- **Governance.** Psionic's promoted-vs-candidate contract
  (`COMPILED_AGENT_PROMOTED_ARTIFACT_CONTRACT.md`, `..._SHADOW_GOVERNANCE.md`)
  is the right home for a learned coordinator: ship it as a **candidate** in
  shadow, compare against the heuristic router on verified-work-per-sat, promote
  only on a clean win. Don't let a learned router take live dispatch authority
  un-shadowed.
- **Capability-gating still applies.** A router may *want* to send executor work
  to a device, but dispatch is gated by the receipted capability envelope
  (`2026-06-11-tassadar-capability-envelope-pylon-consumer-evidence.md`). The
  coordinator selects *within* the capability-eligible set; it never overrides
  the receipt gate.
- **Don't keyword-route.** Per the workspace contract, the coordinator must be a
  typed semantic/learned selector, which both Sakana designs already are — but
  the Blueprint binding for Seam B must keep using the semantic signature lookup,
  not string matching.

## Concrete next steps

1. **Spec the router I/O** against the trace-contribution dispatch + the
   inference gateway: features in, `(worker, role)` out. One short design note.
2. **Stand up a sep-CMA-ES trainer** in Psionic as a new optimizer alongside
   SFT/RL; reward = verified-work-per-sat over a replayed batch. (Port SVF too.)
3. **Shadow-deploy** the learned router as a candidate artifact; log
   router-choice + confidence on every assignment receipt; compare to heuristic.
4. Only after A proves out: **define the Conductor output schema** as a Blueprint
   signature and wire GRPO using the existing rollout/validator loop, grounding
   reward in the verification command / replay verdict.

See [`coordinator-as-verified-work.md`](coordinator-as-verified-work.md) for the
deeper point: our verifier is cryptographic, not an LLM judge, and the
coordinator can itself become a paid Tassadar work definition.
