# CND-035 GCP Workroom Smoke

Status: implemented scaffold

This smoke proves the first no-wallet workroom path on a managed GCE node:

```text
node -> oa-workroomd sidecar -> bounded command -> artifact -> closeout -> lifecycle receipts
```

The smoke is intentionally small. It writes a deterministic summary artifact
with `workroom_wallet_authority=false`, uploads it through `oa-workroomd`,
submits artifact closeout, and closes the lifecycle.

## Local Verification

```bash
scripts/gcp-node-workroom-smoke.sh --local
```

This runs the same lifecycle locally through `cargo run -p oa-workroomd`.

## GCE Dry Run

```bash
scripts/gcp-node-workroom-smoke.sh \
  --project "$PROJECT_ID" \
  --zone us-central1-a \
  --region us-central1 \
  --node-name oa-node-dev-01 \
  --env dev \
  --image-tag "$IMAGE_TAG"
```

Dry-run mode prints the exact IAP SSH command and the remote script body.

## GCE Apply

```bash
scripts/gcp-node-workroom-smoke.sh \
  --project "$PROJECT_ID" \
  --zone us-central1-a \
  --region us-central1 \
  --node-name oa-node-dev-01 \
  --env dev \
  --image-tag "$IMAGE_TAG" \
  --apply
```

The remote script:

1. creates `/var/lib/openagents/workrooms/smoke-<env>`;
2. writes a bounded summary artifact;
3. runs `oa-workroomd metadata init`;
4. requires the `summary` artifact;
5. runs lifecycle `create` and `start`;
6. uploads the summary artifact;
7. submits closeout;
8. emits lifecycle `closeout`, `archive`, and `destroy` receipts;
9. prints recent `openagents-oa-node` logs.

## Boundary

- No wallet authority is attached.
- No public ingress is enabled.
- No raw provider secret is mounted.
- The bounded command is deterministic and limited to writing one artifact.
- The smoke uses existing artifact and lifecycle receipts as evidence.
