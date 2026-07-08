# GEPA in OpenAgents — How We Use It, and a Fleet-Delegation Optimization Loop for Khala Code

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-30

Status: research + design audit. Documents what GEPA is, how OpenAgents **does**
and **plans to** use it, and defines a concrete GEPA optimization loop for making
Khala Code fleet delegation (Khala → Pylon → Codex) work smoothly. Flips no
promise state, changes no runtime authority, broadens no public copy.

Sources: `projects/repos/gepa` (the library), `/Users/christopherdavid/work/mutalisk`
(the offline lane), `docs/research/2026-06-28-dspy-rlm-python-backend-vs-effect-audit.md`
(the decision record), `docs/artanis/2026-06-08-bounded-gepa-scheduled-runner.md`
(today's bounded runner), the Blueprint governance modules, and the
`backroom` FRLM archaeology. A companion research sweep across these is in flight;
this doc is written from the authoritative planning record plus first-hand
experience running the fleet this session.

---

## 1. What GEPA is

**GEPA (Genetic-Pareto)** optimizes *any system with textual parameters* —
prompts, code, agent architectures, configs, scheduling policies — against *any*
evaluation metric. Unlike RL/gradient methods that collapse a run into one scalar
reward, GEPA uses an LLM to **read full execution traces** (errors, logs,
reasoning, profiling) to diagnose *why* a candidate failed, then proposes
targeted edits. Through iterative **reflection → mutation → Pareto-aware
selection**, it evolves high-performing variants with very few evaluations
(~100–500 vs 5,000–25,000+ for GRPO; ~35x fewer rollouts, ~90x cheaper than a
frontier baseline). It is *the* DSPy optimizer (`dspy` depends on `gepa[dspy]`).

**The core API is a single call** (`projects/repos/gepa/README.md`):

```python
result = gepa.optimize(
    seed_candidate=...,        # the text parameter(s) to optimize, as a dict
    trainset=..., valset=...,  # examples with inputs + ground truth / checks
    task_lm=...,               # the model that runs the task being optimized
    reflection_lm=...,         # the (stronger) model that reads traces & mutates
    max_metric_calls=...,      # rollout budget
)
# result.best_candidate -> the optimized text parameters
```

The consumer supplies: (1) a **seed candidate** — a `dict[str,str]` mapping named
system components to their text, (2) a **metric/feedback function** that scores a
rollout *and returns rich textual feedback* (the trace GEPA's `reflection_lm`
reads), and (3) a **dataset** of tasks. GEPA owns the search. Base package has
zero hard deps; the `full` extra pulls `litellm`, `datasets`, `mlflow`, `wandb`.

The real integration point is the **`GEPAAdapter` protocol**
(`projects/repos/gepa/src/gepa/core/adapter.py`): implement `evaluate(batch,
candidate, capture_traces) -> EvaluationBatch` (outputs + per-example scores +
optional trajectories) and `make_reflective_dataset(candidate, eval_batch,
components) -> {component: [{"Inputs","Generated Outputs","Feedback"}]}`. The
`Feedback` is GEPA's **Actionable Side Information (ASI)** — the diagnostic text
that acts as the text-optimization analogue of a gradient. There is also
`gepa.optimize_anything(seed_candidate, evaluator, objective, config)`
(`src/gepa/optimize_anything.py`) for any artifact given just a scoring evaluator
that logs ASI via `oa.log(...)`. The engine loop lives in `src/gepa/core/engine.py`
(`GEPAEngine.run`): Pareto-select → execute+capture traces → reflect → mutate
(`src/gepa/proposer/reflective_mutation/`) → minibatch-accept → full-val + Pareto
update, with optional system-aware `MergeProposer`. Built-in adapters include
`DefaultAdapter`, `DSPyAdapter`/`DSPyFullProgramAdapter`, `MCPAdapter` (optimizes
MCP tool descriptions), and `TerminalBenchAdapter` (optimizes a terminal coding
agent) — i.e. GEPA already targets agent/tool systems, not just single prompts.

The load-bearing idea for us: **GEPA optimizes text against measured outcomes by
reading traces.** We already produce rich, redacted traces for every Codex
delegation (lifecycle events, closeout refs, token rows, PR outcome). That makes
fleet delegation a natural GEPA target.

---

## 2. The decided architecture: Python offline → Effect online authority

`docs/research/2026-06-28-dspy-rlm-python-backend-vs-effect-audit.md` is the
decision record. Headline: **Hybrid, with a strict tier boundary.**

- **Offline optimize/compile tier = real Python GEPA + DSPy** (`projects/repos/gepa`,
  `projects/repos/dspy/dspy/teleprompt/`). Reimplementing GEPA's Pareto-reflective
  search in Effect would be large, low-leverage duplication of a fast-moving,
  production-proven library. Run it as a **non-Worker batch service**.
- **Online serving + governance = native Effect/TS** on Cloudflare Workers: the
  Blueprint **signature-lookup selector**
  (`packages/probe/packages/runtime/src/blueprint/signature-lookup.ts`) and the
  **evidence-only action-submission boundary**
  (`packages/probe/packages/runtime/src/blueprint/action-submission.ts`). These
  are latency-sensitive and *are* the governance moat. No Python on the hot path.

**The unifying rule:** *Python is offline/leaf compute that produces untrusted
candidates and evidence; Effect is the online authority that selects, gates, and
admits them.* A GEPA win is a **candidate**, never an auto-promotion.

---

## 3. How we DO use GEPA today

The honest current state:

- **Live "GEPA" is a bounded status-projection loop, not real optimization.**
  `docs/artanis/2026-06-08-bounded-gepa-scheduled-runner.md` +
  `apps/openagents.com/workers/api/src/artanis-gepa-scheduled-runner-proof.ts`:
  a Cloudflare minute-cron that explicitly **denies** assignment dispatch, model
  training, provider mutation, runtime promotion, settlement, and wallet-spend
  authority. It is the *governance/projection wrapper* — the place a real Python
  GEPA job will later plug into — not an optimizer.
- **The online-authority gate for Mutalisk candidates already exists.**
  `apps/openagents.com/workers/api/src/probe-gepa-standing-optimization-loop.ts`
  (schema `omega.probe_gepa_standing_optimization_loop.v1`) is an Effect-Schema
  governance projection with actions `observe | emit_candidates | promote_live`
  and decisions `blocked | candidate_artifacts_ready | needs_more_evidence`. Its
  inputs already carry `mutaliskLaneRefs`, `candidateManifestRefs`,
  `optimizerRunRefs`, `dspyRlmAuditRefs`, and `releaseGateRefs` — it is purpose-
  built to ingest Mutalisk's offline candidates and never auto-promote.
- **Pylons are modeled as GEPA benchmark metric-call runners.**
  `apps/pylon/src/gepa-capability.ts` (`pylon.capability.gepa.benchmark_runner.v0.3`,
  envelope `openagents.pylon.gepa_capability_envelope.v0.3`, `stage: "gepa-first"`,
  `supportsTraining: false`) — Pylons run the *evaluations* GEPA needs, not the
  optimizer itself.
- **A GEPA feedback (ASI) builder already exists.**
  `packages/probe/packages/runtime/src/benchmark/studybench-gepa-feedback.ts`
  (`probe.studybench_gepa_feedback.v0`) turns OpenAgents StudyBench rubric scores
  into GEPA-style textual feedback — the template for the metric/feedback function
  the fleet-delegation loop needs. The candidate seam is the manifest schema
  **`psionic.probe_gepa_candidate_manifest.v1`** (referenced in
  `packages/probe/packages/runtime/src/benchmark/candidate-execution.ts` and
  `closeout-writer.ts`), which Mutalisk's `Candidate` must match.
- **Mutalisk is the offline lane, scaffolded.** `/Users/christopherdavid/work/mutalisk`
  is "the standalone OpenAgents Python DSPy/GEPA offline-optimization lane"
  (sibling to `hydralisk`, the NVIDIA inference lane). It is a **non-Worker batch
  service** that runs DSPy + GEPA over executed traces/evals and emits
  **candidate artifacts only**. `pyproject.toml` depends on `dspy>=2.5` +
  `gepa>=0.1.1` and exposes a `mutalisk-optimize` CLI. `src/mutalisk/optimizer.py`
  defines two optimizers behind one `Optimizer` protocol: a dependency-free
  deterministic `LocalSearchOptimizer` (the green default) and **`GepaOptimizer`**,
  which calls `gepa.optimize(seed_candidate, trainset, valset,
  adapter=MutaliskOfflineAdapter(), max_metric_calls=..., seed=...)` — where
  `MutaliskOfflineAdapter` implements the real `GEPAAdapter` (`evaluate`,
  `make_reflective_dataset`, `propose_new_texts`) **fully offline, no LM/network**,
  stamping `optimizer@version` (e.g. `gepa@0.1.1`). `src/mutalisk/candidate.py`
  defines the frozen candidate contract (the seam): `Candidate{ signature,
  base_module_ref, optimized_module, metric_name, metric_value, optimizer,
  eval_evidence_refs, trace_provenance_refs }` with fail-closed `validate()`; the
  R2-sink `CandidateEmitter` is the build-out task, and the shared schema must
  match the Effect side's `psionic.probe_gepa_candidate_manifest.v1`. Mutalisk
  **never mutates production**. (See `mutalisk/README.md`,
  `mutalisk/docs/ARCHITECTURE.md`, `mutalisk/src/mutalisk/{optimizer,candidate}.py`.)
- **Governance moat is built (Effect).** `signature-lookup.ts` (chooses program
  signatures, module versions, tool scopes, release gates, evidence/receipt
  requirements; enforces `safeProjection`,
  `actionSubmissionRequiredForDirectEffects: true`) and `action-submission.ts`
  (forces any externally-effecting action — `create_pull_request`, `deploy`,
  `spend_money`, … — through a proposal-only record with
  `directExecution: false`, `programRunAuthorityBoundary: "evidence_only"`,
  `approvalRequired: true`). This is what makes any GEPA output **safe to ingest**.
- **Historical GEPA/RLM/FRLM/Adjutant is pruned → `backroom`.** openagents
  removed most DSPy/GEPA/Adjutant/RLM/FRLM code on 2026-02-25 (commit `d7f53fccc`).
  The archaeology matters because it includes **direct delegation-optimization
  prior art**:
  - A **full Rust-native GEPA optimizer** — `backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/dsrs/src/optimizer/gepa.rs`
    (with `mipro.rs`, `copro.rs`, `pareto.rs`): `GEPACandidate` (instruction,
    module, per-example `example_scores`, `parent_id`, `generation`), Pareto-front
    selection, LLM reflection/mutation — a faithful port of the GEPA paper. The
    project chose to adopt **upstream** GEPA via Mutalisk rather than maintain this.
  - **Adjutant DSPy decision pipelines** — `backroom/oanix/src/dspy_*.rs` +
    `backroom/openagents-docs-rust-archive-2026-02-11/docs/dspy/openagents-usage.md`:
    the **complexity, delegation, and RLM-trigger pipelines were DSPy-first
    signatures, explicitly planned to be compiled with MIPROv2 by default and
    GEPA for multi-objective tradeoffs (quality vs cost/latency/tool-calls)**, with
    training data collected from real runs and compiled artifacts promoted via
    "policy bundles" (`policy_bundle_id`) through shadow/canary gates. This is the
    exact precedent for the fleet-delegation loop below — delegation as an
    optimizable, Pareto-multi-objective signature.
  - The **FRLM conductor** — `.../crates/frlm/src/conductor.rs` (+ `dspy_signatures.rs`):
    recursive decomposition as **sub-query fanout over NIP-90 with a local-executor
    fallback** (budget/verify policy, trace emission). Issue #6654 direction.

So: **we do not yet run a real GEPA optimization loop in production.** We have the
governance wrapper, the offline lane scaffold, and the candidate/admission seam.

---

## 4. How we PLAN to use GEPA (staged)

From the decision record (§9), the staged adoption:

- **Stage 0 (now):** online serving/governance stays Effect; the GEPA scheduled
  runner stays a bounded status projection. No Python in the product runtime.
- **Stage 1 (offline GEPA/DSPy compile):** stand up the Mutalisk Python service
  (container or Psionic-hosted) running real `gepa.optimize` + DSPy teleprompt
  against **public-safe eval sets**; emit candidate artifacts to R2, index in D1;
  wire them as Blueprint **module-version candidates** behind existing release
  gates. The Effect runner becomes the governance wrapper over real results.
- **Stage 2 (RLM leaf executor):** adopt upstream `rlms` / DSPy's RLM module as a
  sandboxed leaf executor on the Pylon/cloud tier, invoked through the existing
  Khala → Pylon assignment path with evidence + redacted traces; keep the
  FRLM-conductor / NIP-90 fanout native in Effect.
- **Stage 3 (promotion):** promote GEPA-optimized signatures into production via
  release-gate + action-submission once eval gains are proven and reproducible.

**Hard nots:** run Python in Workers; move selection/governance into Python; let
any Python output bypass the evidence/receipt boundary.

Mutalisk + Blueprint are the two halves: **Mutalisk produces candidates;
Blueprint's signature-lookup + release-gate + action-submission admit them.**

---

## 5. The GEPA optimization loop for Khala Code fleet delegation

This is the ASAP target. "Fleet delegation" = Khala → Pylon → Codex: an operator
or Artanis delegates a GitHub issue to a Codex worker, which materializes a
bounded workspace, implements it, runs a verifier, and opens a PR. Running the
full Codex-port backlog through the fleet this session exposed exactly the
failure surface GEPA is built to optimize.

### 5.0 The deterministic delegation program (the bundle GEPA optimizes)

**Observed failure mode (2026-06-30).** From Khala Code Desktop, a one-line
"delegate this issue for analysis" request through `codex_spawn` hard-failed:

```
codex_spawn_failed: No Pylon Codex assignment capacity is available right now
(0/1 available). Wait for the running assignment to finish or retry.
```

Root cause: `codex_spawn` runs a *partial* sequence — `pylon_ensure` →
`presence heartbeat` → ready-account selection → dispatch — but it **never
advertises per-account capacity**. `provider go-online` *without*
`OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY` reports the floor `max=1, available=0`;
*with* it set to 5 the same Pylon reports `max=5, available=1`. So when the single
default slot is busy, the tool errors instead of running the one module that would
*raise* capacity. There is no fallback ladder: a missing precondition is a dead
end, not a recoverable step. This is the same brittleness the whole session hit
manually — the wrong env var capping a 10-wide fanout at 3, stale-heartbeat 409s,
`duplicate_active_assignment`, a dead `status` account — each a missing or
mis-ordered deterministic step.

**The target: one prompt → a deterministic bundle that spins up all config and
goes.** Delegation should be a single typed **program/signature**
(`khala.fleet.delegate`) the user triggers with one prompt. The program is a
**fixed, deterministic** sequence of modules, each with a precondition, an action,
a typed result, and a **deterministic fallback** that runs the module which
establishes the missing precondition:

| Module | Precondition | Deterministic fallback if unmet |
| --- | --- | --- |
| `ensure_pylon` | a local Pylon is online | start/adopt it (`pylon_ensure`) |
| `advertise_capacity` | ≥1 advertised free Codex slot | set `ACCOUNT_CONCURRENCY` for the ready-account count, fresh `presence heartbeat`, re-read capacity |
| `select_account` | a ready account with a free slot | skip `credentials_missing`/`revoked`; fall to the next ready account; if none, surface "connect an account" |
| `prepare_work` | a pinned workspace or fixture | use the fixture when no repo/commit/verify pins are given |
| `dispatch` | capacity + account + work | on `stale_heartbeat` → re-heartbeat + retry once; on `duplicate_active_assignment` → back off one cycle; on `no_available_codex_capacity` → loop back to `advertise_capacity` (or wait, load-gated) |
| `verify_closeout` | a closeout + exact token rows | re-poll; on `verify_failed` report the typed blocker, never claim success |

The control flow above is **hand-written and deterministic** — not LLM-decided per
call. That is the "more deterministic than now" the failure mode demands: the
bundle cannot dead-end on a missing precondition, because every precondition has a
module that satisfies it. `codex_spawn`'s `0/1` failure is simply the
`advertise_capacity` module (and its fallback) being absent.

**Where GEPA fits: it optimizes the program's *parameters*, not its control
flow.** This is exactly the DSPy model — a deterministic Program/Signature whose
textual/policy parameters are compiled by an optimizer. The deterministic skeleton
stays fixed; GEPA tunes the soft knobs of each module:

- `advertise_capacity`: how many slots to advertise given N ready accounts and
  current machine load (the policy that avoids both `max=1` starvation and overload).
- `select_account`: the account-ranking heuristic (named-ready before default,
  skip missing/revoked, spread load).
- `dispatch`: the retry/backoff policy and the objective-prompt template.
- `verify_closeout`: the success/abort criteria.

GEPA reads delegation traces — including this exact `0/1` failure as a training
example with the feedback `no_available_codex_capacity → advertise_capacity module
absent/insufficient` — and reflects to propose better parameter text. **The
deterministic program makes delegation reliable today; the GEPA loop makes its
parameters self-improving over time.** They are complementary: build the program
first (so a single prompt just works), then optimize its parameters with GEPA.

### 5.1 What to optimize (the seed candidate — textual parameters)

A `delegation policy` candidate dict, e.g. `khala.fleet.delegation.v1`:

- **`objective_template`** — the `--prompt` text sent to each worker. Vague
  objectives produced workers that didn't open a PR or "completed" without a
  mergeable diff; precise ones (cite issue #, exact files, exact verifier, "open
  or update a PR that closes #N") produced clean PRs. This is the single highest-
  leverage text parameter.
- **`verifier_selection`** — how the `--verify` command is chosen per issue
  (wrong/absent verifier → blocked closeout; right one → green gate).
- **`dispatch_policy`** — capacity/heartbeat/retry params: advertise via
  `OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY` (the per-account var; the non-
  account var silently reverted to default and capped a 10-wide fanout at 3),
  fresh `presence heartbeat` immediately before each request (else 409
  stale-heartbeat), back-off on `duplicate_active_assignment`, load-gate (skip
  dispatch when 1-min load > ~14).
- **`merge_resolution_template`** — the prompt for a conflict-resolver worker
  (sibling PRs collide on shared files like `packages/khala-tools/src/index.ts`;
  "checkout, merge main, UNION every lane's additions, test, squash-merge").

### 5.2 The trainset (we already produce it)

Every delegation already emits a redacted trace usable as a GEPA example:

- **Input** = `{ issue, objective_template rendered, verifier, capacity context }`.
- **Rollout trace** = the Pylon assignment lifecycle (`assignment_run.accepted →
  runtime_started → runtime_progress* → completed`), the closeout
  (`khala closeout` checklist, no-spend/settlement refs), the exact token rows
  (`token_usage_events`, `usage_truth: exact`), the redacted ATIF trace
  (`agent_traces`, `visibility: owner_only`), and the **PR outcome** (opened?
  CLEAN? verifier passed? merged? conflicted?).
- **Provenance** rows already exist: `pylon_api_assignments`, `pylon_api_events`,
  `pylon_codex_raw_event_chunks`, plus the GitHub PR/merge state.

These are public-safe/redacted by construction — exactly Mutalisk's input.

### 5.3 The metric + feedback function (the GEPA seam)

A per-assignment scorer that returns **both a scalar and rich textual feedback**
(GEPA's reflection_lm reads the feedback):

- **Scalar** (Pareto dimensions, optimize jointly — GEPA is multi-objective):
  - `single_prompt_success` (one user prompt spun up all config and produced a
    result with no manual intervention — the §5.0 bundle ran end-to-end, including
    `advertise_capacity` when capacity started at `0/1`): the headline reliability bit.
  - `merged_clean` (PR opened → verifier green → squash-merged with no human
    conflict help): the primary success bit.
  - `admitted_first_try` (no `duplicate_active_assignment` / stale-heartbeat
    refusal): dispatch-policy quality.
  - `wall_clock`, `token_cost` (exact rows), `idle_gap` (slot sat empty): cost.
  - `conflict_churn` (did the PR need rebase/resolution): coordination quality.
- **Textual feedback** = the concrete failure refs, fed verbatim to reflection:
  `blocker.public.pylon_dispatch.duplicate_active_assignment`,
  `...no_available_codex_capacity`, `verify_failed`, `vacuous_pr` (completed but
  no diff), `pr_conflicted`, `objective_too_vague`. GEPA reads these to mutate the
  objective/verifier/dispatch text that caused them.

### 5.4 The loop

```
# offline, in mutalisk (Python), against public-safe delegation traces
seed = {
  "objective_template": <current --prompt template>,
  "verifier_selection": <current rule>,
  "dispatch_policy":    <current capacity/heartbeat/retry params>,
  "merge_resolution_template": <current conflict-worker prompt>,
}
result = gepa.optimize(
  seed_candidate=seed,
  trainset=delegation_examples,      # past runs (input + trace + PR outcome)
  valset=held_out_examples,
  task_lm=<the worker-class model surrogate / replay>,
  reflection_lm=<a strong model reading full traces>,
  max_metric_calls=<bounded budget>,
)
# -> result.best_candidate = improved delegation policy text
```

GEPA reflects on *why* delegations failed (vague objective → no PR; wrong env var
→ capped fanout; dispatch at load 17 → timeout; sibling overlap → conflict churn)
and evolves better objective/verifier/dispatch/merge text on the Pareto frontier.

### 5.5 Candidate → admission → live (the governance seam)

1. Mutalisk writes a **candidate artifact** in the existing seam — a
   `Candidate{ signature: "khala.fleet.delegation", base_module_ref,
   optimized_module: <new delegation policy text>, metric_name, metric_value,
   optimizer: "gepa@0.1.1", eval_evidence_refs, trace_provenance_refs }`
   conforming to `psionic.probe_gepa_candidate_manifest.v1` → R2, indexed in D1.
   (No new seam needed — reuse the manifest schema the benchmark path already uses.)
2. The Effect online authority ingests it through the **already-built**
   `probe-gepa-standing-optimization-loop.ts` (`observe → emit_candidates →
   promote_live`, carrying `candidateManifestRefs`/`releaseGateRefs`), gated by
   Blueprint `signature-lookup.ts` selectability + the evidence-only
   `action-submission.ts` boundary. A delegation-policy change is a proposal, not
   a direct effect — `approvalRequired: true`, never auto-promoted. The metric
   builder follows the existing `studybench-gepa-feedback.ts` ASI pattern.
3. On admission, the optimized `objective_template` / `dispatch_policy` /
   `merge_resolution_template` become the live values used by the **Pylon dispatch
   path and the fleet watcher** (the same `khala request` / `assignment
   run-no-spend` / auto-merge loop this session used).
4. The promoted policy runs more delegations → new traces → the next GEPA round.
   The metric trends toward "fleet delegation working smoothly": high
   merged-clean rate, high first-try admission, low idle, low conflict churn,
   low token cost.

This is a closed reflective loop that **never lets the optimizer self-promote**:
Mutalisk proposes, Blueprint admits, the watcher executes, traces feed back.

### 5.6 Phased plan

| Phase | Deliverable | Acceptance |
| --- | --- | --- |
| **GD-P** (prerequisite) | **The deterministic delegation program** (§5.0): the `ensure_pylon → advertise_capacity → select_account → prepare_work → dispatch → verify_closeout` module pipeline with typed fallbacks, wired into `codex_spawn` / the bundle | core program exists in `packages/khala-tools/src/fleet-delegate-program.ts`; live Desktop/Pylon wiring now advertises per-account Codex capacity before selection/dispatch; the adverse-condition regression matrix covers `0/1` capacity, stale heartbeat, duplicate active assignment, missing/revoked credentials, and high-load typed blockers |
| GD-0 | Trace export: a public-safe `delegation_example` view joining assignment lifecycle + closeout + token rows + redacted ATIF + PR/merge outcome | `openagents.khala.delegation_example.dataset.v0` is implemented in `apps/openagents.com/workers/api/src/khala-delegation-example-dataset.ts`; sample fixture: `docs/gepa/khala-delegation-example.dataset.v0.json`; tests assert no raw prompts/secrets/local paths |
| GD-1 | Metric + feedback function (scalar Pareto dims + textual failure refs) | `openagents.khala.delegation_gepa_feedback.v0` is implemented in `apps/openagents.com/workers/api/src/khala-delegation-gepa-feedback.ts`; it scores known-good vs known-bad delegation examples, emits the scalar dimensions, and names concrete blocker refs including the `0/1` no-capacity case |
| GD-2 | `gepa.optimize` job in Mutalisk over the dataset; emit a `khala.fleet.delegation.v1` candidate to R2/D1 | a candidate artifact with measurable val-set gain over the seed objective/dispatch policy |
| GD-3 | Effect admission: signature-lookup selectability + action-submission proposal for the delegation-policy candidate | `projectKhalaFleetDelegationCandidateAdmission` in `probe-gepa-standing-optimization-loop.ts` ingests a `khala.fleet.delegation` candidate summary plus Blueprint selection and emits only an approval-required, evidence-only Action Submission proposal; no auto-promotion, live-promotion, runtime-promotion, or direct-execution path exists |
| GD-4 | Wire the admitted policy into the live Pylon dispatch + watcher; close the loop | `openagents.khala.fleet_delegation.parameters.v0` is decoded from `OPENAGENTS_KHALA_FLEET_DELEGATION_ADMITTED_PARAMETERS_JSON` and used by the core delegate program, Khala Code Desktop dispatch, and `khala fleet run`; admitted parameters tune capacity advertisement, account ranking, retry/backoff, objective rendering, and default verifier criteria, while missing/invalid admission falls back to safe defaults |

GD-0 and GD-1 are pure data/eval work (no production risk) and are the ASAP
starting point. Each phase lands behind the existing evidence/receipt boundary.

GD-0's exporter reads only public-safe columns and refs: assignment refs and
`public_projection_json`, lifecycle event public projections, exact
`token_usage_events` rows keyed by `task_ref = assignment_ref`, and redacted ATIF
rows with `trajectory_id` prefixes `pylon_codex:<assignmentRef>:` /
`pylon_claude:<assignmentRef>:`. It deliberately omits `coding_assignment_json`
and event bodies, then runs the shared ATIF redactor plus a dataset tripwire
before returning the Mutalisk-readable bundle.

GD-1's feedback builder consumes one GD-0 example and emits only bounded metrics
and opaque refs: `single_prompt_success`, `merged_clean`, `admitted_first_try`,
`wall_clock_seconds`, `token_cost_tokens`, `idle_gap_seconds`, and
`conflict_churn`, plus admission/verification/coordination blocker refs such as
`blocker.public.pylon_dispatch.no_available_codex_capacity`,
`blocker.public.pylon_dispatch.duplicate_active_assignment`,
`blocker.public.pylon_dispatch.pylon_stale`,
`blocker.public.pylon_assignment.verify_failed`,
`blocker.public.khala_delegation.vacuous_pr`, and
`blocker.public.khala_delegation.pr_conflicted`. The feedback object is
explicitly evidence-only: no runtime promotion, payout, public-claim authority,
raw prompts, raw traces, or judge rationale. A recovered `0/1` Codex-capacity
start is retained as a precondition ref; it only becomes a failure blocker when
the delegation dead-ends before a clean merge.

GD-3's admission projector is the Effect-side gate for a Mutalisk candidate. It
requires the standing loop to have admissible candidate artifacts, a
`psionic.probe_gepa_candidate_manifest.v1` summary with signature
`khala.fleet.delegation`, and a Blueprint signature lookup containing
`program_signature.khala.fleet.delegation.v1`,
`program_type.khala.fleet.delegation_policy.v1`, release gates, module versions,
tool scopes, evidence requirements, `safeProjection: true`,
`directMutationAllowed: false`, and
`actionSubmissionRequiredForDirectEffects: true`. A valid candidate becomes a
`probe_blueprint_action_submission_proposal` with `approvalRequired: true`,
`proposalOnly: true`, `directExecution: false`,
`directProgramRunExecutionAllowed: false`, and
`programRunAuthorityBoundary: "evidence_only"`; missing lookup or any live
promotion request blocks admission instead of creating a proposal.

GD-4 is the live read path for an admitted candidate. The admitted artifact is
reduced to the bounded parameter schema
`openagents.khala.fleet_delegation.parameters.v0` and supplied explicitly or via
`OPENAGENTS_KHALA_FLEET_DELEGATION_ADMITTED_PARAMETERS_JSON`. The deterministic
program reads only those bounded knobs: `advertiseCapacity` slot ceilings,
`accountRanking` (`named_ready_highest_slots`,
`default_ready_highest_slots`, or `lexicographic_ready`), `retryBackoff`,
`objectiveTemplate`, and `verifyCriteria.defaultVerify`. Khala Code Desktop and
`khala fleet run` both use the same renderer and capacity helpers, so switching
the admitted set changes the next dispatch/watch plan; clearing the env var or
omitting the option restores the hard-coded defaults.

---

## 6. Non-goals

- No Python in Cloudflare Workers; no Python on the online hot path.
- No optimizer self-promotion — every GEPA win is an evidence-bearing candidate
  admitted through release gates + action-submission.
- No GEPA authority over dispatch, provider mutation, settlement, model
  promotion, or wallet spend (the same denials the bounded runner already lists).
- No reimplementation of GEPA's search in Effect; use upstream `gepa.optimize`.
- No raw prompts/secrets/local paths in candidate artifacts or public projections;
  reuse ATIF redaction + exact token accounting.

## 7. References

- GEPA lib: `projects/repos/gepa/README.md`, `src/gepa/api.py`, `optimize_anything.py`, `core/engine.py`, `core/adapter.py`, `proposer/reflective_mutation/`, `strategies/candidate_selector.py`, `adapters/` (incl. `MCPAdapter`, `TerminalBenchAdapter`, `DSPyAdapter`).
- Mutalisk: `/Users/christopherdavid/work/mutalisk/{README.md,AGENTS.md,docs/ARCHITECTURE.md,pyproject.toml}`, `src/mutalisk/{optimizer.py,candidate.py}` (offline lane + `GepaOptimizer` + candidate contract).
- `docs/research/2026-06-28-dspy-rlm-python-backend-vs-effect-audit.md` (the hybrid decision; staged plan).
- Live GEPA scaffolding (Effect): `apps/openagents.com/workers/api/src/{artanis-gepa-scheduled-runner-proof.ts,probe-gepa-standing-optimization-loop.ts}`, `apps/pylon/src/gepa-capability.ts`, `packages/probe/packages/runtime/src/benchmark/{studybench-gepa-feedback.ts,candidate-execution.ts,closeout-writer.ts}` (seam: `psionic.probe_gepa_candidate_manifest.v1`).
- Admission moat: `packages/probe/packages/runtime/src/blueprint/{signature-lookup.ts,action-submission.ts}`.
- `backroom` archaeology: `openagents-prune-20260225-205724-wgpui-mvp/crates/dsrs/src/optimizer/{gepa.rs,mipro.rs,copro.rs,pareto.rs}` (Rust-native GEPA), `crates/frlm/src/{conductor.rs,dspy_signatures.rs}` (FRLM dispatch, #6654), `oanix/src/dspy_*.rs` + `openagents-docs-rust-archive-2026-02-11/docs/dspy/openagents-usage.md` (Adjutant delegation/complexity/RLM-trigger pipelines compiled by MIPROv2/GEPA — the delegation-optimization precedent).
- Blueprint note: `blueprint/` is **not cloned in this workspace**; its original `docs/programs-optimization-and-rlm.md` / `master-spec.md` are historical references not on disk. The live embodiment is the signature-lookup + action-submission modules above.
- This session's fleet runbook + learnings: `docs/ops/2026-06-29-khala-codex-fleet-manager-runbook.md` (the delegation failure modes GD-1's feedback function encodes).
- Codex→Khala port + bond next-step: `docs/codex/2026-06-30-codex-to-khala-code-porting-audit.md`, `docs/labor/2026-06-30-forfeitable-bond-next-step.md` (the backlog the fleet delegation just executed).

## 8. Status

| Item | State |
| --- | --- |
| GEPA library available (`projects/repos/gepa`) | yes (reference) |
| Mutalisk offline lane | scaffolded; `GepaOptimizer` + `Candidate` contract defined; R2 emitter is the build-out |
| Live GEPA runner | bounded status projection only (no real optimization) |
| Online-authority gate (`probe-gepa-standing-optimization-loop.ts`) | built; ingests `candidateManifestRefs`, can't auto-promote |
| Pylon GEPA capability (`gepa-capability.ts`) | built (benchmark metric-call runner) |
| GEPA feedback/ASI builder (`studybench-gepa-feedback.ts`) | built (template for the delegation metric) |
| Candidate seam (`psionic.probe_gepa_candidate_manifest.v1`) | defined on both sides; must align Mutalisk `Candidate` to it |
| Blueprint admission moat | built (signature-lookup + action-submission) |
| Deterministic delegation program (GD-P, §5.0) | core built in `@openagentsinc/khala-tools` as `khala.fleet.delegate`: typed module/precondition/blocker taxonomy plus an adverse-condition matrix for cold `0/1` capacity recovery, stale-heartbeat refresh, duplicate-assignment retry, credentials-missing/revoked blockers, high-load gating, account selection, dispatch fallbacks, and closeout blockers; `codex_spawn`, `pylon khala spawn`, `pylon khala request --workflow codex_agent_task`, and `khala fleet run` now compute and publish the required per-account capacity before dispatch |
| Fleet-delegation GEPA loop (GD-0..GD-4) | GD-0, GD-1, GD-3, and GD-4 built: the dataset contract joins public-safe delegation traces, `openagents.khala.delegation_gepa_feedback.v0` scores them with scalar Pareto dimensions plus concrete blocker refs, `projectKhalaFleetDelegationCandidateAdmission` gates `khala.fleet.delegation` candidates through evidence-only Action Submission, and admitted `openagents.khala.fleet_delegation.parameters.v0` sets now drive live delegation behavior in the core program, Khala Code Desktop, and `khala fleet run`; GD-2 remains |
| Desktop fleet-status process accounting (#7737) | built: `codex_fleet_status` counts only real `codex exec` agent turns, excludes Codex.app GUI helpers and runner supervisors, labels `ps etime` as elapsed time, and reconciles that count against active assignment markers |
