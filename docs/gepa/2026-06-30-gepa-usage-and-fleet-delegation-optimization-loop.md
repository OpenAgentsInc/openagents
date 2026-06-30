# GEPA in OpenAgents — How We Use It, and a Fleet-Delegation Optimization Loop for Khala Code

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

The consumer supplies: (1) a **seed candidate** (the current system text), (2) a
**metric/feedback function** that scores a rollout *and returns rich textual
feedback* (the trace GEPA's `reflection_lm` reads), and (3) a **dataset** of
tasks. GEPA owns the search. Base package has zero hard deps; the `full` extra
pulls `litellm`, `datasets`, `mlflow`, `wandb`.

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
- **Mutalisk is the offline lane, scaffolded.** `/Users/christopherdavid/work/mutalisk`
  is "the standalone OpenAgents Python DSPy/GEPA offline-optimization lane"
  (sibling to `hydralisk`, the NVIDIA inference lane). It is a **non-Worker batch
  service** that runs DSPy + GEPA over executed traces/evals and emits
  **candidate artifacts only**. Its candidate contract (the seam) is:
  `{ signature, base_module, optimized_module, metric, eval_evidence_refs,
  trace_provenance }`, written to a shared store (R2/object). Mutalisk **never
  mutates production**; the Effect side reads candidates, runs its own acceptance
  gate, and only then promotes. (See `mutalisk/README.md`, `mutalisk/docs/ARCHITECTURE.md`.)
- **Governance moat is built (Effect).** `signature-lookup.ts` (chooses program
  signatures, module versions, tool scopes, release gates, evidence/receipt
  requirements; enforces `safeProjection`,
  `actionSubmissionRequiredForDirectEffects: true`) and `action-submission.ts`
  (forces any externally-effecting action — `create_pull_request`, `deploy`,
  `spend_money`, … — through a proposal-only record with
  `directExecution: false`, `programRunAuthorityBoundary: "evidence_only"`,
  `approvalRequired: true`). This is what makes any GEPA output **safe to ingest**.
- **Historical GEPA/RLM/FRLM is pruned → `backroom`.** openagents removed most
  DSPy/GEPA/Adjutant/RLM/FRLM code on 2026-02-25 (commit `d7f53fccc`). The FRLM
  conductor archaeology lives at
  `backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/frlm/src/conductor.rs`
  — it models recursive decomposition as **sub-query scheduling/fanout over
  NIP-90 with a local executor fallback** (the orchestration is market/dispatch
  logic; the leaf "execute code over a context fragment" is what upstream RLM does).

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

1. Mutalisk writes a **candidate artifact**:
   `{ signature: "khala.fleet.delegation.v1", base_module, optimized_module:
   <new delegation policy text>, metric, eval_evidence_refs, trace_provenance }`
   → R2, indexed in D1.
2. The Effect online authority (Khala/Artanis) reads it, runs the **release-gate +
   action-submission** acceptance (`signature-lookup.ts` selectability +
   `action-submission.ts` evidence-only boundary). A delegation-policy change is a
   proposal, not a direct effect — `approvalRequired: true`.
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
| GD-0 | Trace export: a public-safe `delegation_example` view joining assignment lifecycle + closeout + token rows + PR/merge outcome | a Mutalisk-readable dataset of past delegations, redacted |
| GD-1 | Metric + feedback function (scalar Pareto dims + textual failure refs) | scores a known-good and known-bad delegation correctly; feedback names the real blocker refs |
| GD-2 | `gepa.optimize` job in Mutalisk over the dataset; emit a `khala.fleet.delegation.v1` candidate to R2/D1 | a candidate artifact with measurable val-set gain over the seed objective/dispatch policy |
| GD-3 | Effect admission: signature-lookup selectability + action-submission proposal for the delegation-policy candidate | candidate surfaces as a gated proposal; no auto-promotion |
| GD-4 | Wire the admitted policy into the live Pylon dispatch + watcher; close the loop | next fanout uses the optimized objective/dispatch text; new traces flow back to GD-0 |

GD-0 and GD-1 are pure data/eval work (no production risk) and are the ASAP
starting point. Each phase lands behind the existing evidence/receipt boundary.

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

- `projects/repos/gepa/README.md`, `projects/repos/gepa/` (the library; `gepa.optimize` API).
- `/Users/christopherdavid/work/mutalisk/README.md`, `mutalisk/docs/ARCHITECTURE.md` (offline lane + candidate contract).
- `docs/research/2026-06-28-dspy-rlm-python-backend-vs-effect-audit.md` (the hybrid decision; staged plan).
- `docs/artanis/2026-06-08-bounded-gepa-scheduled-runner.md` (today's bounded GEPA runner; the governance wrapper).
- `packages/probe/packages/runtime/src/blueprint/signature-lookup.ts`, `action-submission.ts` (admission moat).
- `backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/frlm/src/conductor.rs` (FRLM dispatch archaeology, issue #6654).
- This session's fleet runbook + learnings: `docs/ops/2026-06-29-khala-codex-fleet-manager-runbook.md` (the delegation failure modes GD-1's feedback function encodes).
- Codex→Khala port + bond next-step: `docs/codex/2026-06-30-codex-to-khala-code-porting-audit.md`, `docs/labor/2026-06-30-forfeitable-bond-next-step.md` (the backlog the fleet delegation just executed).

## 8. Status

| Item | State |
| --- | --- |
| GEPA library available (`projects/repos/gepa`) | yes (reference) |
| Mutalisk offline lane | scaffolded; candidate contract defined |
| Live GEPA runner | bounded status projection only (no real optimization) |
| Blueprint admission moat | built (signature-lookup + action-submission) |
| Fleet-delegation GEPA loop (GD-0..GD-4) | not started — GD-0/GD-1 are the ASAP next step |
