# Pylon GEPA Coding-Agent Benchmark Run

Date: 2026-06-08
Status: proposal for a first GEPA-only optimization lane
Scope: Probe, Blueprint, Pylon, Psionic, public benchmark-cloud,
Terminal-Bench, SWE/SWT-style coding tasks, and the local `gepa-ai/gepa`
reference clone at `projects/repos/gepa`.

## Thesis

The first serious benchmark-climbing run should be GEPA-only. Do not start with
LoRA, GRPO, custom model training, or a broad agent rewrite. Start by using the
Pylon network to run a large GEPA optimization loop over the text artifacts that
already control Probe and Blueprint behavior:

- Probe system prompts and backend-specific prompt addenda.
- Blueprint Program Signature selection playbooks.
- tool-menu projection constraints.
- Terminal-Bench task-family playbooks.
- retained failure repair notes.
- closeout and verification instructions.

The target is not to claim a leaderboard immediately. The target is to build a
repeatable optimizer lane that turns real coding-agent benchmark failures into
candidate Blueprint improvements, tests those candidates on held-out tasks, and
promotes only the candidates that improve measured benchmark performance without
weakening runtime policy.

GEPA is a good first optimizer because it is explicitly designed to optimize
textual artifacts from evaluator feedback. Its `optimize_anything` API supports
single-task search, multi-task transfer, and generalization across train and
validation sets. Its gskill work shows the direct pattern for coding agents:
generate or collect verifiable tasks, run agent rollouts, feed pass/fail
results, traces, and test output to a stronger reflection model, and evolve
skills that transfer into another agent harness.

## Source Material

- GEPA blog: "Automatically Learning Skills for Coding Agents"
  `https://gepa-ai.github.io/gepa/blog/2026/02/18/automatically-learning-skills-for-coding-agents/`
- Local GEPA reference: `projects/repos/gepa`
- GEPA gskill README:
  `projects/repos/gepa/src/gepa/gskill/README.md`
- GEPA TerminalBench example:
  `projects/repos/gepa/src/gepa/examples/terminal-bench/train_terminus.py`
- Probe benchmark docs:
  `probe/docs/benchmarks/2026-06-08-probe-continual-benchmark-learning-apparatus.md`
  and
  `probe/docs/benchmarks/2026-06-08-workspace-benchmark-systems-audit.md`
- OpenAgents product surface Nexus/Pylon update:
  `openagents/docs/nexus/2026-06-08-pylon-live-assignment-closeout-smoke.md`
  plus the adjacent Pylon Agent API, release freeze, release gate, wallet
  readiness, and self-serve registration runbooks.
- Artanis Forum posting/readback:
  `https://openagents.com/AGENTS.md` and
  `openagents/docs/forum/2026-06-07-artanis-forum-posting-runbook.md`.

Important source observations:

- The GEPA blog says gskill combines GEPA `optimize_anything` with SWE-smith
  generated tasks, then learns repository-specific skill files for coding
  agents.
- The reported gskill experiment used roughly 300 tasks per repo, with train,
  validation, and holdout test splits.
- The learned skills improved Mini-SWE-Agent resolve rate on Jinja from 55% to
  82% and on Bleve from 24% to 93%, then transferred to Claude Code while also
  reducing average task duration.
- The local gskill README describes an output directory with `best_skills.txt`,
  config, iteration metrics, proposer logs, prompt hashes, cost summaries, and
  resumable state.
- The local TerminalBench example already demonstrates the shape we need:
  create TerminalBench train, validation, and test splits; evaluate a seed
  prompt; call GEPA with a rollout budget; evaluate the optimized prompt on the
  held-out test set.

## What We Optimize First

The first candidate should be a structured text bundle, not a single prompt.
GEPA can optimize a dictionary of named text parameters, which lets us keep the
Blueprint system readable and separately promotable.

Initial candidate components:

```text
probe_system_prompt
terminal_bench_global_playbook
signature_selection_policy
tool_menu_policy
patch_and_test_policy
failure_family_playbooks
closeout_policy
```

Each component should remain policy-subordinate. GEPA may propose better
instructions for using Blueprint signatures, tests, tools, and retained
failure evidence. GEPA must not propose new runtime authority, bypass approval
rules, bypass release gates, add ad hoc intent routing, or replace typed
Blueprint selection with keyword matching.

The "Blueprint optimization" boundary is therefore:

- optimize how Probe uses existing Blueprint authority;
- optimize which approved Program Signatures are selected for a benchmark task
  family;
- optimize tool-menu projection hints and verification discipline;
- do not let an optimized text candidate become Source Authority.

Promotion remains a normal OpenAgents policy event. Optimizer acceptance is not
runtime promotion.

## Pylon's Role

Pylon should supply rollout scale. A useful GEPA run is dominated by agent
executions, not by the reflection calls. Pylon can parallelize those executions
across local Macs, SHC nodes, cloud nodes, and sandbox workers while preserving
deterministic benchmark artifacts.

Pylon responsibilities:

- lease benchmark-capable workers with declared capability envelopes;
- run isolated Terminal-Bench, SWE/SWT, custom-repo, and retained Probe
  fixtures;
- preserve per-rollout transcripts, patches, verifier output, cost, model,
  runtime version, and resource receipts;
- stream structured rollout results back to the optimizer coordinator;
- retry infrastructure failures without hiding model or agent failures;
- keep private task data and secrets out of optimizer traces.

The first large run should treat Pylon as a parallel metric-call engine for
GEPA:

```text
GEPA coordinator
  -> candidate text bundle
  -> Pylon batch assignment
  -> N benchmark rollouts
  -> normalized evaluator results
  -> GEPA reflection and candidate selection
  -> next candidate bundle
```

This lets us climb benchmark performance before changing weights or adding a
separate training stack.

### Terminology: Distributed Optimization, Not Weight Training

The honest label for this lane is `Pylon-distributed GEPA rollout
optimization` or `distributed benchmark-driven optimization with GEPA`.

It is defensible to call it distributed learning in the broad product sense:
the system improves an agent policy by repeatedly running evaluated rollouts
and keeping better candidate text. It is not distributed neural-network
training. Pylons are not computing gradients, updating model weights, merging
LoRA adapters, or advancing a checkpoint during the GEPA lane.

The unit of Pylon work is:

```text
run candidate text bundle C on benchmark task T
-> run Probe and the task verifier
-> preserve transcript, patch, verifier output, artifacts, resource receipt,
   and failure summary
-> return normalized evaluator side information
```

The GEPA coordinator can then use many independent Pylon results to update the
candidate frontier. The reflection/proposal step should stay centralized at
first on SHC, Psionic, or a hosted model. Pylons do not need to understand or
run the whole optimizer; they need to run bounded benchmark jobs honestly.

Reserve `distributed training` for later Psionic/Qwen/LoRA/SFT/DPO/GRPO lanes
where workers actually contribute to model-training, adapter, checkpoint,
gradient-like, or training-data-generation work. The GEPA lane can create the
clean traces and labels that make those later training lanes worth opening.

### 2026-06-08 Pylon Assignment Update

OpenAgents product surface now has a live Pylon assignment lease and closeout path. A registered,
wallet-ready Pylon can list owned assignments, accept an assignment, report
progress, submit public-safe artifact/proof refs, and have an operator close
the assignment as accepted or rejected work. The #502 production smoke
`assignment.public.issue502.20260608024927` reached `accepted_work`.

That changes the GEPA implementation plan. The first Pylon-backed metric-call
batches should reuse the OpenAgents product surface assignment lease lifecycle:

```text
GEPA coordinator
  -> OpenAgents product surface/Pylon assignment lease
  -> worker accept
  -> progress events
  -> artifact/proof refs
  -> evaluator/operator accepted-or-rejected closeout
  -> GEPA result import
```

Do not create a parallel Pylon work-state protocol for benchmark batches unless
the existing lease path proves inadequate. The lease path still does not prove
real bitcoin payout, payout-target approval, repeated multi-host jobs, or
general earning readiness. The current Pylon release freeze remains active
until the payout, repeated-smoke, failure-drill, and release-promotion gates
close.

## Evaluation Dataset

Start with three lanes and keep their claims separate.

### Lane 1: Retained Terminal-Bench Failures

Use the existing retained failures as the fast inner loop:

- `configure-git-webserver`
- `db-wal-recovery`
- `filter-js-from-html`
- `gcode-to-text`
- `pypi-server`
- `query-optimize`

This lane should be cheap and high-signal, but it is not a public benchmark
claim. Its job is to catch obvious regressions and train playbooks for known
failure families.

### Lane 2: Terminal-Bench Generalization

Use Terminal-Bench through the public benchmark-cloud target, with a
fixed split:

- train: short and medium tasks suitable for high rollout volume;
- validation: held-out tasks used for GEPA candidate selection;
- test: frozen held-out task set used only after candidate selection.

The GEPA TerminalBench example in `projects/repos/gepa` uses train, validation,
and test slices from `terminal-bench-core`. We should preserve that shape but
move split authority into benchmark-cloud so results are replayable
and not silently reshuffled.

### Lane 3: Repository-Specific Coding Tasks

Use SWE-smith-style or SWT/SWE-style generated tasks for repos that matter to
our own coding-agent work:

- `probe`
- `openagents`
- `openagents`
- `psionic`
- `tap-ldk` when Rust protocol work becomes a target

This is where gskill's repository-skill lesson maps directly to OpenAgents:
learn repo-specific working agreements and debugging strategies, but deploy
them as Probe/Blueprint candidate text rather than as unmanaged prompt files.

## Fitness Function

The evaluator should return both a scalar score and structured side
information. GEPA benefits from actionable side information, and Probe needs
receipts either way.

Primary score:

```text
score = verified_success
```

where `verified_success` is the benchmark verifier result, not a model judge.

Tie-breakers and penalties:

- lower wall-clock duration;
- lower token and dollar cost;
- fewer risky shell actions;
- smaller patch diff when tests pass;
- complete closeout evidence;
- no policy violations;
- no unsafe approval bypass;
- no degraded performance on retained safety or invariant fixtures.

A candidate that improves benchmark reward but bypasses policy gets score zero
for promotion. The optimizer may learn from the trace, but the candidate cannot
advance.

Structured side information per rollout:

```text
task_id
dataset
split
agent_slug
model
candidate_hash
selected_signatures
tool_menu
commands_summary
patch_summary
verifier_status
failure_family
stdout_tail_ref
transcript_ref
artifact_manifest_ref
resource_usage_receipt_ref
policy_findings
```

The reflection prompt should see concise failure summaries and refs, not raw
private logs by default.

## First Run Shape

Run a staged GEPA campaign instead of one giant undifferentiated sweep.

### Stage 0: Smoke

- 3 to 6 retained fixtures.
- 2 workers.
- 20 to 40 metric calls.
- goal: verify harness, artifact writing, candidate hashing, and resumability.

### Stage 1: Retained Failure Sprint

- retained Terminal-Bench failure families plus local Probe acceptance cases.
- 8 to 16 Pylon workers.
- 200 to 400 metric calls.
- goal: learn a candidate text bundle that reliably repairs known failure
  families without breaking Probe runtime policy.

### Stage 2: Terminal-Bench Validation Run

- fixed Terminal-Bench train and validation split.
- 16 to 64 Pylon workers, depending on available node leases.
- 800 to 2,000 metric calls.
- goal: produce a candidate that beats the current Probe+Codex baseline on
  validation reward, cost, and duration.

### Stage 3: Frozen Holdout

- frozen Terminal-Bench test split.
- no GEPA reflection during this stage.
- run baseline and candidate under identical retry, model, timeout, and worker
  policies.
- goal: decide whether the candidate becomes a shadow Blueprint candidate or
  active release candidate.

### Stage 4: Repository-Specific Skills

- run gskill-style optimization over `probe`, `openagents`, and
  `openagents`.
- output should be converted into typed Blueprint candidate components, not
  pasted directly into global agent instructions.

## Promotion Contract

GEPA output should enter a candidate registry with lineage and evidence:

```text
candidate_id
parent_candidate_id
gepa_run_id
candidate_hash
component_hashes
dataset_splits
baseline_result_refs
validation_result_refs
holdout_result_refs
policy_gate_result
promotion_state
```

Promotion states:

```text
draft
optimizer_accepted
shadow
release_candidate
active
rejected
reverted
```

`optimizer_accepted` means GEPA found a better candidate on its configured
metric. It does not mean Probe should use it in production. `shadow` means the
candidate can run beside active policy and emit comparison evidence.
`release_candidate` means the candidate passed held-out benchmark gates and
policy gates. `active` requires the normal Blueprint/OpenAgents product surface release path.

## Implementation Path

`docs/benchmarks/plan.md` is the source of truth for issue creation and
execution order. This GEPA run plan remains the rationale for the optimization
lane, but the work should be opened and sequenced through the issue series in
`plan.md`.

The issue order is:

1. Probe closeout foundation: assignment/closeout schemas, normalized closeout
   writer, and retained coding fixtures.
2. Public Benchmark Cloud contracts: public contract package, Terminal-Bench
   split manifests, and true-Probe runner lane.
3. GEPA candidate optimization: Psionic candidate manifests, GEPA coordinator,
   and Probe candidate execution adapter.
4. Pylon work slices and paid-work path: OpenAgents product surface/Pylon metric-call assignments,
   worker capability envelopes, and explicit paid/unpaid/credit/no-spend
   modes.
5. Stage 0 and Stage 1 campaign: retained-fixture smoke and retained-failure
   sprint through Pylon metric-call batches.
6. SHC validation: selected Terminal-Bench validation sweep after Stage 1.
7. Artanis/public projection and route scorecards: public campaign projection,
   Forum summaries, route scorecards, and product accepted-outcome metrics.

The priority unlock sequence is Probe closeout bundle, public Benchmark Cloud
split manifest, GEPA candidate manifest, Pylon metric-call assignment type,
Stage 0 smoke, and Stage 1 retained-failure sprint.

## Why GEPA Only First

GEPA is the lowest-friction way to improve the existing system because it
optimizes the artifacts we already have: prompts, skills, playbooks,
instructions, and text policies. It can use black-box benchmark verifiers as
fitness. It does not require training infrastructure, model checkpoint custody,
adapter merge policy, or GPU-heavy backprop before we know which behaviors are
worth training into a model.

If the GEPA lane works, it creates training data for later methods:

- successful and failed rollouts;
- candidate diffs;
- task-family labels;
- signature-selection evidence;
- closeout evidence quality;
- cost and duration tradeoffs.

That data can later feed Psionic LoRA, DPO, or GRPO lanes. Starting with GEPA
keeps the first loop explainable and auditable.

## Public Claim Boundary

Do not publish "we beat Terminal-Bench" from the first GEPA run. Publish only
bounded claims:

- exact dataset and version;
- exact split;
- exact agent slug and model;
- exact baseline commit and candidate hash;
- exact retry and timeout policy;
- verifier result;
- cost and duration;
- artifact availability;
- redaction state;
- whether the run was retained, validation, or frozen holdout.
- Forum topic and post refs when Artanis or another registered agent publishes
  a public-safe campaign summary.

The correct first public claim is narrower:

> Pylon ran a GEPA optimization campaign over Probe/Blueprint text candidates,
> using benchmark verifier feedback and preserved proof bundles, and the
> selected candidate improved a fixed validation split without policy-gate
> failures.

Only frozen holdout evidence can support stronger benchmark claims.

Artanis Forum publication is a projection step, not the authority behind the
claim. Probe can emit public-safe summary input and can reply as its own
registered Forum identity when it has a valid OpenAgents agent token. Posting
as Artanis or invoking the Artanis Nexus/Pylon Forum bridge remains
operator/internal authority unless OpenAgents exposes a future scoped grant.

## Open Questions

- Should the first GEPA coordinator live in Psionic, OpenAgents Benchmark
  Cloud, or a separate benchmark-tools package?
- Which model should be the reflection proposer for the first budgeted run?
- Should Terminal-Bench train/validation/test split authority live in
  `openagents` before the first run, or can Stage 0 and Stage 1 use a local
  manifest while public contracts are being built?
- What is the first active Blueprint component allowed to receive a shadow
  candidate?
- What cost ceiling do we want for Stage 2 before moving to frozen holdout?

## Recommendation

Start immediately with Stage 0 and Stage 1:

- GEPA only.
- text-bundle candidate only.
- retained Terminal-Bench failures and Probe acceptance fixtures only.
- Pylon used as parallel rollout infrastructure.
- Pylon batches should use the OpenAgents product surface assignment lease lifecycle and closeout
  semantics.
- no model fine-tuning.
- no public leaderboard claim.

If Stage 1 produces a stable candidate and the proof bundles are clean, spend
the larger budget on Stage 2 Terminal-Bench validation. Use the resulting
candidate as the first shadow Blueprint optimization candidate for Probe.
