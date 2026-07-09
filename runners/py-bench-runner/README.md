# OpenAgents Benchmark Runner

Status: CND-044 local skeleton

This runner is the local contract proof for Benchmark Cloud. It executes one
normalized benchmark task, writes required artifacts, and exits after producing
a normalized result. It is intentionally dataset-neutral; Terminal-Bench,
SWE-bench, SWT-Bench, and custom repo tasks should enter through adapters that
produce the same `BenchmarkTask` envelope.

## Command

Local file mode:

```bash
python3 -m openagents_bench.run_task \
  --run-id run_local_fake \
  --task-run-id taskrun_fake_pass \
  --task-spec fixtures/tasks/fake-pass.json \
  --artifact-dir /tmp/oa-bench-fake-pass \
  --agent fake-agent \
  --model fake-model
```

GCS mode used by Cloud Batch:

```bash
python3 -m openagents_bench.run_task \
  --run-id "$RUN_ID" \
  --task-run-id "$TASK_RUN_ID" \
  --task-spec-gcs "$TASK_SPEC_GCS" \
  --artifact-prefix "$ARTIFACT_PREFIX" \
  --agent "$AGENT" \
  --model "$MODEL"
```

Terminal-Bench dry-run through the Harbor adapter:

```bash
OPENAGENTS_BENCH_HARBOR_DRY_RUN=1 \
python3 -m openagents_bench.run_task \
  --run-id run_tb2_dry \
  --task-run-id taskrun_tb2_oracle \
  --task-spec fixtures/tasks/terminal-bench-oracle-dry-run.json \
  --artifact-dir /tmp/oa-bench-tb2-dry \
  --agent oracle \
  --model terminal-bench-oracle
```

Terminal-Bench dry-run through the OpenAgents/Codex adapter:

```bash
OPENAGENTS_BENCH_CODEX_DRY_RUN=1 \
python3 -m openagents_bench.run_task \
  --run-id run_tb2_codex_dry \
  --task-run-id taskrun_tb2_codex \
  --task-spec fixtures/tasks/terminal-bench-codex-dry-run.json \
  --artifact-dir /tmp/oa-bench-tb2-codex-dry \
  --agent openagents-codex \
  --model codex-account
```

Terminal-Bench retained failure fixture through Probe+Codex signature routing:

```bash
OPENAGENTS_BENCH_CODEX_DRY_RUN=1 \
python3 -m openagents_bench.run_task \
  --run-id run_tb2_signature_fixture \
  --task-run-id taskrun_service_readiness_probe \
  --task-spec fixtures/signature-routing/terminal-bench-retained-configure-git-webserver.json \
  --artifact-dir /tmp/oa-bench-tb2-signature-fixture \
  --agent probe-codex \
  --model codex-account
```

Custom repo dry-run:

```bash
OPENAGENTS_BENCH_REPO_DRY_RUN=1 \
python3 -m openagents_bench.run_task \
  --run-id run_custom_repo_dry \
  --task-run-id taskrun_custom_repo \
  --task-spec fixtures/tasks/custom-repo-dry-run.json \
  --artifact-dir /tmp/oa-bench-custom-repo-dry \
  --agent openagents-codex \
  --model codex-account
```

Required artifacts:

- `result.json`
- `events.jsonl`
- `metadata.json`
- `resource_usage_receipt.json`
- `cloud_execution_closeout.json`
- `artifact_manifest.json`
- `proof_bundle.json`

Signature-routing fixtures also write:

- `signature_selector_trace.json`

The fake adapter also writes:

- `commands.jsonl`
- `transcript.md`
- `agent_stdout.log`
- `agent_stderr.log`
- `verifier_stdout.log`
- `verifier_stderr.log`

## Local Checks

```bash
cd runners/py-bench-runner
python3 -m unittest discover -s tests
```

Retained signature-improvement evaluation:

```bash
python3 -m openagents_bench.evaluate_signatures --fixture-dir fixtures/signature-routing --json
```

This compares the retained raw Codex reward recorded in each fixture against
the expected Probe+Codex signature reward after the selected playbook is loaded.
It is an internal regression metric, not a public Terminal-Bench score.

## Cloud Batch Image

```bash
PROJECT_ID=openagents-bench-dev
REGION=us-central1
IMAGE="$REGION-docker.pkg.dev/$PROJECT_ID/oa-benchmark-runners/py-bench-runner:dev"

gcloud builds submit \
  --project "$PROJECT_ID" \
  --tag "$IMAGE" \
  .
```

## Contract Notes

- Benchmark failure is data, not infrastructure failure. A runner can return a
  normalized `failed`, `timeout`, or `error` result after writing artifacts.
- `cloud_execution_closeout.json` is execution evidence only. It must keep
  `walletAuthority=false`, `payoutAuthority=false`, and
  `publicClaimAuthority=false`; Omega/Vortex remains responsible for public
  projection and settlement gates.
- Dataset adapters must not dump raw environment variables, provider tokens,
  customer data, or broad cloud credentials into artifacts.
- Artifact writers redact secret-like values in text and JSON before closeout
  and proof-bundle generation. Later real adapters must preserve that no-secret
  artifact boundary.
