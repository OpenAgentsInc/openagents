# Nexus Health Agent Production Drill - 2026-04-26

## Summary

This drill completed the production health-agent path for `OpenAgentsInc/openagents#4449`.

Production Nexus is reachable and the hosted health runner is executing from Google Cloud, not from the local admin laptop. After deploying `0c6e5b63ace6b83753f7e6183b8fd9f8db73b51e`, the hosted health-runner log at `2026-04-26T20:21:07Z` reported:

```text
status=completed
health_state=healthy
scheduler_status=hosted
external_status=reachable
snapshot_status=captured
summary=all health predicates passed
```

The local verification pack after deployment also reported `observation_status=healthy`, all required gates passed, and only one advisory blocker remained: two readiness-blocked Pylon beneficiaries.

## Deployment

Deployed Nexus relay commit:

```text
0c6e5b63ace6b83753f7e6183b8fd9f8db73b51e
```

Change deployed:

- `training_public_state.launch_health.overall_status` no longer escalates ordinary artifact resolver and signed-access latency warnings to `bad` unless the measured p95 exceeds 10x the configured SLO.
- The prior behavior made Nexus look hard-degraded while training dispatch and payouts were still functioning.

Deployment receipts:

- `docs/reports/nexus/20260426-194534-warm-builder-build-0c6e5b63ace6.json`
- `docs/reports/nexus/20260426-201255-binary-release-upload-0c6e5b63ace6.json`
- `docs/reports/nexus/20260426-201533-binary-release-activate-0c6e5b63ace6.json`

Focused tests run before deploy:

```text
cargo fmt -p nexus-control --check
cargo test -p nexus-control public_stats_launch_health_tracks_artifact_resolver_and_signed_access_latency -- --nocapture
cargo test -p nexus-control health_agent -- --nocapture
cargo test -p nexus-control health_verification -- --nocapture
```

## Incident During Deploy

During the first upload attempt, `gcloud compute scp --tunnel-through-iap` hung while copying the 101 MB release archive to `nexus-mainnet-1`. Shortly after, public Nexus returned Cloudflare `530` / `1033`.

Observed facts:

- GCE API still reported `nexus-mainnet-1` as `RUNNING`.
- IAP SSH failed with `Connection timed out during banner exchange`.
- Serial output showed `nexus-relay` still running and `cloudflared` forwarding requests, but Cloudflare edge returned `1033` publicly.
- The public watchdog falsely reported healthy during startup grace and did not recover the tunnel state.

Recovery action:

```text
gcloud compute instances reset nexus-mainnet-1 --project openagentsgemini --zone us-central1-a --quiet
```

Result:

- `https://nexus.openagents.com/healthz` returned HTTP 200 immediately after boot.
- IAP SSH recovered.
- `nexus-relay` and `nexus-cloudflared` were both active.
- Upload was retried and activation completed.

Follow-up needed:

- The public watchdog must treat Cloudflare `1033` as a hard outage even during startup grace when the VM has already been up and serving: `OpenAgentsInc/openagents#4450`.
- The deploy runbook should avoid leaving long IAP uploads unbounded; add an upload timeout and an explicit public-edge check before and after binary activation: `OpenAgentsInc/openagents#4452`.

## Health Evidence

Post-deploy `/api/stats` at `2026-04-26T20:17Z`:

```text
nexus_wallet_runtime_status=connected
nexus_payout_loop_health=idle
nexus_wallet_balance_sats=19250
nexus_payout_sats_paid_total=1183482
nexus_accepted_work_payout_sats_paid_total=3347
nexus_accepted_work_payout_sats_paid_24h=25
training_public_state.launch_health.overall_status=warn
```

Launch-health alerts after deploy were warnings only:

- `run_backlog`
- `validator_backlog`

The required health verification pack initially failed after deployment only because payout confirmations were just over the 30-minute freshness threshold. An admin treasury refresh settled additional confirmations:

```text
payouts_confirmed_24h: 49 -> 62
pending_confirmation_count: 25 -> 23
last_confirmed_payout_at_unix_ms: 1777232866000 -> 1777234417000
```

The final verification pack reported:

```text
status=advisory
snapshot.observation_status=healthy
classification.summary=all health predicates passed
payout_capability=passed
training_dispatch=passed
website_stats_freshness=passed
infra_availability=passed
```

Important final treasury fields:

```text
wallet_runtime_status=connected
payout_loop_health=running
payout_sats_in_flight_total=25
availability_stipend_payout_sats_in_flight_total=25
payouts_dispatched_24h=1
payouts_confirmed_24h=74
payouts_failed_24h=0
last_dispatch_lag_ms=28921
last_confirmation_lag_ms=29838
eligible_online_payout_targets=21
```

After one more payout interval, public stats showed `nexus_payout_sats_paid_total=1185207`, up from `1183482` after activation, and wallet balance moved down to `17525` sats. That confirms treasury sats are leaving the wallet for online-presence payouts.

## Training Work Evidence

Previously proven during this issue's `pylon-v0.1.15` production proof:

- Isolated `pylon-v0.1.15` worker wallet received two completed accepted-work Spark payments of 25 sats each.
- Worker wallet balance reached 50 sats.
- Public accepted-work paid total moved from `3322` to `3347`.

Additional post-deploy broad dispatch attempted with current version floor:

```json
{
  "run_count": 1,
  "max_contributors_per_run": 2,
  "amount_sats": 25,
  "only_online": true,
  "min_pylon_version": "0.1.15",
  "require_updated_build": true
}
```

Result:

```text
launched_run_count=0
failed_run_count=1
reason=homework_launch_no_eligible_pylons
```

Fleet reality at that point:

- 4 `pylon/0.1.15` sessions were online for presence payouts.
- The only two online admitted training nodes were `openagents.pylon@0.1.1` nodes scoped to `trainnet.cs336.a1.4368proof4`.
- Of those, one was admitted as a worker and one as a validator.

Controlled broad dispatch against the admitted training network:

```json
{
  "run_count": 1,
  "max_contributors_per_run": 2,
  "amount_sats": 25,
  "total_budget_sats": 50,
  "network_id": "trainnet.cs336.a1.4368proof4",
  "only_online": true,
  "min_pylon_version": "0.1.1",
  "require_updated_build": false
}
```

Result:

```text
training_run_id=run.cs336.a1.healthdrill-oldtrain-20260426202549_20260426202553_537aaa81_0001.20260426202553.319fe4d4
current_window_id=window.cs336.a1.healthdrill-oldtrain-20260426202549_20260426202553_537aaa81_0001.20260426202553.319fe4d4.0001
launch_state=created
launch_phase=leaseable
matched=1
assigned=1
assigned_node=prod-iso-worker-1-20260417
assigned_release=openagents.pylon@0.1.1
```

Follow-up status:

```text
run_status=running
window_state=active
assigned_contributors=1
accepted_contributors=0
validator_challenges_open=0
validator_challenges_queued=0
```

Conclusion: production can launch and lease homework work to the currently admitted worker, but it cannot currently assign multiple updated `pylon-v0.1.15+` clients because those live sessions are not advertising/admitted as training workers. That is a separate fleet/admission gap, not a Nexus health-agent outage.

## Current State

Nexus health:

```text
healthy
```

Hosted monitor:

```text
running every minute from Cloud Run / Cloud Scheduler
latest observed hosted state: healthy
```

Treasury:

```text
connected
presence payouts moving and confirming
accepted-work payout queue clear
```

Training:

```text
dispatch path operational
launch health warning due historical backlog
updated v0.1.15 presence clients are not yet admitted training workers
```

## Next Work

1. Fix the public watchdog so Cloudflare `1033` is an immediate recovery trigger, including during startup grace when the VM is already running.
2. Add timeout and retry controls around binary release upload so a stalled IAP transfer cannot mask or contribute to a public-edge incident.
3. Add a fleet admission path that turns updated `pylon-v0.1.15+` online sessions into admitted homework workers, or explicitly surfaces why they are presence-only.
4. Add a health-agent check that distinguishes "presence payout eligible" from "homework worker eligible" and reports both counts.
5. Add a runbook step to verify at least two online admitted workers exist before claiming multi-Pylon training payout success.

Tracking issues:

- `OpenAgentsInc/openagents#4450` - self-heal public Cloudflare `1033`.
- `OpenAgentsInc/openagents#4451` - admit updated online Pylons as homework workers or surface why they are presence-only.
- `OpenAgentsInc/openagents#4452` - bound Nexus binary upload time and verify public edge before activation.
