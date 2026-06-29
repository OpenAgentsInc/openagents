# Probe GEPA Live Network System Audit

Date: 2026-06-08

Status: current tranche closed; follow-on live Pylon and Stage 0 receipt work
open.

## Summary

Probe now has a coherent cross-repo benchmark-learning system around GEPA,
Blueprint usage, route scorecards, Pylon rollout assignments, Psionic frontier
imports, and Artanis public projection. The system is still early, but it is no
longer only a paper plan. The current tranche proved the public Benchmark Cloud
runner shape, ran one real SHC Harbor Terminal-Bench smoke, imported live
closeout-shaped evidence into Psionic, and connected Probe GEPA route
scorecards to Coding on Autopilot outcome metrics in OpenAgents product surface.

The strongest current claim is:

```text
Probe GEPA has retained and live-smoke evidence plumbing for coding-agent
benchmark rollouts, including one SHC Harbor Terminal-Bench failure receipt,
typed closeout bundles, Psionic import receipts, OpenAgents product surface/Pylon assignment
contracts, and Artanis/public projection gates.
```

The system must not yet claim:

```text
Probe beats Terminal-Bench.
Pylon benchmark work is generally paid or settled.
GEPA on Pylons is distributed neural-network training.
Any Probe GEPA candidate is active production runtime.
Artanis can publish or activate candidates without OpenAgents product surface/operator authority.
```

## Repos And Authority

Probe remains the coding-agent runtime and evidence emitter. It owns assignment
intake, selected Blueprint signatures, tool-menu constraints, run records,
closeout bundles, route scorecards, retained fixtures, and candidate execution
seams. Probe does not own scoring authority, public benchmark claims, runtime
promotion, settlement, or Artanis posting authority.

OpenAgents owns public Benchmark Cloud. The implementation lives in
`openagents/crates/benchmark-cloud`. It defines public-safe task, split, run,
result, event, artifact, proof, resource, verifier, and redaction contracts.
It now has an observed Probe runner path and a retained SHC Harbor live-smoke
receipt example.

Psionic owns GEPA optimization state. It owns candidate manifests, local
deterministic Stage 0 rollout coordination, candidate-frontier state,
reflection/proposal state, and live OpenAgents product surface/Pylon closeout imports. Psionic does
not dispatch Pylons directly and does not promote Probe runtime candidates.

OpenAgents product surface owns Pylon lease lifecycle, release gates, product projection, Artanis
public report surfaces, Forum summary generation, and Coding on Autopilot
accepted-outcome metrics. It decides whether benchmark evidence is only
benchmark-only, shadow, release-candidate, or active product authority.

Artanis is the public-safe campaign narrator and overseer. Artanis reads
public-safe refs and summarizes retained smoke, retained summary, or validation
measured status. It is not the benchmark runner, scorer, optimizer, settlement
engine, or candidate promotion authority.

## How It Works Today

The current intended data flow is:

1. Psionic creates or loads a Probe GEPA candidate manifest for a text bundle:
   prompt, Blueprint signature policy, tool-menu policy, patch/test policy,
   failure-family playbooks, and closeout policy.
2. OpenAgents Benchmark Cloud selects a public-safe Terminal-Bench task ref
   from retained, validation, or holdout splits.
3. OpenAgents product surface can create a Pylon GEPA metric-call assignment with explicit payment
   mode and closeout requirements, or OpenAgents can run an SHC/Harbor fallback
   lane.
4. Probe executes the assignment with selected Blueprint signatures and allowed
   tool-menu refs, then writes a normalized closeout bundle.
5. Benchmark Cloud records result, event, artifact, proof, resource, verifier,
   route scorecard, and closeout refs.
6. Psionic imports local or live closeout evidence into the same coordinator
   frontier state.
7. OpenAgents product surface projects campaign state for Artanis and maps route scorecards into
   Coding on Autopilot accepted-outcome evidence.
8. OpenAgents product surface/Blueprint gates decide whether a candidate stays benchmark-only,
   becomes shadow, becomes a release candidate, or can ever become active.

GEPA distribution means Pylon-distributed rollout optimization. Pylons can run
many independent benchmark rollouts in parallel. The optimized objects are
text artifacts and policy bundles. This is not distributed gradient training.
Model-weight work such as LoRA/Qwen/MLX comes later after clean GEPA traces
exist.

## What Is Done

Probe has the runtime-side contract foundation:

- benchmark assignment, run, closeout, decision trace, candidate, route
  scorecard, and promotion-decision schemas;
- a closeout bundle writer;
- retained Terminal-Bench fixture refs;
- GEPA candidate execution seams;
- Blueprint signature lookup, tool-menu planning, Program Run evidence, action
  submission boundaries, and contribution release-gate docs;
- Apple FM as the first backend plan and early contract surface.

OpenAgents has public Benchmark Cloud foundations:

- benchmark task/result/event/artifact/proof/resource/split/run/verifier
  contracts;
- Terminal-Bench 2 Stage 0/1 split manifest;
- a typed Probe assignment materialization path;
- an observed Probe benchmark runner path requiring normalized closeout files;
- a live SHC Harbor smoke receipt example for
  `terminal-bench/db-wal-recovery`.

The SHC Harbor live smoke result is retained as failed evidence, not a score
claim:

- SHC host label: `oa-shc-katy-01`;
- task: `terminal-bench/db-wal-recovery`;
- Harbor job id: `e487217a-715e-448c-8d45-e528b76980e7`;
- Harbor trial id: `a6c6c245-b9c0-44a8-a8c0-0c7fe5cc3383`;
- checksum:
  `c18abdc4fdc3a01bf374c55a9700708fe6a9662077d29db81abb692f0a3c5f6f`;
- reward: `0.0`;
- result: failed with a nonzero agent exit;
- public claim: none.

Psionic now has:

- content-addressed Probe GEPA candidate manifests;
- a deterministic local Stage 0 rollout coordinator;
- candidate frontier, lineage, cache, and reflection/proposal state;
- live closeout import types and receipts;
- explicit payment mode handling:
  `unpaid_smoke`, `operator_credit`, `payable_pending_settlement`,
  `settled_bitcoin`, and `rejected_no_pay`;
- `settled_bitcoin` rejection unless settlement receipt refs are present;
- imported rollout states for accepted, rejected, infrastructure failure,
  model/agent failure, timeout, and policy-blocked runs.

OpenAgents product surface now has:

- Pylon GEPA metric-call assignment lifecycle;
- production-equivalent Artanis Probe GEPA/Pylon smoke gates;
- bounded scheduled-runner proof gates;
- Probe GEPA campaign public projection;
- Probe GEPA Forum summary generation;
- Probe GEPA accepted-outcome metrics;
- public/operator audience projections for Probe GEPA outcome metrics;
- Artanis public report Probe GEPA summary from public-safe refs;
- Coding on Autopilot mission tests proving Probe GEPA route scorecard refs can
  attach to coding workrooms for team/operator audiences.

Probe tracker issue `OpenAgentsInc/probe#188` now has all included current
tranche issues closed:

- OpenAgents `#4563`;
- OpenAgents `#4564`;
- Psionic `#1093`;
- OpenAgents product surface `#511`;
- OpenAgents product surface `#512`;
- OpenAgents product surface `#513`;
- OpenAgents `#4555`.

## What Was Tested

OpenAgents:

```sh
cargo test -p benchmark-cloud
scripts/benchmarks/validate-benchmark-cloud-contracts.sh
cargo run -p benchmark-cloud --example probe_observed_runner_smoke
```

The live SHC Harbor smoke was run through Harbor on the SHC host and recorded
as a failed retained-smoke receipt with public-safe refs.

Psionic:

```sh
cargo test -p psionic-train probe_gepa_rollout_coordinator --lib
cargo run -q -p psionic-train --example probe_gepa_live_closeout_import
```

The focused Psionic tests passed 8/8. The example emitted a live-import receipt
with `closeout_state=agent_failure`, `payment_mode=unpaid_smoke`,
`imported_result_status=agent_failed`, `frontier_candidate_count=1`, and
`completed_rollout_count=1`.

OpenAgents product surface:

```sh
bun run --cwd workers/api test -- probe-gepa-outcome-metrics.test.ts probe-gepa-forum-summary.test.ts artanis-public-report.test.ts coding-autopilot-missions.test.ts
bun run --cwd workers/api typecheck
```

The focused OpenAgents product surface tests passed 4 files and 22 tests. Typecheck passed.

Probe:

The Probe repo itself was not changed in this tranche until this audit doc.
Current Probe runtime tests were not rerun for this audit because the latest
implementation work landed in OpenAgents, Psionic, and OpenAgents product surface.

## What Is Not Done

The live runner still needs to execute real benchmark tasks as the new Probe
runtime, not only retained fixtures or Probe+Codex-signature smoke. The SHC
smoke was a real Harbor Terminal-Bench run, but current Probe does not yet
expose a full standalone `probe benchmark run` live command that replaces the
fallback agent path.

OpenAgents product surface still needs to wire unpaid Probe GEPA leases to a real Pylon worker in
the current OpenAgents monorepo Pylon path. The existing lease lifecycle is
typed and tested, but the next proof should show worker accept, progress refs,
artifact/proof submission, accepted/rejected closeout, and Psionic import from
the live worker path.

OpenAgents still needs a live Stage 0 receipt bundle that preserves actual
assignment ids, closeout refs, verifier artifacts, proof bundles, resource
receipts, route scorecards, and failure classifications from live SHC or live
Pylon assignments.

Stage 1 promotion must be shadow-only at first. No candidate should activate
from retained or validation evidence. OpenAgents product surface and Blueprint gates must decide
shadow status from retained evidence, validation evidence, policy findings,
and accepted-outcome metrics.

Artanis public summaries should be generated from projection authority and
posted only through the existing OpenAgents product surface/operator path. Probe may prepare
public-safe copy, but Probe should not post as Artanis or invoke the Artanis
bridge.

Paid-work settlement remains later. The no-spend path must become boring
before moving from `unpaid_smoke` to `operator_credit`,
`payable_pending_settlement`, or `settled_bitcoin`.

LoRA/Qwen/MLX training should start only after GEPA creates clean traces,
failure-family deltas, route scorecards, and split-aware evidence. That lane is
model training. It is separate from GEPA rollout optimization.

## Fit Into The Bigger Picture

This system is the first practical bridge between Coding on Autopilot product
quality and public benchmark-driven learning.

The important product loop is not "win a benchmark" in isolation. The loop is:

```text
benchmark task failure
-> Probe closeout and route scorecard
-> GEPA candidate text mutation
-> validation and holdout evidence
-> Coding on Autopilot workroom outcome delta
-> OpenAgents product surface/Blueprint promotion gate
-> Artanis public-safe status
```

That makes benchmarks evidence for better workrooms. The product metrics now
have a place to record before/after acceptance rate, review minutes, retries,
turns, cost per accepted outcome, artifact completeness, proof completeness,
failure-family reduction, regression count, selected signatures, tool menus,
and route scorecards.

Pylons matter because they can turn idle user and operator machines into
parallel rollout workers. SHC matters because it gives the project a stable
first live benchmark host. Psionic matters because it keeps optimizer state and
later model-training state distinct. OpenAgents product surface matters because it prevents the
system from turning benchmark evidence into product, payout, or public-claim
authority too early.

## Next Work

Current follow-on issues created from the recommended network:

- `OpenAgentsInc/openagents#514`: wire unpaid Probe GEPA leases to a real
  Pylon worker.
- `OpenAgentsInc/openagents#4565`: add a live Stage 0 Probe GEPA receipt
  bundle.
- `OpenAgentsInc/openagents#515`: gate Probe GEPA Stage 1 candidates to
  shadow-only promotion.
- `OpenAgentsInc/openagents#516`: publish Artanis Probe benchmark
  summaries from projection authority.
- `OpenAgentsInc/openagents#517`: add settlement readiness after no-spend
  Probe GEPA batches.
- `OpenAgentsInc/psionic#1094`: start the Probe LoRA/Qwen/MLX training lane
  from clean GEPA traces.

Recommended execution order:

1. Wire one real unpaid Pylon worker assignment in OpenAgents product surface/OpenAgents.
2. Produce a live Stage 0 receipt bundle with actual assignment and closeout
   refs.
3. Import that bundle into Psionic and verify candidate-frontier state updates.
4. Promote any Stage 1 candidate only to shadow.
5. Generate Artanis public summary copy from projection authority.
6. Keep settlement work blocked until no-spend receipts and operator accounting
   are stable.
7. Start LoRA/Qwen/MLX only from clean GEPA traces, not from ad hoc raw logs.

## Maintenance Notes

Keep Probe docs honest about current authority boundaries. If a future change
adds real live worker dispatch, public benchmark claims, paid settlement, or
runtime candidate promotion, update this audit, the benchmark README, and the
owning repo invariant ledger in the same change.

Do not put new implementation into the deprecated old Probe shape. The old
Probe repo history remains source material for patterns, tests, and failure
cases only. The new runtime should stay Bun/Effect, OpenAgents product surface-aware,
Blueprint-aware, Pylon/SHC deployable, and product-gated through
OpenAgents.com.
