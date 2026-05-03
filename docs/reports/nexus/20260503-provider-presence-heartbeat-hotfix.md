# 2026-05-03 Provider Presence Heartbeat Hotfix

## Trigger

The linked Pylon page showed the node as an error even while Codex was ready.
The local Pylon diagnostic was:

```text
nexus payout-target sync failed: nexus payout-target challenge failed:
{"error":"forbidden","reason":"provider_payout_target_requires_live_presence"}
```

Earlier in the same incident window, public Nexus intermittently returned
Cloudflare 1033 / 502 responses.

## Root Cause

`POST /api/provider-presence/heartbeat` could return a successful `online`
response without persisting the heartbeat when the Nexus store lock was
contended. The endpoint used `try_write()` and returned a success-shaped
response on `WouldBlock`.

The Pylon then immediately called
`POST /api/provider-payout-target/challenge`. That endpoint correctly requires a
live persisted provider session, so it rejected the request with
`provider_payout_target_requires_live_presence`.

## Fix

Commit `3442f3b3b04b` changes the heartbeat endpoint to acquire the store write
lock before returning success. A heartbeat response now means the live session is
actually recorded, and the public stats cache invalidation happens after the
record is written.

Targeted test:

```shell
cargo test -p nexus-control provider_payout_target_registration_requires_live_presence_and_updates_stats -- --nocapture
```

## Deploy

Built and pushed:

```text
us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:3442f3b3b04b
```

The deploy script restarted `nexus-relay.service` onto that image. During the
post-deploy warmup, the VM stopped accepting IAP SSH and public health briefly
returned Cloudflare 502. This matched the existing 2026-05-02 recovery pattern,
so `nexus-mainnet-1` was reset. After boot:

- `nexus-relay.service` was active.
- `nexus-cloudflared.service` was active.
- The running container image was `nexus-relay:3442f3b3b04b`.
- `https://nexus.openagents.com/healthz` returned HTTP 200.
- `https://nexus.openagents.com/api/stats` returned HTTP 200.
- The formal deploy verifier completed successfully.

## Pylon Verification

After the deploy and VM reset, the local linked Pylon reported:

```text
runtime_state: online
eligible_product_count: 1
products: psionic.cluster.training.adapter_contributor.cluster_attached
codex_agent: ready
```

`pylon status --json` also reported no runtime `last_error` and no
`degraded_reason_code`.

## Remaining Separate Issue

The provider-presence / payout-target sync path is fixed. Nexus treasury is
still separately degraded:

```text
wallet_runtime_status: error
wallet_last_error: wallet_hydration_zero_balance_after_sync_wallet_then_cached_balance:1636581:1520032
payout_loop_health: degraded
degraded_reason: continuity_alert:confirmations_stalled
pending_confirmation_count: 58
```

Recent relay logs also show Spark wallet send selection failures:

```text
Failed to select leaves: TreeServiceError(InsufficientFunds)
```

That treasury condition predates this hotfix and should be handled as a separate
wallet funding / pending-confirmation recovery task, not as evidence that the
provider heartbeat fix failed.
