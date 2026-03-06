# Nexus GCP Runbook

Date context: March 6, 2026.

Issue tracking:
- Deployment config: https://github.com/OpenAgentsInc/openagents/issues/3047
- Staging validation: https://github.com/OpenAgentsInc/openagents/issues/3048
- Production cutover: https://github.com/OpenAgentsInc/openagents/issues/3049

## 1) Hosting decision

The first real Nexus hosting path is:

- `Compute Engine VM`
- `persistent SSD`
- `SQLite-backed durable relay store`
- one public Nexus Rust service in the container itself

Why this path:

- the imported durable relay is SQLite-first today
- the old Cloud Run-style stateless model is the wrong fit for a durable relay
- this keeps Nexus as one Rust service while giving it a stateful disk and restart persistence

This runbook does **not** claim that public DNS/TLS cutover is complete. That is handled in later staging/cutover work. The purpose here is to make the deployed runtime itself stateful and durable.

## 2) Baseline assumptions

- project: `openagentsgemini`
- region / zone: `us-central1` / `us-central1-a`
- VPC / subnet: `oa-lightning` / `oa-lightning-us-central1`
- VM: `nexus-mainnet-1`
- data disk: `nexus-relay-data-mainnet`
- default public host assumption: `nexus.openagents.com`
- default websocket assumption: `wss://nexus.openagents.com/`

The scripts are parameterized through env vars in `scripts/deploy/nexus/common.sh`.

## 3) Scripted deployment flow

All scripts are in `scripts/deploy/nexus/`.

1. Build and push the Nexus image.

```bash
scripts/deploy/nexus/01-build-and-push-image.sh
```

2. Provision the baseline VM, service account, persistent disk, and IAP SSH access.

```bash
scripts/deploy/nexus/02-provision-baseline.sh
```

3. Configure the VM, mount the disk, and start the Nexus service.

```bash
scripts/deploy/nexus/03-configure-and-start.sh
```

4. Verify health and emit a deploy receipt.

```bash
scripts/deploy/nexus/04-verify-gates.sh
```

## 4) Runtime model

The deployed service is the `nexus-relay` container built from `apps/nexus-relay/Dockerfile`.

It runs:

- durable relay storage under `${NEXUS_DATA_DIR}` (`/var/lib/nexus-relay` by default)
- the in-process authority/API routes merged into the same service
- receipt persistence through `NEXUS_CONTROL_RECEIPT_LOG_PATH`

The baseline bind is `0.0.0.0:8080` on the VM. Public DNS/TLS exposure is a later step; the app/runtime no longer depends on ephemeral in-memory relay storage.

## 5) Deploy artifacts

Verification receipts land under:

- `docs/reports/nexus/*-deploy-receipt.json`

## 6) Operational notes

- The baseline VM is private-by-default and intended to be reached through `gcloud compute ssh --tunnel-through-iap` until staging/public cutover is ready.
- The persistent disk is mounted at `/var/lib/nexus-relay` and should survive service restarts and VM reboots.
- The deploy path assumes the VM service account can read from Artifact Registry.
- The durable relay data path and Nexus control receipt log path both live on the persistent disk.

## 7) Public cutover

The public cutover path uses a Cloudflare tunnel from the production VM.

Why this path:

- it keeps the durable Nexus runtime on a private stateful VM
- it avoids pushing the durable relay back behind a stateless Cloud Run shape
- it lets `nexus.openagents.com` move without needing a separate external load balancer first

Script:

```bash
scripts/deploy/nexus/05-cutover-public-host.sh
```

What it does:

- creates or reuses the named Cloudflare tunnel
- routes `nexus.openagents.com` to that tunnel
- installs a `nexus-cloudflared.service` unit on the VM
- forwards public HTTPS / websocket traffic to `http://127.0.0.1:8080`

Required local prerequisites:

- `cloudflared` installed locally
- local Cloudflare auth already present (`cloudflared login` completed previously)
- access to the `openagents.com` zone in Cloudflare

The VM remains private-by-default. Public ingress is handled through the tunnel rather than by assigning a public VM IP.

## 8) Retire the old Cloud Run surface

Once the public hostname is confirmed on the durable VM path, remove the old stateless Cloud Run Nexus surface:

```bash
scripts/deploy/nexus/06-retire-cloud-run-surface.sh
```

What it removes:

- the old `nexus.openagents.com` Cloud Run domain mapping
- `openagents-nexus-relay`
- `openagents-nexus-control`

This keeps the live infra aligned with the durable single-service Nexus runtime instead of leaving a second stale Nexus path behind.

## 9) What this runbook intentionally does not cover yet

- backup / restore drills and retention policy
- abuse controls and operator hardening

Those are tracked in later Nexus migration issues rather than hidden behind this baseline deploy story.
