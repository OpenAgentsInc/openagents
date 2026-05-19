# Nexus Training Validator Backlog Cleanup

Date: 2026-05-18

Issue: <https://github.com/OpenAgentsInc/openagents/issues/4507>

## Summary

This change separates fresh launch health from historical retained training
backlog. Stale active runs, stale unreconciled windows, and stale queued or
leased validator challenges are now counted separately from fresh work. The
public launch-health path can warn about retained backlog without reporting
`overall_status: bad` solely because old retained records still exist.

Production cleanup was completed on 2026-05-19 after four integrated changes:

- `95b11e5c4` added the live admin cleanup endpoint.
- `b14bafa66` kept accepted-work evidence out of retained backlog health.
- `4f6a3c398` stopped terminal accepted-progress rows from counting active.
- `cc86037ad` limited no-open-work first-window standby accounting to the
  first 10 minutes after run creation.

It also adds an explicit operator command for retiring historical backlog with
receipts:

```bash
nexus-control training backlog-cleanup \
  --retention-hours 24 \
  --report-path /var/lib/nexus-relay/reports/training-backlog-cleanup-dry-run.json
```

Apply only after reviewing the dry run:

```bash
nexus-control training backlog-cleanup \
  --apply \
  --retention-hours 24 \
  --report-path /var/lib/nexus-relay/reports/training-backlog-cleanup-applied.json
```

## What The Command Does

- Cancels stale active training runs older than the retention cutoff.
- Reconciles stale unreconciled adapter windows older than the retention cutoff.
- Force-times-out stale open validator challenges with the explicit failure
  code `stale_retained_backlog`.
- Removes retired run IDs from the in-memory scheduler indexes when applied.
- Preserves runs and windows with accepted-work evidence.
- Writes a `kernel.training.backlog.cleanup` receipt when applied and changed.
- Emits a JSON report with before and after retained counts, retired rows,
  protected runs, and the receipt ID.

## Protected Evidence

Cleanup intentionally does not erase accepted-work payout evidence. Runs with
accepted outcomes are reported under `protected_runs` and are not cancelled.
Windows with accepted outcome links are not reconciled away by the cleanup
pass. Retired validator challenges are terminalized with an explicit stale
retention failure code instead of being silently deleted.

## Health Semantics

Launch health now has both total and fresh/retained counters:

- `active_runs`
- `fresh_active_runs`
- `retained_active_runs`
- `pending_validation_windows`
- `fresh_pending_validation_windows`
- `retained_pending_validation_windows`
- `validator_challenges_open`
- `fresh_validator_challenges_open`
- `retained_validator_challenges_open`
- `validator_challenges_queued`
- `fresh_validator_challenges_queued`
- `retained_validator_challenges_queued`

Critical `run_backlog` and `validator_backlog` alerts use fresh counters only.
Historical retained backlog produces a warning alert named
`retained_training_backlog`.

## Local Proof

The local default-state dry run was executed from the repo root:

```bash
cargo run -p nexus-control --bin nexus-control -- \
  training backlog-cleanup \
  --retention-hours 24 \
  --report-path docs/reports/nexus/2026-05-18-training-validator-backlog-cleanup-dry-run.json
```

Result:

- changed: false
- retained active runs: 0
- retained active windows: 0
- retained pending validation windows: 0
- retained open validator challenges: 0
- retained queued validator challenges: 0
- protected active runs with accepted outcomes: 0

The full local dry-run JSON is committed at:

```text
docs/reports/nexus/2026-05-18-training-validator-backlog-cleanup-dry-run.json
```

This proves the command path, parser, report writer, and local state load. It
does not claim production backlog was applied. Production cleanup must be run
inside the deployed Nexus environment or against a copied production kernel
state path.

Fresh training proof was then run against the changed code:

```bash
target/debug/oa proof run cs336-a1 \
  --namespace issue-4507-backlog-cleanup-3 \
  --workers 1 \
  --validators 1 \
  --timeout-seconds 600 \
  --json
```

Result:

- status: completed
- training run:
  `run.cs336.a1.proof.issue.4507.backlog.cleanup.3`
- reconciled window:
  `window.cs336.a1.proof.issue.4507.backlog.cleanup.3.0001`
- accepted contributions: 1
- closeout: rewarded
- workers quiesced: 1
- validators quiesced: 1
- open validator challenges after proof: 0
- queued validator challenges after proof: 0
- caveats: 0

Proof artifacts:

```text
/Users/christopherdavid/.openagents/pylon/proof/namespaces/issue-4507-backlog-cleanup-3/fleet/run-report.json
/Users/christopherdavid/.openagents/pylon/proof/namespaces/issue-4507-backlog-cleanup-3/fleet/proof-summary.json
/Users/christopherdavid/.openagents/pylon/proof/namespaces/issue-4507-backlog-cleanup-3/fleet/authority-state-trace.json
```

Earlier proof attempts under `issue-4507-backlog-cleanup` and
`issue-4507-backlog-cleanup-2` timed out while the worker-side Psionic runtime
was still cold-building. They were torn down and not used as acceptance proof.

## Verification

Executed:

```bash
cargo check -p nexus-control
cargo test -p openagents-validator-service force_timeout_terminalizes_retained_challenge_without_lease
cargo test -p nexus-control launch_health_keeps_retained_training_backlog_out_of_critical_path
cargo fmt --check
bash scripts/deploy/nexus/test-ldk-deploy-invariants.sh
git diff --check
```

## Production Execution

The production image deployed for the final accounting fix was:

```text
us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:cc86037ad518
```

Deployment used the normal registry-backed path:

```bash
DEPLOY_IMAGE=us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:cc86037ad518 \
  bash scripts/deploy/nexus/03-configure-and-start.sh

DEPLOY_IMAGE=us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:cc86037ad518 \
  bash scripts/deploy/nexus/04-verify-gates.sh
```

Receipts:

```text
docs/reports/nexus/20260519-052054-cloudbuild-image-cc86037ad518.json
docs/reports/nexus/20260519-053217-deploy-receipt.json
```

The live cleanup was run through the authenticated Nexus admin endpoint:

```bash
POST https://nexus.openagents.com/v1/admin/training/backlog-cleanup
```

Use the endpoint for live production cleanup. Running `docker exec
nexus-control training backlog-cleanup` against mounted state from a running
relay is unsafe as the steady operator path because the live relay may rewrite
the same kernel state while the process-local command is reading and writing.
The admin endpoint executes inside the running authority process and writes the
cleanup receipt through the same state owner.

Production apply summary:

```json
{
  "applied": true,
  "changed": true,
  "before": {
    "active_runs": 1,
    "active_windows": 1,
    "pending_validation_windows": 1,
    "validator_challenges_open": 2,
    "validator_challenges_queued": 2,
    "protected_active_runs_with_accepted_outcomes": 537
  },
  "after": {
    "active_runs": 0,
    "active_windows": 0,
    "pending_validation_windows": 0,
    "validator_challenges_open": 0,
    "validator_challenges_queued": 0,
    "protected_active_runs_with_accepted_outcomes": 537
  },
  "retired_runs": 1,
  "retired_windows": 1,
  "retired_challenges": 2,
  "protected_runs": 537,
  "receipt_id": "receipt.kernel.training.backlog.cleanup:sha256:1a3edeb71243bf5fd44e5a9311730a2deb544637a358ffc84b58f6ddd8e613c9"
}
```

Production no-op replay summary:

```json
{
  "applied": true,
  "changed": false,
  "before": {
    "active_runs": 0,
    "active_windows": 0,
    "pending_validation_windows": 0,
    "validator_challenges_open": 0,
    "validator_challenges_queued": 0,
    "protected_active_runs_with_accepted_outcomes": 537
  },
  "after": {
    "active_runs": 0,
    "active_windows": 0,
    "pending_validation_windows": 0,
    "validator_challenges_open": 0,
    "validator_challenges_queued": 0,
    "protected_active_runs_with_accepted_outcomes": 537
  },
  "retired_runs": 0,
  "retired_windows": 0,
  "retired_challenges": 0,
  "protected_runs": 537,
  "receipt_id": null
}
```

Cleanup reports:

```text
docs/reports/nexus/20260519-standby-accounting-backlog-cleanup-apply.json
docs/reports/nexus/20260519-standby-accounting-backlog-cleanup-noop.json
```

## Production Verification

Public health:

```bash
curl -fsS https://nexus.openagents.com/healthz
```

Result:

```json
{
  "ok": true,
  "service": "nexus-relay",
  "relay_backend": "durable-upstream",
  "authority_mode": "in-process",
  "managed_groups_mode": "recovery-proxy",
  "recovery_proxy": true
}
```

Public stats after the final deploy and cleanup:

```json
{
  "training_runs_active": 1,
  "training_windows_active": 1,
  "training_windows_pending_validation": 0,
  "training_validator_challenges_open": 0,
  "training_validator_challenges_queued": 0,
  "nexus_treasury_provider": "ldk",
  "nexus_wallet_balance_sats": 3843,
  "launch_health": {
    "overall_status": "good",
    "active_runs": 1,
    "fresh_active_runs": 1,
    "retained_active_runs": 0,
    "run_backlog_slots": 0,
    "pending_validation_windows": 0,
    "fresh_pending_validation_windows": 0,
    "retained_pending_validation_windows": 0,
    "validator_challenges_open": 0,
    "fresh_validator_challenges_open": 0,
    "retained_validator_challenges_open": 0,
    "validator_challenges_queued": 0,
    "fresh_validator_challenges_queued": 0,
    "retained_validator_challenges_queued": 0,
    "accepted_work_pending_payout_count": 0,
    "accepted_work_attention_payout_count": 0,
    "active_alert_count": 0,
    "critical_alert_count": 0
  }
}
```

The remaining single active run is a fresh hosted starter run created after
cleanup by the normal lease-claim path. It is not retained backlog. It has one
fresh active window and no open or queued validator challenges.

The treasury status is LDK-backed and not degraded:

```json
{
  "wallet_balance_sats": 3843,
  "degraded_reason": null
}
```

Additional focused verification for the final accounting fix:

```bash
cargo fmt
cargo fmt --check
cargo test -p nexus-control training_summary_does_not_count_stale_first_window_standby_run_as_active --lib
cargo test -p nexus-control training_operator_summary_and_stats_surface_live_run_state --lib
cargo test -p nexus-control training_backlog --lib
git diff --check
```

All focused tests passed. Existing Rust dead-code warnings remain unrelated to
this cleanup.
