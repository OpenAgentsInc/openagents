# Workspace Benchmark Systems Audit

Date: 2026-06-08
Status: audit and Probe integration roadmap
Scope: Probe, private Cloud Benchmark Cloud source material, public OpenAgents
Benchmark Cloud target architecture, Psionic legal benchmark lanes, OpenAgents product surface
product/evidence contracts, OpenAgents Nexus/Pylon benchmark package surfaces,
historical Autopilot/Backroom source material, and external reference repos.

## Executive Summary

Probe should not grow into a standalone benchmark product. The active workspace
already has several benchmark systems with separate jobs:

- Private `cloud` currently contains the coding benchmark execution lane:
  normalized `BenchmarkTask` runs, Terminal-Bench 2 through Harbor,
  Codex-compatible runner adapters, custom repo and SWE/SWT-style adapters,
  proof bundles, and resource-usage receipts. That apparatus should move or be
  rebuilt into public `openagents`; private `cloud` should become source
  material, not benchmark authority.
- Psionic owns legal benchmark execution and training evidence: Harvey-style
  Rust schemas, artifact receipts, deterministic public suites, synthetic
  legal signature-routing fixtures, Qwen legal adapter training receipts,
  distributed Pylon-worker SFT simulations, and promotion-gate inputs.
- OpenAgents product surface owns product projection and review contracts: read-only public
  Benchmark Cloud evidence, Probe coding-runtime run projections, Model Lab
  promotion decisions, public/private redaction, and false authority flags for
  launch, payment, routing, deployment, and public-claim upgrades.
- OpenAgents Nexus/Pylon owns compute-market registration, benchmark package
  refs, training-run benchmark package requirements, provider capability
  envelopes, and Pylon `benchmark_lane_available` admission facts.
- Deprecated Autopilot4, Autopilot3, Backroom, and workspace docs contain
  useful source material, but they are not active implementation homes.

Probe's role is the runtime and evidence bridge. It should consume benchmark
assignments and Blueprint signature selections, execute or delegate coding-agent
sessions, emit redacted runtime evidence, preserve retained failures, and make
public benchmark-cloud, Psionic, and OpenAgents product surface able to compare raw Codex,
Probe+Codex, local Apple FM, swarm inference, and future backends against the
same benchmark evidence contracts.

The most important implementation gap is not "add a benchmark runner to
Probe." The gap is to make Probe a first-class agent slug in existing benchmark
lanes, with typed assignment intake, signature lookup, safe event/proof export,
and replayable retained-failure receipts.

## Current Probe State

Probe is currently a Bun/Effect v4 workspace with a minimal runtime package.
It has no Terminal-Bench runner, no legal benchmark runner, and no benchmark
orchestration service.

What Probe does have:

- OpenAgents product surface account and grant contracts for ChatGPT/Codex account linking.
- Per-run auth materialization and scrub contracts.
- Runner identity gating for SHC/Pylon/sandbox use.
- Apple FM backend contracts, tool callback sessions, availability/failure
  receipts, and retained Apple FM acceptance cases.
- Blueprint consumer contracts, static registry fixtures, typed signature
  lookup, backend-independent tool-menu planning, Program Run evidence, action
  submission boundaries, and contribution/release-gate models.

The retained Apple FM acceptance cases in
`packages/runtime/src/backends/apple-fm/acceptance.ts` are currently Probe's
closest benchmark-like runtime checks:

- `read_file_answer`
- `list_then_read`
- `search_then_read`
- `shell_then_summarize`
- `patch_then_verify`
- `approval_pause_or_refusal`

Those are acceptance checks for local backend/tool behavior. They are not
external benchmark scores.

The old Rust Probe implementation is deprecated. Prior workspace audits record
useful patterns from that tree: a retained acceptance runner, richer reporting
fields, replay and decision dataset exports, and optimizer-facing comparison
scorecards. The new Probe should harvest those patterns into a first-party
Bun/Effect runtime only when they support the final product surface.

## Active Benchmark Inventory

| Area | Active repo | Current authority | Probe implication |
| --- | --- | --- | --- |
| Terminal-Bench / coding tasks | `openagents` target, private `cloud` source material today | public benchmark-cloud runner and SHC/GCP execution; private Cloud runner is backfill | Rebuild or move the runner contracts into public OpenAgents, then add Probe as a runtime/agent adapter and evidence emitter. |
| Retained coding failures / signatures | `openagents` target, private `cloud` source material, `probe`, `openagents` | Public retained fixtures plus Probe Blueprint signature lookup plus OpenAgents product surface release gates | Move fixture playbooks into public retained fixture packages and route through typed lookup. |
| Legal / Harvey-style benchmark execution | `psionic` | Psionic Rust legal benchmark engine | Probe should run legal-agent sessions only under Psionic task envelopes and return answer/tool/evidence refs. |
| Legal Qwen training and Pylon SFT | `psionic`, `openagents` | Psionic training receipts; Nexus/Pylon package and worker admission | Probe should provide coding/legal-agent runtime traces; Psionic remains training/eval authority. |
| GEPA prompt/Blueprint optimization | `psionic`, `openagents`, `probe` | GEPA-style candidate manifests, Pylon rollout receipts, public benchmark-cloud split manifests | Start with text-bundle candidates over retained Terminal-Bench failures before LoRA or broad model training. |
| Product benchmark projection | `openagents` | Read-only public benchmark-cloud and Probe run projections | Probe emits safe refs and closeout evidence that OpenAgents product surface can project. |
| Provider/node benchmark admission | `openagents` | Nexus/Pylon compute benchmark packages and benchmark lane availability | Probe should advertise benchmark-capable runtime profiles, not mutate admission state directly. |
| Historical Coder evals | `autopilot3` | Deprecated/older product smoke and UI replay suite | Reuse scenario ideas for Probe/OpenAgents product surface acceptance, not implementation. |
| Historical legal benchmark governance | `autopilot4-deprecated` | Deprecated Rust/Maud campaign/source material | Reuse protocol/release-gate ideas; new authority is OpenAgents product surface plus Psionic. |
| Historical Terminal-Bench HUD | `backroom` | Source material only | Reuse UI/user-story ideas if Probe gets a local HUD, not as active runtime code. |
| External references | `projects/repos/benchmarks`, `projects/repos/harvey-labs` | Read-only reference repos | Study harness patterns only; do not vendor. |

## public benchmark-cloud Target

Private `cloud/docs/BENCHMARK_CLOUD.md` is the current source-material execution
plan. It should not remain the canonical implementation home because the
benchmark apparatus needs to be public. Rebuild or move the useful parts into
`openagents` as a public Benchmark Cloud area.

The proposed public layout is:

- `openagents/docs/benchmarks/` for specs, runbooks, audits, and claim
  boundaries.
- `openagents/crates/benchmark-cloud/` for normalized benchmark
  contracts, artifact manifests, proof bundles, and scheduler-facing Rust code.
- `openagents/scripts/benchmarks/` for local proof, SHC, and Harbor commands.
- `openagents/fixtures/benchmarks/` for public retained fixtures and replay
  inputs.
- `openagents/proto/openagents/benchmark/` only if a stable cross-language
  protocol is needed.

Benchmark Cloud should remain a general benchmark/workload execution lane with
Terminal-Bench 2 as the first dataset adapter. It is not a leaderboard and not
a public claim authority.

The normalized runner contract is:

- `BenchmarkTask`
- `BenchmarkResult`
- `BenchmarkEvent`
- `BenchmarkArtifactManifest`
- `BenchmarkProofBundle`
- `openagents.resource_usage_receipt.v1`

The private local runner lives in `cloud/runners/py-bench-runner`. The public
OpenAgents runner should preserve the same artifact behavior: write required
artifacts even for failed, timed-out, or errored runs.

- `result.json`
- `events.jsonl`
- `metadata.json`
- `artifact_manifest.json`
- `proof_bundle.json`
- `resource_usage_receipt.json`

Dataset adapters currently cover:

- fake local pass/timeout/error tasks;
- Terminal-Bench through Harbor;
- OpenAgents/Codex-style Terminal-Bench runs;
- `custom-repo`;
- `swe-bench`;
- `swt-bench`;
- retained Probe+Codex signature-routing fixtures.

The Codex adapter supports agent slugs including:

- `codex`
- `openagents-codex`
- `openagents-coder`
- `probe-codex`
- `probe-codex-signatures`
- `openagents-probe-codex`

For `probe-codex`, the private adapter already injects a signature prompt
addendum when the task metadata includes `signatureRouting`. That is the first
concrete Probe-shaped benchmark integration, but today it is implemented in
private Cloud's Python runner rather than Probe's own runtime or public
benchmark-cloud.

### Measured Terminal-Bench Evidence

The measured SHC reports are internal substrate evidence, not public
Terminal-Bench claims.

`CND-050` records a one-task SHC smoke on `oa-shc-katy-01`:

- dataset: `terminal-bench@2.0`
- task: `terminal-bench/openssl-selfsigned-cert`
- agent: Harbor `codex`
- Codex package: `@openai/codex@0.135.0`
- model: `gpt-5.5`
- reward: `1.0`
- verifier: 6 passed, 0 failed
- reported cost: `$0.120667`

`CND-051` records a selected 8-task smoke:

- dataset size reported by Harbor: 89 tasks
- tasks run: 8
- reward-1 tasks: 6
- mean reward: `0.75`
- reported cost: `$3.697649`
- failed tasks: `filter-js-from-html`, `vulnerable-secret`

`CND-052` records a preserved selected 16-task smoke:

- tasks run: 16
- reward-1 tasks: 11
- mean reward: `0.6875`
- reported cost: `$13.300340`
- failed tasks: `configure-git-webserver`, `db-wal-recovery`,
  `gcode-to-text`, `query-optimize`, `pypi-server`
- raw Harbor traces and tarballs preserved on the SHC host, not committed

This is exactly the evidence shape Probe should plug into: run the runtime,
preserve artifacts, emit proof bundles, and keep public claims disabled until
the proof projection can safely disclose dataset/version, task selector, agent,
model, retry policy, costs, artifacts, verifier/scorer result, and redaction
state.

### Retained Terminal-Bench Signature Fixtures

`CND-053` and `CND-054` define the retained coding-agent improvement lane.

The private retained fixtures currently live under:

```text
cloud/runners/py-bench-runner/fixtures/signature-routing/
```

Covered failure families:

| Failure family | Retained task | Expected signature |
| --- | --- | --- |
| service readiness | `configure-git-webserver` | `coding.service_readiness` |
| local PyPI/simple index | `pypi-server` | `coding.python_package_index` |
| query optimizer workflow | `query-optimize` | `coding.query_optimizer_workflow` |
| SQLite/WAL recovery | `db-wal-recovery` | `coding.sqlite_wal_recovery` |
| G-code parser contract | `gcode-to-text` | `coding.gcode_parser_guard` |
| XSS sanitizer policy | `filter-js-from-html` | `coding.xss_sanitizer_policy` |
| runner stall | `query-optimize` operational stall | `benchmark.runner_supervisor` |

The private evaluator reports:

- retained fixtures: 7
- improved fixtures: 7
- raw Codex mean reward: `0.000`
- expected Probe+signature mean reward: `0.900`
- expected mean reward delta: `+0.900`

Only `db-wal-recovery` has preserved account-backed live evidence for the full
`0.0 -> 1.0` rerun after the learned SQLite/WAL rule was revised. The rest are
retained-regression expected reward fixtures until rerun live.

Probe should pull this signature lane into its own Blueprint lookup/runtime
surface. public benchmark-cloud should own the dataset harness, and
Probe should own signature package selection, tool-menu projection, prompt/tool
context, and closeout evidence for Probe-backed slugs.

The GEPA follow-up plan makes this the first optimizer lane. Start with a
GEPA-only campaign over structured text bundles, not LoRA or model training.
The first retained sprint should use Pylon as the metric-call engine, public
benchmark-cloud as the split and artifact authority, and Probe as
the runtime that evaluates candidate prompts, Blueprint usage instructions,
tool-menu policy, and closeout policy.

This is distributed benchmark-driven optimization, not distributed
neural-network training. Pylons run independent Probe rollouts for candidate
text bundles and return verifier results, artifacts, receipts, and failure
summaries. The GEPA coordinator updates the text-candidate frontier. Reserve
distributed training language for later Psionic/Qwen/LoRA/SFT/DPO/GRPO lanes
where workers contribute to model-weight, adapter, checkpoint, or
training-data work.

## Psionic Legal Benchmark System

Psionic is the active legal benchmark and legal training substrate.

`psionic/docs/LEGAL_BENCHMARK_ENGINE.md` defines the current legal benchmark
engine boundary:

- upstream Harvey Python is reference/backfill only;
- Psionic owns Rust legal benchmark execution/evaluation substrate;
- Autopilot/OpenAgents product surface Blueprint policy owns upgradable prompts/modules, provider
  adapter selection, judge policy, release gates, and promotion;
- provider names such as Gemini, OpenAI-compatible local servers, and Qwen
  fine-tunes are adapter metadata, not benchmark authority.

The core schema surface in `crates/psionic-eval/src/legal_benchmark.rs`
includes:

- `BenchmarkTaskSpec`
- `ArtifactManifest`
- `SourceArtifact`
- `DeliverableSpec`
- `CriterionSpec`
- `JudgePolicy`
- `ToolPolicy`
- `RunConfig`
- `RunRecord`
- `TranscriptEvent`
- `ToolCallRecord`
- `RunMetrics`
- `CoverageSnapshot`
- `CriterionResult`
- `ScoreReport`
- `ComparisonReport`

The legal engine records answer integrity: required answer files must be
created by model-authored write/edit tool calls, with pre-score and post-score
hashes preserved. Scoring cannot mutate the final answer and still count as a
valid run.

The engine also has:

- failed trajectory capture for bad-run examples;
- public/synthetic legal signature-routing fixtures;
- deterministic replay suites;
- SFT/DPO/reward/GRPO data builders;
- sharding manifests and worker receipts;
- adapter manifests and promotion decisions;
- Pylon worker job protocols;
- settlement handoff evidence hooks.

### Legal Signature Routing

`psionic/docs/LEGAL_BENCHMARK_SIGNATURE_ROUTING.md` defines the public/synthetic
legal signature-routing fixture lane for Probe+Codex. It tests whether typed
legal failure families select the right Probe signatures without exposing
hidden Harvey labels or private scoring rubrics.

Fixture code:

- `crates/psionic-eval/src/legal_benchmark_signature_routing.rs`
- `crates/psionic-eval/examples/legal_benchmark_signature_routing_report.rs`

Fixture files:

- `fixtures/legal_benchmark/signature_routing/harvey_public_synthetic_signature_routing_suite.json`
- `fixtures/legal_benchmark/signature_routing/harvey_public_synthetic_signature_routing_report.json`

Covered failure families and Probe signatures:

| Failure family | Expected signatures |
| --- | --- |
| `missing_deliverable` | `legal.deliverable_file_workflow`, `legal.output_path_contract`, `legal.answer_integrity_guard` |
| `wrong_output_path` | `legal.output_path_contract`, `legal.deliverable_file_workflow`, `legal.answer_integrity_guard` |
| `source_grounding_missing` | `legal.source_grounding_trace`, `legal.citation_provenance_check`, `legal.answer_integrity_guard` |
| `citation_provenance_missing` | `legal.citation_provenance_check`, `legal.source_grounding_trace`, `legal.answer_integrity_guard` |
| `answer_integrity_invalid` | `legal.answer_integrity_guard`, `legal.deliverable_file_workflow`, `legal.output_path_contract` |
| `judge_supervisor_needed` | `benchmark.legal_judge_supervisor`, `legal.answer_integrity_guard`, `legal.source_grounding_trace` |

Current retained report:

- fixtures: 6
- selector pass rate: `10000` bps
- raw Codex deterministic fixture mean: `2222` bps
- Probe+Codex deterministic fixture mean: `10000` bps
- mean fixture delta: `7777` bps

This is a workflow/evidence gate, not proof of live legal quality or private
Harvey performance.

Probe's missing work is clear: legal signatures need to exist in the same
Blueprint signature lookup/tool-menu system as coding signatures, and a
Probe-backed legal run must return required answer files, source refs, answer
integrity receipts, score report refs, and judge sidecar refs to Psionic.

### Legal Qwen Training And Pylon SFT Evidence

`psionic/docs/QWEN_LEGAL_FINETUNE_LANE.md` defines a substantial legal
fine-tuning lane. Important implemented surfaces include:

- `psionic-train legal ft <command>` operator command catalog;
- SFT, DPO, reward, and GRPO dataset builders;
- local Qwen3.6-27B target-path smoke;
- public Harvey three-task deterministic suite;
- two-worker Pylon/Psionic local distributed SFT milestone;
- signed worker receipts and payable decision records;
- adapter merge receipts and promotion receipts;
- Bitcoin/Lightning settlement evidence hooks, while payment execution remains
  Treasury/Nexus-owned.

Recorded public deterministic milestones:

- `reports/legal-ft-milestone-001.md`
  - champion score: `3333` bps
  - candidate score: `10000` bps
  - delta: `6667` bps
  - candidate promoted: `true`
  - private benchmark tasks used for training: `false`
- `reports/legal-ft-distributed-run-001.md`
  - two local Pylon workers
  - all worker receipts signed: `true`
  - all worker outputs hash verified: `true`
  - all worker payments payable: `true`
  - champion score: `3333` bps
  - candidate score: `10000` bps
  - decision: `Promote`
- `reports/qwen36-27b-legal-ft-001.md`
  - model: `Qwen/Qwen3.6-27B`
  - model load verified: `true`
  - base score: `3333` bps
  - promoted candidate: `qwen36_27b_sft_grpo_round_001`
  - promoted score: `10000` bps
  - no Python invoked: `true`
  - private benchmark tasks used for training: `false`

All of these are bounded public/deterministic or target-path milestones. They
do not claim private Harvey scores or production legal reasoning quality.

### Harvey Reference Audit

Workspace docs under `projects/` audited `projects/repos/harvey-labs` at
commit `5aa41694`.

Corpus inventory from the audit:

- tasks: 1,251
- practice areas: 24
- rubric criteria: 74,990
- source documents: 9,537
- expected deliverables: 1,655

The reusable pattern is:

1. A task directory contains `task.json` and read-only documents.
2. The agent runs in a closed workspace with a small tool surface.
3. The model writes named deliverables under `output/`.
4. Deliverables are graded against many small binary rubric criteria.
5. Task-level scoring is all-pass, while criterion pass rate also matters.

Probe should not import Harvey data or code. It should implement the runtime
side of that pattern: bounded legal-agent sessions, tool policies, answer file
receipts, source provenance, and redacted transcript export.

## OpenAgents product surface Product/Evidence Contracts

OpenAgents product surface is the active product surface for `openagents.com`. The relevant
benchmark contracts are read-only evidence surfaces.

`openagents/docs/omni/2026-06-06-benchmark-cloud-evidence-contract.md`
and `workers/api/src/omni-model-lab-benchmark-cloud.ts` define
`OmniBenchmarkCloudRecord` and projections for:

- suites;
- tasks;
- eval jobs;
- scorecards;
- regressions;
- flakes;
- comparisons;
- aggregate Benchmark Cloud packets.

The contract rejects raw/private benchmark inputs, raw datasets, provider
payloads, raw logs, model weights, payment/wallet material, private repos, raw
timestamps, and mutable authority. It also keeps these authority booleans hard
false:

- benchmark launch;
- eval execution;
- payment spend;
- payout mutation;
- provider mutation;
- public claim upgrade;
- raw benchmark input copy;
- routing mutation;
- runtime promotion;
- settlement mutation.

`workers/api/src/probe-coding-runtime-contract.ts` defines OpenAgents product surface's current
Probe run projection:

- `OpenAgentsProbeRunRequest`
- `OpenAgentsProbeTurnEvent`
- `OpenAgentsProbeToolCallSummary`
- `OpenAgentsProbeRunRecord`
- `OpenAgentsProbeRunProjection`

Succeeded Probe runs require closeout receipt refs plus artifact or diff refs.
Failed and timed-out Probe runs require failure refs plus retained-failure refs.
Unsafe raw logs, provider payloads, credentials, private repo refs, wallet or
payment material, and raw timestamps are rejected.

For benchmark work, this means Probe should emit:

- safe turn refs;
- safe tool-call summaries;
- diff refs;
- artifact refs;
- test/benchmark result refs;
- retained-failure refs;
- closeout receipts;
- cost/resource refs when available.

OpenAgents product surface then decides whether those refs are enough for product review, Model Lab
promotion, public reporting, or provider admission.

## OpenAgents Nexus/Pylon Benchmark Package Surfaces

`openagents` contains the compute-market and Pylon side of benchmark admission.

Nexus control defines benchmark package refs used by training and validation
runs. Important constants include:

- `benchmark://a1-minimal-distributed-lm/validation-loss-v1`
- `benchmark://harvey/legal-benchmark/smoke-eval-v1`
- `dataset://openagents/legal-benchmark/harvey-smoke@v1`

The kernel supports registering and listing `ComputeBenchmarkPackage` records.
Training run definitions can require benchmark packages, and the scheduler
checks whether a node has the benchmark lane available before assigning
benchmark-required work.

Pylon training capability envelopes carry:

- `benchmark_lane_available`;
- work-class eligibility for validation replay and adapter training;
- per-work-class `benchmark_lane_required` flags;
- backend family, memory, throughput, replay, artifact-upload, and capability
  labels.

This is not benchmark execution. It is admission and routing. Probe should
eventually report runtime/backend facts that help a Pylon or SHC node prove it
can run a requested benchmark-backed assignment, but Probe should not register
benchmark packages, decide payments, or mark providers accepted.

### 2026-06-08 OpenAgents product surface Pylon Assignment Update

The active OpenAgents product surface Nexus/Pylon docs add a concrete assignment lifecycle that the
Probe benchmark apparatus should reuse:

- `openagents/docs/nexus/2026-06-07-pylon-agent-api-runbook.md`
- `openagents/docs/nexus/2026-06-07-pylon-network-readiness-release-freeze.md`
- `openagents/docs/nexus/2026-06-08-pylon-live-assignment-closeout-smoke.md`

The OpenAgents product surface Worker now exposes Pylon registration, heartbeat, wallet-readiness,
payout-target admission, owned assignment listing, assignment accept/progress,
artifact/proof submission, public-safe payment/settlement evidence refs, and
operator assignment closeout. The #502 production smoke proved:

```text
pylon.issue502.local.20260608024927
-> assignment.public.issue502.20260608024927
-> accept/progress/artifact-proof events
-> operator closeout
-> state accepted_work
```

For Probe, this means future GEPA and benchmark batches should map benchmark
work into OpenAgents product surface/Pylon assignment leases with accepted or rejected closeout refs.
Do not create a separate Pylon benchmark state machine unless the live lease
path cannot carry a benchmark field after focused review.

The same docs also keep the boundary sharp. #502 is not a release-unfreeze
event and not a paid-work proof. Real bitcoin payout, payout-target approval,
repeated multi-host jobs, failure drills, and the next package release remain
separate #503 through #505 gates. Probe benchmark docs must therefore treat
the assignment lease path as an execution/closeout substrate, not as payment,
settlement, or broad Pylon earning authority.

## Historical Source Material

### Autopilot4 Deprecated

`autopilot4-deprecated` contains a broad legal benchmark governance system:

- `src/benchmark_baseline.rs`
- `src/psionic_benchmark.rs`
- `src/benchmark_imports.rs`
- `src/benchmark_dashboard.rs`
- `src/work_orders.rs`
- `src/legal_benchmark_protocol.rs`
- `src/benchmark_improvement_planner.rs`
- `src/benchmark_candidates.rs`
- `src/benchmark_release_gates.rs`
- `src/benchmark_campaign_workflow.rs`
- `src/benchmark_review.rs`
- `src/benchmark_github_export.rs`

Docs such as `2026-05-19-harvey-benchmark-system-audit-runbook.md` describe an
older split where Psionic executes benchmark jobs and Autopilot4 imports,
governs, hill-climbs, and exports failure-cluster issues.

Useful source-material patterns:

- legal benchmark operating protocol;
- ordered modules for document inventory, evidence mapping, issue/fact
  extraction, deliverable outlining, coverage planning, revision planning, and
  final self-check;
- hidden-rubric isolation;
- release gates;
- failure clusters;
- GitHub issue export from benchmark misses;
- pinned Harvey baseline fixture imports.

Do not route new work there. New product authority belongs in OpenAgents product surface and new
runtime work belongs in Probe/Psionic/Cloud.

### Autopilot3

`autopilot3/docs/coder-eval-and-smoke-suite.md` and
`src/lib/coder/evalSuite.ts` define an older Coder eval suite with 12 required
scenarios:

- repo inspection with source citation;
- approved GitHub issue creation;
- bounded edit/test;
- approved draft PR delivery;
- approval denial;
- human handoff controls;
- missing repository;
- failed checkout;
- runtime limit receipt;
- repository index cache disclosure;
- multi-repo task planning;
- UI replay receipt.

This is useful as a product regression checklist for future OpenAgents product surface/Probe
workroom acceptance. It is not the active benchmark runner.

### Backroom

`backroom/autopilot-benchmarks/BENCHMARKS.md` records an older generic coding
benchmark suite with file operations, git operations, testing, code generation,
debugging, documentation, issue workflow, error handling, optimization, and
integration tasks.

`backroom/reference/openagents-docs/hud/TERMINAL-BENCH.md` and related testing
docs record a Terminal-Bench HUD concept with live run/task messages,
WebSocket/RPC flow, task browsing, run history, and E2E gaps.

These are good UI and user-story references if Probe later gets a local HUD or
OpenAgents product surface gets a benchmark workroom. They are not active execution code.

## Other Psionic Benchmark/Eval Families

Psionic has additional benchmark/eval surfaces that are not directly Probe
coding-agent benchmarks but matter for the broader "all benchmark stuff"
inventory:

- Parameter Golf contest readiness, CUDA training coverage, RunPod/H100
  receipts, record-folder compatibility, promotion receipts, and submission
  evidence.
- Compiled Agent module evals that separately measure route selection,
  tool-policy correctness, tool-argument correctness, grounded answers, and
  verify/fallback/refusal behavior.
- Tassadar article, Sudoku, plugin conformance, universality, and compiled
  weight eval reports.
- Gemma 4 26B competitive benchmark gates comparing Psionic local inference
  against same-host Ollama and `llama.cpp`, with fail-closed sparse-execution
  and throughput checks.
- CSM speech benchmark fixtures and MLX example benchmark fixtures.

These lanes mostly inform Psionic training/eval infrastructure and Model Lab.
Probe should consume them only when a Probe runtime assignment is explicitly
under evaluation or when a Blueprint signature/tool policy needs retained
evidence from those evals.

## Probe Integration Roadmap

### 1. Add Probe Benchmark Assignment Intake

Add Probe-side schemas for normalized benchmark assignment intake. These should
mirror Cloud/OpenAgents product surface refs without copying raw benchmark inputs:

- `probe.benchmark_assignment.v1`
- `probe.benchmark_result_ref.v1`
- `probe.benchmark_closeout.v1`

The assignment should carry:

- benchmark run ref;
- task run ref;
- dataset slug and version;
- task ref or public-safe task checksum;
- runtime/backend profile;
- model/account grant refs;
- selected Blueprint signature refs;
- required artifact refs;
- tool policy refs;
- timeout/budget refs;
- callback/proof sink refs.

It should not carry raw provider credentials, raw benchmark secrets, raw
dataset payloads, hidden verifier details, wallet material, or unbounded logs.

### 2. Make Probe The Owner Of Signature Context

Move the selected-signature logic out of ad hoc benchmark prompt strings and
into Probe's Blueprint signature lookup/tool-menu path.

The coding retained fixtures should become Probe signature packages or
Blueprint Program Signatures for:

- `coding.service_readiness`
- `coding.python_package_index`
- `coding.query_optimizer_workflow`
- `coding.sqlite_wal_recovery`
- `coding.gcode_parser_guard`
- `coding.xss_sanitizer_policy`
- `benchmark.runner_supervisor`

The legal retained fixtures should become Probe legal signatures for:

- `legal.deliverable_file_workflow`
- `legal.output_path_contract`
- `legal.source_grounding_trace`
- `legal.citation_provenance_check`
- `legal.answer_integrity_guard`
- `benchmark.legal_judge_supervisor`

Selection must stay typed and structured. Do not add keyword matching over
prompts or task text. Use exact refs, structured failure-family enums,
Blueprint registry entries, or embedding/semantic selectors once the central
selector exists.

### 3. Rebuild Benchmark Cloud In Public OpenAgents For Real Probe

Private Cloud's `probe-codex` slug currently behaves like a Codex adapter with
a Probe signature addendum. The next step is to rebuild the runner lane in
public OpenAgents and add a true Probe adapter that launches the Probe
CLI/runtime and lets Probe decide backend routing.

The adapter should:

- start Probe with a benchmark assignment JSON file;
- pass auth/account refs through the existing materialization system;
- stream Probe events to the public OpenAgents artifact recorder;
- write `probe-run-record.json`;
- write `probe-closeout.json`;
- write retained-failure refs on failure or timeout;
- preserve the existing normalized `BenchmarkResult` and proof bundle.

public benchmark-cloud should keep dataset setup, Harbor wrapping,
GCP/SHC scheduling, and proof bundle normalization.

### 4. Add Legal Benchmark Bridge To Psionic

Probe should accept a Psionic legal task envelope for public/synthetic legal
runs and return:

- submitted answer file refs;
- source grounding refs;
- citation/provenance refs;
- answer integrity receipt refs;
- tool-call summary refs;
- redacted transcript refs;
- judge sidecar refs when required;
- retained failure refs when invalid.

Psionic should remain the scorer, training data builder, adapter promoter, and
Pylon worker receipt authority.

### 5. Emit OpenAgents product surface-Safe Benchmark Evidence

Probe closeout should map into OpenAgents product surface's existing contracts:

- `OpenAgentsProbeRunRecord` for runtime view;
- `OmniBenchmarkCloudRecord` refs for benchmark evidence;
- Model Lab Training Run or Promotion Decision refs when a benchmark is tied to
  a candidate;
- retained-failure refs for failed or timed-out sessions;
- resource/cost refs with explicit unavailable-token reasons when exact token
  counts are not available.

Probe should never mark product work accepted, promote runtime behavior,
publish benchmark claims, pay providers, or settle Pylons.

### 6. Add Probe Local Smoke Matrix

Before live benchmark reruns, add a local smoke matrix inside Probe:

- Apple FM retained acceptance cases.
- Fake benchmark assignment decode/closeout.
- Probe+signature prompt/tool-menu projection.
- Retained coding fixture dry-run using one public fixture.
- Retained legal fixture dry-run using one public/synthetic fixture.
- Failure closeout with retained-failure refs.

This should run with `bun run test` and should not require live benchmark data,
provider credentials, or SHC access.

### 7. Re-run Retained Failures Live

After the Probe adapter exists:

1. Run raw Codex retained fixture.
2. Run Probe+Codex without selected signatures.
3. Run Probe+Codex with fixed selected signatures.
4. Run Probe with any local/swarms/API backend profile under the same task
   envelope.
5. Preserve proof bundles and cost/resource receipts.
6. Import public-safe refs into OpenAgents product surface/public Benchmark Cloud evidence.

Start with `configure-git-webserver` for coding and one public/synthetic legal
fixture for legal.

## Execution Issue Series

`docs/benchmarks/plan.md` supersedes this audit's older near-term issue list.
Use it as the source of truth when opening implementation issues.

The ordered issue series is broader than Probe:

- Probe closeout foundation: benchmark assignment/closeout schemas, normalized
  closeout writer, and retained coding fixture package.
- Public Benchmark Cloud contracts: public contract package, retained and
  validation split manifests, and true-Probe Terminal-Bench runner lane.
- GEPA candidate optimization: text-bundle candidate manifests, the Psionic
  GEPA coordinator, and Probe candidate execution adapter.
- Pylon work slices and paid-work path: OpenAgents product surface/Pylon metric-call assignments,
  benchmark-capable worker envelopes, and explicit paid/unpaid/credit/no-spend
  payment modes.
- Stage 0/1 execution: retained-fixture smoke, Pylon-distributed retained
  sprint, and selected SHC Terminal-Bench validation.
- Artanis/public projection: campaign report fields and Forum summary
  generator.
- Route scorecards/product impact: benchmark route scorecards and connection
  to Coding on Autopilot accepted-outcome metrics.

Open issues in the order specified by `plan.md`. The first practical unlocks
are Probe closeout bundles, public Benchmark Cloud split manifests, GEPA
candidate manifests, Pylon metric-call assignments, Stage 0 smoke, and Stage 1
retained-failure sprint.

## Hard Boundaries

- Do not copy raw benchmark traces into Probe docs.
- Do not copy provider auth, ChatGPT auth JSON, API keys, cookies, wallet
  material, payment proofs, private repo refs, or benchmark-local secrets into
  artifacts or issue comments.
- Do not call retained expected-reward fixtures a public score.
- Do not treat local public Harvey/Qwen deterministic milestones as private
  Harvey performance.
- Do not let Probe choose product acceptance, payout, settlement, provider
  admission, or public claim state.
- Do not preserve old Probe/Autopilot compatibility paths without real callers.
- Do not add ad hoc keyword routing for benchmark, legal, tool, or signature
  selection.

## Bottom Line

The benchmark work is already real, but it is distributed:

- Private Cloud can execute coding benchmarks and preserve proof bundles today;
  that capability should move or be rebuilt into public OpenAgents Benchmark
  Cloud.
- Psionic can execute and train on legal benchmarks.
- OpenAgents product surface can project benchmark evidence safely.
- OpenAgents Nexus/Pylon can admit benchmark-capable compute.
- Probe can become the runtime that makes those benchmark lanes comparable
  across Codex, API-key models, Apple FM, local inference, swarm inference, and
  future OpenAgents backends.

The path to parity is therefore not to rebuild every benchmark system inside
Probe. The path is to make Probe a first-class benchmark runtime participant:
typed assignments in, Blueprint signatures selected, tools executed, evidence
and retained failures out.
