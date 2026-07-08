# StudyBench And OpenAgents Repo-Studying Benchmark Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-17
Status: audit / roadmap proposal, not a benchmark claim or product promise
Scope: how the StudyBench dataset should inform the OpenAgents machine-studying,
Tassadar, Probe, Psionic, Blueprint, and Forge Autopilot Coder plans.

## Implementation Status

- 2026-06-17: #5282 / MSB-MVP-00 complete. The machine-studying README now
  freezes the MVP source boundaries, names the initial OpenAgents StudyBench
  refs, and states the answer-mode, patch-mode, calibration, public-retained,
  private validation, private holdout, Probe, Psionic, Tassadar, Blueprint, and
  product-promise boundaries.
- 2026-06-17: #5283 / MSB-MVP-01 complete. Probe runtime now exports
  `openagents.studybench_task.v0`,
  `openagents.studybench_rubric_claim.v0`,
  `openagents.studybench_evidence_span.v0`,
  `openagents.studybench_dataset_package.v0`,
  `probe.studybench_claim_score.v0`, and
  `probe.studybench_rubric_score.v0` contracts with validation for public
  safety, rubric weight totals, evidence-span resolution, and public package
  visibility boundaries.
- 2026-06-17: #5284 / MSB-MVP-02 complete. Probe runtime now exports
  `openagents.repo_corpus_manifest.v0`,
  `openagents.repo_corpus_entry.v0`, and
  `openagents.repo_corpus_evidence_span.v0` helpers for deterministic repo
  corpus manifests, exclusion-aware file admission, stable manifest and span
  hashes, and line-numbered StudyBench-style evidence-span extraction.
- 2026-06-17: #5285 / MSB-MVP-03 complete. Probe runtime now exports an
  external StudyBench calibration manifest for
  `hf://jacobli/studybench/dspy` and
  `hf://jacobli/studybench/openclaw`, preserves CC-BY-4.0 and upstream MIT
  attribution refs, and rejects vendored upstream row payloads from calibration
  manifests.
- 2026-06-17: #5286 / MSB-MVP-04 complete. The first public-retained
  `openagents_studybench.v0` fixture package now has 10 launch-focused JSONL
  rows covering launch claims, Tassadar projection truth, settlement truth,
  Customer #1 evidence, Forge Coder repo memory, Blueprint/Probe/GEPA
  boundaries, Pylon launch priority, schema adaptation, answer-versus-patch
  mode, and product-promise/marketplace gates. Probe tests validate the rows as
  an `openagents.studybench_dataset_package.v0`.
- 2026-06-17: #5287 / MSB-MVP-05 complete. The
  `openagents-studybench/private-boundary.md` policy now defines private
  validation and private holdout split refs, checksum refs, ignored local row
  storage, evaluator/scorer access, leak response, and the rule that private
  holdout rows cannot feed study packets or GEPA training.
- 2026-06-17: #5288 / MSB-MVP-06 complete. Probe runtime now exports
  StudyBench rubric scorer helpers that validate score vectors against task
  rubrics, support manual/judge-supplied and deterministic-check scorer refs,
  compute weighted basis-point scores, apply the strict core-claim gate, and
  reject raw evaluator rationale text from public score summaries.
- 2026-06-17: #5289 / MSB-MVP-07 complete. Probe closeout bundles now carry
  stable StudyBench task-ref and rubric-score summaries, preserve existing
  Terminal-Bench closeouts, project StudyBench task/score/evidence-use refs
  into the GEPA live-runner gate as evidence only, and keep product, payout,
  and public-score authority disabled unless separate gates are supplied.
- 2026-06-17: #5290 / MSB-MVP-08 complete. Probe runtime now exports an
  answer-mode StudyBench runner that validates task/assignment split
  compatibility, builds candidate-visible input with private gold/rubric
  scorer material withheld, validates optional GEPA candidate manifests,
  computes rubric scores from supplied claim scores, and emits normal Probe
  closeout bundles with StudyBench score refs.
- 2026-06-17: #5291 / MSB-MVP-09 complete. Probe runtime now exports a
  patch-mode StudyBench runner for pre-recorded MVP agent transcripts over
  pinned checkout refs, allowed tool menus, budget policies, test refs, patch
  artifact refs, runner identity refs, and rubric scoring. The runner rejects
  out-of-menu tools and max-tool-call violations, turns timeout overruns into
  timed-out closeouts, withholds private gold/rubric/evidence material from
  candidate input, and emits normal Probe closeout bundles.
- 2026-06-17: #5292 / MSB-MVP-10 complete. Probe runtime now exports a
  StudyBench-to-GEPA feedback bridge that converts rubric score failures into
  deterministic Psionic feedback refs for failed claims, missed evidence spans,
  forbidden claim refs, skipped tests, wrong files, and budget failures while
  keeping optimizer acceptance separate from runtime promotion, public claims,
  and payout authority.
- 2026-06-17: #5293 / MSB-MVP-11 complete. Blueprint contribution gates now
  recognize the StudyBench task-authoring, evidence-span extraction, rubric
  authoring, rubric judging, and repo-study-packet contribution kinds, map them
  onto existing evidence-only capability families, require retained failure
  refs before release-gate readiness, and allow Action Submission proposals to
  cite StudyBench closeout and study-packet refs as evidence while staying
  pending approval with no direct execution authority.
- 2026-06-17: #5294 / MSB-MVP-12 complete. The first public-retained launch
  study packet now lives at
  `openagents-studybench/study-packets/openagents-launch-study-packet-v0.md`
  with packet/source refs, source-package digest, public row map, authority
  map, invariant map, forbidden-claim trap catalog, test catalog, launch-edit
  playbooks, and a glossary while excluding private validation and holdout
  material.
- 2026-06-17: #5295 / MSB-MVP-13 complete. Forge Autopilot Coder now exposes
  StudyBench/study-packet repository memory as refs-only Context snapshot
  state: study-packet, corpus-manifest, dataset, public-retained score,
  private-validation trend, holdout-evaluation, freshness, and blocked-claim
  refs. The projection labels the lane internal dogfood, carries an
  evidence-only authority boundary with no mutation authority, and omits
  hidden rubrics, hidden gold answers, raw repo archives, private customer
  source refs, local paths, raw commands, and credential-shaped material before
  rendering.
- 2026-06-17: #5296 / MSB-MVP-14 complete. The first recorded public-safe
  comparison now covers baseline, study-packet, and GEPA-packet candidate arms
  across 10 public-retained rows, 5 private-validation refs, and 2 patch-mode
  rows. Probe docs now carry a machine-readable summary with 45 derived
  closeout attempts, answer-mode versus patch-mode metrics, closeout/rubric
  score ref coverage, and blocked product-promise/marketplace authority. The
  machine-studying run report mirrors the result without committing private
  validation row bodies, hidden rubrics, hidden gold answers, private evidence
  spans, or raw repo archives.
- 2026-06-17: #5297 / MSB-MVP-15 complete. The product-promise registry now
  includes `autopilot.repo_study_packets.v1` as a yellow internal-dogfood
  claim backed by the MVP-14 comparison and refs-only study packet docs. The
  gate review keeps customer repo studying, trained repo expert copy,
  marketplace packaging, payout eligibility, settlement, and paid-work claims
  blocked until separate customer, privacy, marketplace, pricing, payout, and
  settlement gates exist.

## Short Answer

The prior machine-studying plan incorporated the StudyBench idea at the concept
level, but not enough at the concrete benchmark level. It did not explicitly
adopt the `jacobli/studybench` dataset, its row schema, its weighted
source-grounded rubrics, or its public/open evaluation limitation.

That should change.

OpenAgents should do both:

1. Use StudyBench directly as an external calibration and smoke benchmark,
   especially the `dspy` subset because our Probe and Psionic work already uses
   GEPA/DSPy-adjacent optimization patterns.
2. Build an OpenAgents-owned StudyBench-style dataset over the `openagents` repo
   so agents can be measured on source-grounded codebase knowledge and launch
   correctness before we try to productize repo studying for customers.

StudyBench should not replace Terminal-Bench, Harbor, Probe closeouts, Psionic
GEPA candidate manifests, or Tassadar deterministic receipts. It fills a gap:
expert-curated, source-grounded coding questions with weighted claim rubrics and
evidence spans. That is exactly the missing shape for repo studying.

## External StudyBench Snapshot

Source:

- `https://huggingface.co/datasets/jacobli/studybench`

As of the reviewed dataset card, StudyBench is a small public benchmark of
expert-level coding questions over real codebases. It has two published
configs:

- `dspy`: 30 questions across 6 topics, grounded in DSPy.
- `openclaw`: 20 questions across 4 topics, grounded in OpenClaw.

Each row has:

- `id`: stable opaque identifier.
- `topic`: coarse task category.
- `question`: self-contained coding task prompt.
- `gold_answer`: reference solution.
- `rubric`: weighted claims that define a correct answer.
- `evidence`: source spans that ground the rubric.

The rubric is the important part for OpenAgents. Each claim has an id, a
core/supporting type, a weight, a statement, and span ids pointing into the
evidence list. The judge scores the candidate answer against each claim, then
computes a weighted question score. An optional strict gate can require every
core claim to pass.

The evidence is also important. Every span has a source path, line range, and
line-numbered excerpt from a pinned upstream commit. The judge can grade against
the evidence without checking out the upstream repo.

Limitations:

- The benchmark is public and fully transparent. Treat it as open calibration,
  not a hidden leaderboard.
- Models and agents may already have seen some or all of it.
- The answers reflect the pinned source snapshots, not necessarily current
  upstream APIs.

Licensing and attribution:

- StudyBench questions, gold answers, and rubrics are CC-BY-4.0.
- Embedded DSPy and OpenClaw source excerpts remain under the upstream MIT
  licenses.
- If OpenAgents vendors rows or derived material, we need explicit attribution
  and license handling. The safer first step is to reference the Hugging Face
  dataset by dataset ref instead of copying rows into this repo.

## What We Already Have

OpenAgents already has several benchmark-adjacent systems, but none exactly
matches StudyBench.

### Probe Benchmark Contracts

Probe owns runtime-local benchmark assignment, run, closeout, decision trace,
candidate, route scorecard, promotion decision, and GEPA live-runner-gate
contracts. These contracts are public-safe evidence records. They distinguish
retained, validation, holdout, and live splits, and they reject raw secrets,
hidden verifier content, wallet/payment material, private repo refs, raw logs,
public-claim authority, and runtime-promotion authority.

Relevant docs and code:

- `packages/probe/docs/probe-benchmark-contracts.md`
- `packages/probe/docs/probe-retained-terminal-bench-fixtures.md`
- `packages/probe/docs/probe-gepa-candidate-execution.md`
- `packages/probe/packages/runtime/src/contracts/benchmark.ts`
- `packages/probe/packages/runtime/src/benchmark/fixtures.ts`
- `packages/probe/packages/runtime/src/benchmark/candidate-execution.ts`

Current gap:

Probe has assignment and closeout envelopes, but no native StudyBench-style row
shape with `question`, `gold_answer`, weighted claim rubrics, and source
evidence spans.

### Probe Retained Terminal-Bench Lane

Probe has retained Terminal-Bench-style fixtures for coding-agent behavior,
including failure families such as webserver setup, WAL recovery, JavaScript
filtering, G-code conversion, PyPI serving, query optimization, and runner
stall supervision.

That lane is valuable because it is executable and sandbox-oriented. It tests
whether an agent can operate in a terminal and close out work.

Current gap:

Terminal-Bench-style fixtures do not provide StudyBench's source-grounded
weighted claim rubrics. They are complementary:

- Terminal-Bench lane: executable task outcome, sandbox artifacts, logs,
  verifier refs, closeout bundles.
- StudyBench lane: source-grounded API/codebase expertise, weighted claims,
  evidence spans, richer evaluator feedback.

### Psionic GEPA Candidate Manifests

Psionic has Probe GEPA candidate manifests for text-bundle optimization. The
candidate components include:

- `probe_system_prompt`
- `terminal_bench_global_playbook`
- `signature_selection_policy`
- `tool_menu_policy`
- `patch_and_test_policy`
- `failure_family_playbooks`
- `closeout_policy`

Psionic also has a rollout coordinator design for evaluating candidates over
benchmark tasks, preserving failure semantics, and importing live closeouts
without turning optimizer acceptance into runtime promotion.

Relevant docs:

- `../psionic/docs/PROBE_GEPA_CANDIDATE_MANIFESTS.md`
- `../psionic/docs/PROBE_GEPA_ROLLOUT_COORDINATOR.md`
- `../psionic/fixtures/probe/gepa/probe_gepa_candidate_manifest_stage_0_1_seed_v1.json`

Current gap:

The GEPA candidate lane has a place to optimize text bundles, but it lacks a
repo-specific StudyBench-style task corpus with claim-level evidence feedback.
StudyBench-style rows would give GEPA a much sharper reflection signal than a
single binary pass/fail.

### Psionic Legal Benchmark Engine

Psionic also has a legal benchmark engine with task specs, artifact manifests,
source artifacts, deliverable specs, criteria, judge policy, run records,
transcripts, metrics, coverage snapshots, criterion results, score reports, and
comparison reports.

Relevant docs and fixture:

- `../psionic/docs/LEGAL_BENCHMARK_ENGINE.md`
- `../psionic/docs/LEGAL_BENCHMARK_SIGNATURE_ROUTING.md`
- `../psionic/tasks/synthetic/legal-workflow-v1/rubrics/`

Current gap:

The legal benchmark rubric shape is simpler than StudyBench for codebase
studying. It has useful scoring discipline, but not source-path evidence spans
and weighted claim ids tied to exact repo lines.

## What StudyBench Adds

StudyBench should be treated as a benchmark design pattern, not only as a
dataset.

The important pattern is:

1. Pin a source snapshot.
2. Write questions that require using that source correctly.
3. Provide gold answers.
4. Decompose correctness into weighted claims.
5. Tie each claim to exact source evidence spans.
6. Let a judge grade claim by claim with the evidence in view.
7. Keep public/open rows separate from any private holdout.

This pattern maps cleanly to the OpenAgents repo-studying goal. Our agents need
to know which files are authorities, which docs are historical, which claims are
forbidden, which gates are live, and which tests matter. A generic hidden
repo-edit exam is useful, but a StudyBench-style exam gives us a reusable data
unit: one row can train, evaluate, explain, and generate retained failures.

## Proposed OpenAgents StudyBench Lane

Working name:

`openagents_studybench.v0`

Primary source corpus:

- The `openagents` repo at a pinned commit.

Initial visibility:

- Internal/private for gold answers, rubrics, and holdout tasks.
- Public-safe retained rows can be published later if they pass redaction,
  license, and product-claim review.

Base row shape should stay compatible with StudyBench:

```json
{
  "id": "openagents_launch_0001",
  "topic": "launch_claims_and_promises",
  "question": "...",
  "gold_answer": "...",
  "rubric": [
    {
      "claim_id": "c1",
      "claim_type": "core",
      "weight": 40,
      "statement": "...",
      "span_ids": ["s1", "s2"]
    }
  ],
  "evidence": [
    {
      "span_id": "s1",
      "path": "docs/launch/JUNE17_ROADMAP.md",
      "start_line": 1,
      "end_line": 40,
      "excerpt": "..."
    }
  ]
}
```

OpenAgents-specific extensions should be additive:

```json
{
  "schema_version": "openagents.studybench_task.v0",
  "repo": "OpenAgentsInc/openagents",
  "commit": "...",
  "corpus_ref": "openagents_repo_corpus_manifest.v0:...",
  "visibility": "private_holdout",
  "authority_refs": ["openagents.invariants", "openagents.product_promises"],
  "test_refs": ["apps/openagents.com product promise tests"],
  "forbidden_claim_refs": ["no_settlement_overclaim", "no_keyword_routing"],
  "private_material_policy_refs": ["no_secrets", "no_raw_customer_data"],
  "expected_files": ["..."],
  "budget_class": "small"
}
```

The extension fields matter because our repo benchmark needs to grade more than
API usage. It needs to grade authority discipline, public-claim boundaries,
privacy, routing invariants, and whether the answer edits the right part of the
repo.

## Initial Topic Set

Start with 30 rows, mirroring the scale of StudyBench's DSPy subset.

Suggested topics:

- `launch_claims_and_promises`: product-promise state, allowed copy, red/yellow
  caveats, and forbidden public claims.
- `tassadar_projection_truth`: public run summary, exact-replay boundaries,
  stale refs, simulation labels, and no fake live motion.
- `settlement_and_wallet_truth`: real Bitcoin moved, pending, expired,
  simulated, recipient-confirmed, and settlement-not-authorized states.
- `customer_one_evidence`: cohort rows, internal dogfood evidence, privacy,
  and public-safe projection.
- `forge_coder_repo_memory`: repository memory, bounded retrieval, Action
  Submission, operator review, and no write-authority bypass.
- `blueprint_probe_gepa_contracts`: Blueprint signatures, Probe assignment and
  closeout contracts, Psionic GEPA candidate manifests, and promotion gates.
- `pylon_assignment_wallet_readiness`: assignment lifecycle, wallet readiness,
  payment modes, and no premature payout claims.
- `studybench_schema_adaptation`: tasks that specifically test whether agents
  can adapt external benchmark schema without violating OpenAgents invariants.

Row examples should ask for actual repo work, not trivia:

- Rewrite a launch paragraph so it respects current product-promise status.
- Add a public-safe `/tassadar` field while preserving simulation caveats.
- Review whether a proposed settlement label overclaims real Bitcoin movement.
- Add a Probe closeout field for rubric-score refs without weakening safety
  validators.
- Draft a Forge Coder repo-memory projection that does not expose private
  corpus text or grant mutation authority.

## Split Strategy

StudyBench itself is public/open. That is useful, but it is not enough for real
OpenAgents claims.

OpenAgents should use four splits:

- `external_public_calibration`: direct StudyBench rows loaded by dataset ref.
- `openagents_public_retained`: public-safe OpenAgents rows with gold/rubric
  visible for regression, examples, and evaluator calibration.
- `openagents_private_validation`: private rows for optimization tracking.
- `openagents_private_holdout`: private rows for claims, promotion evidence,
  and launch readiness decisions.

Do not train, optimize, or packet-build against the private holdout. Use it only
for evaluation. If holdout rows are leaked into study packets or public docs,
retire them and mint new rows.

## Integration Architecture

### Probe

Probe should own the execution envelopes and public-safe closeout records.

Add or model these contracts:

- `openagents.studybench_task.v0`
- `openagents.studybench_rubric_claim.v0`
- `openagents.studybench_evidence_span.v0`
- `probe.studybench_claim_score.v0`
- `probe.studybench_rubric_closeout.v0`

This does not require replacing existing Probe benchmark contracts. A
StudyBench task can fit inside `probe.benchmark_assignment.v1` as a task ref,
dataset ref, split, public checksum, required artifact set, and verifier ref.
The closeout bundle can add a `rubric-score.json` artifact ref and a
claim-score vector ref.

The key invariant is unchanged: rubric scores are evidence, not public-score,
product-promotion, payout, or runtime-promotion authority.

### Psionic

Psionic should own optimizer-side use:

- Import external StudyBench `dspy` rows as an open calibration suite.
- Import OpenAgents public retained rows as GEPA reflection examples.
- Evaluate candidate text bundles on private validation rows.
- Preserve candidate manifests and frontier state separately from runtime
  promotion.

StudyBench-style rubrics can improve GEPA feedback because they identify which
claims failed and which evidence spans should have been used. That is a better
reflection signal than "test failed" alone.

Psionic should not publish product claims, settlement claims, or marketplace
quality claims from these scores. It can emit optimizer evidence.

### Tassadar

Tassadar should only cover deterministic refinery artifacts:

- Corpus manifest digests.
- Evidence span extraction and hashing.
- Redaction checks.
- Row-schema validation.
- Split-manifest validation.
- Replayable deterministic checks in scoring harnesses.

Do not route model answer quality through Tassadar exactness language. A model
answer can be scored, but it is not exact-replay work.

### Blueprint

Blueprint should be the typed capability boundary if these tasks become product
or marketplace artifacts.

Potential future contribution kinds:

- `studybench.task_authoring.v0`
- `studybench.evidence_span_extraction.v0`
- `studybench.rubric_authoring.v0`
- `studybench.rubric_judging.v0`
- `repo_study_packet.v0`

Marketplace publication requires separate validation, pricing, metering,
privacy, payout eligibility, and settlement gates. A StudyBench row or study
packet is not automatically a marketplace package.

### Forge Autopilot Coder

Forge Coder should consume study packets and source-grounded row refs as
repository memory, not as direct mutation authority.

Near-term surface:

- `studyPacketRef`
- `corpusManifestRef`
- `studybenchDatasetRefs`
- `publicRetainedScoreRef`
- `privateValidationTrendRef`
- `holdoutEvaluationRef`
- `freshness`
- `blockedClaimRefs`

The UI can show whether a packet exists, whether it is fresh, and whether it
helped on internal tasks. It should not show raw private rows, hidden rubrics,
or raw source excerpts from private customer repos.

## How This Ties To The Short-Term Launch Roadmap

The current launch roadmap already calls for a hidden launch repo-edit exam.
That exam should use a StudyBench-compatible row shape instead of an ad hoc
rubric format.

Concrete adjustment:

- Rename `machine_studying_openagents_launch_exam.v0` internally to be a
  StudyBench-compatible dataset slice.
- Keep the public artifact name launch-specific if that is clearer.
- Require every task to include source evidence spans from a pinned commit.
- Require weighted core/supporting claims.
- Add `authority_refs`, `forbidden_claim_refs`, and `test_refs` as
  OpenAgents-specific extensions.
- Keep private holdout rows out of study packets and docs.

This gives the launch plan a better measurement spine: the same row can be used
for baseline attempts, packet-assisted attempts, GEPA reflection, closeout
reporting, retained failure examples, and future customer-facing product design.

## Concrete Next Steps

The issue-level implementation plan lives in:

- `docs/research/machine-studying/2026-06-17-openagents-studybench-mvp-issue-roadmap.md`

1. Register StudyBench as an external calibration dataset by ref:
   `hf://jacobli/studybench/dspy`.
2. Draft `openagents.studybench_task.v0` in docs before code.
3. Author 10 public-retained OpenAgents rows from `docs/launch/`,
   `docs/research/machine-studying/`, `docs/tassadar/`, Probe docs, and product
   promise docs.
4. Author 10 private-validation rows and 10 private-holdout rows outside public
   docs, with only public-safe digest refs visible.
5. Add Probe closeout artifact refs for claim-score vectors and rubric-score
   summaries.
6. Add Psionic GEPA coordinator support for StudyBench-style claim feedback.
7. Compare baseline, study-packet, and GEPA-optimized candidates under fixed
   budgets.
8. Only after measured lift, consider a product-promise record for customer repo
   studying in planned/yellow state.

## Kill Conditions

Stop or redesign this lane if:

- Public StudyBench rows become the only evaluation evidence for OpenAgents
  product claims.
- OpenAgents private holdout rows leak into study packets, docs, or prompts.
- Rubrics start grading vibes instead of source-grounded claims.
- Evidence spans include secrets, raw private customer data, raw logs, payment
  material, or private repo content without the declared private path.
- The lane encourages ad hoc keyword routing instead of typed semantic planning.
- Psionic optimizer acceptance is treated as runtime promotion.
- Tassadar exactness is used to describe statistical model answers.
- Marketplace or payout language appears before separate gates exist.

## Recommendation

Make StudyBench the benchmark template for repo studying.

Use the public `jacobli/studybench` dataset to calibrate loaders, judges, and
DSPy/GEPA-specific behavior. Then build `openagents_studybench.v0` over our own
repo to measure whether study packets and GEPA-style prompt optimization make
agents better at editing OpenAgents under fixed budgets.

The first credible product claim should not be "we studied a repo." It should be
evidence that, on private holdout repo-edit tasks, a packet-assisted agent made
fewer authority mistakes, violated fewer claim boundaries, found the right
files faster, and passed more focused tests than the baseline.
