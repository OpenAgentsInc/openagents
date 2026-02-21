#!/usr/bin/env bash
set -euo pipefail

NAMESPACE="${NAMESPACE:-default}"
IMAGE="${IMAGE:-}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MIGRATION_JOB_MANIFEST="$ROOT_DIR/migration-job.yaml"
SMOKE_JOB_MANIFEST="$ROOT_DIR/smoke-job.yaml"

run_job() {
  local name="$1"
  local manifest="$2"
  local container="$3"

  kubectl -n "$NAMESPACE" delete job "$name" --ignore-not-found
  kubectl -n "$NAMESPACE" apply -f "$manifest"

  if [[ -n "$IMAGE" ]]; then
    kubectl -n "$NAMESPACE" set image "job/$name" "$container=$IMAGE"
  fi

  kubectl -n "$NAMESPACE" wait --for=condition=complete "job/$name" --timeout=600s
  kubectl -n "$NAMESPACE" logs "job/$name"
}

run_job "runtime-migrate" "$MIGRATION_JOB_MANIFEST" migrate
run_job "runtime-smoke" "$SMOKE_JOB_MANIFEST" smoke
