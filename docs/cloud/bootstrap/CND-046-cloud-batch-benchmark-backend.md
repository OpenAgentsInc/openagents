# CND-046 Cloud Batch Benchmark Backend

Status: one-task backend scaffold
Last updated: 2026-06-01

This runbook submits one normalized Benchmark Cloud task attempt to Google Cloud
Batch. It uses the Python runner image from `runners/py-bench-runner` and passes
only a bounded task spec GCS URI plus an artifact GCS prefix into the container.

## Build Runner Image

```bash
PROJECT_ID=openagents-bench-dev
REGION=us-central1
IMAGE="$REGION-docker.pkg.dev/$PROJECT_ID/oa-benchmark-runners/py-bench-runner:dev"

gcloud builds submit \
  --project "$PROJECT_ID" \
  --tag "$IMAGE" \
  runners/py-bench-runner
```

The runner image is based on the Google Cloud CLI slim image so it can read the
task spec from Cloud Storage and upload artifacts back to Cloud Storage using
the runner service account.

## Dry-Run Job Config

```bash
scripts/gcp-benchmark-submit-batch.sh \
  --project openagents-bench-dev \
  --region us-central1 \
  --env dev \
  --image "$IMAGE" \
  --run-id run_fake_batch_smoke \
  --task-run-id taskrun_fake_pass \
  --task-spec runners/py-bench-runner/fixtures/tasks/fake-pass.json
```

The dry-run prints the upload command and the generated Batch job JSON. Review
the JSON before applying.

## Submit

```bash
scripts/gcp-benchmark-submit-batch.sh \
  --project openagents-bench-dev \
  --region us-central1 \
  --env dev \
  --image "$IMAGE" \
  --run-id run_fake_batch_smoke \
  --task-run-id taskrun_fake_pass \
  --task-spec runners/py-bench-runner/fixtures/tasks/fake-pass.json \
  --apply
```

This uploads the local task spec to:

```text
gs://openagents-bench-dev-oa-benchmark-specs-dev/runs/run_fake_batch_smoke/tasks/taskrun_fake_pass.json
```

Artifacts are written under:

```text
gs://openagents-bench-dev-oa-benchmark-artifacts-dev/runs/run_fake_batch_smoke/tasks/taskrun_fake_pass/
```

## State Mapping

The first backend maps Batch and runner states this way:

| Batch / runner state | Normalized task state |
| --- | --- |
| job submitted | queued |
| VM allocated / container starting | provisioning |
| `task_started` / `task_loaded` event | running_agent |
| verifier log emitted | running_verifier |
| `result.json.status == passed` | passed |
| `result.json.status == failed` | failed |
| `result.json.status == timeout` | timeout |
| `result.json.status == error` | error |
| Batch cancellation | canceled |

Vortex remains responsible for durable state and receipts. Cloud Batch is only
the execution substrate.

## Guardrails

- The Batch job command is generated from typed fields; Vortex does not pass
  arbitrary shell text.
- The container receives GCS URIs and bounded metadata only.
- The runner writes `result.json`, `events.jsonl`, `metadata.json`,
  `artifact_manifest.json`, and `proof_bundle.json` after it starts.
- The Batch service account is `bench-runner-${ENV}` and should not have
  production-secret access.
- Logs should not include raw credentials or broad environment dumps.
