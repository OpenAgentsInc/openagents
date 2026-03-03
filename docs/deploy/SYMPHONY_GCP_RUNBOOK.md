# Symphony GCP Runbook (Existing `oa-bitcoind` Backend)

Date context: March 2, 2026.

This runbook deploys Maestro Symphony on GCP and connects it to the already-live Bitcoin Core node `oa-bitcoind`.

Issue tracking:
- Baseline deploy: https://github.com/OpenAgentsInc/openagents/issues/2738
- Hardening: https://github.com/OpenAgentsInc/openagents/issues/2739
- Ops/backup/restore: https://github.com/OpenAgentsInc/openagents/issues/2740
- Maestro skill integration: https://github.com/OpenAgentsInc/openagents/issues/2741

## 1) Baseline assumptions

- Project: `openagentsgemini`
- Region/zone: `us-central1` / `us-central1-a`
- VPC/subnet: `oa-lightning` / `oa-lightning-us-central1`
- Existing bitcoind backend:
  - VM: `oa-bitcoind`
  - Internal IP: `10.42.0.2`
  - RPC: `10.42.0.2:8332`
  - P2P: `10.42.0.2:8333`
  - ZMQ: `10.42.0.2:28332`, `10.42.0.2:28333`
- Existing secret: `oa-bitcoind-rpc-creds` (`rpcuser=`, `rpcpassword=`)

## 2) Scripted deployment flow

All scripts are in `scripts/deploy/symphony/`.

1. Build and push Symphony image.

```bash
scripts/deploy/symphony/01-build-and-push-image.sh
```

2. Provision baseline VM + disk + SA + backend firewall.

```bash
scripts/deploy/symphony/02-provision-baseline.sh
```

3. Configure and start Symphony service on VM.

```bash
scripts/deploy/symphony/03-configure-and-start.sh
```

4. Apply network/API hardening.

```bash
scripts/deploy/symphony/04-harden-network.sh
```

5. Bootstrap ops controls (health probes, metrics, policies, snapshots).

```bash
scripts/deploy/symphony/05-ops-bootstrap.sh
```

6. Verify deploy gates and emit receipt.

```bash
scripts/deploy/symphony/06-verify-gates.sh
```

7. Run restore drill and emit report.

```bash
scripts/deploy/symphony/07-restore-drill.sh
```

## 3) RPC credential rotation SOP

Use when rotating bitcoind RPC credentials.

Preconditions:
- You already updated bitcoind to use the new RPC credentials.
- You validated RPC auth works on `oa-bitcoind`.

Then update secret + redeploy Symphony config:

```bash
NEW_RPC_USER="<new-rpc-user>" \
NEW_RPC_PASSWORD="<new-rpc-password>" \
BITCOIND_UPDATED=1 \
  scripts/deploy/symphony/rotate-rpc-creds.sh
```

## 4) Deploy artifacts

Scripts write artifacts under:
- Deploy receipts: `docs/reports/symphony/*-deploy-receipt.json`
- Restore drills: `docs/reports/symphony/*-restore-drill.md`

## 5) Access and safety notes

- Symphony VM is created without an external IP by default.
- API ingress is controlled by `oa-allow-symphony-api` firewall rule.
- `/dump` should remain inaccessible from public internet.
- Keep `oa-bitcoind-rpc-creds` in Secret Manager only; do not commit values.

## 6) Maestro skill usage after deploy

Once the gateway is live, use the local skill:
- `skills/maestro/SKILL.md`

It provides endpoint query recipes for `/tip`, addresses, and runes with operational safety checks.
