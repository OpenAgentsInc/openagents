# CND-045 GCP Benchmark Cloud Substrate

> **Historical bootstrap note (#8591).** Kept for archaeology and ops memory.
> Active Cloud implementation is in the public monorepo (`crates/*`,
> `docs/cloud/`). Deprecated authority names: **Vortex** → Worker/Khala Sync;
> **Treasury product** → Worker credits + MDK/Nexus payout bridge only;
> **Nexus-as-registry** → Worker/Khala Sync (CLI may still say `nexus`).
> Do not treat this note as current product-authority ownership.

Status: dev provisioning runbook
Last updated: 2026-06-01

This runbook creates the first Google Cloud substrate for Benchmark Cloud. It is
for development and smoke tests only. Production rollout needs separate project,
quota, billing, alerting, retention, and approval decisions.

## Resources

The bootstrap script creates:

- Artifact Registry repository: `oa-benchmark-runners`
- Cloud Storage buckets:
  - `${PROJECT_ID}-oa-benchmark-specs-${ENV}`
  - `${PROJECT_ID}-oa-benchmark-datasets-${ENV}`
  - `${PROJECT_ID}-oa-benchmark-artifacts-${ENV}`
  - `${PROJECT_ID}-oa-benchmark-proofs-${ENV}`
- Pub/Sub topics:
  - `benchmark-task-events-${ENV}`
  - `benchmark-run-events-${ENV}`
- Service accounts:
  - `bench-controller-${ENV}`
  - `bench-runner-${ENV}`
- Placeholder Secret Manager secret:
  - `oa-benchmark-provider-placeholder-${ENV}`

## Bootstrap

Dry-run first:

```bash
scripts/gcp-benchmark-bootstrap.sh \
  --project openagents-bench-dev \
  --region us-central1 \
  --env dev
```

Apply:

```bash
scripts/gcp-benchmark-bootstrap.sh \
  --project openagents-bench-dev \
  --region us-central1 \
  --env dev \
  --apply
```

## IAM Boundary

`bench-controller` may create Batch jobs and Cloud Run job executions. It can
write task specs and impersonate the runner service account for execution.

`bench-runner` may:

- read task specs;
- read dataset snapshots;
- write artifacts and proof bundles;
- publish task events;
- access only the benchmark placeholder secret.

`bench-runner` must not receive broad `roles/editor`, broad Secret Manager
access, production Convex credentials, wallet material, or customer data.

## Smoke

After bootstrap, run:

```bash
scripts/gcp-benchmark-smoke.sh \
  --project openagents-bench-dev \
  --env dev
```

If the current principal can impersonate the runner service account, validate
runner-scoped IAM directly:

```bash
scripts/gcp-benchmark-smoke.sh \
  --project openagents-bench-dev \
  --env dev \
  --impersonate-runner
```

The smoke uploads the fake pass task spec, reads it back, writes one artifact
object, and publishes one task event.

## Build Runner Image

The first runner image build target is:

```bash
PROJECT_ID=openagents-bench-dev
REGION=us-central1
IMAGE="$REGION-docker.pkg.dev/$PROJECT_ID/oa-benchmark-runners/py-bench-runner:dev"

gcloud builds submit \
  --project "$PROJECT_ID" \
  --tag "$IMAGE" \
  runners/py-bench-runner
```

The Dockerfile is added by the Cloud Batch backend issue. Until then, the local
runner skeleton can still validate the task/result/artifact contract.

## Cleanup

Dry-run:

```bash
scripts/gcp-benchmark-cleanup.sh \
  --project openagents-bench-dev \
  --region us-central1 \
  --env dev
```

Apply:

```bash
scripts/gcp-benchmark-cleanup.sh \
  --project openagents-bench-dev \
  --region us-central1 \
  --env dev \
  --apply
```

Cleanup deletes dev buckets and their objects. Do not run cleanup against a
shared or production environment without preserving required artifacts and proof
bundles.

## Cost Guardrails

- Keep this in a dedicated dev project or environment.
- Use Cloud Batch task timeouts and per-run budgets before broad sweeps.
- Keep Cloud Storage lifecycle policies short for dev artifacts unless a proof
  bundle is intentionally retained.
- Do not enable GKE in this lane until Batch/Cloud Run compatibility is proven
  insufficient.
- Keep public benchmark claims disabled until Worker/Khala Sync proof projection enforces
  dataset/version, task subset, harness version, artifacts, and redaction state.
