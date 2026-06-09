# Probe Benchmark Contracts

Date: 2026-06-08

Probe now has the first runtime-local contract slice for public Benchmark Cloud
and Pylon-distributed GEPA rollout work. The contracts live in
`packages/runtime/src/contracts/benchmark.ts` and are exported from the runtime
package entry point.

## Implemented Schemas

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
- `probe.gepa_live_runner_gate.v1`

The assignment schema carries the Benchmark Cloud run and task refs, dataset
and split refs, public-safe task checksum or ref, Probe commit, backend and
runtime profile, optional account/grant refs, selected Blueprint signatures,
tool-menu ref, candidate hash, timeout and budget policy refs, required
artifact refs, required proof-bundle refs, and callback/proof sink refs.

The closeout schema carries the assignment and run refs, candidate hash,
selected signatures, tool menu, backend route, verifier/scorer refs, artifact
manifest refs, proof bundle refs, resource/cost refs, policy findings, failure
classification, retained-failure refs, redaction state, run status, split, and
promotion status. It can also reference a route scorecard through
`routeScorecardRef`.

## Route Scorecards

`probe.benchmark_route_scorecard.v1` explains which backend, runner, provider,
isolation profile, verifier, signatures, tool menu, and candidate hash were
used for a benchmark closeout. It also records expected/observed cost refs,
expected/observed latency, privacy tier, trust tier, rejected routes, route
reason, and post-closeout route score.

Supported route kinds are:

- `codex`
- `probe_codex`
- `apple_fm`
- `local_qwen`
- `shc`
- `pylon`

Rejected routes are preserved as evidence so later routing can explain why, for
example, Codex was used instead of local Probe, SHC instead of Pylon, or a
remote API instead of Apple FM. Route scorecards are public-safe refs only and
can feed future route selection plus accepted-outcome analysis.

## Safety Boundary

The contract validators reject public benchmark records containing raw provider
credentials, raw benchmark secrets, hidden verifier content, wallet or payment
material, private repository refs, unbounded raw logs, public-claim upgrade
authority, or runtime-promotion authority. The sanitizer can scrub or drop those
fields before public-safe artifact emission, but decoders reject unsafe input.

Promotion decisions are evidence-only. They can record that retained,
validation, holdout, or live evidence exists, but they cannot promote runtime
behavior or upgrade a public benchmark claim. External OpenAgents product surface/OpenAgents release
gates remain the authority for publication and promotion.

## GEPA Live Runner Gate

`packages/runtime/src/benchmark/closeout-writer.ts` also exposes
`projectProbeGepaLiveRunnerGate`. The projection consumes a normalized closeout
bundle plus runner execution refs and candidate-manifest authority refs, then
emits `probe.gepa_live_runner_gate.v1` for OpenAgents product surface import.

The gate requires public-safe refs for:

- run and closeout records
- artifact manifests or partial artifacts
- proof bundles
- resource usage or a resource-unavailable reason
- verifier and verifier-result refs
- selected signature refs
- tool-menu refs
- route scorecard refs
- candidate refs
- failure-classification refs for non-successful runs
- candidate-manifest authority refs
- live or sandbox runner execution refs

Successful, timed-out, failed, errored, and policy-blocked closeouts can all be
importable as evidence when the refs above are present. That import does not
grant public-score, product-promotion, or payout authority. The gate keeps
`publicScoreClaimAllowed`, `productPromotionAllowed`, and `payoutClaimAllowed`
false unless separate gate refs are supplied by the owning systems.

## Test Coverage

`packages/runtime/tests/benchmark-contracts.test.ts` covers valid assignment,
run, decision-trace, candidate, and promotion-decision schema refs; invalid
closeouts missing artifact or proof refs; unsafe projection rejection and
scrubbing; failed and timed-out retained closeouts; and separate retained,
validation, holdout, and live evidence representations.

`packages/runtime/src/benchmark/closeout-writer.ts` builds and writes the first
normalized closeout bundle. A bundle contains `probe-run-record.json`,
`probe-closeout.json`, `decision-trace-summary.json`,
`selected-signatures.json`, `tool-menu.json`, `candidate-ref.json`,
`artifact-refs.json`, `resource-usage-ref.json`, `policy-findings.json`,
`failure-classification.json`, and `route-scorecard.json`. Successful, failed,
timed-out, and
policy-blocked runs all write the same public-safe file set, with explicit
retained-failure refs, timeout state, partial artifact refs, or policy findings
where relevant.

`packages/runtime/tests/benchmark-closeout-writer.test.ts` covers fake
assignment bundle emission, directory writes, failure closeouts, timeout
closeouts, policy-blocked closeouts, generated and explicit route scorecards,
live GEPA runner-gate projection, timeout and policy-blocked import behavior,
candidate-manifest authority blockers, route scorecard refs, and unsafe writer
or runner-gate input rejection.

`packages/runtime/src/benchmark/candidate-execution.ts` adds the Probe-facing
GEPA candidate execution adapter. It can run a retained fixture with either the
baseline assignment or a supplied Psionic GEPA text-bundle manifest, then emit
the same normalized closeout bundle shape with candidate hash, candidate import
refs, candidate component refs, selected signatures, projected tool-menu
snapshot, verifier result refs, policy findings, and failure classification.
Candidate text is validated before it can affect the assignment projection, and
candidate-selected signatures or tools must remain subordinate to the
assignment and retained fixture constraints.
