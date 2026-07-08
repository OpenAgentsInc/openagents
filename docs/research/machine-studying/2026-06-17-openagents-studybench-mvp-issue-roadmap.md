# OpenAgents StudyBench MVP Issue Roadmap

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-17
Status: implementation roadmap / issue draft
Scope: concrete MVP work for a StudyBench-style, agentic repo-studying
benchmark over the `openagents` repo, grounded in the current Probe,
Blueprint, Psionic, Tassadar, and Forge Autopilot Coder implementation.

## Goal

Build the smallest useful OpenAgents StudyBench MVP:

1. Consume public StudyBench rows as external calibration.
2. Define an OpenAgents-owned StudyBench-compatible task shape over a pinned
   `openagents` repo commit.
3. Support both answer-mode and agentic patch-mode evaluation.
4. Emit Probe closeout evidence with StudyBench claim scores.
5. Feed claim-level failures into Psionic GEPA-style prompt/candidate
   optimization.
6. Mount the resulting study packet as refs-only repository memory for Forge
   Autopilot Coder.
7. Keep Tassadar, Blueprint, product promises, marketplace, payout, and runtime
   promotion boundaries intact.

The MVP is not a public product claim. It is an internal dogfood benchmark and
study-packet loop.

## Current Implementation Anchors

### Probe Benchmark Contracts

Current files:

- `packages/probe/packages/runtime/src/contracts/benchmark.ts`
- `packages/probe/packages/runtime/tests/benchmark-contracts.test.ts`

Already implemented:

- `probe.benchmark_assignment.v1`
- `probe.benchmark_run.v1`
- `probe.benchmark_closeout.v1`
- `probe.benchmark_decision_trace.v1`
- `probe.prompt_candidate.v1`
- `probe.blueprint_candidate.v1`
- `probe.tool_menu_candidate.v1`
- `probe.loop_policy_candidate.v1`
- `probe.benchmark_route_scorecard.v1`
- `probe.benchmark_promotion_decision.v1`

Useful existing behavior:

- Assignment records already carry dataset refs, split refs, task refs,
  candidate refs, selected Blueprint signature refs, tool menu refs, timeout
  and budget policy refs, required artifacts, proof sinks, runtime refs, and
  backend refs.
- Splits already include `retained`, `validation`, `holdout`, and `live`.
- Closeouts already distinguish run status, failure family, redaction state,
  promotion status, verifier/scorer refs, artifact refs, proof refs, resource
  refs, route scorecard refs, and retained failure refs.
- Promotion decisions are evidence-only and cannot grant runtime promotion.

Gap:

- There is no StudyBench-native task, rubric-claim, evidence-span, claim-score,
  or rubric-score contract.

### Probe Retained Fixtures

Current files:

- `packages/probe/packages/runtime/src/benchmark/fixtures.ts`
- `packages/probe/packages/runtime/tests/benchmark-fixtures.test.ts`

Already implemented:

- Static retained Terminal-Bench fixture package.
- Seven retained task ids: `configure-git-webserver`, `db-wal-recovery`,
  `filter-js-from-html`, `gcode-to-text`, `pypi-server`, `query-optimize`, and
  `runner-stall-supervision`.
- Tool-menu constraints, failure families, score expectations, closeout
  requirements, GEPA stage refs, and public-refs-only boundary.
- Tests reject hidden task data and private Harbor trace material.

Gap:

- The fixture shape is Terminal-Bench-oriented. It does not model
  StudyBench-style `question`, `gold_answer`, weighted claims, or source spans.

### Probe Candidate Execution

Current files:

- `packages/probe/packages/runtime/src/benchmark/candidate-execution.ts`
- `packages/probe/packages/runtime/tests/benchmark-candidate-execution.test.ts`

Already implemented:

- `psionic.probe_gepa_candidate_manifest.v1` decoding.
- Candidate component hashes and candidate hash validation.
- Safety boundary validation that rejects new runtime authority and public-claim
  upgrade authority.
- Candidate execution adapter for retained Terminal-Bench fixtures.
- Validation that candidate-selected signatures and tools stay subordinate to
  the assignment and retained fixture constraints.

Gap:

- Candidate execution only adapts retained Terminal-Bench fixtures. It does not
  execute or score StudyBench answer-mode tasks, and it does not run an
  agentic patch-mode repo task.

### Probe Closeout Writer

Current files:

- `packages/probe/packages/runtime/src/benchmark/closeout-writer.ts`
- `packages/probe/packages/runtime/tests/benchmark-closeout-writer.test.ts`

Already implemented:

- A normalized closeout bundle with fixed files:
  `probe-run-record.json`, `probe-closeout.json`,
  `decision-trace-summary.json`, `selected-signatures.json`, `tool-menu.json`,
  `candidate-ref.json`, `artifact-refs.json`, `resource-usage-ref.json`,
  `policy-findings.json`, `failure-classification.json`, and
  `route-scorecard.json`.
- Live GEPA runner-gate projection that requires evidence refs and keeps public
  score, product promotion, and payout claim flags false unless external gates
  are supplied.

Gap:

- There is no `rubric-score.json`, `studybench-task-ref.json`, claim-score
  vector, evidence-use record, or source-span score artifact.

### Probe Tool And File-Mutation Runtime

Current files:

- `packages/probe/packages/runtime/src/llm/tool.ts`
- `packages/probe/packages/runtime/src/llm/tool-runtime.ts`
- `packages/probe/packages/runtime/src/file-mutation.ts`
- `packages/probe/packages/runtime/src/cli.ts`

Already implemented:

- Tool definitions and dispatch.
- Tool result projection.
- Workspace-safe write/edit/patch helpers with permission hooks, BOM handling,
  line-ending handling, per-file locks, diff previews, and workspace path
  resolution.
- CLI paths for backend smoke, completion, and chat flows.

Gap:

- There is no benchmark task runner that gives an agent a pinned repo checkout,
  a StudyBench row, an allowed tool menu, a fixed budget, and a verifier/scorer
  closeout path.

### Blueprint Evidence Boundaries

Current files:

- `packages/probe/packages/runtime/src/blueprint/contribution.ts`
- `packages/probe/packages/runtime/src/blueprint/signature-lookup.ts`
- `apps/openagents.com/workers/api/src/blueprint/repositories/action-submissions.ts`
- `apps/openagents.com/workers/api/src/blueprint/repositories/probe-contributions.ts`

Already implemented:

- Probe Blueprint contribution drafts are content-redacted and carry no runtime
  authority by default.
- Contribution release gates require approved review, fixtures, release gates,
  target refs, and no runtime authority.
- Action Submission proposals require evidence refs and a Blueprint approval
  policy ref, and remain pending approval.
- Repositories reject raw provider, payment, wallet, customer, source archive,
  raw run log, and private repo material.
- Signature lookup uses structured registry selection and risk/surface filters,
  not ad hoc keyword routing.

Gap:

- There are no contribution kinds, refs, or release-gate fixtures for
  StudyBench task authoring, evidence-span extraction, rubric scoring, or
  repo-study packets.

## MVP Cut Line

### In MVP

- Public external StudyBench calibration by dataset ref.
- `openagents.studybench_task.v0` task contracts in Probe runtime.
- Deterministic repo corpus manifest and source-span extraction for public
  `openagents` repo files.
- At least 10 public-retained OpenAgents rows committed as examples/regression.
- Private validation/holdout boundary documented and represented by refs, not
  committed private material.
- Claim-score and rubric-score artifacts in Probe closeout bundles.
- Answer-mode scorer for StudyBench rows.
- Agentic patch-mode runner for at least 2 OpenAgents repo-edit tasks.
- Psionic GEPA candidate feedback bridge that can consume claim-level failures.
- Forge Coder refs-only study packet projection.

### Not In MVP

- Customer-private repo ingestion.
- Marketplace publication.
- Payout eligibility or settlement.
- Public "trained repo expert" claims.
- Distributed Pylon campaign execution.
- Automatic runtime promotion.
- Fully hidden leaderboard claims.
- LoRA or weight training.

## Issue Roadmap

### MSB-MVP-00: Freeze MVP Scope And Source Boundaries

Home:

- `docs/research/machine-studying/`
- `docs/launch/`

Current anchors:

- `docs/research/machine-studying/2026-06-17-studybench-openagents-benchmark-audit.md`
- `docs/launch/2026-06-17-machine-studying-short-term-roadmap.md`

Implementation:

- Add a short source-boundary section to the machine-studying docs declaring:
  external StudyBench is calibration, OpenAgents retained rows are regression
  examples, private validation/holdout rows are not committed, and public
  product claims stay blocked.
- Name the MVP refs:
  `openagents_studybench.v0`, `openagents_repo_corpus_manifest.v0`,
  `openagents_study_packet.v0`, and `probe.studybench_rubric_closeout.v0`.

Acceptance:

- Docs state that answer-mode and patch-mode are both required for MVP.
- Docs state that StudyBench public rows cannot be standalone product-claim
  evidence.
- Docs state that private holdout rows cannot be included in study packets.

Verification:

- Docs review.

### MSB-MVP-01: Add Probe StudyBench Contracts

Home:

- `packages/probe/packages/runtime/src/benchmark/studybench.ts`
- `packages/probe/packages/runtime/src/index.ts`
- `packages/probe/packages/runtime/tests/studybench-contracts.test.ts`

Current anchors:

- `packages/probe/packages/runtime/src/contracts/benchmark.ts`
- `packages/probe/packages/runtime/tests/benchmark-contracts.test.ts`

Implementation:

- Add runtime schemas:
  - `openagents.studybench_task.v0`
  - `openagents.studybench_rubric_claim.v0`
  - `openagents.studybench_evidence_span.v0`
  - `openagents.studybench_dataset_package.v0`
  - `probe.studybench_claim_score.v0`
  - `probe.studybench_rubric_score.v0`
- Keep StudyBench base fields compatible with upstream:
  `id`, `topic`, `question`, `gold_answer`, `rubric`, and `evidence`.
- Add OpenAgents extensions:
  `repo`, `commit`, `corpusRef`, `visibility`, `authorityRefs`, `testRefs`,
  `forbiddenClaimRefs`, `privateMaterialPolicyRefs`, `expectedFiles`, and
  `budgetClass`.
- Add decoders and validators:
  - weights sum to 100;
  - every rubric `span_ids` entry resolves to an evidence span;
  - every evidence span has path, start line, end line, and excerpt;
  - visibility is one of `external_public_calibration`,
    `openagents_public_retained`, `openagents_private_validation`, or
    `openagents_private_holdout`;
  - public packages cannot include private validation or holdout rows;
  - public rows pass existing Probe public projection validation.
- Export the new schemas from `src/index.ts`.

Acceptance:

- New tests decode a valid public-retained OpenAgents task.
- New tests reject weights that do not sum to 100.
- New tests reject missing evidence spans.
- New tests reject private holdout rows inside a public package.
- New tests reject raw provider, wallet, payment, source archive, raw run log,
  private repo, and customer material.

Verification:

- `bun run --cwd packages/probe test studybench-contracts`
- `bun run --cwd packages/probe test benchmark-contracts`

### MSB-MVP-02: Add Deterministic Repo Corpus Manifest And Evidence-Span Extractor

Home:

- `packages/probe/packages/runtime/src/benchmark/repo-corpus-manifest.ts`
- `packages/probe/packages/runtime/tests/repo-corpus-manifest.test.ts`
- `docs/research/machine-studying/openagents-studybench/`

Current anchors:

- `packages/probe/packages/runtime/src/benchmark/fixtures.ts`
- `packages/probe/packages/runtime/tests/benchmark-fixtures.test.ts`
- `docs/launch/2026-06-17-machine-studying-short-term-roadmap.md`

Implementation:

- Add a deterministic corpus manifest builder for a pinned local repo root.
- Include path, byte size, sha256 digest, kind, source authority, and admitted
  visibility tier.
- Exclude `.git/`, `.claude/`, `.git-worktrees/`, `.pylon-local/`, `.secrets/`,
  caches, build outputs, `node_modules/`, `dist/`, DMGs, tarballs, temporary
  files, raw logs, local runtime state, wallet material, payment material, and
  private customer material.
- Add evidence-span extraction from manifest-admitted files by path and line
  range.
- Prefix extracted excerpts with line numbers in the StudyBench style.
- Hash each evidence span as deterministic refinery material.

Acceptance:

- Regenerating the same manifest for the same fixture tree yields the same
  file digests and manifest digest.
- Excluded paths never appear in manifest output.
- Evidence-span extraction fails closed if the file is not admitted, the line
  range is invalid, or the excerpt contains prohibited private material.
- Span hashes are stable across repeated runs.

Verification:

- `bun run --cwd packages/probe test repo-corpus-manifest`

Tassadar boundary:

- This is the first MVP slice that can later become
  `tassadar.repo_refinery_artifact.v0`, because it is deterministic.

### MSB-MVP-03: Register External StudyBench Calibration By Ref

Home:

- `packages/probe/packages/runtime/src/benchmark/studybench-external.ts`
- `packages/probe/packages/runtime/tests/studybench-external.test.ts`
- `docs/research/machine-studying/openagents-studybench/external-calibration.md`

Current anchors:

- `docs/research/machine-studying/2026-06-17-studybench-openagents-benchmark-audit.md`

Implementation:

- Add a public calibration manifest for:
  - `hf://jacobli/studybench/dspy`
  - optional later: `hf://jacobli/studybench/openclaw`
- Do not vendor upstream rows into the repo for MVP.
- Store dataset slug, config, expected split, license refs, source attribution
  refs, and loader instructions.
- Keep network fetching out of runtime validators. Runtime should validate
  loaded rows; operator scripts can fetch datasets.

Acceptance:

- A calibration manifest validates as public-safe.
- The manifest preserves CC-BY-4.0 and upstream MIT attribution refs.
- Tests prove no upstream full rows are required for runtime contract tests.

Verification:

- `bun run --cwd packages/probe test studybench-external`

### MSB-MVP-04: Author First Public-Retained OpenAgents Rows

Home:

- `docs/research/machine-studying/openagents-studybench/public-retained/openagents-launch-v0.jsonl`
- `docs/research/machine-studying/openagents-studybench/README.md`

Current anchors:

- `docs/launch/`
- `docs/research/machine-studying/`
- `docs/tassadar/`
- `docs/promises/`
- `packages/probe/docs/`
- `apps/openagents.com/workers/api/src/blueprint/`

Implementation:

- Author 10 public-retained rows.
- Include topics:
  - `launch_claims_and_promises`
  - `tassadar_projection_truth`
  - `settlement_and_wallet_truth`
  - `customer_one_evidence`
  - `forge_coder_repo_memory`
  - `blueprint_probe_gepa_contracts`
  - `pylon_assignment_wallet_readiness`
  - `studybench_schema_adaptation`
- Every row must include a question, gold answer, weighted rubric, and evidence
  spans from the pinned public corpus manifest.
- Rows should ask for concrete code/doc work, not trivia.

Acceptance:

- At least 10 rows validate through `openagents.studybench_dataset_package.v0`.
- Every row has at least one core claim.
- Every core claim has at least one evidence span.
- No row contains private material.
- Rows are explicitly marked `openagents_public_retained`.

Verification:

- `bun run --cwd packages/probe test studybench-contracts`
- Manual docs review.

### MSB-MVP-05: Define Private Validation And Holdout Storage Boundary

Home:

- `docs/research/machine-studying/openagents-studybench/private-boundary.md`
- optional local ignored path: `docs/research/machine-studying/openagents-studybench/private/.gitignore`

Current anchors:

- `packages/probe/packages/runtime/src/contracts/benchmark.ts`
- `apps/openagents.com/workers/api/src/blueprint/repositories/action-submissions.ts`
- `apps/openagents.com/workers/api/src/blueprint/repositories/probe-contributions.ts`

Implementation:

- Document where private validation and holdout rows live.
- Commit only public-safe dataset refs, split refs, checksums, and policy refs.
- Add or confirm `.gitignore` coverage for any local private row directory.
- Define leak response: retire leaked rows and mint a new holdout split.
- Define evaluator access: scorer may see gold/rubric/evidence for grading;
  candidate agents may not see private gold answers or private rubrics.

Acceptance:

- Public repo contains no private validation or holdout task text.
- Public docs include only split refs and checksums.
- The policy says private holdout cannot feed study packets or GEPA training.

Verification:

- Docs review.
- `rg -n "private_holdout|gold_answer" docs/research/machine-studying/openagents-studybench`
  should only show public retained examples and boundary docs unless the path is
  explicitly ignored/private.

### MSB-MVP-06: Add StudyBench Rubric Scorer Contracts And Score Artifacts

Home:

- `packages/probe/packages/runtime/src/benchmark/studybench-score.ts`
- `packages/probe/packages/runtime/tests/studybench-score.test.ts`

Current anchors:

- `packages/probe/packages/runtime/src/benchmark/closeout-writer.ts`
- `packages/probe/packages/runtime/tests/benchmark-closeout-writer.test.ts`

Implementation:

- Add claim score schema with:
  `claimId`, `claimType`, `weight`, `satisfied`, `scoreBps`,
  `evidenceSpanIds`, `rationaleRef`, and `scorerRef`.
- Add rubric score summary with:
  `taskId`, `candidateHash`, `weightedScoreBps`, `coreGatePassed`,
  `finalScoreBps`, `claimScores`, `goldAnswerRef`, `redactionState`, and
  `evidenceUseRefs`.
- Support two scoring modes:
  - `manual_or_judge_supplied`: MVP mode; validates an externally supplied
    claim-score vector.
  - `deterministic_check`: optional for tasks with exact tests.
- Enforce that weights in the score vector match the task rubric.
- If strict core gate is enabled and any core claim fails, final score is 0.

Acceptance:

- Tests compute weighted score correctly.
- Tests apply the core-claim gate.
- Tests reject score vectors that omit a rubric claim.
- Tests reject score vectors with unknown claim ids.
- Tests reject public summaries that include raw judge rationale text instead
  of rationale refs.

Verification:

- `bun run --cwd packages/probe test studybench-score`

### MSB-MVP-07: Extend Probe Closeout Bundles With StudyBench Score Refs

Home:

- `packages/probe/packages/runtime/src/benchmark/closeout-writer.ts`
- `packages/probe/packages/runtime/tests/benchmark-closeout-writer.test.ts`

Current anchors:

- `PROBE_BENCHMARK_CLOSEOUT_BUNDLE_FILE_NAMES`
- `makeProbeBenchmarkCloseoutBundle`
- `projectProbeGepaLiveRunnerGate`

Implementation:

- Add optional StudyBench closeout inputs:
  `studybenchTaskRef`, `studybenchScoreRef`, `studybenchRubricScore`, and
  `studybenchEvidenceUseRefs`.
- Emit either:
  - new bundle files `studybench-task-ref.json` and `rubric-score.json`, or
  - public-safe artifact refs in `artifact-refs.json` if backward-compatible
    fixed file names are preferred for the first code patch.
- Keep older Terminal-Bench closeouts valid.
- Ensure `projectProbeGepaLiveRunnerGate` imports StudyBench rubric-score refs
  as evidence only. It must not flip public score, promotion, or payout flags
  without separate gates.

Acceptance:

- Existing closeout writer tests still pass for Terminal-Bench.
- New tests show a StudyBench closeout carries task and rubric-score refs.
- New tests show failed StudyBench patch runs still include failure family,
  retained failure refs, and rubric score summary.
- Live-runner gate projection includes StudyBench evidence refs but keeps
  product/payout/public-score authority false without external gates.

Verification:

- `bun run --cwd packages/probe test benchmark-closeout-writer`
- `bun run --cwd packages/probe test studybench-score`

### MSB-MVP-08: Implement Answer-Mode StudyBench Runner

Home:

- `packages/probe/packages/runtime/src/benchmark/studybench-answer-runner.ts`
- `packages/probe/packages/runtime/tests/studybench-answer-runner.test.ts`

Current anchors:

- `packages/probe/packages/runtime/src/benchmark/candidate-execution.ts`
- `packages/probe/packages/runtime/src/llm/`

Implementation:

- Add `runProbeStudybenchAnswerCandidate`.
- Input:
  task row, assignment, candidate answer artifact/ref, scorer mode, optional
  candidate manifest, resource refs, and route scorecard refs.
- Validate task and assignment split compatibility.
- Validate that candidate agents do not receive private gold answers or private
  rubrics.
- Produce rubric score summary and normal Probe closeout bundle.
- Allow external calibration rows to run without repo patching.

Acceptance:

- A public-retained row can be scored from a supplied candidate answer.
- A failed answer produces claim-level feedback and retained failure refs.
- Private validation/holdout rows do not expose gold/rubric material to the
  candidate input.
- Candidate manifest safety validation remains in force.

Verification:

- `bun run --cwd packages/probe test studybench-answer-runner`

### MSB-MVP-09: Implement Agentic Patch-Mode StudyBench Runner

Home:

- `packages/probe/packages/runtime/src/benchmark/studybench-patch-runner.ts`
- `packages/probe/packages/runtime/tests/studybench-patch-runner.test.ts`

Current anchors:

- `packages/probe/packages/runtime/src/llm/tool-runtime.ts`
- `packages/probe/packages/runtime/src/file-mutation.ts`
- `packages/probe/packages/runtime/src/workspace.ts`
- `packages/probe/packages/runtime/src/benchmark/candidate-execution.ts`

Implementation:

- Add `runProbeStudybenchPatchCandidate`.
- Input:
  task row, assignment, pinned repo checkout ref, allowed tool menu, budget
  policy, test command refs, candidate manifest, runner identity, and scorer.
- Mount only task-visible material for the candidate:
  question, allowed files or corpus refs, public evidence spans if the task is
  public-retained, budget, and tool constraints.
- Do not mount private gold answers or private holdout rubrics into the agent
  loop.
- Run an agent/tool loop through existing tool dispatch and workspace mutation
  primitives, or accept a pre-recorded runner transcript for the first MVP
  slice if the live loop is not ready.
- Capture patch refs, command/test refs, transcript summary refs, failure
  family, resource refs, and rubric score refs.
- Emit the normal Probe closeout bundle.

Acceptance:

- At least two public-retained OpenAgents rows run in patch mode against a
  temporary fixture checkout.
- The runner enforces max tool calls and timeout budget.
- The runner rejects tools outside the assignment tool menu.
- The runner emits test refs and patch artifact refs.
- The runner emits claim-level rubric scores and closeout evidence.
- Failed runs become retained failure refs.

Verification:

- `bun run --cwd packages/probe test studybench-patch-runner`

### MSB-MVP-10: Add Psionic GEPA Feedback Bridge For StudyBench Claims

Home:

- `packages/probe/packages/runtime/src/benchmark/studybench-gepa-feedback.ts`
- `packages/probe/packages/runtime/tests/studybench-gepa-feedback.test.ts`
- parallel Psionic issue in `../psionic`

Current anchors:

- `packages/probe/packages/runtime/src/benchmark/candidate-execution.ts`
- `../psionic/docs/PROBE_GEPA_CANDIDATE_MANIFESTS.md`
- `../psionic/docs/PROBE_GEPA_ROLLOUT_COORDINATOR.md`

Implementation:

- Convert StudyBench rubric score summaries into GEPA feedback records:
  failed core claims, failed supporting claims, evidence spans missed, forbidden
  claim refs violated, tests skipped, wrong files touched, and budget failures.
- Add candidate target suite refs:
  `openagents_studybench.public_retained.v0` and
  `openagents_studybench.private_validation.v0`.
- Add optional candidate component or playbook refs for:
  `source_grounded_repo_study_policy`,
  `openagents_authority_navigation_policy`, and
  `studybench_patch_closeout_policy`.
- Keep Psionic optimizer acceptance separate from Probe runtime promotion and
  OpenAgents product claims.

Acceptance:

- A rubric score summary with failed claims produces deterministic feedback
  refs suitable for GEPA reflection.
- Feedback records omit raw private holdout answers and raw judge rationale.
- Candidate manifests still fail if they try to add runtime authority, bypass
  Blueprint gates, or upgrade public claims.

Verification:

- `bun run --cwd packages/probe test studybench-gepa-feedback`
- Psionic-side tests to be added in the paired repository.

### MSB-MVP-11: Add Blueprint Contribution And Action Submission Integration

Home:

- `packages/probe/packages/runtime/src/blueprint/contribution.ts`
- `apps/openagents.com/workers/api/src/blueprint/repositories/probe-contributions.ts`
- `apps/openagents.com/workers/api/src/blueprint/repositories/action-submissions.ts`
- corresponding tests in both packages.

Current anchors:

- Existing contribution family includes `context_package`, `retrieval_package`,
  `route_policy`, `tool_package`, and `program_signature`.
- Contributions and action submissions already reject private/raw material and
  require evidence refs.

Implementation:

- Add or map contribution kinds for:
  - `studybench.task_authoring.v0`
  - `studybench.evidence_span_extraction.v0`
  - `studybench.rubric_authoring.v0`
  - `studybench.rubric_judging.v0`
  - `repo_study_packet.v0`
- Keep all studybench contribution projections content-redacted and
  evidence-only.
- Require fixture refs and retained failure refs before release gate readiness.
- Ensure Action Submission can reference StudyBench closeout refs as evidence
  without granting direct mutation or execution authority.

Acceptance:

- StudyBench contribution drafts cannot carry runtime authority.
- Release-gate readiness requires review, fixtures, release gates, target refs,
  and retained failure refs.
- Action Submission proposals can reference study packet refs and rubric
  closeout refs as evidence, but remain pending approval.
- Unsafe refs with raw source, raw run logs, payment, wallet, provider, customer
  data, or private repo material are rejected.

Verification:

- `bun run --cwd packages/probe test blueprint-contribution`
- `bun run --cwd apps/openagents.com test -- blueprint/repositories/probe-contributions`
- `bun run --cwd apps/openagents.com test -- blueprint/repositories/action-submissions`

### MSB-MVP-12: Generate First Launch Study Packet

Home:

- `docs/research/machine-studying/openagents-studybench/study-packets/openagents-launch-study-packet-v0.md`
- eventual structured artifact in Probe or Blueprint docs.

Current anchors:

- `docs/launch/2026-06-17-machine-studying-short-term-roadmap.md`
- `docs/research/machine-studying/2026-06-17-tassadar-openagents-repo-studying-roadmap.md`
- `docs/research/machine-studying/2026-06-17-studybench-openagents-benchmark-audit.md`

Implementation:

- Build a compact packet from the pinned corpus manifest:
  source map, authority map, invariant map, trap catalog, test catalog,
  launch-edit playbooks, and glossary.
- Include refs to public-retained OpenAgents StudyBench rows.
- Exclude private validation and holdout rows.
- Include known failure patterns from public-retained attempts only.

Acceptance:

- Packet has a digest/ref.
- Packet cites source authority refs rather than broad summaries.
- Packet carries forbidden claim boundaries.
- Packet contains no private material or hidden answer keys.

Verification:

- Docs review.
- Optional schema validation once `repo_study_packet.v0` exists.

### MSB-MVP-13: Add Forge Autopilot Coder Refs-Only Projection

Home:

- `apps/openagents.com/apps/web/src/page/loggedIn/autopilot-work/`
- `docs/autopilot-coder/terminal-agent-systems/`
- potential API projection under `apps/openagents.com/workers/api/src/`

Current anchors:

- Existing Forge Autopilot Coder roadmap and current untracked/modified
  autopilot-work files in the worktree.
- `docs/launch/2026-06-17-machine-studying-short-term-roadmap.md`

Implementation:

- Add a refs-only model/projection for:
  `studyPacketRef`, `corpusManifestRef`, `studybenchDatasetRefs`,
  `publicRetainedScoreRef`, `privateValidationTrendRef`,
  `holdoutEvaluationRef`, `freshness`, and `blockedClaimRefs`.
- Do not expose raw private row content, hidden rubrics, hidden gold answers,
  raw repo archives, or private customer source.
- Do not give the packet mutation authority.

Acceptance:

- UI/model tests show the projection is evidence-only.
- UI/model tests reject or omit private/hidden material.
- Projection language says internal dogfood unless a product-promise gate
  changes state.

Verification:

- `bun run --cwd apps/openagents.com test -- autopilot-work`

### MSB-MVP-14: Run Baseline, Packet, And GEPA Candidate Comparison

Home:

- `packages/probe/docs/benchmarks/`
- `docs/research/machine-studying/openagents-studybench/runs/`

Current anchors:

- `packages/probe/docs/benchmarks/2026-06-08-pylon-gepa-coding-agent-benchmark-run.md`
- `packages/probe/docs/benchmarks/2026-06-08-probe-continual-benchmark-learning-apparatus.md`
- `packages/probe/docs/benchmarks/2026-06-08-probe-gepa-benchmark-system-closeout-audit.md`

Implementation:

- Run three fixed-budget conditions:
  - baseline with no study packet;
  - study packet mounted;
  - study packet plus GEPA-optimized candidate text bundle.
- For MVP, use public-retained rows and private validation rows only.
- Capture pass rate, weighted score, core-gate pass rate, token/tool budget,
  wrong-file reads, forbidden-claim violations, test pass rate, and closeout
  completeness.
- Emit a public-safe run summary with refs, not raw private rows.

Acceptance:

- At least 10 public-retained rows and 5 private validation rows are run.
- At least 2 rows run in patch mode.
- Every attempt has a Probe closeout bundle.
- The report distinguishes answer-mode from patch-mode scores.
- The report does not claim product readiness or customer availability.

Verification:

- Closeout bundle validation.
- Docs review.

### MSB-MVP-15: Product Promise And Marketplace Gate Review

Home:

- `docs/promises/`
- `docs/research/machine-studying/`
- Blueprint marketplace docs if/when this becomes product work.

Current anchors:

- Product-promise and launch docs.
- Blueprint contribution gates.
- Probe GEPA live-runner gate keeps promotion and payout flags false without
  external gates.

Implementation:

- Add a planned/yellow product-promise record only after internal validation
  shows lift.
- Keep marketplace package work out of MVP unless separate validation,
  metering, pricing, privacy, payout eligibility, and settlement gates exist.
- State that StudyBench rows and study packets are not automatically paid work.

Acceptance:

- No public copy says "trained repo expert", "customer repo studying is live",
  "marketplace package", or "payout eligible" before gates exist.
- Any promise record is evidence-linked and caveated.
- Public projection links to refs only.

Verification:

- Product-promise tests and docs review.

## MVP Dependency Order

1. MSB-MVP-00: freeze scope and boundaries.
2. MSB-MVP-01: Probe StudyBench contracts.
3. MSB-MVP-02: corpus manifest and evidence spans.
4. MSB-MVP-03: external StudyBench calibration manifest.
5. MSB-MVP-04: first public-retained OpenAgents rows.
6. MSB-MVP-05: private split boundary.
7. MSB-MVP-06: rubric scorer.
8. MSB-MVP-07: closeout score refs.
9. MSB-MVP-08: answer-mode runner.
10. MSB-MVP-09: patch-mode runner.
11. MSB-MVP-10: Psionic GEPA feedback bridge.
12. MSB-MVP-11: Blueprint contribution integration.
13. MSB-MVP-12: launch study packet.
14. MSB-MVP-13: Forge Coder projection.
15. MSB-MVP-14: fixed-budget comparison.
16. MSB-MVP-15: product promise and marketplace gate review.

## First Implementation Slice

The first code slice should be small:

1. Add `benchmark/studybench.ts` contracts and tests.
2. Add `benchmark/studybench-score.ts` contracts and tests.
3. Add 2 public-retained sample rows under
   `docs/research/machine-studying/openagents-studybench/public-retained/`.
4. Add closeout bundle refs for rubric score without changing runtime
   promotion behavior.

This slice proves the data model and closeout shape before building the full
agentic patch runner.

## MVP Exit Criteria

The MVP is complete when:

- Public external StudyBench calibration rows can be validated by ref.
- At least 10 OpenAgents public-retained rows validate.
- At least 5 private validation rows exist outside public repo content and are
  represented by public-safe refs.
- Answer-mode runs emit StudyBench rubric score closeouts.
- Patch-mode runs emit StudyBench rubric score closeouts for at least 2
  OpenAgents repo-edit tasks.
- A launch study packet improves at least one fixed-budget metric against
  baseline on validation rows.
- GEPA candidate feedback can consume claim-level failures without leaking
  private holdout material.
- Forge Coder can show refs-only study packet freshness and score refs.
- No public product, marketplace, payout, or trained-model claim has been
  upgraded by the benchmark lane itself.

## Risks

- Public StudyBench contamination makes external calibration look stronger than
  it is. Mitigation: never use it as the only product-claim evidence.
- Public-retained OpenAgents rows can become memorized. Mitigation: use them
  for regression and examples only.
- Private holdout leakage invalidates claims. Mitigation: keep rows out of the
  public repo, retire leaked rows, and mint a new split.
- Patch-mode runner can accidentally expose gold answers. Mitigation: separate
  candidate-visible task packets from scorer-visible gold/rubric packets.
- Rubrics can become subjective. Mitigation: require source-grounded claims and
  exact evidence span ids.
- Study packets can bypass authority boundaries. Mitigation: mount as refs-only
  context and keep Action Submission, Blueprint, product-promise, and Probe
  gates active.
- The lane can drift into keyword routing. Mitigation: use typed task topics,
  structured refs, semantic retrieval/planner selection, and no ad hoc
  user-facing intent routing.

## Non-Goals To Keep Repeating

- This is not model weight training.
- This is not a customer repo ingestion product yet.
- This is not marketplace work yet.
- This does not grant write authority.
- This does not grant settlement or payout authority.
- This does not make Tassadar responsible for model answer quality.
- This does not make Psionic optimizer acceptance into runtime promotion.
- This does not replace Terminal-Bench; it complements Terminal-Bench with
  source-grounded claim rubrics.
