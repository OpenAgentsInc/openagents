# Train Orchestrator Reference

> Status: canonical `#3573` train-orchestrator record, updated 2026-03-14
> after landing the runnable harness in
> `scripts/release/check-psionic-train-orchestrator.sh`.

This document records the first explicit Psionic train orchestrator.

## What Landed

The issue added a new `orchestrator` module inside `psionic-train` with:

- `TrainingOrchestratorState` over the run graph, target policy revision, and
  active policy-weight broadcast
- orchestrator ownership of contributor selection and `TrainingWindow`
  lifecycle transitions
- `TrainingWindowAssignmentPosture` for deterministic assignment seed, policy
  revision, and weight-broadcast digest
- lightweight rollout and sampled-eval assignments that carry ids, digests, and
  policy refs rather than heavy artifacts
- lightweight `RolloutArtifactRef` and `TrainerBatchAssemblyRequest` contracts
  for trainer-batch control flow

## Canonical Runner

Run the harness from the repo root:

```bash
scripts/release/check-psionic-train-orchestrator.sh
```

## Workload Shape

The current reference path proves one bounded but real train-control workload:

1. mirror cluster membership into a run graph
2. use persistent participant ranking to select a bounded contributor set
3. plan a window with explicit assignment posture
4. activate the window and accept rollout artifacts only from assigned
   contributors
5. seal the window and assemble a trainer batch from lightweight rollout refs
6. score and reconcile the window while preserving inspectable orchestrator
   state

## Pass Criteria

The train orchestrator is green only if all of the following are true:

- contributor selection is owned by an explicit orchestrator layer rather than
  trainer-local timing
- rollout and trainer-batch control messages stay lightweight and machine
  legible
- assignment seed and contributor-set revision are visible in typed state
- at least one replay-safe workload proves the admitted set, contributor set,
  and trainer batch can differ

## Expected Signals

The current harness should prove:

- a new window is created with explicit rollout and eval assignments
- a standby participant is refused if it submits a rollout against the active
  window
- trainer-batch assembly references rollout ids and digests rather than
  embedding heavy payload bytes in the request
- window status transitions remain delegated to the typed run-graph state

## Current Limitations

This issue intentionally does not claim:

- worker-heartbeat or claim protocol completion
- validator-owned sampled adjudication over rollout uploads
- environment runtime execution or eval runtime interleaving beyond assignment
  refs

Off-policy pruning and stale-rollout budgeting now live in the follow-on record
`TRAIN_OFF_POLICY_BUDGET_REFERENCE.md`. This issue makes window control,
assignment posture, and trainer-batch orchestration real first.
