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

## Public Payout Accounting

The hosted treasury now exposes payout classes through the same `/api/stats`
and `/v1/treasury/status` path that already powers public payout state.

Canonical totals:

- `nexus_payout_sats_paid_total`
- `nexus_payout_sats_paid_24h`

Split totals:

- `nexus_accepted_work_payout_sats_paid_total`
- `nexus_accepted_work_payout_sats_paid_24h`
- `nexus_placeholder_payout_sats_paid_total`
- `nexus_placeholder_payout_sats_paid_24h`
- `nexus_beta_bonus_payout_sats_paid_total`
- `nexus_beta_bonus_payout_sats_paid_24h`
- `nexus_weak_device_accepted_work_payout_sats_paid_total`
- `nexus_weak_device_accepted_work_payout_sats_paid_24h`
- `nexus_strong_lane_accepted_work_payout_sats_paid_total`
- `nexus_strong_lane_accepted_work_payout_sats_paid_24h`

Interpretation rules:

- `nexus_payout_sats_paid_total` remains the umbrella hosted-treasury total.
- `nexus_accepted_work_*` is the accepted-work slice inside that umbrella.
- `nexus_placeholder_*` is the legacy liveness / placeholder slice.
- `nexus_beta_bonus_*` is the bonus / operator-adjusted slice.
- `nexus_weak_device_accepted_work_*` and
  `nexus_strong_lane_accepted_work_*` subdivide accepted-work payouts by lane.
- the strong-lane slice is where progress-bearing closeouts such as
  adapter/full-island/grouped-stage training land today

Training closeouts do not use a second payout system. Accepted training work is
queued into the existing hosted Nexus treasury loop with receipt metadata that
classifies:

- payout class
- payout basis
- work class
- progress class
- accepted outcome id
- training run id
- window id
- contribution id
- assignment id
- share basis and weight metadata
- weak-device versus strong-lane bearing

For the current launch-hardening slice, accepted-work closeouts still settle off
the shared `payout_sats_per_window` treasury setting. That means the split
counters are real and public now, but they are still fed by the same wallet,
dispatch loop, and budget policy that drive the generic hosted treasury totals.

Weak-device accepted-work payouts are allowed to dispatch without requiring the
node to still be online at payout time, as long as the closeout was accepted
and the node has a registered payout target. That preserves payout continuity
for validation-replay style work classes that may close after the worker has
gone idle.

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
- `NEXUS_CONTROL_TREASURY_MAX_CONCURRENT_SENDS`
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

`run_server()` now starts a dedicated treasury payout loop every 2 seconds. The
provider heartbeat route only updates presence; it no longer dispatches wallet
sends inline. The treasury loop keeps only one live payout cycle in flight at a
time, reconciles any missed per-identity windows after restarts, and clamps
recovery to `NEXUS_CONTROL_TREASURY_RECONCILIATION_HORIZON_SECONDS` so a stale
node does not try to replay an unbounded backlog blindly.

`NEXUS_CONTROL_TREASURY_MAX_CONCURRENT_SENDS` controls how many live wallet
sends can be dispatched concurrently inside one payout cycle. The default is
`16`, clamped to `64`. This matters in production because too-low concurrency
can hold the wallet-operation lock long enough that a nominal `20s` payout
interval stretches into `40-60s` effective receive spacing once many Pylons are
eligible at the same time.

For the hosted production Nexus, the current safe reference treasury policy is:

- `NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW=25`
- `NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS=600`
- `NEXUS_CONTROL_TREASURY_DAILY_BUDGET_CAP_SATS=1000000`

That policy keeps the hosted treasury under `864000 sats/day` even if the
eligible set reaches `240` providers and holds the steady-state Spark send rate
to `0.4` transfers/second instead of the unsustainable `5+` transfers/second
range that a `2 sats / 20s` policy reaches once the provider set crosses `100`.

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
`nexus-control` refreshes wallet-backed treasury stats every 3 seconds by
default. Treasury snapshots are treated as stale only after two missed refresh
windows, with a minimum 15 second stale budget, and the background refresh now
reads cached wallet balance plus bounded recent payment history instead of
walking the entire Spark payment ledger on every refresh. The refresh loop
tracks unresolved payout payment IDs and caps each cycle to a small page budget
so the paid-total counter keeps moving even after the wallet has accumulated
tens of thousands of payouts. `/api/stats` and
`GET /v1/treasury/status` no longer trigger wallet refresh inline.

If treasury state JSON ever deserializes badly on restart, `nexus-control` now
tries to recover from cached/derived fields first and, as a last resort,
salvages the persisted payout total from the state payload instead of silently
resetting the public paid-total counter to zero. That recovery path surfaces a
runtime error in treasury status until the next healthy refresh.

## Production Watchdog

Production payout continuity now also has a host-side watchdog installer:

```bash
scripts/deploy/nexus/10-install-treasury-watchdog.sh
```

`03-configure-and-start.sh` runs that installer by default when
`NEXUS_TREASURY_WATCHDOG_ENABLED=true`.

The watchdog runs on the VM every 5 minutes and uses two signals:

- the local `http://127.0.0.1:8080/v1/treasury/status` endpoint
- recent `Inserted payment ... status: Completed` journal lines from
  `nexus-relay`

That split matters operationally:

- a stale public snapshot alone should not trigger restart if fresh completed
  sends are still flowing
- the watchdog now honors a startup grace window, so a fresh restart is not
  judged against stale pre-restart dispatch and confirmation timestamps before
  the service has had time to finish wallet sync and reach the first payout
  window
- wallet/runtime hard errors, unreachable local treasury status, or sustained
  payout idleness with sellable Pylons online should trigger an automatic
  `systemctl restart nexus-relay`
- the default restart ceiling is `12/hour`, which matches the worst-case upper
  bound for a 5-minute timer and avoids suppressing legitimate recovery during
  a bad hour

Watchdog knobs:

- `NEXUS_TREASURY_WATCHDOG_INTERVAL_SECONDS`
- `NEXUS_TREASURY_WATCHDOG_MAX_IDLE_SECONDS`
- `NEXUS_TREASURY_WATCHDOG_MAX_CONFIRM_LAG_SECONDS`
- `NEXUS_TREASURY_WATCHDOG_MAX_RESTARTS_PER_HOUR`
- `NEXUS_TREASURY_WATCHDOG_STARTUP_GRACE_SECONDS`
- `NEXUS_TREASURY_WATCHDOG_LOCAL_STATUS_URL`
- `NEXUS_TREASURY_WATCHDOG_SERVICE_NAME`

## Deploy Smoke Rollback

`scripts/deploy/nexus/03-configure-and-start.sh` now runs a post-restart payout
smoke check by default. The rollout only sticks if the freshly started image
produces completed payout sends after restart. If the smoke check times out, it
automatically rolls production back to the previous image.

The smoke gate now treats the first `NEXUS_DEPLOY_POST_RESTART_WARMUP_GRACE_SECONDS`
after restart as explicit treasury warmup time and logs that phase as
`warming_up` instead of a generic stall. The default smoke timeout is now
`360s`, which gives production wallet sync and the first payout window room to
settle before rollback is considered.

Smoke knobs:

- `NEXUS_DEPLOY_POST_RESTART_SMOKE_ENABLED`
- `NEXUS_DEPLOY_POST_RESTART_SMOKE_TIMEOUT_SECONDS`
- `NEXUS_DEPLOY_POST_RESTART_WARMUP_GRACE_SECONDS`
- `NEXUS_DEPLOY_POST_RESTART_SMOKE_POLL_SECONDS`

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
- `training_payout_ledger_summary`
- `payout_target_identities`
- `recent_training_payouts`

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
- dispatch and confirmation stall detection now keys off the oldest still-
  pending payout work and is recomputed live from current treasury state, so a
  hung dispatch cycle still surfaces a critical alert through
  `/v1/treasury/status` and `/api/stats`.
- critical alerts are also reflected directly in `payout_loop_health` and
  `degraded_reason`, so operators do not need to infer failures from homepage
  behavior.

Reason metrics:

- `skip_reason_metrics_24h` is the 24-hour grouped breakdown of skipped payout
  reasons such as `daily_budget_cap_reached` and `missing_payout_target`
- `fail_reason_metrics_24h` is the 24-hour grouped breakdown of failed payout
  reasons such as wallet dispatch failures or dispatch timeouts

Canonical training payout ledger:

- payout destination enrollment and rotation continue to use the same
  node-identity-backed `treasury.payout_target.registered` flow; there is no
  separate training-only payout identity system
- `training_payout_ledger_summary` gives operators the current reconciliation
  state for the payout ledger, including pending, attention-required, and
  accepted-work-specific counts
- `payout_target_identities` projects the currently registered payout targets
  keyed by node public key together with confirmed payout totals for that
  identity
- `recent_training_payouts` projects the recent canonical payout ledger rows,
  including payout class, weak-device and progress-bearing flags, accepted
  outcome references, payout target, wallet payment id, and reconciliation
  status

Deployment gating:

- `scripts/deploy/nexus/04-verify-gates.sh` now measures `/healthz`,
  `/api/stats`, and `/v1/treasury/status` latency directly on the VM and fails
  the rollout if latency exceeds the configured thresholds
- the verifier now also runs repeated local-origin probes against `/healthz`,
  `/api/stats`, and `/api/provider-presence/heartbeat?dry_run=true`, then fails
  the rollout if p95 or p99 tail latency exceeds the configured budget
- the deploy verifier now fails if live treasury policy diverges from the VM env
  file, if snapshot freshness regresses, or if critical treasury continuity
  alerts are active
- the deploy receipt now includes explicit gate pass/fail rows, endpoint
  latency, tail-latency samples, treasury policy evidence, recent payout
  activity, snapshot freshness, active continuity alerts, and the current
  training rollout-policy snapshot from `/api/training/rollout`
- Transcript 222 launch operations now use
  `docs/plans/transcript-222-training-launch-slos.md` for accepted-work payout
  latency thresholds and
  `docs/plans/transcript-222-training-incident-taxonomy.md` for payout backlog
  and reconciliation incident classification
