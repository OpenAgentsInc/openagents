# Agentic SFT + RL Reference Program

> Status: canonical `PSI-280` / `#3585` reference-program record, updated
> 2026-03-14 after landing the runnable pilot in
> `scripts/release/check-psionic-agentic-sft-rl-reference-program.sh`.

This document records the first end-to-end Psionic training reference program
that crosses the current Rust-owned train stack instead of proving each
subsystem in isolation.

## Canonical Runner

Run the pilot from the repo root:

```bash
scripts/release/check-psionic-agentic-sft-rl-reference-program.sh
```

The harness executes the `psionic-train` reference-program test and then runs
the ownership-boundary guard.

## Contract Surface

The pilot is implemented as typed library surface in
`crates/psionic/psionic-train/src/reference_program.rs`.

The canonical entrypoints are:

- `AgenticSftRlReferenceProgramSpec`
- `AgenticSftRlReferenceProgramReport`
- `AgenticSftRlReferenceLineage`
- `AgenticSftRlReferenceOperatorView`
- `run_agentic_sft_rl_reference_program`

This is intentional. The integration gate is not hidden in one shell script or
one app-local demo path. It is a reusable typed report that other operator
surfaces can inspect.

## Workload Shape

The pilot uses one weather-agent environment package and one benchmark package,
then runs these connected phases:

1. warm a reusable sandbox pool and execute two staged weather-tool tasks
2. lift sandbox outputs into environment-session summaries
3. ingest `general_sft` traces, promote a checkpoint, ingest one
   tool-bearing `agentic_sft` trace, and promote into `rl`
4. deliver policy weights through the datastream broadcast path
5. build cluster-backed run-graph and orchestrator state
6. run rollout-worker heartbeat, claim, and upload flows for one exact-policy
   and one bounded off-policy rollout
7. validate both rollouts through typed validator bundles and verdicts
8. assemble a trainer batch and execute one fixed-budget trainer step
9. run one online eval and one benchmark-mode eval
10. derive one condensed operator view from the full typed report

## What This Proves

The pilot is green only if all of the following are true:

- environment packages carry versioned dataset bindings, tool schema, rubric
  hooks, artifact expectations, and benchmark profiles
- stage-program lineage crosses `general_sft -> agentic_sft -> rl`
- checkpoint promotions stay explicit and machine-legible
- policy-weight delivery happens over `psionic-datastream` rather than an
  implicit in-memory shortcut
- sandbox reuse is proven by warm-pool and repeated-iteration receipts
- rollout-worker protocol surfaces heartbeat, claim, and outcome receipts
- validator-aware adjudication emits typed verdict history
- online eval and benchmark eval both consume the same environment package
  identity
- trainer-step execution uses the orchestrator-produced trainer batch rather
  than a disconnected toy batch
- the final report includes an operator summary without discarding the deeper
  typed artifacts underneath it

## Expected Signals

The current pilot should surface at least these signals:

- two sandbox iteration receipts with the second marked `reused_workspace = true`
- one `TrainingStageProgramState` with:
  - two completion receipts
  - two checkpoint-promotion receipts
  - current stage `rl`
- one delivered policy-weight broadcast receipt
- two worker-outcome receipts:
  - one `uploaded_accepted_exact`
  - one `uploaded_accepted_off_policy`
- two validator verdicts with `accepted` disposition
- one trainer-batch assembly record with two rollout artifacts
- one fixed-budget training outcome with `completed_steps = 1`
- one online eval summary with a non-empty average score
- one benchmark aggregate summary with a non-empty aggregate score
- one `AgenticSftRlReferenceOperatorView` that condenses the above into
  rollout, validator, sandbox, eval, and trainer counters

## Current Limits

This pilot is intentionally bounded:

- it uses in-memory cluster, datastream, sandbox, and validator substrate
- it runs one small weather workload, not a production-scale training corpus
- it proves the stack composition, not distributed optimizer maturity
- it does not replace kernel or Nexus authority for canonical accepted-outcome
  truth

Those are acceptable limits for this issue. The goal here is to make the
current all-Rust train stack cohere as one inspectable reference program.

## Claim Rule

This pilot is sufficient to claim that Psionic now has one real integrated
agentic-SFT-plus-RL reference program that exercises:

- environment packages
- dataset lineage
- checkpoint lineage
- datastream-delivered policy weights
- rollout workers
- validator-aware adjudication
- sandbox reuse
- online eval
- benchmark aggregation
- trainer-step execution
- operator-facing typed inspection

It is not sufficient to claim that the full distributed training stack is
complete. Remaining work after this reference program is mostly around
production multi-device runtime behavior, replay guarantees, security
hardening, artifact lifecycle, accounting, chaos, research loops, and the
deeper tensor/compiler/runtime work needed for a full Rust-native training
engine.
