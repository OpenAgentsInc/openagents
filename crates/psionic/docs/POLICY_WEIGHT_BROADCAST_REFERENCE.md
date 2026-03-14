# Policy Weight Broadcast Reference

> Status: canonical `#3572` policy-weight broadcast record, updated
> 2026-03-14 after landing the runnable harness in
> `scripts/release/check-psionic-policy-weight-broadcast.sh`.

This document records the first explicit Psionic policy-weight broadcast layer.

## What Landed

The issue extended `psionic-datastream` with:

- `DatastreamSubjectKind::PolicyWeights` for first-class training weight
  artifacts
- `DatastreamPolicyWeightBinding` for policy id, revision, shard identity,
  assembled-artifact digest, and freshness window
- `DatastreamMirrorLocator` for lightweight HTTP or relay mirror metadata
- `DatastreamPolicyWeightControlPlaneRef` and
  `DatastreamPolicyWeightBroadcastManifest` so train control flow can carry
  refs, digests, and freshness posture rather than heavy payload bytes
- stale-artifact rejection at control-plane export time
- `InMemoryPolicyWeightBroadcast` and
  `DatastreamPolicyWeightBroadcastReceipt` for pipelined multi-shard delivery
  on top of the resumable chunk plane

## Canonical Runner

Run the harness from the repo root:

```bash
scripts/release/check-psionic-policy-weight-broadcast.sh
```

## Workload Shape

The current reference path proves one bounded but real heavy-artifact workload:

1. bind two or more heavy policy-weight shards to the same policy revision
2. attach lightweight mirror metadata and a freshness window
3. export a broadcast manifest that is safe for control-plane carriage
4. reject stale refs before the heavy bytes start moving
5. deliver every shard through the resumable datastream chunk path
6. verify the assembled full-artifact digest and emit a byte-accountable
   broadcast receipt

## Pass Criteria

The policy-weight datastream layer is green only if all of the following are
true:

- policy-weight shards are typed separately from generic served artifacts
- control-plane refs carry only ids, digests, mirror metadata, and freshness
  posture
- stale refs are refused before heavy transfer begins
- assembled-artifact digests are verified after shard delivery
- the heavy artifact plane remains resumable and chunk-accountable through the
  existing datastream client/server model

## Expected Signals

The current harness should prove:

- a multi-shard broadcast can deliver all bytes and produce a stable receipt
- the broadcast control-plane summary remains materially smaller than the heavy
  artifact payload
- mismatched policy bindings across shards are rejected
- stale policy-weight refs are rejected under the declared freshness window

## Current Limitations

This issue intentionally does not claim:

- orchestrator-owned assignment of policy-weight shards to workers
- relay selection or mirror failover heuristics
- cross-region artifact retention policy
- rollout freshness budgets or off-policy worker admission

Those remain later issues. This issue makes the control-plane versus heavy
artifact-plane split explicit for policy weights first.
