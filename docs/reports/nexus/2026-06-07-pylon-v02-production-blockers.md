# Pylon v0.2 production blockers

Date: 2026-06-07

## Current status

Pylon v0.2 is not ready to release publicly yet.

The source tree now uses MoneyDevKit's `@moneydevkit/agent-wallet` as the
default Pylon wallet runtime and keeps native LDK Node as the explicit
lower-level path for direct LDK regression, liquidity, channel telemetry, and
hardening work.

Local MDK evidence is green:

- Pylon wraps MDK under a Pylon-scoped `HOME`.
- Pylon sets a stable Pylon-scoped `MDK_WALLET_PORT`, avoiding the MDK
  default `localhost:3456` daemon collision across multiple Pylon homes.
- Pylon wallet commands return structured JSON through the Pylon CLI for
  status, balance, history, BOLT11 invoice, BOLT12 offer, and lifecycle
  control.
- The two-home MDK wrapper smoke passed.
- The local `cs336-a1-hosted-starter` proof lane completed with simulated
  accepted-work payout behavior.

Production evidence is not green:

- `https://nexus.openagents.com/health` returned HTTP `530` with body
  `error code: 1033`.
- `https://nexus.openagents.com/v1/treasury/status` returned HTTP `530` with
  body `error code: 1033`.
- `https://openagents.com/stats` was reachable, so the outage is scoped to the
  Nexus public edge/origin path rather than the whole public web surface.

## Blocking issues

- `#4450`: public Nexus must recover from Cloudflare `530` / `1033` and the
  deployed public watchdog/recovery-proxy state must be verified.
- `#4548`: the tracked LDK payout reconciliation fix is pushed in source but
  still needs a live Nexus deploy plus admin treasury refresh after Nexus is
  reachable.
- `#4550`: the production recovery/deploy credential path must support
  noninteractive approved-agent or approved-operator recovery so future public
  edge outages can be audited and repaired without waiting on a browser reauth.
  The source implementation now includes
  `scripts/deploy/nexus/34-provision-recovery-identity.sh` for a keyless,
  dry-runnable `nexus-recovery-operator` identity.
- `#4504`: the Pylon v0.2 tracker remains open until public Nexus is healthy,
  `#4548` is verified, and a live real-bitcoin accepted-work payout proof is
  recorded.

## Access blocker from this machine

This machine could not recover or deploy the production Nexus VM:

- the human `gcloud` account required interactive reauthentication;
- the cached `nexus-mainnet` service account lacked `compute.instances.get`;
- Tailnet fallback was unavailable from this shell.

## Required operator sequence

After an operator restores GCP access, use this sequence:

```bash
scripts/deploy/nexus/33-audit-public-watchdog.sh
```

If the audit shows missing or stale public-watchdog or recovery-proxy units,
refresh them:

```bash
scripts/deploy/nexus/16-install-public-watchdog.sh
scripts/deploy/nexus/33-audit-public-watchdog.sh
```

If the audit cannot reach the VM, reset `nexus-mainnet-1` from GCP, then rerun
the audit immediately.

Once public Nexus returns HTTP 200 on health and treasury status is reachable,
deploy current `main` and refresh treasury reconciliation:

```bash
scripts/deploy/nexus/03-refresh-config-and-restart.sh
curl -sS https://nexus.openagents.com/v1/treasury/status
```

Then run the accepted-work proof path against live production and record the
commit, deployed image, run id, contribution id, provider node id, payout id,
and final payout state.

## Release rule

Do not create `pylon-v0.2.0` until all of the following are true:

- public Nexus health is reachable;
- `#4548` is deployed and `/v1/treasury/status` no longer reports
  `continuity_alert:confirmations_stalled`;
- a fresh production accepted-work proof moves real bitcoin to a provider
  wallet;
- a public-safe production proof receipt is committed under
  `docs/reports/nexus/`;
- GitHub issues `#4450`, `#4548`, and `#4504` have closeout comments naming the
  exact evidence.
