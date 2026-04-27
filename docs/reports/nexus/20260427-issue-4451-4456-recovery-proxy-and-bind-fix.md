# Issue 4451 / 4456 Recovery And Closeout Attempt

Date: 2026-04-27

## Summary

The `pylon-v0.1.16` release prerequisite is complete, and production Nexus now
uses `NEXUS_CONTROL_CS336_HOMEWORK_AUTO_DISPATCH_MIN_PYLON_VERSION=0.1.16`.

The remaining #4451 and #4456 acceptance criteria are not safe to close from the
current production state. Production proves multiple updated Pylons can be
homework-worker eligible and that the existing #4451 proof run assigned two
workers, but that run is still sealed/pending validation with no accepted work
or accepted-work payout. The broader #4456 fanout remains blocked by launch
health, validator backlog, payout lag, and artifact resolver latency.

## Production Recovery

After Google reauth, `nexus-mainnet-1` was reset and became reachable again.
The production homework floor was corrected to `0.1.16`.

A current-main binary release was built, uploaded, and activated:

- `docs/reports/nexus/20260427-143625-warm-builder-build-508a561a7ac6.json`
- `docs/reports/nexus/20260427-144151-binary-release-upload-508a561a7ac6.json`
- `docs/reports/nexus/20260427-144626-binary-release-activate-508a561a7ac6.json`

The binary activation exposed a lifecycle bug: if the public shell port was
already occupied, `nexus-relay` could start the durable upstream relay and then
fail to bind the public shell, leaving an upstream-only process alive. This was
fixed in `apps/nexus-relay/src/durable.rs` by binding the public listener before
spawning the upstream relay, with a regression test covering the occupied-port
case.

Verification:

```bash
cargo test -p nexus-relay run_server_does_not_start_upstream_when_public_port_is_occupied
```

## Current Public Service Mode

The public Cloudflare tunnel triggered a WebSocket reconnect storm against
`nexus-relay` that made the VM intermittently unreachable over SSH and caused
Cloudflare `530` / `1033` or `502` responses.

To keep public HTTP/operator surfaces reachable while avoiding another VM lockup,
the VM is temporarily running a local recovery proxy:

- `nexus-relay.service`: container image
  `us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:0df70ee18c59`
- `nexus-http-recovery-proxy.service`: listens on `127.0.0.1:8081`
- `nexus-cloudflared.service`: forwards public traffic to
  `http://127.0.0.1:8081`
- `nexus-cloudflared-b.service`: inactive
- recovery proxy behavior: forwards normal HTTP API traffic to `127.0.0.1:8080`
  and rejects WebSocket upgrades during recovery
- `NEXUS_RELAY_MAX_WEBSOCKETS=4`
- `NEXUS_CONTROL_CS336_HOMEWORK_AUTO_DISPATCH_MIN_PYLON_VERSION=0.1.16`

This is a recovery posture, not the final distributed-training posture. It keeps
`/healthz`, `/api/stats`, and operator HTTP reads usable, but public relay
WebSocket work should be treated as temporarily constrained.

## Current Production Snapshot

`/api/stats` after recovery:

- `pylons_online_now`: 69
- `pylon/0.1.16`: 8 online sessions / 5 online Pylons
- `homework_worker_eligible_pylons_online_now`: 4
- eligible versions: `0.1.16` has 3 online Pylons, `0.1.15` has 1 online Pylon
- `training_assigned_contributors`: 78
- `training_accepted_contributors`: 27
- `training_model_progress_contributors`: 27
- `training_public_state.launch_health.overall_status`: `bad`

Launch health blockers:

- 43 unfilled worker slots across active runs
- 418 pending validation windows
- 497 open validator challenges
- 443 queued validator challenges
- 25 payout failures in the last 24h
- 974 skipped payouts in the last 24h
- artifact resolver p95 around 45.6s

The existing #4451 proof run remains incomplete:

- run:
  `run.cs336.a1.issue4451_proof2_20260427051711_23d5b3bd_0001.20260427051714.bac49a0f`
- `worker_target_count`: 2
- `assigned_contributors`: 2
- `accepted_contributors`: 0
- `model_progress_contributors`: 0
- `latest_window_status`: `sealed`
- `pending_validation_window_count`: 1
- `latest_closeout_status`: `null`

## Issue Status

### #4451

Partially satisfied:

- updated Pylon release exists and includes the packaged Psionic training runtime
- production now has multiple updated homework-worker-eligible Pylons
- the existing proof run assigned two workers

Not satisfied:

- no accepted contribution for the proof run yet
- no accepted-work payout yet
- validation backlog remains high

### #4456

Not safe to run or close:

- `201+` participant fanout requires a healthy closeout path
- launch health is `bad`
- current public mode intentionally constrains WebSocket relay traffic
- broad 250-300 Pylon fanout would add load before validation/payout truth is
  healthy

## Next Required Work

1. Replace the recovery proxy posture with a production-safe relay load-shedding
   implementation that preserves HTTP/admin surfaces under WebSocket reconnect
   load.
2. Rebuild and redeploy the current `nexus-relay` binary after the occupied-port
   lifecycle fix is on `main`.
3. Drain or repair validator backlog enough for #4451 sealed windows to close.
4. Prove the #4451 run reaches accepted work and accepted-work payout.
5. Only then run a guarded #4456 cohort fanout toward the `201+` participant
   target.
