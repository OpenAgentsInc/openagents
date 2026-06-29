# Task: Create GitHub Issue Series For Pylon-Distributed Probe Benchmark Optimization

## Goal

Create a series of GitHub issues that turns the current Probe / GEPA / Pylon / Benchmark Cloud plan into executable work.

The issue series should create a path where:

1. Probe becomes the first-class coding-agent runtime under test.
2. public benchmark-cloud owns benchmark manifests, splits, artifact contracts, and score imports.
3. GEPA optimizes Probe/Blueprint text candidates first, not model weights.
4. Pylon workers receive bounded benchmark rollout assignments.
5. Those assignments can become useful paid work once payout/settlement gates are enabled.
6. Terminal-Bench 2 through Harbor on the SHC box is the first live benchmark lane.
7. The result improves Probe’s coding performance through measured retained, validation, and eventually holdout evidence.

Use the source material in `docs/benchmarks/` as the authority, especially:

* `docs/benchmarks/README.md`
* `docs/benchmarks/2026-06-08-workspace-benchmark-systems-audit.md`
* `docs/benchmarks/2026-06-08-probe-continual-benchmark-learning-apparatus.md`
* `docs/benchmarks/2026-06-08-pylon-gepa-coding-agent-benchmark-run.md`
* `docs/benchmarks/2026-06-08-omni-continual-learning-training-loop.md`
* `docs/benchmarks/2026-06-08-artanis-gepa-benchmark-pylon-focus.md`

Important conceptual boundary:

Do **not** describe the GEPA lane as distributed neural-network training. Describe it as **Pylon-distributed benchmark-driven optimization** or **distributed GEPA rollout optimization**. GEPA benefits from distribution because many benchmark rollouts can run independently across workers. The coordinator can remain centralized while Pylons execute metric-call work slices.

## Required Output

Create GitHub issues across the owning repos. Do not put every issue in Probe.

Use this ownership model:

* `probe`: runtime, benchmark assignment intake, candidate execution, closeout evidence, local fixtures, selected signatures/tool menus.
* `openagents`: public Benchmark Cloud contracts, split manifests, artifact/proof bundle contracts, Terminal-Bench runner lane, Pylon benchmark package surfaces if applicable.
* `openagents`: Artanis public projection, OpenAgents product surface/Pylon assignment lease adaptation, release gates, public claim boundaries.
* `psionic`: GEPA coordinator, candidate frontier, candidate manifests, reflection/proposal jobs, later LoRA/Qwen training path.
* `pylon` or `openagents`: worker capability envelopes, assignment receipt schema, benchmark-capable worker admission, artifact/proof submission path.

If repo boundaries are uncertain, create the issue in the most likely owning repo and include a “Cross-repo dependencies” section.

## Issue Series Structure

Create issues in this order.

---

## Epic 1: Probe Benchmark Closeout Foundation

### Issue 1 — Probe: Add benchmark assignment and closeout schemas

Title:

`Add Probe benchmark assignment and closeout schemas`

Body:

Define typed Probe schemas for benchmark work so Probe can participate in public Benchmark Cloud and Pylon-distributed GEPA rollouts.

Probe should add schemas for:

* `probe.benchmark_assignment.v1`
* `probe.benchmark_run.v1`
* `probe.benchmark_closeout.v1`
* `probe.benchmark_decision_trace.v1`
* `probe.prompt_candidate.v1`
* `probe.blueprint_candidate.v1`
* `probe.tool_menu_candidate.v1`
* `probe.loop_policy_candidate.v1`
* `probe.benchmark_promotion_decision.v1`

The assignment schema should include:

* benchmark run ref
* task run ref
* dataset slug and version
* split ref
* public-safe task checksum or task ref
* Probe commit
* runtime/backend profile
* model/backend ref
* account/grant refs, if applicable
* selected Blueprint signature refs
* tool-menu ref
* prompt/candidate hash
* timeout/budget policy ref
* required artifact refs
* callback/proof sink refs

The closeout schema should include:

* assignment ref
* run ref
* candidate hash
* selected signatures
* tool menu
* backend route
* verifier/scorer refs
* artifact manifest refs
* proof bundle refs
* resource/cost refs
* policy findings
* failure classification
* retained-failure refs, if applicable
* redaction state
* promotion status

Hard boundaries:

* No raw provider credentials.
* No raw benchmark secrets.
* No hidden verifier content.
* No wallet/payment material.
* No private repo refs.
* No unbounded raw logs.
* No public claim upgrade authority.
* No runtime promotion authority.

Acceptance criteria:

* Schemas compile and are covered by tests.
* Invalid closeouts reject missing artifact/proof refs.
* Unsafe fields are rejected or scrubbed.
* Failed/timed-out runs can still emit valid closeout records.
* The schema can represent retained, validation, holdout, and live evidence separately.

---

### Issue 2 — Probe: Implement benchmark closeout writer

Title:

`Emit normalized Probe benchmark closeout bundles`

Body:

Implement a closeout writer that emits normalized benchmark evidence after a Probe benchmark run.

The writer should produce a bundle compatible with public benchmark-cloud expectations:

* `probe-run-record.json`
* `probe-closeout.json`
* `decision-trace-summary.json`
* `selected-signatures.json`
* `tool-menu.json`
* `candidate-ref.json`
* `artifact-refs.json`
* `resource-usage-ref.json`, or explicit unavailable reason
* `policy-findings.json`
* `failure-classification.json`, when relevant

The writer should support successful, failed, timed-out, and policy-blocked runs.

Acceptance criteria:

* A fake benchmark assignment can run and emit a complete closeout bundle.
* A failed run emits retained-failure refs and failure classification.
* A timed-out run emits timeout state and partial artifact refs.
* No raw logs or unsafe data are copied into public-safe artifacts.
* Tests cover success, failure, timeout, and unsafe-field rejection.

---

### Issue 3 — Probe: Add retained coding benchmark fixture package

Title:

`Add retained Terminal-Bench failure fixture package for Probe`

Body:

Add a retained fixture package for the known Terminal-Bench failure families that Probe should optimize against first.

Initial retained tasks/families:

* `configure-git-webserver` → `service_readiness`
* `db-wal-recovery` → `database_recovery` / `sqlite_wal_recovery`
* `filter-js-from-html` → `parser_correctness` / `xss_sanitizer_policy`
* `gcode-to-text` → `parser_correctness` / `gcode_parser_guard`
* `pypi-server` → `package_indexing` / `python_package_index`
* `query-optimize` → `query_optimization`
* runner stall case → `runner_supervision`

Each fixture should include:

* task id
* benchmark suite ref
* split membership
* expected failure family
* expected Blueprint signature refs
* expected tool-menu constraints
* expected artifact/closeout requirements
* score expectation or verifier expectation where safe

Acceptance criteria:

* Fixture package loads in Probe tests.
* Each fixture maps to typed failure family enum values.
* Each fixture maps to expected Blueprint signature refs.
* No hidden benchmark data or private Harbor traces are committed.
* Fixtures can be used by GEPA Stage 0/1 retained runs.

---

## Epic 2: Public Benchmark Cloud Contracts

### Issue 4 — OpenAgents: Create public Benchmark Cloud contract package

Title:

`Create public Benchmark Cloud contracts for Probe coding benchmarks`

Body:

Move or rebuild the useful private Cloud Benchmark Cloud contracts into public OpenAgents infrastructure.

Add public contracts for:

* `BenchmarkTask`
* `BenchmarkResult`
* `BenchmarkEvent`
* `BenchmarkArtifactManifest`
* `BenchmarkProofBundle`
* `openagents.resource_usage_receipt.v1`
* benchmark split manifest
* benchmark run manifest
* scorer/verifier ref
* no-cheat metadata
* redaction state

Target layout:

* `openagents/docs/benchmarks/`
* `openagents/crates/benchmark-cloud/`
* `openagents/scripts/benchmarks/`
* `openagents/fixtures/benchmarks/`

Acceptance criteria:

* Contracts can represent Terminal-Bench 2 through Harbor.
* Contracts can represent retained, validation, holdout, and live evidence.
* Contracts can import Probe closeout refs.
* Failed, timed-out, and errored runs still require artifact/proof bundle records.
* Public claim boundaries are documented.

---

### Issue 5 — OpenAgents: Add Terminal-Bench split manifest for GEPA Stage 0/1

Title:

`Add Terminal-Bench retained and validation split manifests for Probe GEPA`

Body:

Create public split manifests for the first Probe GEPA campaign.

Stage 0/1 retained lane should include the retained Terminal-Bench failures and local Probe acceptance fixtures.

The manifest should distinguish:

* retained fixtures
* validation split
* frozen holdout split
* local smoke fixtures
* public-safe task refs
* scorer/verifier versions
* task selector version
* allowed claim state

Acceptance criteria:

* Manifest is content-addressed or otherwise stable.
* Manifest prevents silent reshuffling.
* Manifest identifies retained vs validation vs holdout.
* Manifest includes no hidden verifier content or private benchmark data.
* Probe and Psionic can reference the manifest by stable ref.

---

### Issue 6 — OpenAgents: Rebuild true-Probe Terminal-Bench runner lane

Title:

`Add true Probe adapter to public Benchmark Cloud Terminal-Bench runner`

Body:

Add a public Benchmark Cloud runner lane that invokes current Probe as the agent under test, instead of using the deprecated/private adapter pattern.

The runner should:

* invoke Probe with a normalized benchmark assignment JSON
* pass only safe account/grant refs, not raw credentials
* stream or collect Probe event refs
* collect `probe-closeout.json`
* write normalized Benchmark Cloud artifacts
* preserve existing result/proof/resource records
* support Terminal-Bench through Harbor on SHC first

Required artifacts:

* `result.json`
* `events.jsonl`
* `metadata.json`
* `artifact_manifest.json`
* `proof_bundle.json`
* `resource_usage_receipt.json`
* `probe-run-record.json`
* `probe-closeout.json`

Acceptance criteria:

* Fake task pass/timeout/error runners work.
* One retained Terminal-Bench fixture can run through Probe.
* The runner preserves artifacts on success and failure.
* Probe-selected signatures and tool menu appear in the proof bundle by ref.
* No raw secrets or private traces are emitted.

---

## Epic 3: GEPA Candidate Optimization

### Issue 7 — Psionic: Add GEPA text-bundle candidate manifest

Title:

`Add GEPA text-bundle candidate manifests for Probe optimization`

Body:

Create Psionic-side manifests for GEPA candidates that optimize structured text artifacts controlling Probe/Blueprint behavior.

Initial candidate components:

* `probe_system_prompt`
* `terminal_bench_global_playbook`
* `signature_selection_policy`
* `tool_menu_policy`
* `patch_and_test_policy`
* `failure_family_playbooks`
* `closeout_policy`

The candidate manifest should include:

* candidate id
* parent candidate id
* campaign id
* candidate hash
* component hashes
* target suites
* target failure families
* split refs
* optimizer run id
* training/evaluation trace digests
* policy gate state
* promotion state

Promotion states:

* `draft`
* `optimizer_accepted`
* `shadow`
* `release_candidate`
* `active`
* `rejected`
* `reverted`

Acceptance criteria:

* Candidate manifests are content-addressed.
* Component hashes are stable.
* Optimizer acceptance is distinct from runtime promotion.
* Candidate manifests can be imported by Probe and Benchmark Cloud.
* Unsafe candidate fields cannot request new runtime authority or bypass release gates.

---

### Issue 8 — Psionic: Implement GEPA coordinator for Pylon-distributed rollouts

Title:

`Implement GEPA coordinator using Pylon-distributed benchmark rollouts`

Body:

Implement a GEPA coordinator that treats Pylon as a parallel evaluator backend.

The coordinator should:

* create or load a candidate text bundle
* assign candidate/task rollout jobs
* collect normalized evaluator side information
* call GEPA reflection/proposal step
* maintain candidate frontier and lineage
* preserve iteration metrics and cost summaries
* support resumable state
* support evaluation cache
* export candidate refs for Probe/OpenAgents product surface/Artanis

Important language:

This is distributed benchmark-driven optimization, not distributed neural-network training. Pylons run independent metric-call rollouts. The GEPA coordinator performs candidate selection/reflection centrally.

Rollout result fields:

* task id
* dataset
* split
* Probe commit
* agent slug
* backend/model
* candidate hash
* selected signatures
* tool menu
* verifier status
* scalar score
* failure family
* artifact manifest ref
* proof bundle ref
* resource usage ref
* policy findings
* cost/duration

Acceptance criteria:

* Local evaluator backend works before Pylon backend.
* Coordinator can run Stage 0 with 20–40 metric calls.
* Coordinator can resume after interruption.
* Candidate hashes and rollout refs are preserved.
* Failed infrastructure jobs are distinguished from model/agent failures.
* Policy-violating candidates cannot advance.

---

### Issue 9 — Probe + Psionic: Add Probe candidate execution adapter

Title:

`Run Probe benchmark tasks with supplied GEPA candidate text bundles`

Body:

Add support for Probe to run a benchmark assignment with a supplied GEPA text-bundle candidate.

Probe should be able to consume:

* candidate manifest ref
* selected candidate components
* Benchmark Cloud assignment ref
* split manifest ref
* backend route profile
* Blueprint signature constraints
* tool-menu constraints

Probe should emit:

* candidate hash in closeout
* selected signatures
* projected tool menu
* candidate component refs
* verifier result refs
* failure classification
* policy findings

Acceptance criteria:

* Probe can run the same retained fixture with baseline and candidate bundle.
* Closeout clearly identifies candidate hash.
* Candidate text cannot bypass Blueprint authority.
* Tool-menu changes remain typed and policy-subordinate.
* Tests compare baseline vs candidate closeout shape.

---

## Epic 4: Pylon Work Slices And Paid-Work Path

### Issue 10 — OpenAgents product surface/Pylon: Adapt assignment lease lifecycle for GEPA metric-call batches

Title:

`Support GEPA benchmark metric-call assignments in OpenAgents product surface/Pylon lease lifecycle`

Body:

Adapt the existing OpenAgents product surface/Pylon assignment lease lifecycle for Probe GEPA metric-call work.

Do not invent a separate benchmark work-state protocol unless required.

Assignment should include:

* campaign id
* benchmark suite ref
* split ref
* task ref
* Probe commit
* candidate hash
* backend/runtime requirements
* expected artifacts
* verifier/scorer refs
* timeout/budget
* payout/payment mode
* closeout requirements

Lifecycle:

* assignment created
* worker lists owned assignment
* worker accepts
* worker reports progress
* worker submits artifact/proof refs
* evaluator/operator closes as accepted or rejected work
* payment evidence ref is attached only when real payout path exists

Acceptance criteria:

* A Pylon can accept a GEPA metric-call assignment.
* A Pylon can submit progress and artifact/proof refs.
* Operator/evaluator can close assignment as accepted or rejected.
* Unpaid/no-spend smoke assignments are explicitly marked.
* Accepted work is not described as settled payout unless settlement evidence exists.
* Assignment records are importable by GEPA coordinator.

---

### Issue 11 — Pylon/OpenAgents: Add benchmark-capable worker capability envelope

Title:

`Add benchmark-capable Pylon worker capability envelope`

Body:

Define worker capability declarations for Pylons that can run Probe benchmark and GEPA rollout work.

Capability fields should include:

* benchmark runner support
* Harbor/Terminal-Bench support
* Probe runtime support
* local model support
* Apple FM support, if applicable
* Qwen adapter support, if applicable
* MLX-class training support, if applicable
* CPU/RAM/disk/GPU constraints
* max wall-clock budget
* max cost budget
* isolation profile
* artifact upload support
* proof/receipt support
* assignment lease support
* closeout support

Acceptance criteria:

* Workers can advertise benchmark capability without overclaiming.
* Scheduler can match GEPA metric-call work to capable workers.
* Capability envelope distinguishes rollout/eval work from model-training work.
* Worker admission does not imply payout readiness.
* Capability records are public-safe.

---

### Issue 12 — OpenAgents product surface/Pylon/OpenAgents: Add paid-work mode flags for benchmark assignments

Title:

`Add explicit paid, unpaid, credit, and no-spend modes for Pylon benchmark work`

Body:

Add explicit payment mode fields for Pylon benchmark assignments so public claims stay honest.

Payment modes:

* `unpaid_smoke`
* `operator_credit`
* `payable_pending_settlement`
* `settled_bitcoin`
* `rejected_no_pay`

Assignment and closeout records should distinguish:

* accepted work
* payable work
* settled work
* rejected work
* no-spend evidence refs
* payment/settlement receipt refs

Acceptance criteria:

* No assignment can imply payout without a settlement ref.
* Public projection can say “accepted unpaid smoke work” safely.
* Public projection can say “settled bitcoin payout” only with receipt refs.
* GEPA coordinator can run unpaid Stage 0/1 without confusing claims.
* Later paid Stage 1/2 batches can attach payment evidence.

---

## Epic 5: Stage 0 And Stage 1 Campaign

### Issue 13 — Cross-repo: Run GEPA Stage 0 retained-fixture smoke

Title:

`Run GEPA Stage 0 smoke for Probe retained benchmark optimization`

Body:

Run a small GEPA-only smoke campaign.

Scope:

* 3–6 retained fixtures
* 2 workers or SHC-only fallback
* 20–40 metric calls
* text-bundle candidates only
* no LoRA
* no model training
* no public leaderboard claim
* no automatic promotion

Goals:

* prove assignment format
* prove candidate hashing
* prove Probe candidate execution
* prove artifact/proof writing
* prove verifier imports
* prove GEPA resumability
* prove public-safe closeout import

Acceptance criteria:

* Campaign id exists.
* Split manifest exists.
* Candidate baseline and at least one mutated candidate exist.
* Probe closeout bundles exist.
* Benchmark Cloud proof bundles exist.
* Pylon assignment refs exist if Pylon was used.
* Accepted/rejected closeout refs exist.
* Artanis/OpenAgents product surface can project a public-safe “measured retained smoke” status.
* No production promotion occurs.

---

### Issue 14 — Cross-repo: Run GEPA Stage 1 retained-failure sprint

Title:

`Run GEPA Stage 1 retained-failure sprint through Pylon metric-call batches`

Body:

Run the first real Pylon-distributed GEPA retained-failure sprint.

Scope:

* retained Terminal-Bench failure families
* local Probe acceptance cases
* 8–16 Pylon workers where available
* 200–400 metric calls
* text-bundle candidates only
* no LoRA/model training
* no public leaderboard claim

Goals:

* learn candidate text bundle that improves retained failures
* preserve rollout receipts
* compare baseline/champion/candidate
* classify failures
* identify regressions
* produce candidate suitable for validation consideration

Acceptance criteria:

* Campaign uses public split manifest.
* Each rollout has candidate hash, task ref, verifier ref, artifact/proof ref, and resource ref.
* Pylon worker assignments close as accepted or rejected.
* Payment mode is explicit for every assignment.
* Candidate improves or preserves retained fixtures without policy-gate failure.
* Candidate enters `optimizer_accepted` or `rejected`, not `active`.
* Public summary names retained evidence only.

---

### Issue 15 — OpenAgents/Benchmark Cloud: Run selected SHC Terminal-Bench validation sweep

Title:

`Run selected SHC Terminal-Bench validation sweep for GEPA candidate`

Body:

After Stage 1 succeeds, run a selected Terminal-Bench validation sweep on SHC through Harbor.

Compare:

* current Probe champion
* GEPA candidate
* baseline backend route, where useful

Use tasks including:

* `db-wal-recovery`
* `configure-git-webserver`
* `pypi-server`
* `filter-js-from-html`
* `gcode-to-text`
* `query-optimize`

Acceptance criteria:

* SHC run uses public Benchmark Cloud contracts.
* Probe closeout bundles are preserved.
* Candidate hash and Probe commit are recorded.
* Split is validation, not holdout.
* Cost, duration, verifier result, and artifact availability are recorded.
* Public claim does not say “Probe beats Terminal-Bench.”
* Candidate may move to `shadow` only if OpenAgents product surface/Blueprint gates approve.

---

## Epic 6: Artanis And Public Projection

### Issue 16 — OpenAgents product surface/Artanis: Add Probe GEPA campaign projection fields

Title:

`Add Artanis public projection fields for Probe GEPA benchmark campaigns`

Body:

Extend Artanis/OpenAgents product surface public report shape with benchmark-campaign fields.

Fields:

* `campaignRef`
* `objectiveRef`
* `stage`
* `claimState`
* `benchmarkSuiteRefs`
* `splitManifestRefs`
* `probeCommitRefs`
* `baselineCandidateRef`
* `activeCandidateRefs`
* `candidateHashRefs`
* `pylonBatchRefs`
* `plannedMetricCalls`
* `completedMetricCalls`
* `validMetricCalls`
* `invalidMetricCalls`
* `retainedResultRefs`
* `validationResultRefs`
* `holdoutResultRefs`
* `artifactManifestRefs`
* `receiptRefs`
* `costSummaryRefs`
* `resourceReceiptRefs`
* `policyFindingRefs`
* `blockerRefs`
* `promotionDecisionRefs`
* `nextActionRefs`

Public projection must not include:

* raw prompts
* raw traces
* raw benchmark fixtures
* provider credentials
* account refs
* bearer material
* wallet material
* invoices/preimages
* private repo paths
* local filesystem paths

Acceptance criteria:

* Artanis can summarize Stage 0/1 campaign status from refs.
* Public route distinguishes retained, validation, and holdout evidence.
* Claim state cannot be upgraded without evidence refs.
* Public projection can show Pylon work without implying payout unless settlement refs exist.

---

### Issue 17 — OpenAgents product surface/Artanis: Add public-safe Forum summary generator for Probe GEPA campaigns

Title:

`Generate public-safe Artanis Forum summaries for Probe GEPA campaigns`

Body:

Add a publication-prep path that generates public-safe Forum copy from retained refs and campaign state.

The generator should produce copy for existing Artanis threads or a new benchmark-specific topic.

It should summarize:

* campaign id
* current stage
* dataset/split
* candidate hash
* completed metric calls
* valid/invalid rollout count
* Pylon assignment refs
* artifact/proof refs
* verifier/scorer refs
* policy findings
* blockers
* next action
* claim boundary

Hard boundary:

Probe may prepare public-safe copy or reply as its own registered agent. Probe must not post as Artanis or invoke the Artanis bridge. Posting as Artanis requires the existing OpenAgents product surface/operator authority path.

Acceptance criteria:

* Generated copy contains no secrets or raw traces.
* Generated copy uses exact claim-state language.
* Retained improvements are not described as public benchmark scores.
* Validation wins are not described as frozen holdout performance.
* Forum copy can be regenerated idempotently from refs.

---

## Epic 7: Route Scorecards And Product Impact

### Issue 18 — Probe/OpenAgents product surface: Add route scorecards for benchmark runs

Title:

`Add route scorecards for Probe benchmark and coding-agent runs`

Body:

Add route scorecards that explain which backend/runner/provider was used and why.

Minimum fields:

* selected model or agent
* selected runner
* selected provider
* selected isolation profile
* selected verifier
* expected cost
* observed cost
* expected latency
* observed latency
* privacy tier
* trust tier
* selected signatures
* tool menu
* candidate hash
* rejected routes
* route reason
* post-closeout route score

Rejected routes are important evidence. If Codex was used instead of local Probe, or SHC instead of Pylon, or remote API instead of Apple FM, record why.

Acceptance criteria:

* Every benchmark closeout can include a route scorecard.
* Scorecard supports comparing Codex, Probe+Codex, Apple FM, local Qwen, SHC, and Pylon routes.
* Scorecard is public-safe.
* Scorecard can feed future route selection and product accepted-outcome analysis.

---

### Issue 19 — Probe/OpenAgents product surface: Connect benchmark evidence to Coding on Autopilot accepted outcomes

Title:

`Connect Probe benchmark learning evidence to Coding on Autopilot outcome metrics`

Body:

Add a planning/projection issue that connects benchmark improvements to buyer-facing Coding on Autopilot metrics.

Metrics:

* acceptance rate
* human review minutes
* turns per accepted outcome
* cost per accepted outcome
* retry count
* route scorecard
* artifact completeness
* public/private proof state
* failure family reduction
* regression count
* closeout quality

Acceptance criteria:

* Benchmark wins are not treated as product wins unless connected to accepted coding outcomes.
* OpenAgents product surface can display whether a candidate is benchmark-only, shadow, release candidate, or active.
* Product metrics can compare before/after for accepted workrooms.
* Claim text distinguishes benchmark validation from paid customer outcome improvement.

---

## Issue Labeling

Use labels consistently:

* `area:probe`
* `area:benchmark-cloud`
* `area:pylon`
* `area:psionic`
* `area:openagents`
* `area:artanis`
* `area:gepa`
* `area:terminal-bench`
* `type:schema`
* `type:runner`
* `type:projection`
* `type:paid-work`
* `type:proof`
* `stage:0-smoke`
* `stage:1-retained`
* `stage:2-validation`
* `claim-boundary`
* `blocked-by-payout`
* `blocked-by-release-gate`

## Milestone Structure

Create milestones:

1. `Probe Benchmark Closeout MVP`
2. `Public Benchmark Cloud Probe Lane`
3. `GEPA Stage 0 Smoke`
4. `Pylon GEPA Stage 1 Retained Sprint`
5. `SHC Terminal-Bench Validation`
6. `Artanis Public Campaign Projection`
7. `Paid Pylon Benchmark Work`

## Public Claim Rules To Include In Relevant Issues

Allowed early claims:

* OpenAgents is building the public Benchmark Cloud apparatus.
* Probe emits benchmark/runtime evidence.
* Pylon workers can run admitted rollout or benchmark jobs.
* A GEPA candidate improved a named retained or validation split, if split, candidate hash, verifier, and artifact state are shown.
* Public-safe artifact manifests and receipt sets exist for a specific batch.

Blocked claims:

* Probe beats Terminal-Bench from retained or validation evidence.
* Pylon benchmark work is paid or settled without settlement receipts.
* GEPA candidate is production without OpenAgents product surface/Blueprint release gates.
* Distributed GEPA rollouts are distributed neural-network training.
* Pylon is generally ready for download or earning from assignment closeout alone.
* Local or Apple routes replaced frontier backends without route scorecards and accepted-outcome evidence.

## Final Deliverable

After creating the issues, produce a summary comment or tracking document with:

* issue links grouped by epic
* owning repo for each issue
* dependencies
* which issues create Pylon work slices
* which issues are required before paid work
* which issues most directly improve Probe benchmark performance
* which issues are claim-boundary/projection only

Prioritize issues that unlock real Pylon work slices:

1. Probe closeout bundle
2. public Benchmark Cloud split manifest
3. GEPA candidate manifest
4. Pylon metric-call assignment type
5. Stage 0 smoke
6. Stage 1 retained-failure sprint

Do not start with LoRA, Qwen training, or broad model-training claims. The first campaign is GEPA-only, text-bundle-only, retained/validation measured, and promotion-gated.
