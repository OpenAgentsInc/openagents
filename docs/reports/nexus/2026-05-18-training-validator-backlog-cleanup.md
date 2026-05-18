# Nexus Training Validator Backlog Cleanup

Date: 2026-05-18

Issue: <https://github.com/OpenAgentsInc/openagents/issues/4507>

## Summary

This change separates fresh launch health from historical retained training
backlog. Stale active runs, stale unreconciled windows, and stale queued or
leased validator challenges are now counted separately from fresh work. The
public launch-health path can warn about retained backlog without reporting
`overall_status: bad` solely because old retained records still exist.

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

## Production Operator Steps

1. Deploy the commit containing this command through the normal Nexus deploy
   path.
2. Run a dry run inside the Nexus environment against the production kernel
   state path.
3. Review the report and confirm `protected_runs` covers accepted-work rows.
4. Run with `--apply`.
5. Archive the applied JSON report under `docs/reports/nexus/` or the
   production reports bucket.
6. Confirm `/api/stats` launch health is not `bad` solely due retained backlog.
7. Run one fresh targeted training proof to confirm fresh worker/validator
   paths remain separate from retained state.
