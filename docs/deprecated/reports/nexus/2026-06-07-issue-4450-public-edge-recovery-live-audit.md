# Issue 4450 Public Edge Recovery Live Audit

Date: 2026-06-07
Repository: `OpenAgentsInc/openagents`
Issue: `#4450`
Integrated commit inspected: `4ee1ac58bd27`

## Summary

Public Nexus was returning Cloudflare `530` / `1033` on
`https://nexus.openagents.com/healthz` and
`https://nexus.openagents.com/v1/treasury/status`.

The VM was still `RUNNING`, but the read-only watchdog audit could not connect
over IAP SSH because port `22` was unreachable. Per the Nexus emergency
runbook, `nexus-mainnet-1` was reset from GCP. After reset, public Nexus
recovered.

## Commands Run

```bash
curl -sS -o /tmp/nexus-health.out -w 'healthz_http=%{http_code}\n' \
  https://nexus.openagents.com/healthz

gcloud compute instances describe nexus-mainnet-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --format='value(status,networkInterfaces[0].networkIP)'

scripts/deploy/nexus/33-audit-public-watchdog.sh

gcloud compute instances reset nexus-mainnet-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --quiet

curl -sS -o /tmp/nexus-healthz-after-reset.out -w '%{http_code}' \
  https://nexus.openagents.com/healthz

scripts/deploy/nexus/33-audit-public-watchdog.sh
```

## Recovery Evidence

- Pre-recovery public health: HTTP `530`, body `error code: 1033`.
- Pre-recovery treasury status: HTTP `530`, body `error code: 1033`.
- VM state before reset: `RUNNING`, private IP `10.42.0.6`.
- Pre-reset watchdog audit: failed before inspection because IAP could not
  connect to VM port `22`.
- Recovery action: GCE reset of `nexus-mainnet-1`.
- Post-reset public `/healthz`: HTTP `200`, Nexus JSON health response.
- Post-reset public `/api/stats`: reachable and reporting Nexus data again.

## Watchdog Audit Evidence

After reset, `scripts/deploy/nexus/33-audit-public-watchdog.sh` reported:

- `nexus-public-watchdog.timer`: enabled and active.
- `nexus-public-watchdog.service`: installed.
- `nexus-http-recovery-proxy.service`: enabled and active.
- `nexus-cloudflared.service`: enabled and active.
- `nexus-relay.service`: enabled and active.
- Installed watchdog files under `/etc/nexus-relay/`,
  `/usr/local/bin/`, and `/etc/systemd/system/` were present.
- `TUNNEL_ORIGIN_URL=http://127.0.0.1:8080`.
- VM-local origin health returned HTTP `200`.
- VM-local recovery proxy health returned HTTP `200`.
- Latest watchdog receipt:
  - `status=healthy`
  - `reason=public_edge_ok`
  - `action=none`
  - `local_health_code=200`
  - `public_edge_code=200`
  - `consecutive_edge_failures=0`
- Edge failure count: `0`.

## Result

The #4450 public Nexus edge regression is recovered and the deployed
watchdog/recovery-proxy state is verified after recovery.

This does not resolve the separate Treasury continuity issue tracked by
`#4548`. After the public edge recovered, `/v1/treasury/status` remained
reachable but still reported:

- `payout_loop_health=degraded`
- `degraded_reason=continuity_alert:confirmations_stalled`
- `pending_confirmation_count=1`
- `tracked_payment_backlog_count=1`

Next production work should proceed to `#4548`.
