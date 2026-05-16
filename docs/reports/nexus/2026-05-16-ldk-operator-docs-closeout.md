# LDK Operator Documentation Closeout

Date: 2026-05-16

Issue: OpenAgentsInc/openagents#4494

This closeout records the operator documentation state for the Nexus/Pylon
v0.2 LDK transition. It is the index future operators should use before
touching funding, payout, channel, backup, restore, Spark final-drain, or
public recovery-proxy paths.

## Phase-To-Runbook Map

| Phase | Shipped documentation |
| --- | --- |
| LDK-01 Spark touchpoint freeze | `docs/reports/nexus/2026-05-16-spark-migration-final-drain-report.md`, `docs/nexus-treasury.md` |
| LDK-02 provider boundary | `docs/nexus-treasury.md`, `docs/2026-05-15-ldk-nexus-treasury-transition-audit.md` |
| LDK-03 operation and receipt store | `docs/nexus-treasury.md` |
| LDK-04 local proof harness | `docs/nexus-treasury.md`, `docs/deploy/NEXUS_LDK_GCP_RUNBOOK.md` |
| LDK-05 LDK Server client boundary | `docs/nexus-treasury.md`, `docs/deploy/NEXUS_LDK_GCP_RUNBOOK.md` |
| LDK-06 hosted topology | `docs/deploy/NEXUS_LDK_GCP_RUNBOOK.md`, `docs/deploy/NEXUS_GCP_RUNBOOK.md` |
| LDK-07 funding invoice cutover | `docs/nexus-treasury.md`, `docs/deploy/NEXUS_TREASURY_FUNDING_INVOICE_RUNBOOK.md` |
| LDK-08 Pylon payout target registration | `docs/deploy/PYLON_NEXUS_EARNING_RELEASE_RUNBOOK.md`, `docs/deploy/NEXUS_LDK_GCP_RUNBOOK.md` |
| LDK-09 payout dispatch | `docs/nexus-treasury.md`, `docs/deploy/PYLON_NEXUS_EARNING_RELEASE_RUNBOOK.md` |
| LDK-10 admin operations | `docs/nexus-treasury.md`, `docs/deploy/NEXUS_LDK_GCP_RUNBOOK.md` |
| LDK-11 degraded states and alerts | `docs/nexus-treasury.md`, `docs/deploy/NEXUS_GCP_RUNBOOK.md` |
| LDK-12 read-only projections | `docs/nexus-treasury.md` |
| LDK-15 Spark decommission | `docs/reports/nexus/2026-05-16-spark-migration-final-drain-report.md`, `docs/deploy/NEXUS_GCP_RUNBOOK.md` |
| LDK-16 final Spark report | `docs/reports/nexus/2026-05-16-spark-migration-final-drain-report.md` |
| LDK-19 operator docs | this closeout |

The roadmap source remains
`docs/2026-05-15-ldk-nexus-treasury-transition-audit.md`.

## Operator Command Index

Public health, stats, and treasury status:

```bash
curl -fsS https://nexus.openagents.com/healthz | jq .
curl -fsS https://nexus.openagents.com/api/stats | jq '{
  pylons_online_now,
  sellable_pylons_online_now,
  nexus_wallet_runtime_status,
  nexus_wallet_balance_sats,
  nexus_payout_loop_health
}'
curl -fsS https://nexus.openagents.com/v1/treasury/status | jq '{
  treasury_enabled,
  registered_payout_identities,
  eligible_online_payout_targets,
  wallet_runtime_status,
  wallet_balance_sats,
  payout_loop_health,
  degraded_reason
}'
```

VM and recovery-proxy checks:

```bash
gcloud compute ssh nexus-mainnet-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command='systemctl is-active nexus-relay nexus-cloudflared nexus-http-recovery-proxy'
```

LDK topology, install, smoke, backup, and restore:

```bash
NEXUS_LDK_TOPOLOGY_DRY_RUN=true \
scripts/deploy/nexus/22-provision-ldk-topology.sh

NEXUS_LDK_INSTALL_DRY_RUN=true \
scripts/deploy/nexus/23-install-ldk-server-host.sh

scripts/deploy/nexus/24-smoke-ldk-server-readonly.sh

NEXUS_LDK_BACKUP_DRY_RUN=true \
scripts/deploy/nexus/25-backup-ldk-server-state.sh

NEXUS_LDK_RESTORE_DRY_RUN=true \
NEXUS_LDK_RESTORE_SNAPSHOT=<snapshot-name> \
scripts/deploy/nexus/26-restore-ldk-server-drill.sh
```

Funding invoice smoke:

```bash
curl -fsS -X POST "https://nexus.openagents.com/v1/treasury/funding-target" \
  -H "Authorization: Bearer ${NEXUS_ADMIN_BEARER_TOKEN}" \
  -H "Content-Type: application/json" \
  --data '{
    "amount_sats": 50000,
    "description": "OpenAgents Nexus treasury funding",
    "expiry_seconds": 3600
  }' |
  jq '{bolt11_invoice, provider_payment_id_hash, phase_timings}'
```

The returned BOLT11 invoice is a live payment request, not proof of payment.
Do not check it into docs, issue comments, commits, or logs after use.

Admin operations:

```bash
curl -fsS -X POST "https://nexus.openagents.com/v1/admin/treasury/operations" \
  -H "Authorization: Bearer ${NEXUS_ADMIN_BEARER_TOKEN}" \
  -H "Content-Type: application/json" \
  --data '{
    "operation": "treasury.status",
    "idempotency_key": "status-operator-check-001",
    "params": {}
  }' | jq .
```

Write-side operations such as `treasury.openChannel`,
`treasury.closeChannel`, `treasury.spliceIn`, `treasury.spliceOut`,
`treasury.payInvoice`, `treasury.payOffer`, `treasury.connectPeer`, and
`treasury.reconcilePayments` require the same admin bearer token plus an
idempotency key. Store only redacted receipts or hashes in proof artifacts.

## Expected Output Fields

`/healthz` should include:

- `ok`
- `service`
- `relay_backend`
- `authority_mode`
- `recovery_proxy` when public traffic is passing through the recovery proxy

`/api/stats` should include:

- `pylons_online_now`
- `sellable_pylons_online_now`
- `nexus_wallet_runtime_status`
- `nexus_wallet_balance_sats`
- `nexus_payout_loop_health`

`/v1/treasury/status` should include:

- `treasury_enabled`
- `registered_payout_identities`
- `eligible_online_payout_targets`
- `wallet_runtime_status`
- `wallet_balance_sats`
- `payout_loop_health`
- `degraded_reason` when degraded

`/v1/treasury/funding-target` should include:

- `bolt11_invoice` for positive-amount LDK funding requests
- `provider_payment_id_hash`
- `phase_timings`

`/v1/treasury/projections` should include read-only arrays for peers,
channels, liquidity bands, payment attempts, terminal states, payout receipts,
Pylon earning events, and degraded states. Raw seeds, API keys, payment
targets, invoices, and private channel state must never appear.

## Failure States

Treat these as blocking or degraded states until explained:

- Cloudflare `530` or `1033` from the public Nexus hostname.
- `nexus-cloudflared.service` restart loops.
- VM metadata lookup failures or `connect: network is unreachable` to the GCP
  metadata IP.
- `nexus_payout_loop_health` or `payout_loop_health` marked `degraded`.
- `degraded_reason` containing `continuity_alert:dispatch_stalled`.
- LDK Server gRPC reachable from public source ranges.
- LDK host has an unexpected external IP.
- `ldk-server-cli get-node-info` or `get-balances` fails.
- `keys_seed`, `api_key`, TLS files, or SQLite node state missing.
- Backup archive or restore drill cannot verify the LDK data read-only.
- Funding-target response lacks `bolt11_invoice` for a positive-amount LDK
  request.
- Logs, issue comments, or runbook output print raw bearer tokens, seeds,
  API keys, TLS private keys, or invoices after use.

## Rollback Conditions

There is no Spark/LDK dual-rail product mode. Spark remains only for historical
read and explicitly gated final-drain/recovery work.

Roll back or pause a phase when:

- LDK topology exposes custody or gRPC material publicly.
- LDK node state cannot be backed up and restored.
- Public Nexus is unreachable and the recovery proxy cannot pass `/healthz`.
- Admin write operations lack idempotency keys or durable operation receipts.
- Payout dispatch cannot identify terminal payment states or reconcile missed
  events.
- Any script or operator flow requires printing raw custody material.

Rollback means restoring or redeploying the previous known-good Nexus image,
pausing payout dispatch, keeping Spark final-drain disabled unless explicitly
approved, and restoring from LDK backup only through the documented restore
drill.

## Proof Artifacts

For each close comment, include:

- commit SHA
- relevant runbook path
- public smoke status
- script syntax checks
- deployment id or image tag when a deployment changed
- redacted Nexus operation ids or receipt hashes when a write operation was
  tested
- known residual degraded state, if any

Do not include raw API keys, bearer tokens, seeds, TLS private keys, or live
invoices after use.

## Closeout Verification

This closeout was prepared after the LDK-16 Spark final migration report and
the public Nexus recovery reset. Public Nexus was reachable through the
recovery proxy, `/api/stats` and `/v1/treasury/status` returned operator-safe
state, and the remaining known degraded state was payout-loop continuity:
`continuity_alert:dispatch_stalled`.
