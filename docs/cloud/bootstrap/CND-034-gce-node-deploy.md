# CND-034 GCE Node Deploy

Status: implemented scaffold

This runbook deploys one managed test GCE VM that starts `oa-node` under
systemd, reports status, writes redacted journald logs, and can be destroyed
cleanly.

## Prerequisites

1. Bootstrap the GCP node substrate:

   ```bash
   scripts/gcp-node-bootstrap.sh --project "$PROJECT_ID" --env dev --apply
   ```

2. Build and push the `oa-node` image:

   ```bash
   scripts/build-cloud-images.sh \
     --registry us-central1-docker.pkg.dev/$PROJECT_ID/oa-cloud \
     --tag "$IMAGE_TAG" \
     --push
   ```

## Deploy

Dry-run:

```bash
scripts/gcp-node-deploy-vm.sh \
  --project "$PROJECT_ID" \
  --zone us-central1-a \
  --region us-central1 \
  --env dev \
  --node-name oa-node-dev-01 \
  --image-tag "$IMAGE_TAG"
```

Apply:

```bash
scripts/gcp-node-deploy-vm.sh \
  --project "$PROJECT_ID" \
  --zone us-central1-a \
  --region us-central1 \
  --env dev \
  --node-name oa-node-dev-01 \
  --image-tag "$IMAGE_TAG" \
  --apply
```

The VM uses:

- Container-Optimized OS;
- no external IP;
- IAP-only SSH from the bootstrap firewall rule;
- service account `oa-node-<env>`;
- Artifact Registry image
  `REGION-docker.pkg.dev/PROJECT_ID/oa-cloud/oa-node:IMAGE_TAG`;
- systemd unit `openagents-oa-node.service`.

## Status

```bash
scripts/gcp-node-status.sh \
  --project "$PROJECT_ID" \
  --zone us-central1-a \
  --node-name oa-node-dev-01
```

This prints the VM metadata, then runs:

```text
systemctl status openagents-oa-node
journalctl -u openagents-oa-node -n 80
```

The service logs only `oa-node status --json` scaffold output and bootstrap
events. Secrets must not be logged.

## Destroy

Dry-run:

```bash
scripts/gcp-node-destroy-vm.sh --project "$PROJECT_ID"
```

Apply:

```bash
scripts/gcp-node-destroy-vm.sh --project "$PROJECT_ID" --apply
```

This deletes the test VM only. Use `scripts/gcp-node-cleanup.sh` to remove the
shared node substrate.

## Boundary

This is a single-node smoke substrate, not production fleet placement. It does
not grant wallet authority, public ingress, private fleet topology, or broad
cloud credentials to workrooms.
