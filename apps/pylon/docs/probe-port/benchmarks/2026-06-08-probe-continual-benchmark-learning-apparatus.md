# Probe Continual Benchmark Learning Apparatus

Date: 2026-06-08

## Intended End State

Probe should continuously improve as a coding agent by running real benchmark
workloads, turning failures into typed improvement candidates, and promoting
only candidates that pass measured gates. The benchmarking apparatus should be
public OpenAgents infrastructure. Terminal-Bench 2 through Harbor on the SHC box
is the first live benchmark lane, but the apparatus should be general enough for
legal, SWE, Probe retained fixtures, and future OpenAgents benchmark suites.

This is not a benchmark leaderboard script. It is a closed learning loop across
Probe, public benchmark-cloud, Psionic, Pylon, and OpenAgents product surface: Probe
executes and records the agent runtime truth, benchmark-cloud runs
normalized benchmark jobs, Psionic optimizes prompts, Blueprint usage, and
later LoRA adapters, Pylon supplies distributed devices for GEPA rollouts first
and training work later, and OpenAgents product surface publishes the operator-facing evidence and
release gates.

The existing private `cloud` repo is source material, not the desired long-term
home. Its Benchmark Cloud contracts, Harbor Terminal-Bench lane, runner
artifacts, proof bundles, and retained coding-agent fixtures should be moved or
rebuilt into the public `openagents` repo so the benchmark apparatus can be
audited, extended, and run by the wider OpenAgents ecosystem.

## Source Material Reviewed

- `probe/docs/benchmarks/2026-06-08-workspace-benchmark-systems-audit.md`
- `probe/docs/2026-06-07-blueprint-signature-lookup-apple-fm-tool-use-audit.md`
- `probe/docs/probe-blueprint-signature-lookup-service.md`
- `probe/docs/probe-blueprint-tool-menu-planner.md`
- `probe/packages/runtime/src/blueprint/signature-lookup.ts`
- `probe/packages/runtime/src/blueprint/tool-menu.ts`
- private `cloud/docs/BENCHMARK_CLOUD.md` source material
- private `cloud/docs/bootstrap/CND-054-coding-agent-benchmark-improvement.md`
  source material
- `openagents/AGENTS.md`
- `probe/docs/benchmarks/2026-06-08-pylon-gepa-coding-agent-benchmark-run.md`
- `work/docs/probe/17-gepa-optimize-anything-code-audit-and-psionic-probe-integration.md`
- `psionic/docs/QWEN_LEGAL_FINETUNE_LANE.md`
- `psionic/reports/qwen36-27b-real-pylon-rehearsal-001.md`
- `psionic/reports/qwen36-27b-real-lora-sft-actual-20260522T043404Z/README.md`
- `openagents/docs/nexus/2026-06-07-pylon-agent-api-runbook.md`
- `openagents/docs/nexus/2026-06-07-pylon-network-readiness-release-freeze.md`
- `openagents/docs/nexus/2026-06-08-pylon-live-assignment-closeout-smoke.md`

## Current Assets We Can Build On

### Probe

Probe is already shaped as a Bun and Effect v4 coding-agent runtime. The old
Probe implementation is deprecated source material, not the future surface. The
current runtime has a typed Blueprint signature lookup service and a tool-menu
projection layer. That matters because benchmark improvement should not become
ad hoc prompt tweaking or string matching. Every benchmark run should record the
selected Program Signatures, tool menu, backend, prompt candidate, tool results,
and closeout evidence as first-class runtime data.

The current Blueprint-related docs already define the direction:

- no Probe-only signature registry;
- lookup against OpenAgents product surface/OpenAgents Blueprint authority;
- backends get projected tool menus from the same signature lookup path;
- Apple FM, Codex-style remote models, local Qwen, swarm inference, and Pylon
  workers should all consume the same signature registry concepts;
- unsafe registry mutation, missing context, failed release gates, unsupported
  surfaces, and excessive risk ceilings are blocked before tool projection.

That gives Probe the right base for benchmark learning. The learning system
should optimize which signatures, playbooks, prompt fragments, and tool-menu
constraints are used for a task family, but it should not bypass the typed
Blueprint selection path.

### public benchmark-cloud

The private `cloud` repo already has the canonical Benchmark Cloud design. That
design should become public OpenAgents infrastructure. The target is a public
Benchmark Cloud area in `openagents`, with private `cloud` used only as
backfill until the public contracts, runners, fixtures, and runbooks exist.

A reasonable public target layout is:

- `openagents/docs/benchmarks/` for specs, runbooks, audits, and claim
  boundaries.
- `openagents/crates/benchmark-cloud/` for normalized benchmark
  contracts, artifact manifests, proof bundles, and scheduler-facing Rust code.
- `openagents/scripts/benchmarks/` for SHC, Harbor, and local proof commands.
- `openagents/fixtures/benchmarks/` for public retained fixtures and safe
  replay inputs.
- `openagents/proto/openagents/benchmark/` only if cross-language protocol
  stability becomes necessary.

The public system should keep the private Cloud design's useful normalized
contracts:

- `BenchmarkTask`
- `BenchmarkResult`
- `BenchmarkEvent`
- `BenchmarkArtifactManifest`
- `BenchmarkProofBundle`
- `openagents.resource_usage_receipt.v1`

The current private Python runner lane writes:

- `result.json`
- `events.jsonl`
- `metadata.json`
- `artifact_manifest.json`
- `proof_bundle.json`
- `resource_usage_receipt.json`

Terminal-Bench 2 is the first adapter, and Harbor is the wrapper path. The SHC
box `oa-shc-katy-01` already produced useful preserved evidence:

- 1-task `openssl-selfsigned-cert`: reward `1.0`, verifier `6/6`, cost
  `$0.120667`.
- 8-task subset: `6/8`, mean `0.75`, cost `$3.697649`.
- 16-task preserved subset: `11/16`, mean `0.6875`, cost `$13.300340`.

The retained failure set is more useful than the wins. Current retained
Terminal-Bench failures include:

- `configure-git-webserver`
- `db-wal-recovery`
- `filter-js-from-html`
- `gcode-to-text`
- `pypi-server`
- `query-optimize`

The private CND-054 work also contains a retained signature-improvement
evaluator. It reports an expected retained-fixture improvement from raw mean
`0.000` to Probe plus selected signatures mean `0.900`, with the caveat that
this is a retained fixture signal, not a fresh live Terminal-Bench run. The
live, account-backed evidence is narrower: `db-wal-recovery` has a preserved
0.0 to 1.0 improvement after playbook revision.

That distinction should be preserved in every future report.

### Psionic

Psionic already has the right training substrate for this apparatus. The legal
fine-tune lane proves these pieces exist in owned code:

- Rust command surface for SFT, DPO, GRPO, adapter merge, adapter registration,
  promotion, replay, and integrity verification.
- Public Harvey legal benchmark runs with champion/candidate scoring.
- Qwen3.6-27B and Qwen3.6-35B target-path smokes.
- Signed two-node Pylon dispatch rehearsal.
- Real-checkpoint Qwen3.6-27B bounded full-layer LoRA SFT over frozen base
  weights.
- Adapter export to `safetensors`.
- Promotion gates based on public deterministic evaluation.

The Qwen3.6-27B real LoRA report is especially relevant. It is honest about
the boundary: actual real-checkpoint bounded LoRA SFT occurred, but it did not
prove private Harvey performance or full transformer backprop through every
layer. That same standard should apply to Probe benchmark learning. We can
train and evaluate Qwen adapters for coding-agent behavior, but every claim
must say exactly what path ran and what it did not prove.

The MLX-related Psionic work should be used carefully. It gives us a path for
Apple Silicon local training and MLX-class adapter work, but it does not mean
Apple Foundation Models can be directly fine-tuned. The first correct claim is:
Apple FM can be a local backend that consumes upfront tool definitions, while
MLX-style Apple Silicon LoRA training can improve local/open models that run
beside Apple FM in the same Probe surface.

### GEPA Audit

The workspace GEPA audit reached the right ownership split:

- Probe owns runtime truth, transcripts, tools, approvals, replay exports,
  decision exports, acceptance evidence, and runtime candidate adoption.
- Psionic owns the optimizer substrate, candidate manifests, frontier tracking,
  lineage, merge, minibatch gates, structured feedback, and optimizer receipts.

Probe should not embed GEPA's Python runtime. The useful parts to port are the
ideas:

- explicit candidate components;
- typed evaluator results;
- shared and per-component reflective feedback;
- minibatch-gated mutation;
- full retained evaluation before promotion;
- frontier tracking;
- lineage-aware merge;
- resumable state and evaluation cache;
- callbacks as durable receipts.

The parts to avoid are also clear:

- raw dictionary candidate models;
- pickle or cloudpickle persistence;
- hidden synthetic params;
- stdout capture as feedback;
- conflating optimizer acceptance with runtime promotion.

The Pylon GEPA run plan now narrows the first optimizer milestone. Before LoRA,
GRPO, or broad model-training work, run a GEPA-only campaign over structured
text bundles: Probe prompts, Blueprint usage instructions, Program Signature
playbooks, tool-menu policy, failure-family playbooks, and closeout policy.
Pylon supplies parallel rollouts, and public benchmark-cloud supplies
split authority and artifact contracts. LoRA and Qwen adapter work should follow
only after that lane produces clean traces and split-aware evidence.

## Proposed System Shape

### Responsibilities

Probe should own:

- executing the coding-agent run;
- selecting and recording backend, prompt candidate, Blueprint Program
  Signatures, tool menu, and context pack;
- streaming tool calls and tool results;
- writing replayable transcripts and decision exports;
- producing benchmark closeout evidence for each run;
- admitting a candidate into shadow or active runtime only after external gates
  pass.

public benchmark-cloud should own:

- Terminal-Bench 2 through Harbor;
- benchmark task normalization;
- SHC and remote runner scheduling;
- artifact and proof bundle collection;
- resource usage receipts;
- live benchmark run custody.

Private `cloud` should own nothing long term. It can provide source material
while we rebuild the apparatus in public.

Psionic should own:

- GEPA-style optimizer jobs;
- prompt and Blueprint candidate manifests;
- Qwen3.6 LoRA, SFT, DPO, and GRPO training jobs;
- adapter merge and registration;
- distributed Pylon worker receipts;
- optimizer frontier and lineage state;
- promotion reports for model and prompt candidates.

Pylon should own:

- device enrollment;
- worker capability advertisement;
- owned assignment lease acceptance and progress events;
- public-safe artifact/proof refs;
- accepted or rejected closeout receipts;
- signed job acceptance and completion receipts;
- local/swarm compute availability;
- payment and settlement evidence when jobs are compensated.

OpenAgents product surface should own:

- Blueprint authority and release gates;
- operator-facing benchmark evidence;
- candidate approval and promotion UI;
- public/private claim projection;
- cross-repo issue and workroom tracking.

### Core Loop

The continual learning loop should run as:

1. Select tasks.
   Pull from Terminal-Bench 2 via Harbor first, plus retained failure fixtures
   and later legal/SWE/generalization suites.

2. Create assignments.
   public benchmark-cloud emits benchmark assignments with task
   metadata, budget, backend constraints, required artifact names, and
   evaluation policy. When a task is routed through Pylon, that assignment
   should map to the OpenAgents product surface Pylon assignment lease lifecycle rather than a
   separate benchmark-only state machine.

3. Run Probe variants.
   Probe executes raw baseline, current champion, and candidate variants under
   controlled conditions. Variants should include API-key backends,
   Codex-style backends, Apple FM where available, local Qwen, and Pylon/swarm
   inference when admitted.

4. Record decision truth.
   Probe records the visible context before each major decision, selected
   signatures, projected tool menu, backend, tool calls, tool results,
   verifier-facing outputs, and closeout evidence.

5. Score and classify.
   public benchmark-cloud scores the benchmark. Probe and Psionic
   classify failures into typed families, not keyword buckets.

6. Generate candidates.
   Psionic proposes prompt, Blueprint, tool-menu, retry-policy, and LoRA
   candidates from the scored traces.

7. Gate candidates.
   Candidates pass fast local tests, retained fixtures, selected SHC live
   sweeps, broader benchmark sweeps, and cross-suite regression checks.

8. Promote or reject.
   OpenAgents product surface approves promotion. Probe can shadow a candidate before it becomes
   the default runtime behavior.

9. Preserve lineage.
   Every candidate keeps parent pointers, training data digest, benchmark
   split membership, artifact digest, receipts, and exact promotion decision.

## Terminal-Bench 2 Through Harbor On SHC

Terminal-Bench 2 should be the first live apparatus because it already catches
the behaviors Probe needs to get right:

- shell command planning;
- filesystem edits;
- service readiness;
- package and dependency setup;
- parser correctness;
- evidence closeout;
- long-horizon recovery after failed commands;
- avoiding false completion.

The SHC box should be the primary testing environment for live Terminal-Bench
2 runs. Local developer machines can run smoke tests and retained fixture
checks, but SHC should be where candidate claims become meaningful live
benchmark evidence. The SHC commands, Harbor wrapper, normalized artifacts, and
safe retained fixtures should be public OpenAgents assets, even if some raw
machine credentials and live account material remain private.

The first SHC lane should not start as a full benchmark sweep. It should start
with retained known failures and a few known successes:

- `db-wal-recovery`
- `configure-git-webserver`
- `pypi-server`
- `filter-js-from-html`
- `gcode-to-text`
- `query-optimize`
- `openssl-selfsigned-cert`

The first goal is not a new public leaderboard claim. The first goal is to
make Probe's current champion and candidate variants reproducible under the
public benchmark-cloud artifact contract.

## Candidate Families

### Prompt Candidates

GEPA-style prompt candidates should mutate explicit prompt components only:

- system instruction module;
- coding-agent loop instruction;
- benchmark closeout instruction;
- shell safety and retry policy;
- evidence checklist;
- failure-family playbook references;
- backend-specific formatting adapter.

Each prompt candidate should be a manifest, not loose text:

```json
{
  "kind": "probe.prompt_candidate.v1",
  "id": "prompt.terminal_bench.service_readiness.001",
  "parentId": "prompt.champion.current",
  "targetSuites": ["terminal_bench_2_retained"],
  "components": [
    {
      "name": "service_readiness_closeout",
      "version": "candidate.001"
    }
  ],
  "trainingTraceDigests": [],
  "expectedFailureFamilies": ["service_readiness"],
  "promotionStatus": "candidate"
}
```

### Blueprint Candidates

Blueprint candidates should not rewrite the registry from Probe. They should
propose new or revised Blueprint module versions through OpenAgents product surface's Blueprint
authority. The candidate should say:

- which Program Signature family it targets;
- which task family justified it;
- which traces were used;
- what tool-menu projection changes;
- what safety/risk constraints apply;
- which retained and live benchmark gates it passed.

The optimizer may suggest a new module version, but only OpenAgents product surface can accept it as
Blueprint authority.

Candidate examples:

- `coding.service_readiness`
- `coding.sqlite_wal_recovery`
- `coding.python_package_index`
- `coding.gcode_parser_guard`
- `coding.query_optimizer_workflow`
- `benchmark.runner_supervisor`

### Tool-Menu Candidates

Probe's signature lookup and tool-menu projection should be optimized as a
typed product surface. Candidate changes can include:

- adding a required shell tool for a benchmark family;
- removing a tool that causes failures or unsafe behavior;
- changing upfront tool schema projection for Apple FM;
- changing when a backend receives a compact tool menu versus a full one;
- adding an evidence tool requirement for service or verifier readiness.

Tool-menu optimization must remain backend-neutral. Apple FM has a special
constraint because tool definitions must be known up front, but the registry
lookup path should also serve Codex-style, Qwen, local, and swarm backends.

### Retry And Continuation Candidates

Many benchmark failures are not model-knowledge failures. They are loop-control
failures: stopping too early, failing to inspect logs, not checking service
readiness, or not rerunning a verifier after a repair. Candidate policies
should therefore cover:

- when to continue after a failing command;
- when to inspect logs;
- when to run a minimal verifier before final answer;
- when to summarize uncertainty instead of closing;
- when to escalate a missing dependency versus install or build it.

These policies should be represented as typed runtime candidates and evaluated
against retained traces before they are used in live SHC runs.

### LoRA And Model Candidates

The model-improvement lane should start with Qwen3.6 adapters through Psionic,
because Psionic already has real Qwen3.6 LoRA and distributed Pylon training
machinery. The first coding-agent adapter work should use public and
self-generated benchmark traces only.

Useful training data shapes:

- SFT positives from successful Probe traces;
- DPO pairs from raw failed runs versus improved runs;
- reward traces from verifier outcomes and tool-result receipts;
- failure-family examples from retained Terminal-Bench tasks;
- Blueprint-selection examples from signature lookup decisions.

No hidden verifier content, private benchmark tasks, production secrets, or
account tokens should enter training data. Split membership and trace digests
must be recorded so future reports can distinguish training, validation,
retained, and fresh live evaluation.

MLX-style Apple Silicon fine-tuning can be part of the local training lane for
open/local models. It should not be described as Apple FM fine-tuning unless an
Apple-supported API actually exists. Apple FM can still be a first-class Probe
backend for local tool use, and MLX-trained Qwen or other local adapters can
run beside it in the same multi-inference surface.

## Multi-Inference Strategy

Probe should present one agent surface while routing inference across multiple
admitted strategies:

- API-key models for strong remote baselines and fallback.
- Codex-style coding backends for high-quality agentic coding.
- Apple FM for local Apple-device tool use where its constraints fit.
- Qwen3.6 local adapters for owned local inference.
- Swarm/local compute coordinated by Pylon.
- Psionic-trained candidates for LoRA and policy improvement.

The benchmark apparatus should compare these strategies without forking the
agent product. The common layer is the Probe runtime transcript, Blueprint
signature lookup, tool-menu projection, and benchmark closeout contract.

## Pylon And Around-The-Clock Distributed Runs

Pylon should let many user-owned or OpenAgents-owned devices contribute
benchmark and training work around the clock. The device pool should include
SHC boxes, Apple Silicon Macs, local developer machines, and eventually other
admitted worker classes.

OpenAgents product surface's #502 live assignment closeout smoke is now the concrete lifecycle to
reuse for Probe benchmark batches. The proved flow is:

```text
registered wallet-ready Pylon
-> owned assignment lease
-> accept
-> progress
-> public-safe artifact/proof refs
-> operator/evaluator closeout as accepted or rejected work
-> post-closeout no-spend payment-evidence refs when unpaid
```

That is strong enough to design GEPA metric-call batches around. It is not
strong enough to claim Pylon earning readiness or autonomous paid benchmark
work. The #499 release freeze still blocks broad Pylon release/download/earning
claims until real payout, repeated multi-host jobs, failure drills, and release
promotion are proven.

A Pylon worker should advertise capabilities, not vague availability:

- benchmark runner support;
- Harbor/Terminal-Bench support;
- local model support;
- Apple FM support;
- Qwen adapter training support;
- MLX-class local training support;
- disk, memory, and accelerator constraints;
- maximum cost/time budget;
- proof and receipt support.
- assignment lease and closeout support.

Psionic should dispatch training and evaluation shards through Pylon, collect
signed receipts, merge adapters, and submit promotion candidates. Probe should
not trust a remote candidate merely because a worker ran it. It should trust
the candidate only after replayable artifacts, benchmark results, and OpenAgents product surface
promotion gates agree.

## Data Model

Probe and Psionic should converge on typed records like:

- `probe.benchmark_assignment.v1`
- `probe.benchmark_run.v1`
- `probe.benchmark_decision_trace.v1`
- `probe.benchmark_failure_classification.v1`
- `probe.prompt_candidate.v1`
- `probe.blueprint_candidate.v1`
- `probe.tool_menu_candidate.v1`
- `probe.loop_policy_candidate.v1`
- `probe.lora_adapter_candidate.v1`
- `probe.benchmark_promotion_decision.v1`

The benchmark run record should include:

- task id and suite;
- benchmark split membership;
- backend;
- model or adapter id;
- prompt candidate id;
- Blueprint signature refs;
- tool menu id;
- tool call/event stream digest;
- artifact manifest digest;
- resource usage receipt digest;
- score;
- verifier status;
- failure classification;
- candidate lineage;
- promotion status.

## Failure Taxonomy

The first failure taxonomy should be typed and compact:

- `service_readiness`
- `database_recovery`
- `parser_correctness`
- `package_indexing`
- `query_optimization`
- `runner_supervision`
- `dependency_installation`
- `tool_result_misread`
- `premature_closeout`
- `context_selection`
- `blueprint_signature_missing`
- `tool_menu_missing`
- `backend_formatting`
- `budget_exhaustion`

This taxonomy should be embedded-driven or model-planned where routing is
semantic. Deterministic parsing is acceptable only after the route is selected,
for bounded fields like task id, benchmark id, enum value, score, or digest.

## Promotion Gates

A candidate should move through these gates:

1. Static validity.
   Manifest parses, references exist, no unsafe registry mutation, no missing
   artifact digests, no secret leakage.

2. Probe unit and fixture tests.
   Bun/Effect tests pass. Retained Probe fixture evaluator passes.

3. Retained Terminal-Bench replay.
   Candidate improves or preserves retained failure tasks without regressions.

4. Selected SHC live sweep.
   Candidate runs on SHC through Harbor for the selected Terminal-Bench subset.

5. Broader live sweep.
   Candidate runs a larger Terminal-Bench sample with cost and time budget
   controls.

6. Cross-suite regression.
   Candidate is checked against legal, SWE, and Probe internal fixtures when
   relevant.

7. Shadow deployment.
   Probe can run the candidate in shadow mode and collect evidence without
   making it default.

8. OpenAgents product surface promotion.
   OpenAgents product surface records operator approval, release gate status, claim text, and public
   or private projection.

GEPA acceptance is not runtime promotion. LoRA eval improvement is not runtime
promotion. Pylon completion is not runtime promotion. Promotion requires the
whole gate chain.

## Operating Schedule

The first around-the-clock schedule should be simple:

- Every commit: Probe Bun/Effect tests and retained local fixtures.
- Hourly: retained Terminal-Bench failure replay where local resources permit.
- Daily until Stage 1 passes: GEPA-only retained failure sprint over text-bundle
  candidates.
- Nightly: SHC selected Harbor sweep.
- Daily: Psionic candidate generation over fresh failures.
- Daily after GEPA Stage 1 is stable: Qwen adapter training or DPO/GRPO job if
  enough new clean traces exist.
- Weekly: broader SHC Terminal-Bench sweep and candidate frontier review.

The scheduler should prioritize cheap information first. Do not spend SHC or
Pylon training budget on candidates that fail local manifest validity or the
retained fixture set.

## First Implementation Sequence

`docs/benchmarks/plan.md` is the source of truth for the first implementation
sequence and GitHub issue creation. This apparatus document describes the
target learning loop; `plan.md` decides how to break it into executable work.

The current issue sequence is:

1. Probe benchmark closeout foundation.
2. Public Benchmark Cloud contracts.
3. GEPA candidate optimization.
4. Pylon work slices and paid-work path.
5. Stage 0 and Stage 1 campaign.
6. Artanis and public projection.
7. Route scorecards and product impact.

The current priority unlocks are:

1. Probe closeout bundle.
2. Public Benchmark Cloud split manifest.
3. GEPA candidate manifest.
4. Pylon metric-call assignment type.
5. Stage 0 smoke.
6. Stage 1 retained-failure sprint.

Do not open every issue in Probe. Follow the ownership model in `plan.md`:
Probe owns runtime and closeout work, OpenAgents owns public Benchmark Cloud,
Psionic owns GEPA frontier/coordinator work, OpenAgents product surface owns projection/release
gates, and Pylon/OpenAgents own worker envelopes and benchmark work slices.

## Immediate Milestone

The first useful milestone is the GEPA-only Stage 0 and Stage 1 lane:

- public benchmark-cloud has a split manifest for retained
  Terminal-Bench failures and local Probe acceptance fixtures;
- Probe can run a supplied text-bundle candidate and emit normalized evaluator
  side information;
- Pylon can run 20 to 40 smoke metric calls, then 200 to 400 retained-failure
  metric calls;
- candidate hashes, rollout receipts, policy findings, artifact manifests, and
  resource receipts are preserved;
- no candidate is promoted automatically;
- no LoRA or model-training work is part of this first milestone.

The first live SHC milestone after that is:

- Probe runs `db-wal-recovery`, `configure-git-webserver`, and
  `pypi-server` through Harbor on SHC.
- The run writes normalized benchmark artifacts.
- The current champion and one prompt/Blueprint candidate are compared.
- The candidate's selected signatures, tool menu, prompt manifest, and failure
  classification are preserved.
- A Psionic GEPA-style optimizer run proposes the next candidate from those
  traces.
- No candidate is promoted automatically.

The second milestone is:

- Psionic trains a small Qwen3.6 LoRA coding-agent adapter from successful
  public Probe traces and DPO pairs.
- The adapter is evaluated against retained Terminal-Bench fixtures.
- If it passes retained gates, it is run in a selected SHC Harbor sweep.
- The promotion report states exactly which model path ran, whether MLX-class
  local training was used, which split was used for training, and what fresh
  live evidence exists.

## Non-Negotiable Boundaries

- Do not train on hidden verifier content.
- Do not train on private benchmark tasks unless the benchmark owner explicitly
  permits it and the split is recorded.
- Do not store secrets, API keys, tokens, or private production data in traces.
- Do not use ad hoc string matching for semantic routing, benchmark failure
  classification, or tool selection.
- Do not let Probe mutate Blueprint authority directly.
- Do not self-promote candidates from optimizer output.
- Do not describe retained fixture improvement as fresh live benchmark
  performance.
- Do not describe MLX-class local model fine-tuning as Apple FM fine-tuning
  unless Apple exposes and permits that exact operation.

## What This Gives Probe

This apparatus turns benchmark work from occasional manual audits into a
measured improvement engine. Terminal-Bench failures become typed training and
prompt-improvement examples. Blueprint usage improves through evidence rather
than intuition. Qwen adapters improve on clean traces and are gated before
runtime use. Pylons supply around-the-clock compute. SHC provides the first
serious live benchmark environment. OpenAgents product surface keeps claims honest and promotion
operator-controlled.

The product result should be simple for users: they ask Probe to solve coding
tasks, and Probe gets better over time because every benchmark run, failure,
candidate, and promotion is recorded in the same OpenAgents infrastructure that
will eventually deploy probes from forum threads, sandboxes, SHC boxes, and
user-owned Pylon devices.
