# Probe GEPA Benchmark System Closeout Audit

Date: 2026-06-08

Status: implementation issue series complete; live production rollout still gated.

## Executive Summary

The `docs/benchmarks/plan.md` issue series has been implemented and closed
across the owning repos. Probe is now the runtime-side evidence emitter for
coding-agent benchmark work. OpenAgents owns the public `benchmark-cloud`
contracts, split manifests, Probe runner adapter shape, worker capability
envelopes, and deterministic Stage 0, Stage 1, and validation campaign
examples. Psionic owns the GEPA text-bundle candidate manifest and early
coordinator state. OpenAgents product surface owns the Pylon lease lifecycle for GEPA metric-call
assignments, explicit payment modes, Artanis public projection, Forum summary
generation, and accepted-outcome metric projection.

The system is now coherent as a ref-first, public-safe benchmark optimization
apparatus. It can represent retained, validation, holdout, and live evidence;
run deterministic retained and validation campaign examples; emit Probe
closeout bundles; import route scorecards; separate optimizer acceptance from
runtime promotion; and prevent public claim or payout overclaims.

The main thing not done is a true live end-to-end campaign through Harbor on
the SHC box with real Pylon workers executing independent metric-call leases.
The current state proves the contracts, deterministic examples, closeout
bundle shapes, safety gates, and coordinator seams. It does not yet prove live
distributed benchmark execution, paid settlement, frozen holdout performance,
or accepted customer outcome improvement.

## Source Of Truth And Tracker State

The source plan is `docs/benchmarks/plan.md`. It created a 19-issue sequence
with the master tracker:

- Probe tracker: `https://github.com/OpenAgentsInc/probe/issues/187`

The tracker is closed. The linked implementation issues are also closed:

- Probe: `#182`, `#183`, `#184`, `#185`, `#186`
- OpenAgents: `#4556`, `#4557`, `#4558`, `#4559`, `#4560`, `#4561`, `#4562`
- Psionic: `#1091`, `#1092`
- OpenAgents product surface: `#506`, `#507`, `#508`, `#509`, `#510`

Every issue has a closeout comment. The touched repos were pushed on `main`.
The only known leftover local state during the closeout audit was an unrelated
untracked Nexus report in `openagents/docs/reports/nexus/`, which was left
untouched.

## How The System Works Now

The system is split by authority instead of being collapsed into Probe.

Probe is the coding-agent runtime under test. It accepts benchmark assignment
refs, applies selected Blueprint signatures and tool-menu constraints, runs
baseline or GEPA candidate-shaped retained fixture paths, and emits normalized
closeout evidence. Probe does not own public benchmark claims, scoring
authority, runtime promotion, or payout settlement.

OpenAgents owns public `benchmark-cloud`. That layer defines public benchmark
task, result, event, artifact, proof, resource, split, run, verifier, no-cheat,
and redaction contracts. It also owns the first public Terminal-Bench 2 through
Harbor lane shape for Probe, retained and validation split manifests, Pylon
benchmark worker capability envelopes, and deterministic campaign examples.

Psionic owns GEPA candidate optimization. It defines content-addressed
text-bundle candidate manifests and an early coordinator that can run local
Stage 0 metric calls, cache evaluator results, separate infrastructure failure
from agent failure, and keep optimizer acceptance distinct from runtime
promotion. Pylon execution is modeled as a future evaluator backend, not
claimed as live in Psionic yet.

OpenAgents product surface owns Pylon lease and projection authority. It records GEPA metric-call
assignments, worker acceptance, progress refs, artifact/proof submissions,
accepted or rejected closeout, explicit payment modes, and coordinator import
records. It also owns Artanis public campaign projection, public-safe Forum
summary generation, and Coding on Autopilot accepted-outcome metric projection.

Artanis is not the executor. It becomes the public-safe narrator and overseer
for campaign summaries from refs. Probe may prepare public-safe copy, but
posting as Artanis still requires the existing OpenAgents product surface/operator authority path.

## End-To-End Data Flow

The intended flow is now explicit:

1. Psionic creates or loads a
   `psionic.probe_gepa_candidate_manifest.v1` text-bundle candidate.
2. OpenAgents `benchmark-cloud` selects retained or validation task refs from a
   stable split manifest.
3. OpenAgents product surface creates a Pylon GEPA metric-call assignment or OpenAgents runs the
   local/SHC fallback lane.
4. A worker receives a bounded assignment with campaign, split, task, Probe
   commit, candidate hash, backend profile, expected artifacts, verifier refs,
   timeout/budget refs, closeout requirements, and explicit payment mode.
5. Probe runs the assignment, constrained by Blueprint signature refs and
   tool-menu refs, then writes a normalized closeout bundle.
6. Benchmark Cloud imports Probe closeout refs into benchmark result,
   artifact, proof, resource, and verifier records.
7. Psionic imports rollout results, updates candidate frontier state, and
   records reflection/proposal evidence.
8. OpenAgents product surface projects campaign state for Artanis and for accepted-outcome analysis.
9. Release gates decide whether a candidate remains benchmark-only, enters
   shadow, becomes a release candidate, or is active.

This is Pylon-distributed benchmark-driven optimization. It is not
distributed neural-network training. Distribution helps because GEPA needs many
independent evaluated rollouts. Pylons can run those rollouts in parallel, but
the reflection/proposal step can remain centralized.

## Probe Runtime State

Probe now implements the benchmark contract foundation:

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

These live in `packages/runtime/src/contracts/benchmark.ts` and are exported by
the runtime package. The validators reject raw provider credentials, raw
benchmark secrets, hidden verifier content, wallet/payment material, private
repo refs, unbounded raw logs, public claim upgrade authority, and runtime
promotion authority.

Probe also now has:

- a normalized closeout writer in
  `packages/runtime/src/benchmark/closeout-writer.ts`;
- retained Terminal-Bench failure fixtures in
  `packages/runtime/src/benchmark/fixtures.ts`;
- a GEPA candidate execution seam in
  `packages/runtime/src/benchmark/candidate-execution.ts`;
- route scorecards for Codex, Probe+Codex, Apple FM, local Qwen, SHC, and
  Pylon routes.

The closeout bundle contains:

- `probe-run-record.json`
- `probe-closeout.json`
- `decision-trace-summary.json`
- `selected-signatures.json`
- `tool-menu.json`
- `candidate-ref.json`
- `artifact-refs.json`
- `resource-usage-ref.json`
- `policy-findings.json`
- `failure-classification.json`
- `route-scorecard.json`

Successful, failed, timed-out, and policy-blocked runs all emit public-safe
bundle shapes. Failed retained runs carry retained-failure refs and failure
classification. Timed-out runs carry timeout state, partial artifact refs, and
explicit resource-unavailable reasons where needed.

## Benchmark Cloud State

The public `benchmark-cloud` implementation now lives in
`openagents/crates/benchmark-cloud`.

It defines:

- `BenchmarkTask`
- `BenchmarkResult`
- `BenchmarkEvent`
- `BenchmarkArtifactManifest`
- `BenchmarkProofBundle`
- `openagents.resource_usage_receipt.v1`
- `BenchmarkSplitManifest`
- `BenchmarkRunManifest`
- `ScorerVerifierRef`
- `NoCheatMetadata`
- `BenchmarkRedactionState`

The first public split manifest is
`fixtures/benchmarks/terminal_bench_probe_gepa_stage_0_1_splits.json` in
OpenAgents. It distinguishes retained fixtures, validation split, frozen
holdout split, local smoke fixtures, public-safe task refs, scorer/verifier
refs, task selector version, and allowed claim state.

The Probe runner lane can build a normalized Probe assignment payload, represent
the command `probe benchmark run --assignment-json -`, and emit:

- `result.json`
- `events.jsonl`
- `metadata.json`
- `artifact_manifest.json`
- `proof_bundle.json`
- `resource_usage_receipt.json`
- `probe-run-record.json`
- `probe-closeout.json`

The fake runner path covers pass, timeout, and error outcomes while preserving
artifact, proof, resource, signature, and tool-menu refs.

## GEPA Candidate And Coordinator State

Psionic now has
`psionic.probe_gepa_candidate_manifest.v1` in
`crates/psionic-train/src/probe_gepa_candidate_manifest.rs`.

The candidate manifest content-addresses:

- `probe_system_prompt`
- `terminal_bench_global_playbook`
- `signature_selection_policy`
- `tool_menu_policy`
- `patch_and_test_policy`
- `failure_family_playbooks`
- `closeout_policy`

Component hashes, candidate hashes, import refs, split refs, trace digests, and
safety refs are stable. Optimizer acceptance is distinct from runtime
promotion. Candidate text cannot grant new runtime authority, bypass release
gates, carry raw secrets, or upgrade public benchmark claims.

Psionic also has an early rollout coordinator in
`crates/psionic-train/src/probe_gepa_rollout_coordinator.rs`. The coordinator
can run local Stage 0 metric calls, cache evaluator results for resumability,
separate `succeeded`, `agent_failed`, `infrastructure_failed`, and
`policy_blocked` statuses, and reject policy-violating candidates before they
advance.

The coordinator currently proves the local evaluator and import/export shape.
It does not yet run live Pylon dispatch.

## Pylon Assignment And Payment State

OpenAgents product surface now defines the Probe GEPA metric-call assignment lifecycle in
`workers/api/src/pylon-gepa-metric-call-assignments.ts`.

The lifecycle is:

1. assignment created;
2. worker accepts and receives a lease ref;
3. worker reports progress refs;
4. worker submits artifact, proof, verifier, closeout, and resource refs;
5. evaluator or operator closes as accepted or rejected;
6. GEPA coordinator imports the normalized public-safe result.

Payment mode is explicit on every record:

- `unpaid_smoke`
- `operator_credit`
- `payable_pending_settlement`
- `settled_bitcoin`
- `rejected_no_pay`

Accepted work is not settled payout. Public projection may claim accepted
unpaid smoke work only when no-spend evidence is present. It may claim settled
bitcoin payout only when both payment and settlement receipt refs are present.

OpenAgents now defines
`openagents.pylon_benchmark_worker_capability.v1` for workers that can run
Probe benchmark and GEPA rollout work. Worker admission and payout readiness
are deliberately separate. A worker can be admitted for no-spend or unpaid
rollout work without being ready for paid settlement.

## Campaign Examples

OpenAgents `benchmark-cloud` now has deterministic examples for the first
campaign lanes.

Stage 0 retained smoke:

- campaign id: `probe-gepa-stage0-retained-smoke-2026-06-08`;
- five retained Terminal-Bench fixture refs;
- one baseline and three mutated text-bundle candidates;
- twenty metric-call records;
- accepted and rejected closeout refs;
- Pylon assignment refs represented;
- no LoRA, no model training, no public leaderboard claim, no promotion.

Stage 1 retained-failure sprint:

- campaign id: `probe-gepa-stage1-retained-failure-sprint-2026-06-08`;
- seven retained fixture refs;
- eight Pylon worker assignment refs;
- ten text-bundle candidates;
- 210 metric-call records;
- explicit `unpaid_smoke` payment mode on every rollout;
- selected candidate decision: `optimizer_accepted`;
- retained evidence summary only.

Validation sweep:

- campaign id: `probe-gepa-validation-sweep-2026-06-08`;
- validation tasks only:
  `db-wal-recovery`, `configure-git-webserver`, `pypi-server`,
  `filter-js-from-html`, `gcode-to-text`, and `query-optimize`;
- three compared routes:
  current Probe champion, GEPA candidate, and baseline backend route;
- eighteen rollout records;
- candidate hash, Probe commit, verifier results, artifact availability,
  cost, duration, and resource refs;
- no holdout use;
- no public "Probe beats Terminal-Bench" claim.

## Projection And Product Metrics State

OpenAgents product surface now has `openagents.probe_gepa_campaign_projection.v1`. It lets Artanis
summarize campaign refs, objective refs, stage, claim state, benchmark suite
refs, split refs, Probe commit refs, baseline and active candidate refs,
candidate hash refs, Pylon batch refs, metric-call counts, retained,
validation, and holdout result refs, artifact and receipt refs, cost and
resource refs, policy finding refs, blocker refs, promotion decision refs, and
next-action refs.

The projection rejects raw prompts, raw traces, raw benchmark fixtures,
provider credentials, account refs, bearer material, wallet material,
invoices/preimages, private repo paths, local filesystem paths, raw logs, and
raw timestamps.

OpenAgents product surface also has a Forum summary generator for
`openagents.probe_gepa_forum_summary.v1`. It creates deterministic public-safe
Forum copy from refs, not raw traces. It uses exact claim language:

- measured retained smoke only;
- retained evidence summary only;
- validation measured only;
- holdout summary only;
- no public benchmark claim.

The accepted-outcome metric projection connects benchmark evidence to Coding on
Autopilot only when accepted workroom outcome refs and proof refs exist. Until
then, the claim text remains:

```text
Benchmark validation only; no paid customer outcome improvement claim.
```

This is the correct bigger-picture boundary. Benchmark improvements are not
product wins until they improve accepted coding outcomes.

## What Was Tested

The completion audit ran the current verification commands after the issue
series was closed.

Probe:

```sh
bun test
```

Result: 133 tests passed across 26 files. Covered benchmark contracts,
closeout writer, retained fixtures, candidate execution, route scorecards,
Blueprint signature lookup, tool-menu planning, Apple FM tool streams, auth,
runner identity, and unsafe-field rejection.

OpenAgents:

```sh
scripts/benchmarks/validate-benchmark-cloud-contracts.sh
cargo run -p benchmark-cloud --example probe_gepa_stage0_smoke
cargo run -p benchmark-cloud --example probe_gepa_stage1_retained_sprint
cargo run -p benchmark-cloud --example probe_gepa_validation_sweep
```

Result: `benchmark-cloud` tests passed, and the Stage 0, Stage 1, and
validation examples emitted the expected summaries. Stage 0 produced 20 metric
calls, Stage 1 produced 210 metric calls, and validation produced 18 rollouts.

OpenAgents product surface:

```sh
bun run --cwd workers/api test -- \
  pylon-gepa-metric-call-assignments.test.ts \
  probe-gepa-campaign-projection.test.ts \
  probe-gepa-forum-summary.test.ts \
  probe-gepa-outcome-metrics.test.ts
bun run --cwd workers/api typecheck
```

Result: 23 targeted tests passed and TypeScript typecheck passed.

Psionic:

```sh
cargo test -p psionic-train probe_gepa_candidate_manifest --lib
cargo test -p psionic-train probe_gepa_rollout_coordinator --lib
cargo run -q -p psionic-train --example probe_gepa_candidate_manifest_fixture -- \
  fixtures/probe/gepa/probe_gepa_candidate_manifest_stage_0_1_seed_v1.json
```

Result: focused manifest and coordinator tests passed, and the fixture example
exited successfully.

## What Is Done

The completed work is meaningful:

- Probe has typed benchmark schemas and public-safe closeout bundle emission.
- Probe has retained Terminal-Bench failure fixtures without hidden task data.
- Probe can run baseline and supplied GEPA candidate-shaped retained fixture
  closeouts through the same normalized bundle shape.
- Probe has route scorecards for backend and runner decisions.
- OpenAgents has public `benchmark-cloud` contracts and fixtures.
- OpenAgents has public retained, validation, holdout, and smoke split
  manifest representation.
- OpenAgents has the Probe Terminal-Bench runner adapter contract and fake
  runner coverage.
- OpenAgents has Pylon benchmark worker capability and work requirement
  matching.
- OpenAgents has deterministic Stage 0, Stage 1, and validation campaign
  examples.
- Psionic has GEPA text-bundle candidate manifests and an early local
  coordinator.
- OpenAgents product surface has GEPA metric-call assignment lifecycle, explicit payment modes,
  public campaign projection, Forum summary generation, and outcome metrics.
- The public claim boundary is wired into validators and docs.
- The GEPA lane terminology is now explicit: the accurate label is
  Pylon-distributed benchmark-driven optimization.

## What Is Not Done

The current implementation should not be oversold.

There is not yet a live Harbor/SHC Terminal-Bench 2 run where Probe executes
real tasks end to end in the sandbox and returns verifier-backed results from
the live environment.

There is not yet a live Pylon-distributed GEPA campaign where multiple real
Pylon workers accept OpenAgents product surface leases, execute independent metric calls, upload
artifact/proof refs, and return results into Psionic's coordinator.

The Psionic coordinator does not yet perform a full production GEPA
reflection/proposal loop against live worker rollouts. The current coordinator
has the local deterministic seam, candidate frontier shape, cache behavior, and
failure semantics.

Probe's candidate execution adapter is not the final live sandbox runner. It
is the typed execution and closeout seam that lets Benchmark Cloud and Psionic
develop against stable outputs.

No public Terminal-Bench score is established. Retained and validation evidence
are not public leaderboard claims. The holdout split is represented but not
used for a public claim.

No runtime candidate is active. `optimizer_accepted` remains separate from
`shadow`, `release_candidate`, and `active`.

No paid Pylon benchmark work has been settled. Accepted work and settlement are
separate, and settled bitcoin claims require settlement receipt refs.

No Coding on Autopilot accepted-outcome improvement has been proven from this
benchmark system yet. The outcome projection exists, but benchmark validation
must still be connected to accepted workroom outcomes.

No LoRA, Qwen fine-tuning, Apple FM MLX fine-tuning, DPO, or GRPO lane has been
run from this system. That remains a later Psionic training path after GEPA
produces clean traces and split-aware evidence.

## Bigger Picture

This system fits the OpenAgents direction as a proof-bearing improvement loop
for coding agents.

The business goal is not just to climb a benchmark. The goal is to make Coding
on Autopilot produce more accepted outcomes with lower review time, lower
retry count, better artifacts, clearer proof, and more efficient routing.
Benchmark work is useful because it creates a controlled, repeatable way to
find failures, mutate Probe/Blueprint text artifacts, validate candidates, and
decide whether those candidates should enter product shadowing.

Pylon provides the distributed compute lane. For GEPA, Pylons are not training
model weights. They run independent evaluated rollouts. That creates useful
work slices that can later become paid work when settlement gates are real.
For later model training, Pylon and Psionic may support Qwen/LoRA/MLX-class
training work, but that is a separate lane with different receipts and claims.

Benchmark Cloud provides the public substrate. It keeps manifests, splits,
artifact contracts, proof bundles, and score imports public rather than
burying them in a private Cloud repo. That matters because public benchmark
claims need stable refs, redaction rules, no-cheat metadata, and explicit
release gates.

Blueprint remains first. GEPA optimizes Probe prompts, Blueprint signature
selection policy, tool-menu policy, patch/test playbooks, failure-family
playbooks, and closeout policy before training model weights. That keeps the
first improvement loop cheap, inspectable, reversible, and aligned with the
agent authority model.

Artanis is the public readback surface. It can summarize the campaign status
and boundaries for users without becoming the executor, optimizer, scorer,
payment authority, or release gate.

## Risks And Watchpoints

The largest technical risk is that the current deterministic examples become a
false sense of live readiness. The next end-to-end step must run real Probe
tasks through Harbor on SHC and preserve actual verifier artifacts.

The largest product risk is claiming benchmark progress before accepted
customer outcomes improve. OpenAgents product surface's outcome projection helps, but public copy
must keep saying benchmark validation only until accepted outcome refs and
proof refs exist.

The largest Pylon risk is mixing accepted work with settled payout. The
payment-mode fields address this, but every live campaign must still preserve
settlement evidence before any payout claim.

The largest optimization risk is overfitting retained failures. Stage 1 can
produce optimizer-accepted candidates, but validation and holdout lanes must
stay separate and frozen.

The largest authority risk is candidate text widening Probe's tool access or
Blueprint authority. Probe currently validates against that. Future live runner
work must keep the same subordinate relationship: candidate text can suggest
behavior, but assignment, Blueprint, tool-menu, and release gates remain
authoritative.

## Recommended Next Work

1. Run a real SHC Harbor Terminal-Bench smoke with Probe.
   Use the public `benchmark-cloud` assignment shape and preserve actual
   `probe-closeout.json`, verifier refs, artifact manifests, proof bundles,
   resource receipts, route scorecards, and failure classifications.

2. Wire OpenAgents product surface lease assignment to a real Pylon worker.
   Use `unpaid_smoke` first. Prove worker accept, progress refs,
   artifact/proof submission, accepted/rejected closeout, and Psionic import
   without payout language.

3. Connect Psionic's coordinator to live Pylon imports.
   Keep the local evaluator as a deterministic fallback, but add the live
   evaluator backend that consumes OpenAgents product surface assignment imports and updates the
   same candidate frontier state.

4. Make the Probe runner execute real benchmark tasks, not just the typed
   retained fixture seam.
   The live runner should materialize only allowed refs, enforce sandbox and
   tool-menu constraints, stream event refs, and write the normalized closeout
   bundle under failure and timeout.

5. Add a real Stage 0 campaign receipt bundle.
   The deterministic example is done. The next receipt should include live SHC
   or live Pylon assignment ids, closeout refs, and verifier artifacts.

6. Promote a Stage 1 candidate only to `shadow`.
   Do not activate it. Let OpenAgents product surface and Blueprint gates decide shadow status from
   retained plus validation evidence and policy findings.

7. Connect route scorecards to Coding on Autopilot workrooms.
   This is how benchmark learning becomes product evidence: before/after
   acceptance rate, review minutes, retries, cost per accepted outcome,
   artifact completeness, and proof quality.

8. Prepare Artanis public summaries from the projection generator.
   Publish only through the existing OpenAgents product surface/operator authority path. The summary
   should say retained smoke, retained summary, or validation measured only,
   not public benchmark score.

9. Add the paid-work settlement path after no-spend batches are boring.
   Move from `unpaid_smoke` to `operator_credit` or
   `payable_pending_settlement` only when receipts and operator accounting are
   stable. Claim `settled_bitcoin` only with settlement receipt refs.

10. Start the LoRA/Qwen/MLX lane after GEPA produces clean traces.
    Treat model training as separate from GEPA rollout optimization. Use GEPA
    traces, route scorecards, and failure-family deltas to select training data
    and evaluate adapters.

## Near-Term Definition Of Done

The next milestone should be considered done only when there is a live
public-safe receipt set for a real Probe benchmark run:

- real SHC Harbor task refs;
- real Probe commit;
- real candidate hash or baseline hash;
- real verifier/scorer refs and results;
- actual `probe-closeout.json`;
- actual Benchmark Cloud artifact and proof refs;
- actual route scorecard;
- explicit split label;
- explicit claim boundary;
- no public score claim unless the evidence is live and release-gated.

Until that exists, the system is a correctly wired benchmark optimization
apparatus with deterministic proof examples, not a live public benchmark
result.
