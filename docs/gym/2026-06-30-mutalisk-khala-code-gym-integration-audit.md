# Mutalisk -> Gym -> Khala Code Integration Audit

Date: 2026-06-30

Status: internal architecture audit. Direction-setting only. This document
does not claim a shipped UI feature, does not publish a benchmark result, does
not promote a candidate, and does not change runtime authority.

## 0. Executive Answer

Mutalisk belongs in the Gym as the offline optimizer lane for Khala Code's
fleet-delegation policy. It should not be a hidden step inside the Khala Code
chat loop, and it should not run Python, DSPy, or GEPA inside a Cloudflare
Worker. The right shape is:

1. Khala Code starts or follows an owner-gated Gym optimization run.
2. The Gym creates a typed run/job record and progress projection.
3. An out-of-process Mutalisk worker runs GEPA over public-safe delegation
   examples and feedback.
4. Mutalisk emits a `khala.fleet.delegation` candidate manifest/artifact.
5. OpenAgents ingests the candidate summary into the existing Effect admission
   loop.
6. The result becomes an approval-required Action Submission proposal, never an
   automatic live policy change.

The blunt readiness answer: this is not ready for UI testing yet. The local
Mutalisk side can produce offline candidate artifacts, and OpenAgents already
has most of the data, feedback, admission, and `khala.fleet.delegate` runtime
pieces. What is missing is the product seam that lets Khala Code ask the Gym to
run Mutalisk, follow progress, ingest the candidate into OpenAgents storage, and
show an admission-ready result without manual CLI work.

## 1. Sources Read

Gym docs:

- `docs/gym/README.md`
- `docs/gym/openagents-gym.md`
- `docs/gym/ROADMAP.md`
- `docs/gym/2026-06-24-openagents-gym-issues-6164-6166-audit.md`
- `docs/gym/2026-06-25-gym-opencode-head-to-head-and-khala-flywheel.md`
- `docs/gym/2026-06-25-harbor-for-gym-terminalbench-and-benchmarks.md`
- `docs/gym/2026-06-25-khala-terminal-bench-through-openagents-run.md`

Adjacent docs and implementation references:

- `docs/gepa/2026-06-30-gepa-usage-and-fleet-delegation-optimization-loop.md`
- `docs/khala-code/2026-06-30-khala-code-fleet-management-spec.md`
- `docs/codex/2026-06-30-codex-to-khala-code-porting-audit.md`
- `docs/benchmarks/2026-06-27-mirrorcode-khala-gym-integration-analysis.md`
- `docs/transcripts/245.md`
- `packages/khala-tools/README.md`
- `packages/khala-tools/src/fleet-delegate-program.ts`
- `apps/openagents.com/workers/api/src/khala-delegation-gepa-feedback.ts`
- `apps/openagents.com/workers/api/src/probe-gepa-standing-optimization-loop.ts`

## 2. What Already Exists

### Gym

The Gym is already the eval and reward factory. It owns the language for
environment, policy, verifier, accepted outcome, report, flywheel, and
decision-grade publication. Relevant shipped pieces include:

- typed environment registration for `terminal-bench`, `khala-code`,
  `long-context-codebase-qa`, and `m8-head-to-head`;
- paid-run planning with owner approval, 402 balance gates, and metering
  contexts;
- `flywheel.ts`, which turns Gym reports into GEPA/TRINITY/Conductor reward
  bundles while keeping runtime promotion approval-gated;
- the decision-grade leaderboard path;
- the Harbor seam: Worker/Gym dispatches a typed job, an out-of-process Python
  harness runs elsewhere, and the Worker ingests public-safe progress and
  summaries without importing the Python runtime;
- `openagents.gym.run_progress.v1`, backed by D1, for live follow-along on
  `/gym`.

That Harbor pattern is the key precedent for Mutalisk.

### Khala Code

Khala Code already has the operator substrate:

- `clients/khala-code-desktop` is the local Electrobun chat and tool surface.
- `@openagentsinc/khala-tools` owns the shared tool runtime.
- `khala.fleet.delegate` now models the deterministic delegation program:
  `ensure_pylon -> advertise_capacity -> select_account -> prepare_work ->
  dispatch -> verify_closeout`.
- Hosts can pass an admitted
  `openagents.khala.fleet_delegation.parameters.v0` parameter set, or use safe
  defaults when no admitted set exists.

This is the right program for GEPA to optimize. The deterministic control flow
stays code. Mutalisk tunes bounded text/policy parameters such as objective
templates, capacity advertisement, account ranking, retry/backoff, verifier
criteria, and merge-resolution wording.

### GEPA And Mutalisk

The OpenAgents GEPA architecture is hybrid:

- Python offline/leaf compute runs DSPy and GEPA.
- Effect/TypeScript online authority selects, gates, admits, and serves bounded
  parameters.
- Every GEPA win is an untrusted candidate until admitted through release gates
  and Action Submission.

OpenAgents already has the surrounding pieces:

- GD-0: public-safe delegation example dataset.
- GD-1: `openagents.khala.delegation_gepa_feedback.v0` metric and ASI feedback.
- GD-3: `projectKhalaFleetDelegationCandidateAdmission`, which turns admissible
  `khala.fleet.delegation` candidates into evidence-only, approval-required
  Action Submission proposals.
- GD-4: the admitted parameter schema read by `khala.fleet.delegate`, Khala
  Code Desktop, and `khala fleet run`.

Recent Mutalisk work fills the offline contract side: an offline GEPA optimizer
path and candidate emitter that can produce a
`psionic.probe_gepa_candidate_manifest.v1` summary for
`khala.fleet.delegation`. That is necessary, but it is still not enough for a
product test. OpenAgents has not yet wired a Gym job, ingestion route, progress
surface, or Khala Code command around it.

## 3. Where It Fits In Gym

Treat Mutalisk as a Gym optimization environment/workflow, not as a normal model
lane.

Recommended environment id:

```text
khala-code-delegation-gepa
```

Recommended job family:

```text
mutalisk.khala_fleet_delegation.optimize.v0
```

Mapping:

| Gym concept | Mutalisk / Khala Code meaning |
| --- | --- |
| Environment | `khala-code-delegation-gepa`: optimize fleet-delegation parameters from public-safe delegation examples |
| Policy | A `khala.fleet.delegation` parameter candidate for the deterministic delegate program |
| Task set | GD-0 delegation examples, split into train/validation/held-out sets by ref |
| Verifier | GD-1 feedback plus held-out delegation outcomes: merged clean, admitted first try, cost, idle gap, conflict churn |
| Runner | Out-of-process Mutalisk Python job, launched like Harbor rather than imported into the Worker |
| Reward | GD-1 scalar reward bps plus Pareto dimensions, mapped to Gym cost-per-accepted-outcome where real execution evidence exists |
| Report | A public-safe Gym optimization report with candidate refs, evidence refs, blocker refs, and freshness |
| Artifact | R2/private object for detailed candidate artifacts; D1/public-safe summary for manifest lookup |
| Promotion | None. The Gym can mark an admission proposal ready, but Action Submission and owner approval decide live use |

This keeps one metric vocabulary. Mutalisk can compute optimizer-specific
details, but the product surface should speak Gym: run, environment, reward,
accepted outcome, decision-grade, blocker, candidate, admission proposal.

## 4. End-To-End Flow

### Desired operator flow

From Khala Code:

1. The operator opens the fleet/delegation surface.
2. They choose "optimize delegation policy" or a command equivalent.
3. Khala Code calls an owner-gated OpenAgents route to create a Gym run for
   `khala-code-delegation-gepa`.
4. The UI receives a `runRef` immediately and follows progress.
5. The backend dispatches a Mutalisk job on a Python-capable host.
6. Mutalisk reads only public-safe dataset refs and feedback refs.
7. Mutalisk emits a candidate manifest summary plus detailed private artifact.
8. OpenAgents ingests the summary, projects a Gym report, and runs the existing
   admission projector.
9. Khala Code shows one of:
   - candidate emitted, admission proposal ready;
   - blocked, with typed blocker refs;
   - completed with no admissible gain;
   - failed, with operator-safe diagnostic refs.
10. If the owner approves the Action Submission, the admitted parameter set can
    be used by the next `khala.fleet.delegate` run.

### What should not happen

- Khala Code should not require the operator to run Mutalisk by hand.
- The Worker should not import Python, DSPy, or GEPA.
- Mutalisk should not directly mutate live delegation behavior.
- A candidate should not be treated as decision-grade unless the backing
  evaluation uses real held-out or live evidence that satisfies the Gym's
  decision-grade gates.
- Raw prompts, raw traces, local paths, bearer material, private endpoint
  material, and optimizer scratch logs should not enter public Gym projections.

## 5. The Harbor Pattern To Reuse

Harbor solved the same class of integration problem for Terminal-Bench:

- the Worker owns the typed job spec and public-safe routes;
- an external Python/Docker harness performs heavy benchmark work;
- a pusher sends D1-backed progress snapshots;
- public `/gym` shows counts, refs, status, and freshness;
- private traces remain private;
- the Worker test suite asserts no runtime import of the external harness.

Mutalisk should follow the same rule:

```text
Worker/Gym job spec -> Mutalisk Python worker -> public-safe summary ingest -> Gym progress/report -> admission projection
```

The difference is that Harbor produces benchmark solve rewards, while Mutalisk
produces optimization candidates. The safety boundary is the same.

## 6. Contracts To Add In OpenAgents

### Job Spec

Add a typed job spec such as:

```text
openagents.gym.mutalisk_khala_delegation_job.v0
```

Minimum public-safe fields:

- `runRef`
- `jobRef`
- `environmentId: "khala-code-delegation-gepa"`
- `signature: "khala.fleet.delegation"`
- `baseModuleRef`
- `seedCandidateRef`
- `datasetRef`
- `trainSplitRefs`
- `validationSplitRefs`
- `feedbackSchemaRef: "openagents.khala.delegation_gepa_feedback.v0"`
- `candidateManifestSchemaVersion: "psionic.probe_gepa_candidate_manifest.v1"`
- `maxMetricCalls`
- `ownerApprovalRef`
- `demandKind: "internal"`
- `demandSource: "gym_khala_code_delegation_gepa"`
- `publicSafetyPolicyRef`

The spec should carry refs and policy, not raw examples or raw traces.

### Progress

Reuse `openagents.gym.run_progress.v1` with a Mutalisk-specific environment and
stage labels:

- `queued`
- `dataset_resolved`
- `feedback_resolved`
- `optimizing`
- `candidate_emitted`
- `summary_ingested`
- `admission_projected`
- `blocked`
- `completed`
- `failed`

Progress can show counts, refs, metric-call counts, best score bps, elapsed
time, and blocker refs. It should not show raw optimizer feedback text if that
text can contain prompt or trace material.

### Summary Ingest

Add an operator ingest route for a public-safe summary such as:

```text
openagents.gym.mutalisk_khala_delegation_summary.v0
```

Minimum fields:

- `runRef`
- `jobRef`
- `candidateManifestRef`
- `candidateRef`
- `signature`
- `baseModuleRef`
- `optimizedModuleRef`
- `metricName`
- `metricValueBps`
- `evalEvidenceRefs`
- `traceProvenanceRefs`
- `optimizerRunRefs`
- `blockerRefs`
- `artifactRefs`
- `publicSafetyChecks`

This summary is the bridge from Mutalisk storage into:

- Gym report generation;
- the `probe-gepa-standing-optimization-loop`;
- `projectKhalaFleetDelegationCandidateAdmission`;
- Khala Code's operator-facing status.

### Storage

Use two layers:

- Private R2 object(s) for detailed candidate artifacts and optimizer traces.
- D1 summary rows for public-safe manifest lookup, run progress, and report
  projection.

The D1 row should be enough for admission and UI status. Detailed R2 artifacts
stay private/operator-only.

## 7. Khala Code UI Shape

Khala Code should expose this as part of fleet management, not as a generic
benchmark screen.

Recommended first UI:

- A fleet/delegation panel action: "Optimize delegation policy".
- A run status row keyed by `runRef`.
- A compact status stack:
  - dataset refs selected;
  - current phase;
  - metric-call budget and consumed calls;
  - best validation score bps;
  - candidate manifest ref;
  - admission decision;
  - blocker refs;
  - approval/action-submission ref when ready.
- A "use admitted defaults" readout showing which
  `openagents.khala.fleet_delegation.parameters.v0` set is active.

Do not put raw GEPA traces or full prompt bodies in this UI. The operator should
see refs, bounded summaries, and explicit blocker labels. The private artifact
can be downloadable only from an operator route if needed.

## 8. Readiness Checklist

This is ready for product/UI testing only when all of these are true:

- Khala Code can start the run without shelling out to Mutalisk manually.
- The backend returns a `runRef` and persists a queued progress snapshot.
- A Mutalisk worker can pick up a typed job spec from OpenAgents or receive it
  through an operator runner script.
- Progress reaches `/api/public/gym/run-progress` or an owner-gated equivalent
  using public-safe fields only.
- Mutalisk emits a `psionic.probe_gepa_candidate_manifest.v1` summary for
  `khala.fleet.delegation`.
- OpenAgents ingests the summary into D1 and stores detailed artifacts privately.
- A Gym report or reward bundle is built from the summary.
- `projectKhalaFleetDelegationCandidateAdmission` runs and returns either
  `gated_proposal_ready` or typed blockers.
- Khala Code displays the run state and the admission result.
- No code path auto-promotes the candidate or bypasses Action Submission.
- Public-safety tests prove no raw prompts, raw traces, local paths, secrets, or
  private endpoints leak into the public Gym projection.

Until that checklist is green, the feature is developer/agent-testable through
contracts and CLI artifacts, not user-testable through the Khala Code UI.

## 9. Phased Implementation Plan

### Phase A: Register the Gym shape

Add the `khala-code-delegation-gepa` environment/job family to the Gym registry
as an owner-gated optimization workflow. It can initially report
`awaiting_runner` rather than pretending a live run exists.

Acceptance:

- typed environment metadata exists;
- fixture path is clearly `decisionGrade:false`;
- docs point to this audit and the GEPA delegation loop;
- no spend or runtime promotion authority is added.

### Phase B: Add Mutalisk job and summary contracts

Implement the job spec, summary schema, D1 table, and operator ingest route.

Acceptance:

- schema tests cover good and bad summaries;
- refs are public-safe;
- route auth is owner/operator-gated for ingest;
- public read path exposes only bounded progress/report data;
- tests assert no Mutalisk Python runtime import in the Worker.

### Phase C: Add a runner bridge

Create a script or service bridge analogous to the Harbor progress pusher:

```text
openagents job spec -> mutalisk-optimize -> summary/progress push
```

Acceptance:

- fake/local Mutalisk run can push queued/running/completed progress;
- a real Mutalisk offline run can emit a candidate summary;
- failed runs emit typed blockers, not raw logs;
- all demand attribution is `internal / gym_khala_code_delegation_gepa`.

### Phase D: Build Gym report conversion

Convert Mutalisk summaries into Gym report/reward bundles and wire them into the
existing flywheel/admission surfaces.

Acceptance:

- candidate refs appear in the Gym report;
- reward dimensions preserve GD-1 metrics;
- decision-grade remains false unless evidence gates are satisfied;
- admission projection is evidence-only and approval-required.

### Phase E: Add Khala Code surface

Expose start/follow/status in the Khala Code fleet-management UI.

Acceptance:

- operator can start a run from the UI;
- run progress updates without manual CLI inspection;
- the final candidate/admission result is visible;
- active admitted parameter set is visible;
- UI never renders raw prompt/trace/log material by default.

### Phase F: End-to-end smoke

Run a bounded smoke:

1. seed a small public-safe delegation dataset;
2. start optimization from Khala Code;
3. dispatch Mutalisk;
4. ingest the candidate summary;
5. build a Gym report;
6. project admission;
7. show result in Khala Code.

Acceptance:

- one transcript proves the whole loop;
- `git diff --check`, relevant schema tests, route tests, and Khala Code UI tests
  pass;
- the smoke remains no-spend unless owner explicitly arms a real run.

## 10. Risks And Guardrails

- Public-safety leak: optimizer traces can contain raw prompts, logs, local
  paths, or private endpoint hints. Mitigation: refs-only job specs, private R2
  artifacts, public projection tripwires.
- Authority confusion: a high-scoring candidate could be mistaken for live
  policy. Mitigation: always route through Action Submission; no auto-promotion
  path; keep `runtimePromotionAllowed:false`.
- Metric duplication: Mutalisk could invent a parallel score vocabulary.
  Mitigation: GD-1 remains the metric/ASI source, Gym remains the report/reward
  vocabulary.
- Runtime coupling: Worker imports Python or shells out directly. Mitigation:
  Harbor-style out-of-process runner and no-runtime-import tests.
- Overclaiming: synthetic/offline examples could look like decision-grade
  improvements. Mitigation: `decisionGrade:false` unless held-out/live evidence
  meets Gym gates.
- Operator burden: if this requires shell commands, the UI test is not real.
  Mitigation: Khala Code owns start/follow/status once Phase E lands.
- Ref drift: Mutalisk D1/R2 refs and OpenAgents candidate/admission refs can
  diverge. Mitigation: one summary contract and decode-on-read tests.

## 11. Decision

Build this as a Gym-backed Khala Code optimization workflow.

The next concrete engineering slice should not be another offline Mutalisk unit
test. It should be the OpenAgents product seam:

```text
typed Gym job + progress + Mutalisk summary ingest + admission projection
```

Once that exists, Khala Code can add a small UI affordance and the operator can
test the real loop from the product. Until then, the system is contract-ready and
developer-testable, but not ready for end-to-end UI testing.
