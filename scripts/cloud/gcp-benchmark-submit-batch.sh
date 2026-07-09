#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/gcp-benchmark-submit-batch.sh --project PROJECT_ID --image IMAGE_URI \
    --run-id RUN_ID --task-run-id TASK_RUN_ID --task-spec FILE_OR_GS_URI \
    [--region us-central1] [--env dev] [--agent fake-agent] [--model fake-model] \
    [--task-class normal_coding] [--max-run-duration 7200s] [--apply]

Builds a one-task Google Cloud Batch job config for Benchmark Cloud. The default
mode is dry-run: it prints the gcloud command and job JSON. Pass --apply to
upload a local task spec, submit the job, and leave artifacts under the
Benchmark Cloud artifact bucket.
USAGE
}

project_id=""
region="us-central1"
env_name="dev"
image_uri=""
run_id=""
task_run_id=""
task_spec=""
agent="fake-agent"
model="fake-model"
task_class="normal_coding"
max_run_duration="7200s"
apply="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --project) project_id="${2:-}"; shift 2 ;;
    --region) region="${2:-}"; shift 2 ;;
    --env) env_name="${2:-}"; shift 2 ;;
    --image) image_uri="${2:-}"; shift 2 ;;
    --run-id) run_id="${2:-}"; shift 2 ;;
    --task-run-id) task_run_id="${2:-}"; shift 2 ;;
    --task-spec) task_spec="${2:-}"; shift 2 ;;
    --agent) agent="${2:-}"; shift 2 ;;
    --model) model="${2:-}"; shift 2 ;;
    --task-class) task_class="${2:-}"; shift 2 ;;
    --max-run-duration) max_run_duration="${2:-}"; shift 2 ;;
    --apply) apply="true"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

for value_name in project_id image_uri run_id task_run_id task_spec; do
  if [[ -z "${!value_name}" ]]; then
    echo "--${value_name//_/-} is required" >&2
    usage >&2
    exit 2
  fi
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
runner_root="${repo_root}/runners/py-bench-runner"
spec_bucket="${project_id}-oa-benchmark-specs-${env_name}"
artifact_bucket="${project_id}-oa-benchmark-artifacts-${env_name}"
task_topic="benchmark-task-events-${env_name}"
runner_sa="bench-runner-${env_name}@${project_id}.iam.gserviceaccount.com"
job_id="oa-bench-${run_id//_/-}-${task_run_id//_/-}"
job_id="${job_id:0:60}"

if [[ "$task_spec" == gs://* ]]; then
  task_spec_gcs="$task_spec"
else
  task_spec_gcs="gs://${spec_bucket}/runs/${run_id}/tasks/${task_run_id}.json"
fi

artifact_prefix="gs://${artifact_bucket}/runs/${run_id}/tasks/${task_run_id}/"
job_config="$(mktemp)"
trap 'rm -f "$job_config"' EXIT

(
  cd "$runner_root"
  python3 -m openagents_bench.gcp_batch \
    --image-uri "$image_uri" \
    --run-id "$run_id" \
    --task-run-id "$task_run_id" \
    --task-spec-gcs "$task_spec_gcs" \
    --artifact-prefix "$artifact_prefix" \
    --agent "$agent" \
    --model "$model" \
    --service-account "$runner_sa" \
    --task-class "$task_class" \
    --max-run-duration "$max_run_duration" \
    --task-event-topic "$task_topic" \
    >"$job_config"
)

if [[ "$apply" == "true" ]]; then
  gcloud config set project "$project_id" >/dev/null
  if [[ "$task_spec" != gs://* ]]; then
    gcloud storage cp "$task_spec" "$task_spec_gcs"
  fi
  gcloud batch jobs submit "$job_id" \
    --project "$project_id" \
    --location "$region" \
    --config "$job_config"
else
  if [[ "$task_spec" != gs://* ]]; then
    printf '+ gcloud storage cp %q %q\n' "$task_spec" "$task_spec_gcs"
  fi
  printf '+ gcloud batch jobs submit %q --project %q --location %q --config %q\n' \
    "$job_id" "$project_id" "$region" "$job_config"
  cat "$job_config"
fi
