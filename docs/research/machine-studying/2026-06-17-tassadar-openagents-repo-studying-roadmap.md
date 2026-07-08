# Tassadar OpenAgents Repo Studying Audit And Roadmap

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-17
Status: audit / roadmap / speculation
Scope: internal dogfood plan for pairing the current Tassadar run with machine studying over the `openagents` repository

## Executive Summary

The current Tassadar run proves that OpenAgents can route, execute, validate,
reject, and account for bounded work with receipt-backed evidence. It does not
yet have a real corpus or a specific studying direction beyond executor-trace
work. The most useful next corpus is the `openagents` repo itself.

The bet is simple: turn the active repo into a measured study environment where
agents become materially better at understanding and editing this codebase. The
first product is not a public claim about autonomous repo editing. It is a
private, evidence-backed "repo studying" loop that produces source maps, study
packets, hidden editing exams, retained failure cases, and deterministic corpus
artifacts. Once it works for us, the same shape can become a customer-facing
repo-knowledge product.

This should remain claim-disciplined:

- Tassadar exactness applies to digest-pinned work, deterministic corpus
  refinery steps, replayable traces, and verifier receipts.
- Agent editing remains statistical until proven otherwise.
- Public projections should expose refs, digests, accepted/rejected counts, and
  bounded capability statements, not broad "agents know any repo" claims.

## Current Audit

### Tassadar

Tassadar already has a live executor-trace lane. The active public run summary is
rooted around `run.tassadar.executor.20260615`, with accepted and rejected
exact-replay work, receipt refs, settlement caveats, and public-safe projection
discipline. The run has real verification semantics for bounded executor work,
but the present workload families are still generic examples such as article
closeout, sudoku traces, Hungarian traces, and kernel traces.

That is valuable infrastructure, but it is not yet a training direction. It does
not yet answer: what should OpenAgents agents study first, how will we measure
improvement, and what corpus should become the first dogfood loop?

### Machine Studying

The machine-studying research note frames expertise as a measurable relationship
between score and inference compute. The important shift is from "does the model
know this fact" to "how much useful performance can the agent extract from a
corpus under a fixed budget." For a repo, that means measuring whether a study
packet helps an agent find the right authority, avoid traps, choose the right
tests, make smaller edits, and pass review-oriented checks with fewer tokens and
fewer wrong turns.

The existing Blueprint synthesis already proposes `BlueprintStudyPacket` and a
`machine_studying_blueprint_probe.v1` style benchmark. This doc narrows that
idea onto the `openagents` repo as the initial corpus.

### Blueprint, Probe, Pylon, And Artanis

Blueprint gives us typed capability and work contracts. Probe gives us the
runtime/evidence bridge for benchmark execution, retained failures, tool-menu
planning, and proof bundles. Pylon gives us assignment and contribution rails.
Artanis should coordinate campaigns and promotion state, but should not become
the runner, scorer, optimizer, or settlement authority.

The repo-studying loop should use those boundaries instead of inventing a new
parallel control plane.

## Thesis

OpenAgents should dogfood a "repo expert" system by studying itself first.

Initial target:

- Corpus: the tracked public-safe contents of the `openagents` repo at a pinned
  commit.
- Student task: become better at safe, authority-aware codebase navigation and
  editing.
- Evaluation: hidden repo-edit exams plus deterministic checks and review-style
  rubrics.
- Evidence: source refs, corpus digests, study-packet versions, attempt traces,
  test outputs, accepted/rejected verdicts, and retained failure fixtures.
- Product path: package the same mechanism as a customer repo studying product
  once the dogfood loop shows measurable lift.

## Proposed Work Class

Working name:

`machine_studying.openagents_repo.v0`

Related future contribution kind:

`tassadar.repo_refinery_artifact.v0`

The distinction matters. `machine_studying.openagents_repo.v0` is the study and
evaluation program. `tassadar.repo_refinery_artifact.v0` is a future bounded
Tassadar-compatible work class for deterministic artifacts such as corpus
manifests, index transforms, file digests, redaction checks, and replayable exam
harness outputs.

Do not describe the model's repo edits as exact execution. The exact lane should
verify the deterministic substrate around the studying loop.

## Corpus Contract

The initial corpus should be the `openagents` repo at a specific commit hash,
with a manifest that records every admitted file by path, byte length, content
digest, language/kind, and source authority.

Admit:

- `AGENTS.md`, `INVARIANTS.md`, and child repo guidance.
- `docs/promises/` and other product-claim authority docs.
- `docs/research/machine-studying/`.
- `docs/tassadar/`.
- Blueprint source and docs under `apps/openagents.com/workers/api/src/blueprint`
  and adjacent marketplace/promise gates.
- Probe docs and benchmark planning docs.
- Pylon, training-run, and public projection code relevant to Tassadar.
- `packages/tassadar-executor/` docs, fixtures, and tests.
- Focused tests and fixtures that define real behavior.

Exclude:

- `.git/`, `.claude/`, `.pylon-local/`, caches, build outputs, `node_modules/`,
  generated bundles, DMGs, tarballs, and temporary directories.
- `.secrets/` and any private local operator material.
- Raw production logs, private customer data, private wallet data, or private
  payment material.
- Any source not suitable for the declared study visibility tier.

Public projections should expose manifest refs and digests, not a raw archive of
the repo.

## Study Packet Shape

The first repo study packet should be a structured artifact, not a long context
dump. A useful `BlueprintStudyPacket` for this repo would include:

- Source map: which directories own which surfaces, and which files are primary
  authority.
- Invariant map: workspace routing, claim discipline, semantic routing,
  no-secret rules, and direct-write boundaries.
- Typed ref glossary: run refs, promise refs, contribution kinds, package refs,
  receipt refs, and public projection refs.
- Trap catalog: deprecated repos, stale Nexus surfaces, public-copy
  overclaiming, owner-operated exclusions, fake live motion, raw private data,
  and keyword-routing shortcuts.
- Test command catalog: focused commands for Blueprint, Probe, Pylon, Tassadar,
  public projections, and docs-only changes.
- Edit playbooks: small recipes for adding docs, modifying public projection
  copy, changing Blueprint signatures, updating training-run surfaces, and
  extending Probe benchmarks.
- Failure fixtures: retained attempts where agents chose the wrong authority,
  edited deprecated repos, overclaimed product status, or skipped required
  tests.

This packet should be versioned and digest-addressed. Agent attempts should
record which packet version they received.

## Evaluation Design

The evaluation should test repo editing, not just repo Q&A.

Exam families:

- Authority navigation: identify the right repo, docs, package, and test owner
  for a proposed change.
- Claim discipline: rewrite or reject public copy that exceeds current promises.
- Blueprint extension: add or adjust a typed contribution signature without
  weakening routing invariants.
- Tassadar projection: modify a run-summary or page-facing surface while
  preserving public-safe receipt semantics.
- Probe benchmark: add a focused fixture or evidence-bundle test without
  turning Probe into an optimizer authority.
- Docs synthesis: produce a research/audit doc that accurately ties systems
  together and avoids unsupported product claims.

Measure:

- Pass/fail against hidden tests and deterministic checks.
- Review score for authority correctness, edit scope, and invariant compliance.
- Tokens, wall-clock time, number of tool calls, and number of wrong-file reads.
- First divergence from the ideal trajectory.
- Whether the correct source refs entered context before the edit.
- Whether the agent used the refs correctly after finding them.

Budget curves:

- No study packet baseline.
- Study packet only.
- Study packet plus source-map hints.
- Study packet plus retained failure examples.
- Larger inference budgets with the same packet.

The core metric should be expertise lift at equal or lower budget, not raw
memorization.

## Tassadar Pairing

The initial pairing should be two-lane:

1. Statistical repo studying lane.
   Agents receive repo study packets, attempt hidden tasks, and produce code or
   doc edits. Probe records trajectories, tests, failures, and review evidence.

2. Deterministic Tassadar refinery lane.
   Tassadar-compatible work verifies corpus manifests, content digests,
   redaction checks, index transforms, fixture generation, exam-harness
   material, and replayable scoring steps where exactness is real.

The output of lane 2 can feed lane 1. Lane 1 should not be marketed as exact.

Candidate deterministic artifacts:

- `openagents_repo_corpus_manifest.v0`
- `openagents_repo_redaction_report.v0`
- `openagents_repo_source_map_index.v0`
- `machine_studying_openagents_repo_exam_manifest.v0`
- `machine_studying_openagents_repo_attempt_receipt.v0`

## Productization Path

If the dogfood loop works, the customer product is not "upload a repo and get a
chatbot." It is a managed repo-studying system with evidence:

- Private repo corpus admission with clear exclusion rules.
- Study packet generation that respects repo authority and customer policy.
- Hidden customer-specific editing exams.
- Measured lift curves across inference budgets.
- Retained failure cases that become future training and evaluation material.
- Blueprint capability signatures for what the repo expert is allowed to do.
- Marketplace routing only after work classes, quality gates, pricing, and
  settlement are proven.

For OpenAgents, this can become a product promise only after we can show that the
system improves edit outcomes on our own repo without overfitting to public docs
or leaking private material.

## Roadmap

### Phase 0: Contract And Freeze

- Pick a repo commit as the first corpus root.
- Write the corpus admission/exclusion policy.
- Define visibility tiers: private dogfood, public-safe refs, and future
  customer-private corpora.
- Create `openagents_repo_corpus_manifest.v0`.
- Add a no-secrets and no-generated-output validation check before admission.

Exit criterion: a reproducible manifest with content digests and explicit
exclusions.

### Phase 1: Study Packet MVP

- Build the first source map and invariant map.
- Add the typed ref glossary.
- Add the trap catalog and test command catalog.
- Package it as `BlueprintStudyPacket` material, even if the first version is a
  docs artifact.

Exit criterion: an agent can receive the packet and orient itself without a raw
repo dump.

### Phase 2: Hidden Repo-Edit Exam

- Create 20 to 50 small tasks that require real repo understanding.
- Split tasks into public docs, API contracts, tests, projection copy, and
  research synthesis.
- Keep answer keys and scoring rubrics separate from the study packet.
- Record baseline attempts without the packet.

Exit criterion: baseline score and failure taxonomy exist.

### Phase 3: Probe-Backed Study Runs

- Run the same exams with the study packet.
- Record trajectories, source refs, commands, diffs, test outputs, and review
  verdicts.
- Compare expertise curves at fixed token budgets.
- Promote retained failures into packet updates only after they are labeled.

Exit criterion: measured lift or a clear falsification for this packet shape.

### Phase 4: Tassadar Deterministic Refinery

- Convert corpus manifesting, redaction, source-map indexing, and exam-manifest
  generation into bounded deterministic work.
- Add exact replay where replay is meaningful.
- Emit accepted/rejected refinery receipts.
- Keep model edit attempts outside the exact claim boundary.

Exit criterion: deterministic study substrates have Tassadar-style verifier
receipts.

### Phase 5: Dogfood Product Surface

- Add an internal dashboard or run summary for repo studying.
- Show corpus version, packet version, exam score, lift curve, stale/projection
  state, and accepted/rejected deterministic artifacts.
- Keep public copy boring and receipt-first.

Exit criterion: OpenAgents operators can see whether repo studying is improving
agent edits.

### Phase 6: External Repo Product

- Define customer repo admission and privacy policy.
- Define per-customer Blueprint capabilities.
- Add marketplace work only after validation, pricing, settlement, and customer
  review gates exist.
- Consider fine-tuning or adapter training only after study packets, exams, and
  data controls are working.

Exit criterion: a customer-private repo studying pilot can run without relying
on OpenAgents-specific shortcuts.

## Risks And Kill Conditions

Risks:

- The packet improves repo trivia but not real edits.
- Exams leak into the study packet and create false lift.
- The corpus admission path accidentally includes secrets or private local data.
- Agents overfit to docs and ignore tests.
- The repo changes faster than packet freshness can track.
- Public projections imply stronger product claims than the evidence supports.
- The exact Tassadar language gets applied to statistical model behavior.
- Routing regresses into ad hoc keyword matching instead of typed semantic
  selectors or structured planners.

Kill or redesign if:

- No measurable edit-quality lift appears at fixed budgets after multiple packet
  revisions.
- Redaction cannot be made reliable enough for private corpora.
- Retained failures show repeated invariant violations that the study packet does
  not reduce.
- The evaluation cannot distinguish real repo understanding from memorized docs.

## Success Criteria

The first dogfood loop is successful if:

- Agents pass more hidden repo-edit tasks at the same budget.
- Agents read fewer wrong files before finding the right authority.
- Invariant and claim-discipline violations decrease.
- Focused tests pass more often without broader, unrelated edits.
- Every attempt has packet refs, corpus refs, and evidence refs.
- Deterministic corpus/refinery artifacts are reproducible by digest.
- The public story can remain conservative while the internal metrics improve.

## Immediate Next Artifacts

- `openagents_repo_corpus_manifest.v0`
- `BlueprintStudyPacket` schema for repo studying
- `machine_studying_openagents_repo_probe.v0` benchmark plan
- Probe fixture comparing baseline vs packet-assisted file selection
- `tassadar.repo_refinery_artifact.v0` design note for deterministic corpus
  artifacts

The useful next move is not to train a model immediately. It is to make the
repo corpus, study packet, exam harness, and evidence ledger concrete enough
that any later training or marketplace claim has something real to stand on.
