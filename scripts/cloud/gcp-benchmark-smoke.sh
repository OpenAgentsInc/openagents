#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/gcp-benchmark-smoke.sh --project PROJECT_ID [--env dev] [--impersonate-runner]

Runs a narrow substrate smoke after gcp-benchmark-bootstrap.sh:
  - uploads the fake pass task spec
  - reads it back
  - writes a small artifact object
  - publishes one task event

When --impersonate-runner is provided, read/write/publish calls use the runner
service account to validate runner IAM. The current principal must have
iam.serviceAccounts.getAccessToken for that service account.
USAGE
}

project_id=""
env_name="dev"
impersonate="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --project)
      project_id="${2:-}"
      shift 2
      ;;
    --env)
      env_name="${2:-}"
      shift 2
      ;;
    --impersonate-runner)
      impersonate="true"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "$project_id" ]]; then
  echo "--project is required" >&2
  usage >&2
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
task_spec="${repo_root}/runners/py-bench-runner/fixtures/cloud/tasks/fake-pass.json"
spec_bucket="${project_id}-oa-benchmark-specs-${env_name}"
artifact_bucket="${project_id}-oa-benchmark-artifacts-${env_name}"
task_topic="benchmark-task-events-${env_name}"
runner_sa="bench-runner-${env_name}@${project_id}.iam.gserviceaccount.com"
spec_uri="gs://${spec_bucket}/runs/smoke/tasks/fake-pass.json"
artifact_uri="gs://${artifact_bucket}/runs/smoke/tasks/fake-pass/smoke.txt"

runner_args=()
if [[ "$impersonate" == "true" ]]; then
  runner_args+=(--impersonate-service-account "$runner_sa")
fi

gcloud config set project "$project_id" >/dev/null
gcloud storage cp "$task_spec" "$spec_uri"
gcloud storage cat "$spec_uri" "${runner_args[@]}" >/dev/null
printf 'benchmark smoke artifact\n' | gcloud storage cp - "$artifact_uri" "${runner_args[@]}"
gcloud pubsub topics publish "$task_topic" \
  --message '{"type":"smoke","source":"gcp-benchmark-smoke"}' \
  "${runner_args[@]}" \
  >/dev/null

echo "benchmark GCP substrate smoke passed"
