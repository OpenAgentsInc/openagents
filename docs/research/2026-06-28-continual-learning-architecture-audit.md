# OpenAgents Continual-Learning Architecture Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-28
Status: research audit / architecture synthesis, not a product promise
Scope: continual learning for OpenAgents, Autopilot, Khala, Artanis, Probe,
Pylon, Tassadar, Blueprint, GEPA, StudyBench, traces, receipts, and operator
observability.

This document is public-safe. It cites local repository paths and public issue
refs, names current surfaces versus future work, and does not claim that any
future capability is live.

## Executive Summary

For OpenAgents, "continual learning" should not mean a model silently updates
itself after every chat. The durable local architecture points at a narrower and
more auditable definition:

```text
executed work
-> public-safe traces, exact token rows, receipts, verdicts, and closeouts
-> StudyBench / benchmark / accepted-outcome evaluation
-> GEPA / DSPy-style candidate optimization or RLM decomposition candidates
-> Blueprint release gates and product-promise gates
-> promoted modules, study packets, context packs, policies, or operator playbooks
-> monitored deployment with rollback, attribution, and staleness controls
```

The repository already has many of the substrate pieces: ATIF trace storage,
`token_usage_events`, Blueprint program-run evidence, StudyBench contracts and
comparison results, Probe GEPA candidate manifests, Tassadar exact replay,
Artanis public-report/GEPA gates, product-promise discipline, and docs for
Khala-on-Blueprint session capture. The missing piece is an end-to-end
promotion loop that reconciles all of them into one governed learning lane.

The recommended target is a five-stage continual-learning pipeline:

1. **Data**: capture accepted and rejected work as exact token rows, redacted
   owner/public-safe ATIF traces, raw-private archives where allowed, receipts,
   route scorecards, and source refs.
2. **Eval**: score work through StudyBench, Terminal-Bench/Harbor, hidden
   validation, product-promise gates, exact replay, accepted-outcome receipts,
   and rate/capacity telemetry.
3. **Optimize**: run GEPA/DSPy-like offline compile jobs and RLM decomposition
   experiments against fixed eval splits, producing candidates only.
4. **Promote**: admit candidates through Blueprint release gates,
   contribution/review gates, product-promise gates, privacy/tripwire checks,
   and operator approval.
5. **Govern**: observe drift, rate limits, reset windows, cost, accepted
   outcomes per kWh, staleness, settlement, and rollback health before any
   candidate becomes public copy or active runtime.

## Transcript 232-234 Extraction

The issue asked for transcripts 232, 233, and 234 to be re-read in full. The
continual-learning content is uneven but useful.

### Transcript 232: accepted outcomes per kWh

`docs/transcripts/232.md` is primarily about the energy economics of AI
inference. The reusable design fragments are:

- Inference should be split into fast "answer inference" and flexible
  "agentic inference." Agentic inference is more schedulable and likely to
  dominate long-running machine work.
- The system should optimize "electrons in, orchestration, tokens out," not
  just "electrons in, tokens out." That means the scheduler sees workload
  urgency, energy price, supply state, and verification value at the same time.
- The OpenAgents metric proposed there is **accepted outcomes per kilowatt
  hour**: not tokens burned, but the cost of turning energy into accepted agent
  work.

Continual-learning implication: learning should optimize accepted outcome rate
per unit of compute/energy, not model fluency or token volume. The learning
loop needs cost and capacity telemetry in the same record family as acceptance,
verdict, and trace evidence.

### Transcript 233: monorepo consolidation

`docs/transcripts/233.md` says the launch-week architecture is consolidated in
the public `openagents` Bun/Effect monorepo, while Psionic remains separate as
the Rust ML library. This matters operationally: the public control-plane docs,
Worker routes, Effect Schema contracts, Probe/Pylon code, and product promise
records should be the governed learning loop's source of truth.

Continual-learning implication: do not hide the learning architecture in an
untracked side repo or a private prompt collection. Candidate evidence,
promotion gates, and public-safe audit records belong in the monorepo or in
explicitly referenced private operator stores with public checksum refs.

### Transcript 234: agent-verifiable product truth

`docs/transcripts/234.md` introduces Product Promises as a way for agents to
ask what is actually live and what remains red/yellow/withdrawn. It also says
agents can be pointed at recent transcripts to produce promise audits and then
turn gaps into coding work or bounties.

Continual-learning implication: product truth is part of learning. If a new
candidate improves a benchmark but has no product-promise evidence, it has not
become a product claim. The learning loop must feed `docs/promises/` before
public copy broadens.

## Current Surfaces

### Trace and token data

Current:

- `apps/openagents.com/INVARIANTS.md` defines the Agent Trace Store. It stores
  only public-safe ATIF-v1.7 projections in `agent_traces`, supports
  `public`, `unlisted`, and `owner_only` visibility, and treats traces as
  evidence only.
- The same invariant file defines default-on free-tier trace capture as
  gated, redacted-before-tripwire, private-by-default, fail-soft, and
  idempotent.
- Demand-origin segmentation on `agent_traces` and `token_usage_events`
  separates external, internal, own-capacity, and unlabeled traffic so the
  corpus can distinguish real users from dogfood and coding delegation.
- `token_usage_events` is the canonical served-token ledger, with migrations
  starting at `apps/openagents.com/workers/api/migrations/0137_token_usage_events.sql`
  and demand attribution in later migrations.

Gap:

- Trace rows and token rows are not yet presented as one first-class
  learning-dataset unit with shared task refs, accepted/rejected outcomes,
  route scorecards, cost, energy/capacity annotations, privacy tier, and
  promotion eligibility.
- Raw-private archives for Harbor/Pylon/Codex are intentionally separate from
  public ATIF traces. The learning architecture needs a reconciler that can
  say "raw private evidence exists, public-safe summary exists, exact token row
  exists" without copying raw material into public docs.

### StudyBench and machine studying

Current:

- `docs/research/machine-studying/README.md` defines OpenAgents StudyBench MVP
  boundaries: external StudyBench calibration, OpenAgents-owned row shapes,
  public-retained rows, private validation/holdout refs, Probe evidence,
  Psionic optimization, Tassadar verification, Blueprint gates, and
  product-promise boundaries.
- `docs/research/machine-studying/2026-06-17-studybench-openagents-benchmark-audit.md`
  records StudyBench row concepts: source-grounded tasks, weighted rubric
  claims, evidence spans, answer mode, and patch mode.
- `docs/research/machine-studying/2026-06-17-openagents-studybench-mvp-issue-roadmap.md`
  lists the MVP implementation status through MSB-MVP-15: contracts, corpus
  manifests, external calibration, public-retained rows, private boundaries,
  scorer helpers, answer/patch runners, GEPA feedback, Blueprint contribution
  gates, study packets, Forge projection, comparison report, and product-promise
  gate review.
- `packages/probe/docs/benchmarks/2026-06-17-openagents-studybench-mvp-14-comparison.md`
  records a public-safe fixed-budget comparison: baseline no packet, study
  packet, and GEPA packet arms across public-retained and private-validation
  refs, with improved weighted score and core-gate pass rates.

Gap:

- The MVP comparison is internal dogfood evidence only. It does not yet become
  a repeatable continual-learning service with scheduled split rotation,
  candidate registry, promotion history, and degradation monitoring.
- Private validation and holdout boundaries are documented in
  `docs/research/machine-studying/openagents-studybench/private-boundary.md`,
  but there is no universal "candidate cannot train on holdout" enforcement
  across every future GEPA/RLM/Blueprint lane.

### Probe GEPA and candidate optimization

Current:

- `packages/probe/docs/benchmarks/2026-06-08-probe-continual-benchmark-learning-apparatus.md`
  frames Probe improvement as a closed learning loop: runtime truth, benchmark
  jobs, Psionic optimization, Pylon rollout capacity, and product-surface
  release gates.
- `packages/probe/docs/benchmarks/2026-06-08-omni-continual-learning-training-loop.md`
  ties the loop to accepted coding outcomes: Blueprint-governed workrooms,
  selected backend routes, diffs/tests/previews/logs, human review, accepted or
  rejected outcomes, route scorecards, failure lessons, benchmark candidates,
  and gated promotion.
- `packages/probe/docs/probe-gepa-candidate-execution.md` documents the
  implemented runtime-local adapter that runs retained benchmark assignments
  with a baseline or Psionic GEPA text-bundle candidate manifest and emits
  normalized closeout bundles.
- `apps/openagents.com/INVARIANTS.md` contains GEPA campaign gates: metric-call
  assignments are benchmark evidence only; Stage 0 is no-spend only; paid-mode
  claims require explicit payment and settlement evidence; outcome metrics are
  not product wins unless tied to accepted coding outcomes and proof refs.

Gap:

- Today's GEPA surfaces are mostly typed candidate/execution/projection
  seams. A real optimizer run should be an offline compile tier that consumes
  fixed train/validation splits, emits candidate artifacts, and cannot
  self-promote.
- GEPA success needs to be reported as "candidate improved on split X under
  budget Y," not "the product learned," until it passes Blueprint and product
  gates.

### Blueprint signature, evidence, and release gates

Current:

- `apps/openagents.com/INVARIANTS.md` states that Blueprint Program Run
  records are evidence only and do not authorize deploys, emails, spend, source
  mutation, direct business mutation, public claim promotion, or provider side
  effects.
- Probe Blueprint Action Submission proposals are pending review records with
  direct execution disabled.
- Blueprint contribution records are release-gate evidence, not
  self-promoting runtime authority.
- `docs/khala/2026-06-24-khala-brain-and-blueprint-hookup-audit.md` maps DSPy
  concepts to live Blueprint surfaces: signatures, modules, optimizer runs,
  metrics, program-run evidence, and release gates.
- `docs/khala/2026-06-24-khala-session-distiller-and-program-wiring-spec.md`
  sketches the future shape for Khala turns as Blueprint program runs, session
  traces, distillation into typed signatures/modules, GEPA refinement, and
  release-gated skill/e2e emitters.

Gap:

- The Khala docs explicitly say the live Blueprint turn runtime is not yet
  called from the Khala request path. That is the first hard integration gap
  for making chat sessions become governed learning evidence.
- There is no single "LearningCandidate" schema that points at trace refs,
  token rows, evaluator results, candidate artifact refs, release gates, and
  product-promise refs across Khala, Probe, Artanis, and Tassadar.

### Tassadar exact execution and receipts

Current:

- `docs/tassadar/2026-06-18-tassadar-run-actual-state-and-real-training-gap-audit.md`
  corrects the framing: Tassadar's paradigm is analytic construction and
  exact replay of compiled capability modules, not normal gradient descent.
  The live loop verifies digest-pinned compiled-program execution through
  independent replay and receipt-backed settlement gates.
- `docs/tassadar/work-that-proves-itself.md` frames exact execution as work
  whose trace is the receipt, making verification structurally cheaper than
  fuzzy agent work.
- `docs/tassadar/2026-06-19-agentic-kernel-optimization-work-definition-and-parity-protocol.md`
  defines a future accepted-work shape for kernel optimization: named target,
  baseline throughput, optimized deliverable, output parity, and verification
  through exact trace replay.

Gap:

- Tassadar exactness should not be inflated into statistical agent-learning
  claims. It can verify deterministic substrate: corpus manifests, digests,
  replayable traces, parity checks, compiled modules, and exact reducers.
- The bridge between fuzzy learned candidates and exact replay should be a
  typed verification-class field. A candidate can be `exact_trace_replay` only
  for the deterministic part that truly replayed.

### RLM and Conductor

Current:

- Public issue `#6654` asks for Artanis to compose unbounded responses through
  Recursive Language Model decomposition rather than one capped completion. It
  cites the FRLM conductor pattern: environment/context fragments, scheduler,
  budget/policy, trace emitter, Local/Swarm/Remote/Codex execution, and final
  composition.
- `docs/research/2026-06-28-dspy-rlm-python-backend-vs-effect-audit.md`
  recommends a hybrid: Python DSPy/GEPA for offline optimize/compile, Python
  RLM only as a sandboxed leaf executor, and Effect/TypeScript for online
  serving and governance.

Gap:

- RLM is not itself learning. It is a decomposition/execution architecture that
  can create better traces and subtask evidence. Its continual-learning role is
  to generate structured sub-call histories, failure spans, and reusable
  decomposition policies that GEPA/Blueprint can later optimize and promote.
- RLM needs observability for fanout, budget, partial failures, and account
  rate limits before it can be safely used for operator-scale Artanis work.

### AgentCL and memory evaluation

Current:

- `docs/research/agentcl/openagents-implications.md` warns that storing prior
  work is not the same as continual learning. It proposes memoryless, first-pass,
  frozen-memory second-pass, and held-out views, with separate Plasticity,
  Stability, and Generalization gains.

Gap:

- OpenAgents memory and study-packet claims need those separate metrics. A
  retrieved trace that is topically similar but semantically wrong can hurt.
  Continual-learning claims must therefore report reuse benefit, retention
  after memory is frozen, and held-out harmlessness separately.

### Operator observability and rate limits

Current:

- Public issue `#6637` asks for per-account rate-limit observability: which
  Codex/Claude accounts are rate-limited or available, hourly/weekly capacity,
  usage, remaining quota, cooldown reset times, manual reset counts, an
  owner-only dashboard, and APIs.
- The Khala/Pylon delegation runbook in `AGENTS.md` already requires exact
  downstream Codex token rows, owner-only ATIF traces, raw private event chunks,
  durable resume proof, and public counter reconciliation.

Gap:

- Continual learning cannot optimize dispatch if it cannot see capacity and
  rate-limit state. RLM fanout, GEPA rollout scheduling, StudyBench evals, and
  accepted-outcome work all need account/capacity status in the same
  observability plane as cost, tokens, and acceptance.
- Manual reset use is an operator-governed resource. It should never become an
  optimizer-controlled side effect.

## Architecture Target

### 1. Learning data record

Introduce a conceptual record family, whether as one schema or a coordinated
set of existing rows:

```text
LearningEvidenceUnit {
  taskRef
  demandKind
  demandSource
  sourceRefs[]
  traceRefs[]              // ATIF public/unlisted/owner_only refs
  rawPrivateArchiveRefs[]  // private R2/D1 refs, never public copied
  tokenUsageEventRefs[]
  receiptRefs[]
  routeScorecardRefs[]
  outcome: accepted | rejected | inconclusive | timed_out
  verificationClass: none | seeded | test_passed | exact_trace_replay | human_review
  privacyTier: public_safe | owner_only | operator_only | private_holdout
  candidateEligible: boolean
  blockerRefs[]
}
```

Current rows can supply most fields. The work is reconciliation, not wholesale
replacement.

### 2. Evaluation plane

Evaluators should be typed and split-aware:

- StudyBench answer-mode and patch-mode for repo expertise.
- Probe/Terminal-Bench/Harbor for coding-agent terminal behavior.
- Product-promise gates for public truth.
- Tassadar exact replay for deterministic substrate and parity.
- Accepted-work receipts for buyer-facing outcomes.
- AgentCL-style streams for memory reuse, stability, and generalization.
- Operator/capacity observability for cost and scheduling metrics.

Every evaluation report needs:

- train/validation/holdout split refs;
- candidate ref and baseline ref;
- exact budget, model/backend, route, and account/capacity context;
- public-safe closeout refs;
- rejected and timed-out rows, not just wins;
- explicit "no product authority" unless a separate gate passed.

### 3. Optimizer plane

Use the hybrid from `docs/research/2026-06-28-dspy-rlm-python-backend-vs-effect-audit.md`:

- Offline Python GEPA/DSPy compile jobs produce optimized prompt text,
  few-shot demos, policy bundles, signature variants, or tool-menu candidates.
- RLM runs as sandboxed leaf execution or as an Effect-native conductor that
  schedules subqueries, emits sub-call traces, and composes outputs.
- Effect/TypeScript remains the online authority: Blueprint signature lookup,
  action submissions, release gates, product promises, account observability,
  and public projections.

Optimizer outputs are untrusted candidates. They enter Blueprint as candidate
module versions, context packs, study packets, tool policies, or action
submission proposals. They do not mutate live runtime, provider config,
settlement, public copy, or account resets.

### 4. Promotion plane

A promotion should require:

- candidate artifact refs and digests;
- evaluation report refs over retained and validation splits;
- held-out or private-validation policy status where relevant;
- trace/token/receipt reconciliation;
- no public-safety tripwire findings;
- no self-promotion attempt;
- release-gate approval;
- product-promise update if public copy changes;
- rollback plan and staleness policy.

This is where Blueprint becomes the central governance model. A candidate can
be useful and still fail promotion because it lacks held-out evidence, leaks
private material, exceeds risk ceiling, overclaims a product state, worsens
capacity use, or cannot be rolled back.

### 5. Governance and monitoring plane

Continual learning must remain observable after promotion:

- token usage and reasoning-token accounting in `token_usage_events`;
- trace quality and redaction failures;
- accepted/rejected outcome rates;
- cost, latency, account limits, reset windows, and manual reset inventory;
- accepted outcomes per kWh where energy/capacity data is available;
- public projection freshness and product-promise state;
- degraded or stale candidates blocked from green copy;
- rollback trigger refs.

## Staged Plan

### Phase 0: Name the boundary

- Add a short `continual_learning.v0` glossary for "evidence unit,"
  "candidate," "optimizer run," "promotion," "active module," and
  "product claim."
- Document that traces, token rows, and benchmark wins are evidence only until
  release and product gates pass.

### Phase 1: Reconciled evidence query

- Build an owner/operator command that resolves a `taskRef` or assignment ref
  to `token_usage_events`, `agent_traces`, raw-private archive metadata,
  closeout/proof refs, and outcome refs.
- This directly closes the Khala/Pylon runbook gap that currently requires
  direct D1 queries to prove exact token rows and trace rows.

### Phase 2: LearningEvidenceUnit projection

- Emit a public-safe summary for dogfood tasks and an owner/operator-only
  version with private refs.
- Include demand origin, split refs, verifier class, outcome, token totals,
  trace refs, and blocker refs.

### Phase 3: Split-aware StudyBench cadence

- Re-run the existing StudyBench comparison shape on a cadence, with frozen
  baseline, study-packet, and GEPA/RLM candidate arms.
- Keep private holdout rows out of study packets and optimizer feedback.
- Add AgentCL-style first-pass, frozen-memory second-pass, and held-out
  harmlessness reports for repo memory.

### Phase 4: Offline GEPA compile service

- Run upstream GEPA/DSPy outside Workers against public-safe train/validation
  refs.
- Store candidates in R2/D1 with digest refs.
- Import candidates into Blueprint as optimizer candidates with no runtime
  authority.

### Phase 5: RLM conductor traces

- For Artanis and long-form operator work, implement RLM/FRLM-style
  decomposition with typed sub-call signatures, budget, route/capacity state,
  and trace emission.
- Feed sub-call failure spans and composition quality into the same optimizer
  and evaluator planes.

### Phase 6: Promotion and product promises

- Wire candidate promotion to Blueprint release gates and `docs/promises/`
  updates.
- A candidate that improves benchmark score can become active only when
  accepted outcome refs, proof refs, privacy gates, and rollback/staleness
  gates pass.

### Phase 7: Capacity-aware learning economics

- Tie `#6637` rate-limit observability into learning schedulers.
- Report accepted outcomes per token, per dollar, and where measured per kWh.
- Prefer latency-tolerant GEPA/StudyBench/RLM jobs during cheap or otherwise
  idle capacity, while preserving external demand preemption.

## Claim Discipline

Allowed current statements:

- OpenAgents has public-safe trace storage, token usage ledgers, StudyBench MVP
  evidence, Probe GEPA candidate seams, Blueprint evidence/release gates,
  Tassadar exact replay lanes, and product-promise gates.
- These surfaces form the substrate for a governed continual-learning loop.
- StudyBench MVP-14 showed internal dogfood improvement for study-packet and
  GEPA-packet arms under fixed budgets.

Disallowed without new evidence:

- "Khala continually learns from every user."
- "Artanis autonomously promotes learned skills."
- "GEPA candidates are active product improvements."
- "StudyBench wins prove customer repo expertise."
- "Tassadar exact replay proves fuzzy agent reasoning is exact."
- "Trace upload or free-tier capture automatically trains a model."
- "Manual rate-limit resets or account routing are optimizer-controlled."

## References

- `docs/transcripts/232.md`
- `docs/transcripts/233.md`
- `docs/transcripts/234.md`
- `docs/research/machine-studying/README.md`
- `docs/research/machine-studying/2026-06-17-studybench-openagents-benchmark-audit.md`
- `docs/research/machine-studying/2026-06-17-openagents-studybench-mvp-issue-roadmap.md`
- `docs/research/machine-studying/2026-06-17-tassadar-openagents-repo-studying-roadmap.md`
- `docs/research/machine-studying/2026-06-17-blueprint-marketplace-ties.md`
- `docs/research/machine-studying/openagents-studybench/private-boundary.md`
- `docs/research/2026-06-28-dspy-rlm-python-backend-vs-effect-audit.md`
- `docs/research/agentcl/openagents-implications.md`
- `docs/tassadar/2026-06-18-tassadar-run-actual-state-and-real-training-gap-audit.md`
- `docs/tassadar/2026-06-19-agentic-kernel-optimization-work-definition-and-parity-protocol.md`
- `docs/tassadar/work-that-proves-itself.md`
- `docs/khala/2026-06-24-khala-brain-and-blueprint-hookup-audit.md`
- `docs/khala/2026-06-24-khala-session-distiller-and-program-wiring-spec.md`
- `packages/probe/docs/benchmarks/2026-06-08-probe-continual-benchmark-learning-apparatus.md`
- `packages/probe/docs/benchmarks/2026-06-08-omni-continual-learning-training-loop.md`
- `packages/probe/docs/benchmarks/2026-06-17-openagents-studybench-mvp-14-comparison.md`
- `packages/probe/docs/probe-gepa-candidate-execution.md`
- `apps/openagents.com/INVARIANTS.md`
- Public issue `#6654`: RLM/FRLM-style Artanis composition.
- Public issue `#6637`: per-account rate-limit observability and operator
  dashboard.
