# Nexus Treasury

`nexus-control` now owns a hosted Spark treasury wallet beside the existing
provider-presence and receipt infrastructure. Operators can inspect wallet state
and generate fresh funding targets without touching wallet storage directly.

## Operator Surfaces

CLI:

```bash
cargo run -p nexus-control -- treasury status
cargo run -p nexus-control -- treasury funding-target
cargo run -p nexus-control -- treasury funding-target --amount-sats 2100 --description "fund nexus treasury"
cargo run -p nexus-control -- treasury recovery-report --work-dir /tmp/nexus-treasury-recovery --json
cargo run -p nexus-control -- treasury recovery-cutover --report-path /tmp/nexus-treasury-recovery/recovery-report.json --json
```

HTTP:

- `GET /v1/treasury/status`
- `POST /v1/treasury/funding-target`

`treasury funding-target` uses the repo-owned Spark integration and returns the
current treasury Spark receive address, Bitcoin receive address, and an optional
Bolt11 invoice when an amount is requested.

## Supported Breez Floor

The repo-owned Spark integration is now pinned to `breez/spark-sdk 0.12.2`.
Do not roll production Nexus treasury back onto `0.6.6`.

Why this floor exists:

- `0.6.6` hard-failed on newer backend tree node statuses such as
  `PARENT_EXITED`
- that failure can collapse treasury wallet visibility to `0 sats` even when
  funds still exist in the wallet
- `0.12.2` includes the upstream `PARENT_EXITED` fix and tolerant unknown-status
  parsing

If a copied treasury wallet still reports suspiciously low or zero balance on
`0.12.2`, treat that as stale local wallet state, not proof that funds are
gone. Rebuild validation from the mnemonic into a fresh storage dir before
making operator decisions about payout continuity or treasury solvency.

## Runtime Configuration

The hosted treasury wallet runtime is still env-backed, but the payout policy
is now a persisted runtime object inside the treasury state file. On first boot,
`nexus-control` bootstraps that policy from env. After that, the persisted
policy is authoritative by default.

Wallet/runtime envs:

- `NEXUS_CONTROL_TREASURY_WALLET_MNEMONIC_PATH`
- `NEXUS_CONTROL_TREASURY_WALLET_STORAGE_DIR`
- `NEXUS_CONTROL_TREASURY_WALLET_NETWORK`
- `NEXUS_CONTROL_TREASURY_WALLET_API_KEY_ENV`
- `NEXUS_CONTROL_TREASURY_WALLET_STATUS_REFRESH_SECONDS`
- `NEXUS_CONTROL_TREASURY_RECONCILIATION_HORIZON_SECONDS`
- `NEXUS_CONTROL_TREASURY_REGISTRATION_CHALLENGE_TTL_SECONDS`

Bootstrap / explicit policy-apply envs:

- `NEXUS_CONTROL_TREASURY_ENABLED`
- `NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW`
- `NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS`
- `NEXUS_CONTROL_TREASURY_REQUIRE_SELLABLE`
- `NEXUS_CONTROL_TREASURY_DAILY_BUDGET_CAP_SATS`
- `NEXUS_CONTROL_TREASURY_POLICY_APPLY_ENV`
- `NEXUS_CONTROL_TREASURY_POLICY_ALLOW_DESTRUCTIVE_ENV_CHANGE`
- `NEXUS_CONTROL_TREASURY_POLICY_CHANGE_REASON`

`NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS` is still the per-identity
stipend cadence. `nexus-control` now phases each identity deterministically
within that interval, so online Pylons still receive one payout per interval
but dispatches roll across the window instead of bunching on a single wall-clock
boundary.

Any server path that mounts the in-process authority APIs must also start the
treasury background runtime. The standalone `nexus-control run_server()` path
does this directly, and the production durable shell in
`apps/nexus-relay/src/durable.rs` must use
`build_api_router_with_background_tasks(...)` rather than only merging the
routes.

That treasury runtime starts a dedicated payout loop every 2 seconds. The
provider heartbeat route only updates presence; it no longer dispatches wallet
sends inline. The treasury loop keeps only one live payout cycle in flight at a
time, reconciles any missed per-identity windows after restarts, and clamps
recovery to `NEXUS_CONTROL_TREASURY_RECONCILIATION_HORIZON_SECONDS` so a stale
node does not try to replay an unbounded backlog blindly.

For the production VM, `scripts/deploy/nexus/03-configure-and-start.sh` now
loads the persisted policy from `${NEXUS_CONTROL_TREASURY_STATE_PATH}` by
default and writes those values back into the container env file. That keeps
redeploys and rollbacks aligned with the live policy on the data disk.

To intentionally change policy through deploy env:

1. set the new policy env values
2. set `NEXUS_CONTROL_TREASURY_POLICY_APPLY_ENV=true`
3. set `NEXUS_CONTROL_TREASURY_POLICY_CHANGE_REASON=<why>`
4. if the change is destructive, also set `NEXUS_CONTROL_TREASURY_POLICY_ALLOW_DESTRUCTIVE_ENV_CHANGE=true`

Destructive policy changes include disabling treasury, lowering payout amount,
lowering the daily budget cap, widening the payout interval, or turning on
`require_sellable`. Without the explicit destructive override, the deploy
script now fails closed.

If `NEXUS_CONTROL_TREASURY_WALLET_STATUS_REFRESH_SECONDS` is unset,
`nexus-control` now treats wallet snapshots as stale after 3 seconds by
default. A dedicated background wallet refresh loop wakes every 1 second,
refreshes only when the cached wallet snapshot is missing or stale, and gives
each wallet refresh a 1.5 second timeout budget. `/api/stats` and
`GET /v1/treasury/status` no longer trigger wallet refresh inline.

## Upgrade Validation

Before or immediately after a Spark SDK roll-forward, validate all of the
following on the upgraded tree:

```bash
cargo test -p openagents-spark
cargo check -p nexus-control -p pylon -p autopilot-desktop -p openagents-provider-substrate
cargo run -p nexus-control -- treasury status
```

For production-like recovery work:

- use a copied mnemonic and copied wallet storage, never the live production
  files in place
- if the reused storage still reports `0 sats` or an obviously stale balance on
  `0.12.2`, rebuild into a fresh storage dir from the same mnemonic and compare
- do not conclude that funds were spent merely because the old local storage
  view is empty

## Wallet Recovery Workflow

Use the recovery flow when treasury reports `0 sats` or an obviously stale
balance despite funded receive history.

1. Validate on a copied wallet first.

```bash
export NEXUS_CONTROL_TREASURY_WALLET_MNEMONIC_PATH=/path/to/copied/treasury.mnemonic
export NEXUS_CONTROL_TREASURY_WALLET_STORAGE_DIR=/path/to/copied/treasury-wallet
export NEXUS_CONTROL_TREASURY_STATE_PATH=/path/to/copied/treasury-state.json

cargo run -p nexus-control -- treasury recovery-report --work-dir /tmp/nexus-treasury-recovery --json
```

What `recovery-report` does:

- copies the current wallet storage into `backup/current-storage`
- copies the mnemonic and treasury state into the same recovery work dir
- builds a fresh wallet state from the same mnemonic into `rebuilt-storage`
- writes a machine-readable `recovery-report.json`
- records the latest report summary in treasury state/status

The report compares, at minimum:

- wallet identity pubkey
- current-storage reported balance
- rebuilt-storage reported balance
- completed receive/send payment counts and totals
- unclaimed deposit counts
- whether the rebuilt wallet materially diverges from the copied current storage

2. Only cut over after the report says `validation_passed=true`.

Local/manual cutover:

```bash
cargo run -p nexus-control -- treasury recovery-cutover --report-path /tmp/nexus-treasury-recovery/recovery-report.json --json
```

Production VM cutover:

```bash
export NEXUS_TREASURY_RECOVERY_REPORT_PATH=/var/lib/nexus-relay/treasury-wallet-recovery-<stamp>/recovery-report.json
scripts/deploy/nexus/09-recover-treasury-wallet.sh
```

The cutover path:

- preserves the live wallet storage by renaming it into a rollback dir
- atomically swaps the validated rebuilt storage into the active wallet path
- updates treasury state so status surfaces show `wallet_storage_runtime_mode=rebuilt`
- seeds treasury state with the rebuilt wallet balance so payouts can resume
  immediately after restart

Rollback procedure:

1. stop `nexus-relay`
2. move the active rebuilt storage dir aside
3. move `wallet_storage_rollback_dir` back onto
   `NEXUS_CONTROL_TREASURY_WALLET_STORAGE_DIR`
4. start `nexus-relay`

## Public Stats

`nexus-control` now persists an atomic last-good treasury public snapshot inside
the treasury state. The website-facing stats route reads that snapshot directly
and only computes freshness metadata live, so a slow wallet serves stale-safe
data instead of blocking the request path.

Public-safe treasury counters now project through `nexus-control /api/stats`:

- `nexus_wallet_runtime_status`
- `nexus_wallet_last_error`
- `nexus_wallet_storage_runtime_mode`
- `nexus_wallet_balance_sats`
- `nexus_wallet_balance_updated_at_unix_ms`
- `nexus_treasury_snapshot_generated_at_unix_ms`
- `nexus_treasury_snapshot_age_ms`
- `nexus_wallet_sync_lag_ms`
- `nexus_payout_loop_health`
- `nexus_treasury_degraded_reason`
- `nexus_treasury_enabled`
- `nexus_treasury_payout_sats_per_window`
- `nexus_treasury_payout_interval_seconds`
- `nexus_treasury_require_sellable`
- `nexus_treasury_daily_budget_cap_sats`
- `nexus_registered_payout_identities`
- `nexus_payout_sats_paid_total`
- `nexus_payout_sats_paid_24h`
- `nexus_payouts_dispatched_24h`
- `nexus_payouts_confirmed_24h`
- `nexus_payouts_failed_24h`
- `nexus_payouts_skipped_24h`

Operator-safe loop health now projects through `GET /v1/treasury/status`:

- `wallet_storage_runtime_mode`
- `wallet_storage_report_path`
- `wallet_storage_rollback_dir`
- `wallet_storage_cutover_at_unix_ms`
- `wallet_recovery_last_report_generated_at_unix_ms`
- `wallet_recovery_last_report_validation_passed`
- `payout_loop_runtime_status`
- `payout_loop_last_error`
- `last_payout_reconciliation_at_unix_ms`
- `payout_loop_last_started_at_unix_ms`
- `payout_loop_last_completed_at_unix_ms`
- `public_snapshot_generated_at_unix_ms`
- `snapshot_age_ms`
- `wallet_sync_lag_ms`
- `eligible_online_payout_targets`
- `sellable_pylons_online_now`
- `latest_eligible_window_started_at_unix_ms`
- `last_dispatch_at_unix_ms`
- `last_confirmed_payout_at_unix_ms`
- `eligible_window_lag_ms`
- `dispatch_lag_ms`
- `confirm_lag_ms`
- `skip_reason_metrics_24h`
- `fail_reason_metrics_24h`
- `active_continuity_alerts`
- `payout_loop_health`
- `degraded_reason`

Operator-safe policy audit now also projects through `GET /v1/treasury/status`:

- `policy_schema_version`
- `policy_checksum`
- `policy_runtime_status`
- `policy_last_error`
- `recent_policy_changes`

Continuity alerts:

- `treasury.alert.raised` receipts fire when Nexus detects payout continuity
  breakage such as dispatch stalls, confirmation stalls, budget-cap exhaustion,
  policy-runtime blocking, or stale treasury snapshots.
- `treasury.alert.cleared` receipts fire when that condition recovers.
- critical alerts are also reflected directly in `payout_loop_health` and
  `degraded_reason`, so operators do not need to infer failures from homepage
  behavior.

Reason metrics:

- `skip_reason_metrics_24h` is the 24-hour grouped breakdown of skipped payout
  reasons such as `daily_budget_cap_reached` and `missing_payout_target`
- `fail_reason_metrics_24h` is the 24-hour grouped breakdown of failed payout
  reasons such as wallet dispatch failures or dispatch timeouts

Deployment gating:

- `scripts/deploy/nexus/04-verify-gates.sh` now measures `/healthz`,
  `/api/stats`, and `/v1/treasury/status` latency directly on the VM and fails
  the rollout if latency exceeds the configured thresholds
- the deploy verifier now fails if live treasury policy diverges from the VM env
  file, if snapshot freshness regresses, or if critical treasury continuity
  alerts are active
- the deploy receipt now includes explicit gate pass/fail rows, endpoint
  latency, treasury policy evidence, recent payout activity, snapshot freshness,
  and active continuity alerts
