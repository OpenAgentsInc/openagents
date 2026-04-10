# Pylon Distributed Training Rehearsal Matrix

This document is the release gate for the admitted-node distributed-training
MVP.

It exists to answer one question honestly:

**Can `Psionic`, `Pylon`, and `Nexus` already run one bounded admitted
distributed-training lane end to end, survive the expected failure classes,
and publish an auditable closeout trail without silently widening the claim
surface?**

This is not a speculative roadmap note. This is the retained launch block for
Phase 5 in `docs/pylon/distributed-training-phase-tracker.md`. If this matrix
is not green, the distributed-training MVP is not ready to claim launch
readiness.

## Launch Truth Checklist

These statements must remain true in docs, code, operator guidance, and
release notes:

- The MVP is an admitted-node lane, not a permissionless hostile-network lane.
- The accepted unit is one sealed window, not one individual contribution.
- The first supported topology is homogeneous CUDA-only windowed data parallel
  training.
- Elastic membership may change only at window boundaries.
- `Nexus` is the authority for leases, windows, closeouts, and reputation.
- `Pylon` is the authority for local supervision, upload state, and retained
  operator state.
- `Psionic` owns runtime execution truth, validator replay, checkpoint truth,
  and machine-readable status packets.
- Release claims stop at the rehearsed surface. Mixed-backend windows,
  permissionless admission, and generalized hostile-network verifiability are
  not part of this gate.

If any public or operator-facing artifact violates those statements, the
rollout should stop.

## Canonical Gate

The canonical automation entrypoint is:

```bash
scripts/release/check-pylon-distributed-training-mvp.sh
```

The script writes a retained summary under
`target/pylon-distributed-training-rehearsal/<timestamp>/` and fails fast if
any required rehearsal bucket fails.

By default it expects a sibling standalone `psionic` checkout at
`../psionic`. Override that with:

```bash
OPENAGENTS_PSIONIC_REPO=/absolute/path/to/psionic \
scripts/release/check-pylon-distributed-training-mvp.sh
```

The gate is intentionally test-first. It uses focused exact rehearsals that
already exercise the admitted runtime, checkpoint, validator, scheduler,
publication, and restart seams. That is a better MVP release block than
pretending there is already one external multi-machine soak harness that proves
more than the current code can actually prove.

## First Proving Slice

The proving slice is the first mandatory release gate inside the broader
matrix. It must prove:

- one worker
- one validator
- one window
- one local checkpoint
- one durable upload
- one sealed-window closeout
- one published TRN trail

The retained proving slice is satisfied only when all of these exact checks
pass together:

```bash
cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --lib launch_manifest_requires_explicit_output_root -- --nocapture
cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --lib worker_manifest_requires_node_pubkey -- --nocapture
cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --lib validator_manifest_requires_replay_target_paths -- --nocapture
cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --lib sync_checkpoint_writeback_publishes_atomically -- --nocapture
cargo test -p pylon --lib training_artifact_courier_uploads_downloads_and_verifies_bundles -- --nocapture
cargo test -p pylon --lib training_checkpoint_server_serves_local_checkpoint_paths -- --nocapture
cargo test -p nexus-control --lib training_window_routes_plan_activate_seal_and_reconcile -- --nocapture
cargo test -p nexus-control --lib publish_training_trn_state_publishes_and_reuses_authoritative_coordinator_events -- --nocapture
```

Those checks bind the machine manifest contract, the local checkpoint and
durable upload path, one sealed training window with validator evidence, and
the published TRN receipt plus closeout trail into one retained proving slice.

## Deterministic Matrix

| Gate | Scenario | Retained checks | Expected result |
| --- | --- | --- | --- |
| A | proving slice | `psionic-train` manifest and checkpoint-writeback tests plus `pylon` artifact courier/checkpoint server plus `nexus-control` sealed-window and TRN publication tests | one worker plus validator plus window path closes successfully with checkpoint lineage and published TRN evidence |
| B | multi-node lease and late join boundary | `psionic-train` membership rejoin tests, `pylon` checkpoint server test, `nexus-control` lease replacement test | lease expiry becomes explicit, replacement workers can be leased, and late joiners have a live checkpoint surface |
| C | crash, drain, and restart recovery | `psionic-train` reliability restart test, `pylon` supervisor failure/drain/restart tests, `pylon` runtime-state restart test, `nexus-control` restart replay test | runtime failures stay receipted, restart does not lose retained state, and `Nexus` restart does not duplicate logical publication |
| D | validator accepted, replay-required, rejected, held, and timeout paths | `psionic-train` validator replay tests plus `nexus-control` digest-mismatch, escalation, and timeout tests | validation failures stay explicit, held windows block promotion, and timeout paths publish held closeouts with poor-validator labels |
| E | TRN outage and later catch-up | `pylon` queued-publication retry test plus `nexus-control` queued-publication retry-across-restart test | relay outage does not lose publication intent and later recovery reuses fingerprints instead of double-publishing |
| F | closeout, reputation, and operator inspection | `pylon` sync/status/admin tests plus `nexus-control` operator summary test plus `psionic-train` environment mismatch guard | operators can inspect the active state, closeouts and labels project into retained state, and environment mismatch refuses before silent drift |

## Repo-Level Gate

Run the full retained gate with:

```bash
scripts/release/check-pylon-distributed-training-mvp.sh
```

That script currently expands into these required buckets:

```bash
cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --lib launch_manifest_requires_explicit_output_root -- --nocapture
cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --lib worker_manifest_requires_node_pubkey -- --nocapture
cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --lib validator_manifest_requires_replay_target_paths -- --nocapture
cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --lib sync_checkpoint_writeback_publishes_atomically -- --nocapture
cargo test -p pylon --lib training_artifact_courier_uploads_downloads_and_verifies_bundles -- --nocapture
cargo test -p pylon --lib training_checkpoint_server_serves_local_checkpoint_paths -- --nocapture
cargo test -p nexus-control --lib training_window_routes_plan_activate_seal_and_reconcile -- --nocapture
cargo test -p nexus-control --lib publish_training_trn_state_publishes_and_reuses_authoritative_coordinator_events -- --nocapture

cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --lib expired_same_node_receipt_rejoins_without_manual_metadata_edits -- --nocapture
cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --lib failure_revision_can_rejoin_later -- --nocapture
cargo test -p nexus-control --lib training_scheduler_claims_leases_for_running_runs_and_replaces_expired_workers -- --nocapture

cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --lib checkpoint_and_restart_faults_recover_cleanly -- --nocapture
cargo test -p pylon --lib training_supervisor_records_logs_heartbeat_and_failure_receipt_on_failed_exit -- --nocapture
cargo test -p pylon --lib draining_and_restarting_training_supervisor_rotates_attempt_logs_without_losing_history -- --nocapture
cargo test -p pylon --lib training_runtime_state_round_trips_across_restart -- --nocapture
cargo test -p nexus-control --lib training_scheduler_state_reloads_after_restart_and_reuses_trn_publications -- --nocapture

cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --lib validator_accepts_replayed_contribution_and_scores_window -- --nocapture
cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --lib validator_marks_missing_sampled_replay_as_replay_required -- --nocapture
cargo test -p nexus-control --lib training_window_reconcile_refuses_stale_manifest_replay_and_bad_artifact_digest -- --nocapture
cargo test -p nexus-control --lib training_window_validation_escalates_and_blocks_held_reconcile -- --nocapture
cargo test -p nexus-control --lib training_window_timeout_publishes_held_closeout_and_validator_poor_label -- --nocapture

cargo test -p pylon --lib training_publish_queues_retry_state_when_relays_are_unavailable -- --nocapture
cargo test -p nexus-control --lib publish_training_trn_state_queues_retry_state_across_restarts_until_relays_recover -- --nocapture

cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --lib curriculum_refuses_environment_mismatch -- --nocapture
cargo test -p pylon --lib training_admin_routes_serve_status_and_refresh_node_records -- --nocapture
cargo test -p pylon --lib training_sync_ingests_closeouts_and_reputation_and_blocks_readvertisement -- --nocapture
cargo test -p pylon --lib training_status_report_surfaces_operator_state -- --nocapture
cargo test -p nexus-control --lib training_operator_summary_and_stats_surface_live_run_state -- --nocapture
```

If those commands do not pass together, the MVP launch gate is not satisfied.

## Scenario Notes

These are the required failure and recovery classes that the gate must keep
covering:

- single-node admitted proving slice
- multi-node lease replacement and membership churn at window boundaries
- late join and rejoin through a live checkpoint surface
- runtime crash, drain, and restart recovery
- lease expiry and reassignment
- checkpoint upload retry and durable artifact verification
- validator accepted path
- validator replay-required path
- validator escalation and held window path
- validator timeout path
- reconciliation after complete validator evidence
- TRN publication outage and catch-up
- `Nexus` restart during active scheduling and publication
- `Pylon` restart with retained local state
- closeout and reputation publication after reconciliation

This is the honest MVP bar. It is not the Apple matrix, not a mixed-backend
matrix, and not a permissionless-network audit program.
