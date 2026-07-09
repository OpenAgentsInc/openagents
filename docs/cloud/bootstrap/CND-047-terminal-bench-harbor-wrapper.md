# CND-047 Terminal-Bench Harbor Wrapper

Status: Harbor wrapper scaffold
Last updated: 2026-06-01

Terminal-Bench 2 is the first dataset adapter for Benchmark Cloud. The adapter
wraps Harbor instead of reimplementing Terminal-Bench task setup, agent
execution, and verification.

## Smoke Task

The checked-in smoke fixture is:

```text
runners/py-bench-runner/fixtures/tasks/terminal-bench-oracle-dry-run.json
```

It uses:

- dataset slug: `terminal-bench`
- dataset version: `2.0`
- Terminal-Bench task id: `tb2-smoke-oracle`
- agent: `oracle`
- model label: `terminal-bench-oracle`

`tb2-smoke-oracle` is a placeholder smoke selector for the first Cloud path. A
real Terminal-Bench 2 task id should replace it before a public claim or broad
sweep.

## Local Dry-Run

```bash
cd runners/py-bench-runner
OPENAGENTS_BENCH_HARBOR_DRY_RUN=1 \
python3 -m openagents_bench.run_task \
  --run-id run_tb2_dry \
  --task-run-id taskrun_tb2_oracle \
  --task-spec fixtures/tasks/terminal-bench-oracle-dry-run.json \
  --artifact-dir /tmp/oa-bench-tb2-dry \
  --agent oracle \
  --model terminal-bench-oracle
```

The dry-run validates normalized artifacts without requiring Harbor.

## Real Harbor Command

For non-dry-run tasks, the adapter executes:

```bash
harbor run \
  --dataset terminal-bench@2.0 \
  --agent oracle \
  --n-concurrent 1 \
  --task-id "$TASK_ID"
```

The runner records:

- `result.json`
- `events.jsonl`
- `metadata.json`
- `artifact_manifest.json`
- `proof_bundle.json`
- `commands.jsonl`
- `transcript.md`
- `harbor_stdout.log`
- `harbor_stderr.log`
- `raw_harbor_result.json`
- verifier stdout/stderr mirrors for Vortex artifact viewers

## Cloud Batch Dry-Run

After the runner image includes Harbor, generate the Batch job:

```bash
PROJECT_ID=openagents-bench-dev
REGION=us-central1
IMAGE="$REGION-docker.pkg.dev/$PROJECT_ID/oa-benchmark-runners/py-bench-runner:dev"

scripts/gcp-benchmark-submit-batch.sh \
  --project "$PROJECT_ID" \
  --region "$REGION" \
  --env dev \
  --image "$IMAGE" \
  --run-id run_tb2_oracle_smoke \
  --task-run-id taskrun_tb2_oracle \
  --task-spec runners/py-bench-runner/fixtures/tasks/terminal-bench-oracle-dry-run.json \
  --agent oracle \
  --model terminal-bench-oracle \
  --task-class small_terminal
```

Use `--apply` only after `scripts/gcp-benchmark-bootstrap.sh --apply` has
created the dev substrate and the image has been built and pushed.

## Claim Boundary

This adapter produces internal evidence only. Do not publish a Terminal-Bench
claim until Vortex has proof projection, redaction checks, dataset/task subset
disclosure, harness version disclosure, artifact retention, retry policy, and
claim state controls.
