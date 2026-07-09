# CND-033 GCP Node Bootstrap

> **Historical bootstrap note (#8591).** Kept for archaeology and ops memory.
> Active Cloud implementation is in the public monorepo (`crates/*`,
> `docs/cloud/`). Deprecated authority names: **Vortex** → Worker/Khala Sync;
> **Treasury product** → Worker credits + MDK/Nexus payout bridge only;
> **Nexus-as-registry** → Worker/Khala Sync (CLI may still say `nexus`).
> Do not treat this note as current product-authority ownership.

Status: implemented scaffold

This runbook defines the first Google Cloud path for a managed OpenAgents Cloud
node test environment. It is intentionally separate from Benchmark Cloud Batch
because this lane exists to boot one `oa-node` VM and run workrooms through
`oa-workroomd`.

## Scripts

```text
scripts/gcp-node-bootstrap.sh
scripts/gcp-node-cleanup.sh
```

Both scripts default to dry-run. They print exact `gcloud` commands unless
`--apply` is provided.

## Bootstrap

```bash
scripts/gcp-node-bootstrap.sh \
  --project "$PROJECT_ID" \
  --region us-central1 \
  --zone us-central1-a \
  --env dev
```

Apply:

```bash
scripts/gcp-node-bootstrap.sh \
  --project "$PROJECT_ID" \
  --region us-central1 \
  --zone us-central1-a \
  --env dev \
  --apply
```

The script creates:

- required APIs:
  - Compute Engine;
  - Artifact Registry;
  - Cloud Storage;
  - IAM;
  - Cloud Logging;
  - Cloud Monitoring;
  - Secret Manager;
  - OS Login;
- Artifact Registry Docker repo `oa-cloud`;
- private VPC `oa-cloud-<env>`;
- regional subnet `oa-cloud-<env>-<region>`;
- IAP-only SSH firewall rule;
- service account `oa-node-<env>`;
- state, artifact, receipt, and log buckets;
- placeholder Secret Manager secret for scoped secret-access smoke tests;
- IAM for artifact reads, storage object admin on the test buckets, log
  writing, metric writing, and placeholder secret access.

## Cleanup

```bash
scripts/gcp-node-cleanup.sh \
  --project "$PROJECT_ID" \
  --region us-central1 \
  --env dev
```

Apply:

```bash
scripts/gcp-node-cleanup.sh \
  --project "$PROJECT_ID" \
  --region us-central1 \
  --env dev \
  --apply
```

Cleanup removes only the named test-environment resources. It does not disable
project APIs.

## Boundary

The bootstrap path creates infrastructure only. It does not:

- bake provider secrets into images;
- grant wallet authority to workrooms;
- expose workrooms publicly;
- configure private fleet topology beyond a single test network;
- claim production readiness.

The next issue, CND-034, installs `oa-node` onto a single test GCE VM using
this substrate.
