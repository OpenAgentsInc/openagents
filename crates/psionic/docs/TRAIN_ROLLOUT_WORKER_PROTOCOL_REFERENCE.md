# Train Rollout Worker Protocol Reference

> Status: canonical `#3575` rollout-worker protocol record, updated 2026-03-14
> after landing the runnable harness in
> `scripts/release/check-psionic-train-rollout-worker-protocol.sh`.

This document records the first trust-aware rollout-worker protocol in
`psionic-train`.

## What Landed

The issue added a new `worker_protocol` module inside `psionic-train` with:

- `RolloutWorkerTrustClass` and `RolloutWorkerIdentity` so trusted trainer
  roles are distinct from semi-trusted or untrusted rollout workers
- `RolloutWorkerHeartbeatReceipt` over protocol-visible heartbeats and heartbeat
  freshness windows
- `RolloutTaskClaim` over one rollout assignment, deterministic
  sample-selection seed, weight-broadcast binding, and claim TTL
- `RolloutUploadLocator` plus inline-versus-external upload policy
- `RolloutWorkerOutcomeReceipt` that records claim expiry, upload-policy
  rejection, or the final orchestrator-backed rollout-admission outcome

## Canonical Runner

Run the harness from the repo root:

```bash
scripts/release/check-psionic-train-rollout-worker-protocol.sh
```

## Workload Shape

The current reference path proves one bounded but real trustless-worker
workload:

1. plan and activate an orchestrated window
2. register a rollout-worker heartbeat with explicit trust posture
3. claim one rollout assignment with a deterministic sample-selection seed
4. upload a rollout through the worker protocol and wrap the orchestrator
   admission receipt in a worker outcome receipt
5. emit local worker receipts for claim expiry or upload-policy rejection

## Pass Criteria

The rollout-worker protocol is green only if all of the following are true:

- fresh heartbeats are required for new claims
- claims bind worker id, assignment id, policy revision, and weight-broadcast
  digest explicitly
- sample-selection seeds are deterministic rather than caller-random
- upload outcomes remain typed even when the failure is local to the worker
  protocol rather than the orchestrator

## Expected Signals

The current harness should prove:

- a worker without heartbeat state cannot claim work
- trust class is visible in heartbeat and outcome receipts
- bounded off-policy uploads compose with the orchestrator admission layer
- expired claims and oversized inline uploads return local typed receipts

## Current Limitations

This issue intentionally does not claim:

- sampled adjudication or duplicate-detection policy
- richer upload transports such as rollout-native datastream subjects
- train-wide durable receipt families beyond worker outcomes

Validator-owned rollout verification bundles now live in the follow-on record
`TRAIN_ROLLOUT_VALIDATION_REFERENCE.md`. This issue makes worker heartbeats,
claims, and upload semantics real first.
