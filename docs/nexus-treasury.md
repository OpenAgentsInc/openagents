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
- `POST /v1/admin/treasury/refresh`
- `POST /v1/treasury/funding-target`
- `GET /v1/treasury/integration/export`
- `POST /v1/treasury/integration/public-snapshot`
- `POST /v1/admin/homework/cs336-a1/dispatch`

`treasury funding-target` uses the repo-owned Spark integration and returns the
current treasury Spark receive address, Bitcoin receive address, and optional
amount-specific Spark and Bolt11 invoices when an amount is requested. Hosted
Nexus pays Pylons to Spark addresses, so the Spark invoice or Spark address is
the preferred funding target for payout liquidity. The Bolt11 invoice remains a
compatibility target for Lightning payers, but Lightning invoice payment alone
is not proof that the wallet has Spark leaves available for Spark-address
payouts.

`POST /v1/admin/treasury/refresh` is the operator-safe manual refresh surface.
It requires the normal Nexus admin bearer token, runs one forced wallet refresh
without creating a wallet implicitly, and returns the same status payload as
`GET /v1/treasury/status`. Use it when payout confirmation visibility matters
now and waiting for the background refresh loop is not acceptable.

To create live operator funding material, call:

```bash
curl -fsS -X POST "https://nexus.openagents.com/v1/treasury/funding-target" \
  -H "Content-Type: application/json" \
  --data '{
    "amount_sats": 50000,
    "description": "OpenAgents Nexus treasury funding",
    "expiry_seconds": 3600
  }' |
  jq '{spark_invoice, spark_address, bolt11_invoice, bitcoin_address}'
```

Only a positive `amount_sats` produces amount-specific invoices. A no-amount
request is useful for receive addresses, not for invoice payment. For Nexus
payout liquidity, prefer `spark_invoice` or `spark_address`. Use
`bolt11_invoice` only when the payer cannot send Spark yet. The returned invoice
is the payment request to give the operator. It is not proof of payment.
Hosted Nexus should create the Spark invoice before attempting the compatibility
Bolt11 invoice; if Bolt11 creation fails, operators should still receive the
Spark invoice rather than losing the direct payout-liquidity path.

After the payer sends funds, verify the result with `/v1/treasury/status`.
Treat the invoice as paid only after the status surface shows the receive in
wallet state and subsequent accepted-work payout dispatch/confirmation. Do not
mistake funding-target creation, an HTTP `504`, a generic account balance
increase, or an unrelated cached-balance refresh for Spark payout liquidity.

If accepted-work payouts are queued with `wallet_balance_insufficient`, the
wallet refresh loop must keep reconciling even when no payout has dispatched
yet. Without that behavior, a paid funding invoice can remain invisible because
the cached funding-target snapshot looks fresh enough to skip refresh. Current
Nexus images should treat balance-blocked queued payouts as wallet
reconciliation work.

Funding target creation is a bounded wallet operation. Hosted Nexus uses
`NEXUS_CONTROL_TREASURY_FUNDING_TARGET_TIMEOUT_MS` and defaults to `10000` ms.
If the Spark wallet path is unhealthy, the endpoint must fail with
`treasury_funding_target_timeout:<ms>` instead of hanging the operator surface.
That timeout is an operator funding-target failure, not by itself proof that the
payout wallet is unusable. It must not overwrite a usable cached wallet balance
or poison post-deploy payout smoke as `wallet_runtime_status=error`; the wallet
refresh and payout dispatch loops own payout-wallet health.

When Nexus is served through the durable relay shell, the relay's embedded
Nexus-control proxy timeout must be longer than the funding-target wallet
timeout. Use `NEXUS_RELAY_AUTHORITY_HTTP_TIMEOUT_MS` for that proxy budget; the
default is `180000` ms. If this relay timeout is too short, operators see a
generic relay `502` before Nexus-control can return the real funding-target
status.

Important 2026-05-15 assessment: raising the public proxy/relay timeout is a
stopgap, not a product-quality fix. Historical Nexus reports already show
Spark wallet operations timing out at materially larger budgets:

- `docs/reports/nexus/20260420-090211-treasury-wallet-recovery-report-9273b5cbf537.json`
  timed out isolated current and rebuilt wallet inspections after `180000 ms`.
- `docs/reports/nexus/20260508-175455-deploy-receipt.json` recorded
  `sync_wallet_timeout:600000; using cached balance and bounded payment scan`.
- `docs/reports/nexus/20260503-190642-deploy-receipt.json` recorded
  `funding target returned without full wallet sync` and a zero-balance with
  receive-history degraded state.
- `docs/reports/nexus/20260423-172434-deploy-receipt.json` recorded
  `sync_wallet_timeout:20000` with a multi-hour wallet sync lag.
- `docs/reports/nexus/issue-4368-local-closure-20260420202905/post-deploy-smoke-funding-timeout.json`
  preserved the `treasury_funding_target_timeout:10000` class.

The current conclusion is that Nexus must stop putting fresh Spark sync,
invoice creation, and spendable-leaf proof on the critical path of an
interactive HTTP request. Keep the current longer proxy budget for safety, but
build toward async funding-target operations with idempotency keys, phase-level
timing, and typed degraded states such as `spark_wallet_sync_slow` or
`spark_leaf_selection_blocked`.

Do not retry production funding-target calls as a debugging loop; reproduce the
wallet/funding behavior locally or in the private treasury runner first, then
use hosted Nexus only as the live confirmation surface.

The periodic wallet refresh loop has a separate sync timeout budget from the
funding-target path. Current Nexus should derive that sync budget from
`wallet_status_refresh_seconds` instead of hard-coding `20000` ms, because the
hosted treasury wallet can accumulate enough history that a full Spark sync
takes materially longer than a short funding-target read. If the live wallet
history grows and refresh starts timing out, raise the configured wallet refresh
interval/timeout budget before treating the wallet as permanently degraded.

## Paced Homework Dispatch

Admins can pace accepted-work payouts by launching bounded batches of homework
runs instead of relying on a single always-reused run. The cron-safe surface is:

```bash
curl -X POST "$NEXUS_BASE_URL/v1/admin/homework/cs336-a1/dispatch" \
  -H "Authorization: Bearer $NEXUS_CONTROL_ADMIN_BEARER_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "run_count": 3,
    "max_contributors_per_run": 1,
    "amount_sats": 7,
    "total_budget_sats": 21,
    "run_slug_prefix": "cron.hourly",
    "reuse_existing_run": false
  }'
```

The endpoint is intentionally narrow: it dispatches CS336 A1 homework through
the same launch, window, validation, accepted-outcome, and treasury paths used
by the default hosted Pylon starter lane. Defaults are conservative:

- `run_count=1`
- `max_contributors_per_run=1`
- `amount_sats=1`
- `reuse_existing_run=false`
- `only_online=true`
- `min_pylon_version=0.1.7`
- `require_updated_build=false`
- `window_duration_seconds=1800`

`reuse_existing_run=false` is the important cron default. Each call creates
fresh run slugs and training run ids, so work may be duplicated intentionally
across intervals while the operator controls spend by call frequency, run
count, contributor count, and sats per accepted contribution. `total_budget_sats`
is a per-call guardrail: Nexus rejects the request when
`run_count * max_contributors_per_run * amount_sats` exceeds that cap.

Dispatch does not pay at launch time. It records a Lightning payout policy on
each homework run with `pay_only_on_accept=true`; payouts are queued only after
homework contributions are accepted during window reconciliation and are then
drained by the existing treasury dispatch loop. Do not use placeholder,
liveness, or every-four-hours payments as evidence for this lane.

## Private Treasury Integration

`nexus-control` now exposes a narrow bridge for the private `treasury` service
to drain backlog and publish canonical payout state without handing over
broader market authority.

Authentication:

- `Authorization: Bearer <token>`
- token source: `NEXUS_CONTROL_TREASURY_INTEGRATION_TOKEN`
- if the token is unset, both integration endpoints fail closed with
  `treasury_integration_disabled`

Export contract:

- `GET /v1/treasury/integration/export`
- returns the canonical payout inputs that the private service needs:
  - current treasury policy
  - current paid-total floor
  - registered payout target identities
  - live online identities from provider-presence

Import contract:

- `POST /v1/treasury/integration/public-snapshot`
- accepts the private service's canonical public treasury snapshot:
  - source and generation time
  - health and mode
  - wallet runtime projection
  - public payout totals / 24h counters
  - backlog counters

Overlay rules:

- a fresh imported canonical snapshot overrides the public treasury counters
  shown through `/api/stats` and `/v1/treasury/status`
- imported payout totals are treated as canonical floors and never allow the
  local paid-total counter to move backward
- local continuity alerts stay local-only; they are hidden from public stats
  when the imported snapshot is the active source
- if the imported snapshot goes stale, `nexus-control` falls back to its local
  treasury projection automatically

## Public Payout Accounting

The hosted treasury now exposes payout classes through the same `/api/stats`
and `/v1/treasury/status` path that already powers public payout state.

Canonical totals:

- `nexus_payout_sats_paid_total`
- `nexus_payout_sats_paid_24h`

Split totals:

- `nexus_accepted_work_payout_sats_paid_total`
- `nexus_accepted_work_payout_sats_paid_24h`
- `nexus_availability_stipend_payout_sats_paid_total`
- `nexus_availability_stipend_payout_sats_paid_24h`
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
- `nexus_availability_stipend_*` is the canonical availability-stipend slice.
- `nexus_placeholder_*` remains as the legacy alias for the same stipend slice.
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

Accepted-work closeouts and availability stipends now share the same hosted
wallet and dispatch loop, but they no longer share one policy object.

- accepted-work defaults use `accepted_work_policy`
- availability stipends use `availability_policy`
- both still project into the same hosted treasury totals and class-specific
  counters

The accepted-work path now reads its default payout amount from
`accepted_work_default_payout_sats` and its 24h budget cap from
`accepted_work_daily_budget_cap_sats`. Availability stipends keep using the
existing stipend-oriented interval, amount, mode, version-floor, and budget
fields. Changing the stipend amount, stipend cadence, or availability-send
concurrency must not change accepted-work closeout payouts.

Weak-device accepted-work payouts are allowed to dispatch without requiring the
node to still be online at payout time, as long as the closeout was accepted
and the node has a registered payout target. That preserves payout continuity
for validation-replay style work classes that may close after the worker has
gone idle.

## Availability Stipend Observability

`GET /v1/treasury/status` and the treasury data folded into `/api/stats` now
separate availability-stipend state from accepted-work state explicitly.

Canonical beneficiary counters:

- `availability_online_identities_now`
- `availability_online_host_clusters_now`
- `availability_stipend_eligible_beneficiaries_now`
- `duplicate_host_blocked_beneficiaries_now`
- `duplicate_payout_target_blocked_beneficiaries_now`
- `missing_payout_target_blocked_beneficiaries_now`
- `version_floor_blocked_beneficiaries_now`
- `readiness_blocked_beneficiaries_now`

Legacy compatibility counters still remain:

- `eligible_online_payout_targets`
- `duplicate_host_placeholder_blocked_online_targets`
- `min_new_accrual_version_blocked_online_targets`
- `min_new_accrual_unknown_version_online_targets`

`treasury status` also exposes `availability_beneficiary_debug_rows`. Each row
traces one current online identity through:

- identity pubkey and client version
- host fingerprint and registered payout target
- beneficiary kind and beneficiary dedupe key
- final stipend verdict reason
- whether the identity is stipend-eligible now
- the current stipend payout key
- the current payout row status and skip reason when a row already exists

Use that debug register when the operator needs to answer "why is this worker
not accruing?" or "which row owns this beneficiary right now?" without reading
raw payout-ledger rows by hand.

## Supported Breez Source

The repo-owned Spark integration must pin `crates/spark/Cargo.toml` to the
newest stable upstream tag from `https://github.com/breez/spark-sdk`. Do not
pin production Nexus treasury code to `AtlantisPleb/spark-sdk` or any other
fork unless the user explicitly approves a temporary emergency fork in writing.

Why this rule exists:

- older SDK pins hard-failed on newer backend tree node statuses such as
  `PARENT_EXITED`
- that failure can collapse treasury wallet visibility to `0 sats` even when
  funds still exist in the wallet
- upstream tags carry the current Spark/Breez wallet fixes; stale forks can lag
  behind wallet hydration, tree-selection, and parser changes

Hosted Nexus configures Breez with `prefer_spark_over_lightning=true` for the
treasury wallet. This keeps newly generated Bolt11 invoices Spark-preferred for
Spark-capable payers while preserving the standard Bolt11 compatibility path.
Payouts still require spendable Spark leaves because provider payout targets are
Spark addresses.

If a copied treasury wallet still reports suspiciously low or zero balance on
the current upstream tag, treat that as stale local wallet state or an upstream
wallet/runtime failure, not proof that funds are gone. Rebuild validation from
the mnemonic into a fresh storage dir before making operator decisions about
payout continuity or treasury solvency.

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
- `NEXUS_CONTROL_TREASURY_INTEGRATION_TOKEN`

Bootstrap / explicit policy-apply envs:

- `NEXUS_CONTROL_TREASURY_ENABLED`
- `NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW`
- `NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS`
- `NEXUS_CONTROL_TREASURY_ACCEPTED_WORK_DEFAULT_PAYOUT_SATS`
- `NEXUS_CONTROL_TREASURY_REQUIRE_SELLABLE`
- `NEXUS_CONTROL_TREASURY_DAILY_BUDGET_CAP_SATS`
- `NEXUS_CONTROL_TREASURY_ACCEPTED_WORK_DAILY_BUDGET_CAP_SATS`
- `NEXUS_CONTROL_TREASURY_AVAILABILITY_MAX_CONCURRENT_SENDS`
- `NEXUS_CONTROL_TREASURY_PLACEHOLDER_PAYOUT_MODE`
- `NEXUS_CONTROL_TREASURY_DEDUPE_PLACEHOLDER_HOSTS`
- `NEXUS_CONTROL_TREASURY_POLICY_APPLY_ENV`
- `NEXUS_CONTROL_TREASURY_POLICY_ALLOW_DESTRUCTIVE_ENV_CHANGE`
- `NEXUS_CONTROL_TREASURY_POLICY_CHANGE_REASON`

Availability policy still owns the per-beneficiary stipend cadence. The current
env mapping is:

- `NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW`:
  availability payout amount per window
- `NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS`:
  availability stipend cadence
- `NEXUS_CONTROL_TREASURY_DAILY_BUDGET_CAP_SATS`:
  availability stipend 24h budget cap
- `NEXUS_CONTROL_TREASURY_AVAILABILITY_MAX_CONCURRENT_SENDS`:
  availability stipend send concurrency
- `NEXUS_CONTROL_TREASURY_PLACEHOLDER_PAYOUT_MODE`:
  availability eligibility mode

Accepted-work policy now bootstraps separately:

- `NEXUS_CONTROL_TREASURY_ACCEPTED_WORK_DEFAULT_PAYOUT_SATS`:
  accepted-work default payout amount
- `NEXUS_CONTROL_TREASURY_ACCEPTED_WORK_DAILY_BUDGET_CAP_SATS`:
  accepted-work 24h budget cap

If the new accepted-work envs are unset, Nexus preserves legacy behavior by
bootstrapping them from the historical shared values on first read of legacy
policy state. After bootstrap, the persisted runtime policy is authoritative.

`NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS` is still the per-beneficiary
availability cadence. `nexus-control` now phases each beneficiary
deterministically within that interval, so eligible Pylons still receive one
stipend per interval but dispatches roll across the window instead of bunching
on a single wall-clock boundary. The beneficiary order is:

- verified host cluster first when `dedupe_hosts=true`
- payout target second
- pubkey identity third

Availability stipends also require a registered payout target plus an admitted
worker-capable training node. Raw provider presence alone is not enough to earn
the availability lane anymore.

`run_server()` now starts a dedicated treasury payout loop every 2 seconds. The
provider heartbeat route only updates presence; it no longer dispatches wallet
sends inline. The treasury loop keeps only one live payout cycle in flight at a
time, reconciles any missed per-identity windows after restarts, and clamps
recovery to `NEXUS_CONTROL_TREASURY_RECONCILIATION_HORIZON_SECONDS` so a stale
node does not try to replay an unbounded backlog blindly.

`NEXUS_CONTROL_TREASURY_MAX_CONCURRENT_SENDS` remains the global send ceiling
for accepted-work and bonus dispatch. The default is `16`, clamped to `64`.
This matters in production because too-low concurrency can hold the
wallet-operation lock long enough that a nominal `20s` payout interval
stretches into `40-60s` effective receive spacing once many Pylons are eligible
at the same time. The hosted production Nexus should currently pin this lower
than the default to avoid wedging entire send batches behind a Spark-side
stall.

`NEXUS_CONTROL_TREASURY_AVAILABILITY_MAX_CONCURRENT_SENDS` now lets the
availability lane run with a different ceiling without changing accepted-work
closeout behavior. If unset, it inherits the global send ceiling.

Accepted-work sends now have an additional hard cap inside `nexus-control`:
even if `NEXUS_CONTROL_TREASURY_MAX_CONCURRENT_SENDS` is higher, accepted-work
closeouts will only dispatch up to `4` Spark sends concurrently per payout
cycle. Placeholder and bonus lanes still follow the configured global cap. The
goal is to keep real worker payouts from tripping Spark transport or leaf
selection failures when several windows reconcile together.

For the hosted production Nexus, the current safe reference treasury policy is
homework-only:

- `NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW=25`
- `NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS=600`
- `NEXUS_CONTROL_TREASURY_DAILY_BUDGET_CAP_SATS=1000000`
- `NEXUS_CONTROL_TREASURY_ACCEPTED_WORK_DEFAULT_PAYOUT_SATS=25`
- `NEXUS_CONTROL_TREASURY_ACCEPTED_WORK_DAILY_BUDGET_CAP_SATS=1000000`
- `NEXUS_CONTROL_TREASURY_MAX_CONCURRENT_SENDS=4`
- `NEXUS_CONTROL_TREASURY_AVAILABILITY_MAX_CONCURRENT_SENDS=4`
- `NEXUS_CONTROL_TREASURY_PLACEHOLDER_PAYOUT_MODE=presence_only`
- `NEXUS_CONTROL_TREASURY_DEDUPE_PLACEHOLDER_HOSTS=true`

That policy keeps the placeholder lane paying live presence and keeps accepted
work closeouts on the same treasury. The availability settings remain present
for new stipends, while accepted-work defaults stay explicit for homework
closeouts. The global `4`-send cap plus the internal accepted-work cap keeps
real homework payout waves small and predictable while still allowing more than
one worker to settle in the same cycle.

`NEXUS_CONTROL_TREASURY_PLACEHOLDER_PAYOUT_MODE` controls what a placeholder
window actually means:

- `presence_only` pays any otherwise-eligible online client
- `inference_ready` only pays clients that are actually advertising a ready
  local Gemma lane or an open backend-ready inventory row. Use this only if you
  intentionally want to tighten the live payout lane.
- `disabled` stops placeholder accrual entirely while leaving accepted-work
  payouts alone

In all non-disabled modes, "otherwise-eligible" always means:

- a registered payout target exists
- host/payout-target dedupe allows a single beneficiary winner for the current
  payout window
- any configured version floor is satisfied

Additional readiness depends on payout mode:

- `presence_only` does not require an admitted training-node record; the
  beneficiary only has to be online under the current provider-presence view
- `inference_ready` still requires the beneficiary to be admitted as a
  worker-capable training node, with an online, eligible, and claimable worker
  lane under current kernel readiness

`NEXUS_CONTROL_TREASURY_DEDUPE_PLACEHOLDER_HOSTS=true` makes host cluster the
first availability beneficiary key. Nexus derives a best-effort host
fingerprint from provider heartbeat telemetry and blocks extra placeholder
payouts when multiple clients appear to be the same underlying machine. Even
when host dedupe is disabled, availability stipends still dedupe by payout
target before falling back to raw pubkey identity. This dedupe does not change
accepted-work payouts; it only stops availability inflation.

Fresh config defaults now use `presence_only` plus host dedupe. Old persisted
policy blobs that predate these fields also deserialize as `disabled`. Any
production Nexus that still shows `inference_ready` should be corrected with an
explicit policy apply before treating deploy payout smoke as meaningful.

When the fleet is ready to stop awarding fresh windows to the old
`0.0.1-rc*` line, Nexus now supports a separate new-accrual version floor:

- `NEXUS_CONTROL_TREASURY_MIN_NEW_ACCRUAL_PYLON_VERSION=<tag>`
- `NEXUS_CONTROL_TREASURY_MIN_NEW_ACCRUAL_STARTED_AT_UNIX_MS=<cutover_ms>`

That split is intentional:

- payout records for windows before the cutoff still reconcile and dispatch,
  even for old RC clients
- payout records for windows at or after the cutoff require the configured
  minimum Pylon version
- missing or invalid client-version claims are treated as blocked for new
  accrual once the cutoff is active

For Episode 223, the intended first floor is
`NEXUS_CONTROL_TREASURY_MIN_NEW_ACCRUAL_PYLON_VERSION=pylon-v0.1.1-rc1`. Leave
both envs unset until the mixed Mac/Linux upgrade path is actually live.

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
lowering the daily budget cap, widening the payout interval, turning on
`require_sellable`, tightening placeholder payouts from `presence_only` to
`inference_ready` or `disabled`, turning on placeholder host dedupe, introducing
a live new-accrual version floor, raising that floor, or moving its cutoff
earlier. Without the explicit destructive override, the deploy script now fails
closed.

If `NEXUS_CONTROL_TREASURY_WALLET_STATUS_REFRESH_SECONDS` is unset,
`nexus-control` refreshes wallet-backed treasury stats every 3 seconds by
default. Treasury snapshots are treated as stale only after two missed refresh
windows, with a minimum 15 second stale budget, and the background refresh now
forces an explicit Spark `sync_wallet` hydration before reading cached wallet
balance plus bounded recent payment history. The refresh loop now refuses to
accept a `0 sats` post-sync balance when treasury state already proves large
completed funding receives beyond the paid-out total; that case is treated as a
wallet hydration error instead of silently feeding dispatch. The refresh loop
tracks unresolved payout payment IDs and caps each cycle to a small page budget
so the paid-total counter keeps moving even after the wallet has accumulated
tens of thousands of payouts. `/api/stats` and
`GET /v1/treasury/status` no longer trigger wallet refresh inline.

Hosted treasury sets
`NEXUS_CONTROL_TREASURY_WALLET_REAL_TIME_SYNC_ENABLED=false` by default. This
keeps the Spark API key available for mainnet wallet operations while avoiding
the Breez real-time data-sync subscription as a production dependency for
treasury payout continuity. Re-enable it only after a targeted recovery report
proves the real-time sync path is healthy for the production wallet.

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
- the default restart mode is now `service_inactive_only`, which means a
  treasury failure degrades the treasury lane without tearing down the public
  Nexus ingress every five minutes
- if you explicitly opt into `NEXUS_TREASURY_WATCHDOG_RESTART_MODE=aggressive`,
  wallet/runtime hard errors, unreachable local treasury status, or sustained
  payout idleness with sellable Pylons online will trigger an automatic
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
- `NEXUS_TREASURY_WATCHDOG_RESTART_MODE`

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
git ls-remote https://github.com/breez/spark-sdk.git 'refs/tags/*'
```

For production-like recovery work:

- use a copied mnemonic and copied wallet storage, never the live production
  files in place
- if the reused storage still reports `0 sats` or an obviously stale balance on
  the current upstream tag, rebuild into a fresh storage dir from the same
  mnemonic and compare
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
export NEXUS_CONTROL_TREASURY_WALLET_RECOVERY_INSPECTION_TIMEOUT_MS=120000
export NEXUS_CONTROL_TREASURY_WALLET_RECOVERY_PARALLEL_INSPECTIONS=false

cargo run -p nexus-control -- treasury recovery-report --work-dir /tmp/nexus-treasury-recovery --json
```

What `recovery-report` does:

- copies the current wallet storage into `backup/current-storage`
- copies the mnemonic and treasury state into the same recovery work dir
- builds a fresh wallet state from the same mnemonic into `rebuilt-storage`
- gives each isolated Spark wallet balance, payment-list, and
  unclaimed-deposit read up to
  `NEXUS_CONTROL_TREASURY_WALLET_RECOVERY_INSPECTION_TIMEOUT_MS` milliseconds,
  defaulting to `120000` and clamped to 30 minutes
- inspects the current and rebuilt wallet storage serially by default; set
  `NEXUS_CONTROL_TREASURY_WALLET_RECOVERY_PARALLEL_INSPECTIONS=true` only for
  local or non-mutating probes where Spark upstream health can absorb two
  simultaneous historical syncs
- writes a machine-readable `recovery-report.json`
- records the latest report summary in treasury state/status

The report compares, at minimum:

- wallet identity pubkey
- current-storage reported balance
- rebuilt-storage reported balance
- completed receive/send payment counts and totals
- unclaimed deposit counts
- whether the rebuilt wallet materially diverges from the copied current storage

2. Only cut over after the report says `validation_passed=true` and
   `recommended_action=cutover_rebuilt_storage_after_service_stop`.

Local/manual cutover:

```bash
cargo run -p nexus-control -- treasury recovery-cutover --report-path /tmp/nexus-treasury-recovery/recovery-report.json --json
```

Production VM cutover:

```bash
export NEXUS_TREASURY_RECOVERY_INSPECTION_TIMEOUT_MS=120000
export NEXUS_TREASURY_RECOVERY_PARALLEL_INSPECTIONS=false
export NEXUS_TREASURY_RECOVERY_RUST_LOG=warn
export NEXUS_TREASURY_RECOVERY_REPORT_ATTEMPTS=3
export NEXUS_TREASURY_RECOVERY_REPORT_PATH=/var/lib/nexus-relay/treasury-wallet-recovery-<stamp>/recovery-report.json
scripts/deploy/nexus/09-recover-treasury-wallet.sh
```

The production wrapper runs the `nexus-control` binary shipped inside the
`nexus-relay` image and overrides the container entrypoint. Do not run recovery
commands against the default relay entrypoint; that starts the relay server
instead of executing the treasury command.

The production wrapper also takes a VM-local recovery lock, pauses the public
and treasury watchdog timers/services, applies a runtime `Restart=no` systemd
drop-in, runtime-masks `nexus-relay`, and removes any stale `nexus-relay`
container while the report or cutover command is inspecting wallet storage. Do
not run a second recovery wrapper in parallel. If the lock fails, wait for the
first recovery command to finish or clean up its recovery containers before
trying again. The wrapper performs registry login and image pull before
stopping `nexus-relay`, then avoids command-substitution capture while cleanup
is armed; recovery JSON is written through a normal temp file so a subshell,
watchdog, or `Restart=always` path cannot unmask or restart `nexus-relay` while
an inspection is still running.
After shell edits to the wrapper, run
`bash scripts/deploy/nexus/test-recover-treasury-wallet-shell-guards.sh`
before touching production.

Each isolated Spark wallet inspection gets up to
`NEXUS_CONTROL_TREASURY_WALLET_RECOVERY_INSPECTION_TIMEOUT_MS`; the production
wrapper exposes that as `NEXUS_TREASURY_RECOVERY_INSPECTION_TIMEOUT_MS`,
defaults it to `120000` ms, and clamps it to 30 minutes. The production wrapper
passes `NEXUS_TREASURY_RECOVERY_PARALLEL_INSPECTIONS=false` by default so the
current and rebuilt storage inspections do not double upstream Spark sync load;
enable it only for deliberate local or non-mutating probes. The wrapper also
passes `NEXUS_TREASURY_RECOVERY_SCAN_PAYMENTS=false` by default. Balance
comparison is enough for bounded wallet-store recovery, and Spark
payment-history or unclaimed-deposit scans can hang during the same class of
wallet-store incident. Set `NEXUS_TREASURY_RECOVERY_SCAN_PAYMENTS=true` only
for deliberate forensics after Nexus is already stable. The wrapper also
defaults `RUST_LOG` to `warn` so large payment-history syncs do not bury the
report JSON in per-payment info logs. Recovery report generation defaults to
three attempts and removes the partial work dir between failed attempts so
transient Spark upstream failures do not leave stale recovery artifacts.

If explicit Spark sync times out during report generation, the inspector makes
one bounded cached-balance read against the isolated local storage. A report
with `runtime_status=cached_after_sync_timeout` can validate the wallet identity
and cached balance comparison for report-only purposes, but it is not cutover
safe. Such reports recommend either `no_cutover_needed_sync_timeout_cached` or
`retry_live_sync_before_cutover`; the cutover command rejects both. Use that
evidence to avoid unnecessary wallet-storage swaps, then resolve live payout or
refresh behavior through the normal deploy smoke and treasury status evidence.

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
- `nexus_payout_sats_in_flight_total`
- `nexus_payout_sats_in_flight_24h`
- `nexus_payouts_dispatched_24h`
- `nexus_payouts_confirmed_24h`
- `nexus_payouts_failed_24h`
- `nexus_payouts_skipped_24h`
- `nexus_placeholder_payout_mode`
- `nexus_placeholder_payout_eligible_online_targets`
- `nexus_inference_ready_online_payout_targets`
- `nexus_duplicate_host_placeholder_blocked_online_targets`
- `inference_ready_pylons_online_now`
- `inference_ready_pylon_sessions_online_now`
- `pylon_reported_hosts_online_now`
- `pylon_sessions_missing_host_fingerprint_online_now`
- `likely_same_host_pylon_sessions_online_now`
- `likely_same_host_pylons_online_now`

Interpretation rules for the new `/api/stats` readiness counters:

- `pylons_online_now` is the distinct-provider count, not raw session count.
- `pylon_sessions_online_now` is the raw online session count.
- `inference_ready_pylons_online_now` and
  `inference_ready_pylon_sessions_online_now` separate ready providers from
  ready sessions so the public surface does not imply more capacity than is
  actually available.
- `pylon_reported_hosts_online_now` is the number of online sessions that
  published a host fingerprint Nexus can group.
- `likely_same_host_pylon_sessions_online_now` and
  `likely_same_host_pylons_online_now` are best-effort duplicate signals so the
  homepage can show when several Pylons appear to come from the same machine.
- `nexus_placeholder_payout_eligible_online_targets` is the count that still
  qualifies for placeholder payout under the active policy.
- `nexus_inference_ready_online_payout_targets` is the stricter inference-ready
  subset the payout loop can use as placeholder policy tightens.
- `nexus_duplicate_host_placeholder_blocked_online_targets` is the online count
  currently blocked from placeholder payout because a same-host duplicate rule
  applied.

Operator-safe loop health now projects through `GET /v1/treasury/status`:

- `last_wallet_sync_at_unix_ms`
- `last_wallet_refresh_attempt_at_unix_ms`
- `wallet_hydration_mode`
- `wallet_payment_scan_mode`
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
- `pending_confirmation_count`
- `tracked_payment_backlog_count`
- `min_new_accrual_pylon_version`
- `min_new_accrual_started_at_unix_ms`
- `min_new_accrual_version_gate_active`
- `placeholder_payout_mode`
- `eligible_online_payout_targets`
- `sellable_pylons_online_now`
- `inference_ready_online_payout_targets`
- `duplicate_host_placeholder_blocked_online_targets`
- `min_new_accrual_version_blocked_online_targets`
- `min_new_accrual_unknown_version_online_targets`
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

`GET /v1/treasury/status` now derives its response from current treasury state
instead of replaying the last persisted public snapshot verbatim. That keeps
tracked-payment visibility honest after payout mutations and makes
`pending_confirmation_count` / `tracked_payment_backlog_count` reflect the
current ledger immediately, even before an unrelated snapshot rebuild lands.

Public payout totals are now split into two explicit buckets:

- `*_payout_sats_paid_*` means confirmed-and-counted payout sats only.
- `*_payout_sats_in_flight_*` means real dispatched payout sats that still have
  a `payment_id` but have not reconciled to `confirmed` yet.

This keeps the public board honest about actual settled payout truth while
still showing operators that treasury has already initiated real sends.

Operator-safe policy audit now also projects through `GET /v1/treasury/status`:

- `policy_schema_version`
- `policy_checksum`
- `policy_runtime_status`
- `policy_last_error`
- `accepted_work_policy`
- `availability_policy`
- `recent_policy_changes`

`accepted_work_policy` and `availability_policy` are separate operator-readable
objects. Current status payloads expose:

- `accepted_work_policy.default_payout_sats`
- `accepted_work_policy.daily_budget_cap_sats`
- `availability_policy.payout_sats_per_window`
- `availability_policy.payout_interval_seconds`
- `availability_policy.require_sellable`
- `availability_policy.daily_budget_cap_sats`
- `availability_policy.max_concurrent_sends`
- `availability_policy.payout_mode`
- `availability_policy.dedupe_hosts`
- `availability_policy.version_floor`
- `availability_policy.version_floor_started_at_unix_ms`
- `availability_policy.version_gate_active`

Continuity alerts:

- `treasury.alert.raised` receipts fire when Nexus detects payout continuity
  breakage such as dispatch stalls, confirmation stalls, budget-cap exhaustion,
  policy-runtime blocking, or stale treasury snapshots.
- `treasury.alert.cleared` receipts fire when that condition recovers.
- When `placeholder_payout_mode=disabled`, legacy
  `placeholder_liveness` payout records are not allowed to keep the service in
  `dispatch_stalled` or `confirmations_stalled`. Homework-only production
  continuity is judged from payout classes that still matter under the active
  policy, especially `accepted_work`; an accepted-work payout stuck in queued,
  dispatching, or dispatched state still raises the critical alert.
- Treasury dispatch separates current wallet reservation from daily-budget
  accounting. `confirmed` payouts from the last 24 hours still count against
  `daily_budget_cap_sats`, but they do not reserve the wallet's current
  spendable balance. Already-`dispatched` payouts remain reconciliation work,
  but they also do not reserve current spendable balance forever. Only active
  `dispatching` records reserve current wallet balance for subsequent dispatch
  decisions.
- The deploy verifier treats stale wallet sync as non-fatal only when the
  wallet runtime is connected and there are no accepted-work payouts awaiting
  reconciliation. That matches the runtime loop, which intentionally avoids
  expensive wallet refreshes when there is no dispatched accepted-work payment
  to reconcile. During or after a real accepted-work payout, wallet freshness
  remains a hard verification input.
- The public continuity signal now follows the same rule. A connected wallet
  with stale sync metadata does not raise `snapshot_stale` or
  `wallet_snapshot_stale` by itself while accepted-work payout reconciliation is
  clean. Stale wallet sync becomes operator-visible again when there is
  reconciliation-relevant payout work, such as a dispatched payment awaiting
  confirmation or a balance-blocked queued accepted-work payout.
- dispatch and confirmation stall detection now keys off the oldest still-
  pending payout work and is recomputed live from current treasury state, so a
  hung dispatch cycle still surfaces a critical alert through
  `/v1/treasury/status` and `/api/stats`.
- wallet refresh now treats tracked payout ids as first-class reconciliation
  targets. If the bounded paged history scan misses a tracked `payment_id`,
  Nexus follows up with a direct Spark payment lookup for that exact id before
  leaving the payout unresolved. A single missing history page should not hold
  `confirmations_stalled` open for hours while newer payouts are already
  confirming.
- availability stipend dispatch is now suppressed while confirmation
  continuity is already broken. If a continuity-relevant dispatched payout is
  older than the stall threshold, or the pending-confirmation backlog reaches
  the bounded refresh page size, the stipend loop stops minting new dispatches
  until reconciliation catches up. This prevents treasury from continuing to
  spray real sends while the operator truth is already telling us confirmation
  visibility is degraded.
- availability stipend dispatch also applies beneficiary-local backpressure.
  If one beneficiary already has an older unresolved stipend in `dispatching`
  or `dispatched`, Nexus does not mint another stipend payout for that same
  beneficiary until the older one settles or fails. Other beneficiaries remain
  eligible. This prevents one slow-settling payout target from accumulating a
  large pile of real sends while the rest of the fleet is waiting.
- Spark leaf-selection failures now count as a first-class treasury
  spendability block. If a payout send fails with
  `wallet_send_retryable:leaf_selection:...`, Nexus marks the wallet runtime as
  unhealthy for dispatch, forces the wallet refresh loop immediately instead of
  waiting for the normal refresh interval, and suppresses all new payout
  dispatch until a successful wallet snapshot clears the block. This prevents
  the service from continuing to mint real payout attempts while Spark has a
  positive nominal balance but no currently selectable spendable leaves.
- very old legacy identity-scoped stipend rows are now quarantined into a
  separate operator-attention bucket once they outlive the normal
  reconciliation horizon. Nexus keeps those historical real-send rows visible
  through treasury status, but they no longer count as current
  continuity-relevant confirmation work and no longer globally suppress the
  current beneficiary-scoped presence rail forever.
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
