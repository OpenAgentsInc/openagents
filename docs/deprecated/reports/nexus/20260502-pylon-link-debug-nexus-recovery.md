# 2026-05-02 Pylon Link Debug Nexus Recovery

## Trigger

While debugging a linked Pylon that appeared on `openagents.com/pylon` with a
bare `Error` runtime state, public Nexus health checks returned Cloudflare
`530` for both `https://nexus.openagents.com/healthz` and
`https://nexus.openagents.com/api/stats`.

## Recovery

- Re-authenticated Google Cloud access as `chris@openagents.com`.
- Confirmed `nexus-mainnet-1` was `RUNNING` in `openagentsgemini/us-central1-a`
  at `10.42.0.6`.
- IAP SSH did not reach the backend, matching the existing emergency runbook
  condition for a `530` outage with a running VM.
- Reset `nexus-mainnet-1` with `gcloud compute instances reset`.
- Verified public `/healthz` and `/api/stats` recovered after the reset.
- Verified VM services were active after boot:
  - `nexus-relay`
  - `nexus-cloudflared`
  - `nexus-http-recovery-proxy`

## Follow-up

The linked-Pylon `Error` was not caused only by the public Nexus outage. Local
Pylon status also showed a stale admin/runtime split: the live admin status
reported a different Pylon public key than the identity used by
`pylon account link`, and its runtime error pointed at a localhost Nexus
provider heartbeat endpoint. The account-link fix now sends the actual runtime
diagnostic fields to the website and ignores live admin status from a different
identity when building the signed link payload.
