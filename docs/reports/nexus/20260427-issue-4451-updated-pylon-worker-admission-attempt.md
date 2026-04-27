# Issue 4451 Updated Pylon Worker Admission Attempt

Recorded: 2026-04-27T05:24:23Z

Issue: `OpenAgentsInc/openagents#4451`

## Summary

The local implementation path for updated Pylon homework-worker admission is
covered, and production now proves that Nexus can match and assign more than
one updated `pylon/0.1.15` worker for a broad homework dispatch.

This attempt does not close `#4451`. The production run has not reached
accepted-work payout. The proof run is sealed and waiting on validation:
`validator_disposition=replay_required`, `accepted_for_aggregation=false`, and
no reward or payout status.

## Local Proofs

Passed:

```bash
cargo test -p nexus-control cs336_homework_auto_dispatch_cycle_targets_all_compatible_online_pylons
cargo test -p pylon pylon_autonomously_closes_homework_assignment_from_worker_completion_to_paid_receipt
```

The first test covers broad dispatch across compatible updated online Pylons.
The second test covers the retained Pylon closeout path from worker completion
through validation, reconcile, and paid receipt under a mock Nexus/Treasury
authority.

## Production Setup

An isolated local Pylon home was created under `/tmp/openagents-issue-4451-pylon`
so the proof would not disturb the operator's normal Pylon state.

The temporary node identity was:

```text
9b771290bbbca2d03b39b4e76fd61233e615222a0d508935af616a37eb808e1b
```

It came online as `pylon/0.1.15`, advertised worker and validator training
capability, and registered a Spark payout destination:

```text
spark1pgssys6f2n7ysznekh55yfddywt70z38ulvhf56vdvm00yf8e7sjzu225xq85g
```

At the time of dispatch, production `/api/training/nodes` showed two online
`0.1.15` worker-capable nodes:

- `7ba17a3af6343cd0070293a9b9b0cb8ac51b19587de947da3b3616a948d67b84`
- `9b771290bbbca2d03b39b4e76fd61233e615222a0d508935af616a37eb808e1b`

Production `/api/stats` later reflected:

```text
homework_worker_eligible_pylons_online_now=2
training_nodes_online=4
pylon/0.1.15 online_pylons=5
```

## Dispatch Proof

The successful dispatch response was:

```text
batch_id=dispatch.cs336.a1.20260427051711.23d5b3bd
training_run_id=run.cs336.a1.issue4451_proof2_20260427051711_23d5b3bd_0001.20260427051714.bac49a0f
window_id=window.cs336.a1.issue4451_proof2_20260427051711_23d5b3bd_0001.20260427051714.bac49a0f.0001
requested_run_count=1
launched_run_count=1
failed_run_count=0
max_contributors_per_run=2
amount_sats=1
max_payout_sats=2
```

The launch matched two eligible updated Pylons:

```text
7ba17a3af6343cd0070293a9b9b0cb8ac51b19587de947da3b3616a948d67b84 pylon/0.1.15 eligible=true
9b771290bbbca2d03b39b4e76fd61233e615222a0d508935af616a37eb808e1b pylon/0.1.15 eligible=true
```

After bootstrap materialization, the run detail reported:

```text
launch_phase=leaseable
assigned_contributors=2
accepted_contributors=0
model_progress_contributors=0
featured_window.status=sealed
```

The isolated Pylon claimed the second worker assignment, ran the bounded
Psionic A1 lane, uploaded/sealed the retained contribution, and released the
worker lease.

## Current Blocker

The production run is not yet accepted or paid.

Observed contribution state:

```text
contributor_node_id=9b771290bbbca2d03b39b4e76fd61233e615222a0d508935af616a37eb808e1b
validator_disposition=replay_required
accepted_for_aggregation=false
reward_sats=null
payout_status=null
```

Observed caveats:

```text
validator_backlog: 368 pending windows, 447 open challenges, 394 queued challenges
treasury_degraded: continuity_alert:dispatch_stalled
window_validation_pending: sealed but not reconciled into a final closeout
```

Pylon local closeout state also recorded:

```text
stage=window_sealed
next_action=await_validator_claim
last_error=training terminal artifact/TRN publication timed out after 30 seconds; direct authority closeout will continue and publication will retry later
```

## Cleanup

The temporary Pylon was set offline and its local worker process was stopped.
The temporary home was intentionally left in `/tmp/openagents-issue-4451-pylon`
for short-term inspection of local proof artifacts. It is not a tracked
workspace artifact.

## Verdict

Do not close `#4451` yet.

What is proven:

- updated `pylon/0.1.15` nodes can advertise homework-worker capability;
- Nexus can distinguish eligible homework workers from presence-only Pylons;
- broad production dispatch with the current version floor can match and assign
  two updated workers;
- one updated worker can claim, execute, and seal the bounded homework lane.

What remains unproven:

- validator replay for this production run;
- accepted aggregation for the sealed contribution;
- accepted-work payout for the updated worker;
- full issue acceptance.
